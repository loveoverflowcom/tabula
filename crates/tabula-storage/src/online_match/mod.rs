//! Bounded isolated join-code admission and live durable apply/commit authority.
//! SQL stays storage-owned (I-15). Registry owns game/config meaning. No production
//! listener, automatic migration, seat replacement or reconnect acceptance is opened.
//! The disposable dataset admits at most 128 distinct room IDs over its lifetime,
//! including completed and expired waiting rooms. This is not reusable capacity.

use crate::session::{LockedCredential, PgSessionPublication, PgSessionStore};
use sqlx::{PgPool, Postgres, Transaction};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tabula_core::{
    canonical_encode, GameId, GameVersion, MatchId, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
};
use tabula_match_journal::{JournalRecord, OperationScope};
use tabula_session::{
    ActivityKind, CredentialOperation, HttpSessionAuthority, SessionError, SessionPublication,
    SessionSnapshot,
};
use tokio::time::Instant;
use uuid::Uuid;

const CODE_LIFETIME_MS: u64 = 600_000;
const ATTEMPT_WINDOW_MS: u64 = 60_000;
const MAX_JOIN_ATTEMPTS: i16 = 8;
// Align the disposable dataset with the bounded resident gateway owner budget.
// Retired room IDs are retained durably and never recycle this lifetime capacity.
const MAX_DATASET_LIFETIME_ROOMS: i64 = 128;
const MAX_USER_ROOMS: i64 = 4;
const MAX_ADMISSIONS: i64 = 64;
const MAX_CONFIG_BYTES: usize = 65_536;
const OPERATION_BUDGET_MS: u64 = 2_000;
static SESSION_MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./session_migrations");
static MATCH_MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./match_migrations");
static ONLINE_MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./online_match_migrations");

/// Public-safe admission outcomes, without code, identity or SQL details.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OnlineMatchError {
    #[error("session unavailable")]
    Session(#[from] SessionError),
    #[error("invalid input")]
    InvalidInput,
    #[error("unavailable")]
    Unavailable,
    #[error("busy")]
    Busy,
    #[error("join unavailable")]
    JoinUnavailable,
    #[error("rate limited")]
    RateLimited,
}
fn unavailable<T>(_: T) -> OnlineMatchError {
    OnlineMatchError::Unavailable
}
fn signed(value: u64) -> Result<i64, OnlineMatchError> {
    i64::try_from(value).map_err(unavailable)
}
fn unsigned(value: i64) -> Result<u64, OnlineMatchError> {
    u64::try_from(value).map_err(unavailable)
}

/// Checked durable seat/session observation. It is not a later apply permit.
/// The host cannot replace the storage-assigned scope with client claims.
///
/// ```compile_fail
/// use tabula_storage::online_match::OnlineMembership;
/// use tabula_match_journal::OperationScope;
/// fn forge(member: &mut OnlineMembership, claim: OperationScope) {
///     member.scope = claim;
/// }
/// ```
#[derive(Clone)]
pub struct OnlineMembership {
    match_id: MatchId,
    snapshot: SessionSnapshot,
    scope: OperationScope,
    game: GameId,
    game_version: GameVersion,
    config: Vec<u8>,
    roster: Option<SeatRoster>,
    started: bool,
}
impl OnlineMembership {
    pub const fn match_id(&self) -> MatchId {
        self.match_id
    }
    pub const fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }
    pub const fn scope(&self) -> OperationScope {
        self.scope
    }
    pub fn game(&self) -> &GameId {
        &self.game
    }
    pub fn game_version(&self) -> &GameVersion {
        &self.game_version
    }
    pub fn config(&self) -> &[u8] {
        &self.config
    }
    /// Complete immutable roster of real occupied accounts, absent while waiting.
    pub const fn roster(&self) -> Option<&SeatRoster> {
        self.roster.as_ref()
    }
    pub const fn ready(&self) -> bool {
        self.roster.is_some()
    }
    /// Whether this checked room already has a canonical journal owner/start.
    pub const fn started(&self) -> bool {
        self.started
    }
}
impl fmt::Debug for OnlineMembership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnlineMembership")
            .field("ready", &self.ready())
            .finish_non_exhaustive()
    }
}
/// Explicit isolated pool. Construction neither migrates nor opens a listener.
#[derive(Clone)]
pub struct PgOnlineMatchStore {
    pool: PgPool,
}
impl fmt::Debug for PgOnlineMatchStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgOnlineMatchStore").finish_non_exhaustive()
    }
}
#[derive(sqlx::FromRow)]
struct RoomRow {
    match_id: Uuid,
    creator_user_id: Uuid,
    game_id: String,
    game_version: String,
    config: Vec<u8>,
    seats: i16,
    created_at_ms: i64,
    code_deadline_ms: i64,
    started: bool,
    completed: bool,
}
#[derive(sqlx::FromRow)]
struct MemberRow {
    user_id: Uuid,
    seat: i16,
    generation: i64,
}
impl RoomRow {
    fn validate(&self) -> Result<(), OnlineMatchError> {
        if self.match_id.is_nil()
            || self.creator_user_id.is_nil()
            || !(2..=8).contains(&self.seats)
            || self.config.is_empty()
            || self.config.len() > MAX_CONFIG_BYTES
            || self.game_id.len() > 256
            || self.game_version.len() > 128
            || self.created_at_ms < 0
            || self
                .code_deadline_ms
                .checked_sub(self.created_at_ms)
                .is_none_or(|n| !(1..=600_000).contains(&n))
            || (self.completed && !self.started)
        {
            return Err(OnlineMatchError::Unavailable);
        }
        GameId::new(self.game_id.clone()).map_err(unavailable)?;
        GameVersion::new(self.game_version.clone()).map_err(unavailable)?;
        Ok(())
    }
}
impl PgOnlineMatchStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    /// Embedded reviewed migration versions, sorted without database I/O.
    pub fn migration_versions() -> Vec<i64> {
        let mut versions: Vec<_> = SESSION_MIGRATIONS
            .iter()
            .chain(MATCH_MIGRATIONS.iter())
            .chain(ONLINE_MIGRATIONS.iter())
            .map(|migration| migration.version)
            .collect();
        versions.sort_unstable();
        versions
    }
    /// Strict composition retaining every original migration version/checksum.
    pub async fn migrate(pool: &PgPool) -> Result<(), OnlineMatchError> {
        let migrations: Vec<_> = SESSION_MIGRATIONS
            .iter()
            .chain(MATCH_MIGRATIONS.iter())
            .chain(ONLINE_MIGRATIONS.iter())
            .cloned()
            .collect();
        let migrator = sqlx::migrate::Migrator::with_migrations(migrations);
        let mut versions = std::collections::BTreeSet::new();
        if migrator
            .iter()
            .any(|migration| !versions.insert(migration.version))
        {
            return Err(OnlineMatchError::Unavailable);
        }
        let mut connection = pool.acquire().await.map_err(unavailable)?;
        // SQLx's session-level migration lock survives early validation errors.
        // Closing this backend also releases it if the migration is canceled;
        // it must never return to the pool with a retained advisory lock.
        connection.close_on_drop();
        let result = migrator.run(&mut *connection).await.map_err(unavailable);
        let closed = connection.close().await.map_err(unavailable);
        result.and(closed)
    }
    /// Creates a waiting room, reserving server-assigned creator seat zero.
    /// Host registry validation must establish game/config/capability meaning.
    /// Completed/expired room IDs still consume the disposable dataset lifetime
    /// budget; the separate per-user limit counts only active rooms.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        request: CredentialOperation,
        match_id: MatchId,
        code_digest: [u8; 32],
        game: GameId,
        game_version: GameVersion,
        config: Vec<u8>,
        seats: u8,
    ) -> Result<OnlineMembership, OnlineMatchError> {
        if match_id.0 == 0
            || code_digest == [0; 32]
            || !(2..=8).contains(&seats)
            || config.is_empty()
            || config.len() > MAX_CONFIG_BYTES
            || game.as_str().len() > 256
            || game_version.as_str().len() > 128
        {
            return Err(OnlineMatchError::InvalidInput);
        }
        let mut tx = begin(&self.pool).await?;
        let mut authority = LockedCredential::lock(&mut tx, request).await?;
        lock_directory(&mut tx).await?;
        let (mut tx, snapshot) = observe_or_commit(tx, &mut authority).await?;
        let now = snapshot.last_observed_at().get();
        check_user_capacity(&mut tx, snapshot.user_id(), now).await?;
        check_dataset_lifetime_capacity(&mut tx).await?;
        let deadline = now
            .checked_add(CODE_LIFETIME_MS)
            .ok_or(OnlineMatchError::Unavailable)?;
        sqlx::query("INSERT INTO online_match_rooms(match_id,creator_user_id,code_digest,game_id,game_version,config,seats,created_at_ms,code_deadline_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(Uuid::from_u128(match_id.0)).bind(Uuid::from_u128(snapshot.user_id().0)).bind(code_digest.as_slice())
            .bind(game.as_str()).bind(game_version.as_str()).bind(config).bind(i16::from(seats)).bind(signed(now)?).bind(signed(deadline)?).execute(&mut *tx).await.map_err(unavailable)?;
        sqlx::query("INSERT INTO online_match_memberships(match_id,user_id,seat,generation) VALUES($1,$2,0,1)")
            .bind(Uuid::from_u128(match_id.0)).bind(Uuid::from_u128(snapshot.user_id().0)).execute(&mut *tx).await.map_err(unavailable)?;
        let room = lock_room(&mut tx, match_id).await?;
        let membership = membership_locked(&mut tx, &room, snapshot).await?;
        finish_operation(
            &mut tx,
            &mut authority,
            ActivityKind::ProtectedMutationCommitted,
            now,
            None,
        )
        .await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(membership)
    }
    /// Durable bounded code attempts; storage assigns the lowest vacant seat.
    pub async fn join(
        &self,
        request: CredentialOperation,
        code_digest: [u8; 32],
    ) -> Result<OnlineMembership, OnlineMatchError> {
        let mut tx = begin(&self.pool).await?;
        let mut authority = LockedCredential::lock(&mut tx, request).await?;
        lock_directory(&mut tx).await?;
        let (mut tx, snapshot) = observe_or_commit(tx, &mut authority).await?;
        let now = snapshot.last_observed_at().get();
        let mut savepoint = false;
        let result = async {
            take_attempt(&mut tx, &snapshot, now).await?;
            sqlx::query("SAVEPOINT online_join_admission").execute(&mut *tx).await.map_err(unavailable)?;
            savepoint = true;
            let room:Option<RoomRow> = sqlx::query_as("SELECT match_id,creator_user_id,game_id,game_version,config,seats,created_at_ms,code_deadline_ms,started,completed FROM online_match_rooms WHERE code_digest=$1 FOR UPDATE")
                .bind(code_digest.as_slice()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
            let room = room.ok_or(OnlineMatchError::JoinUnavailable)?;
            room.validate()?;
            // Resample only after the actual protected room lock, not its locator.
            let current = authority.observe(&mut tx, ActivityKind::Read).await?;
            let current_ms = current.last_observed_at().get();
            let deadline = unsigned(room.code_deadline_ms)?;
            if current_ms>=deadline || room.completed { return Err(OnlineMatchError::JoinUnavailable); }
            let members = members(&mut tx, &room).await?;
            let existing = members.iter().any(|member| member.user_id.as_u128()==current.user_id().0);
            if !existing {
                if room.started || members.len()>=usize::try_from(room.seats).map_err(unavailable)? { return Err(OnlineMatchError::JoinUnavailable); }
                check_user_capacity(&mut tx, current.user_id(), current_ms).await?;
                let seat = (0..room.seats).find(|seat| members.iter().all(|member| member.seat!=*seat)).ok_or(OnlineMatchError::JoinUnavailable)?;
                sqlx::query("INSERT INTO online_match_memberships(match_id,user_id,seat,generation) VALUES($1,$2,$3,1)")
                    .bind(room.match_id).bind(Uuid::from_u128(current.user_id().0)).bind(seat).execute(&mut *tx).await.map_err(unavailable)?;
            }
            let membership = membership_locked(&mut tx, &room, current).await?;
            Ok((membership, !existing, deadline))
        }.await;
        if savepoint {
            if result.is_err() {
                sqlx::query("ROLLBACK TO SAVEPOINT online_join_admission")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            }
            sqlx::query("RELEASE SAVEPOINT online_join_admission")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
        }
        if let Err(OnlineMatchError::Session(error)) = &result {
            authority.save_observation(&mut tx).await?;
            tx.commit().await.map_err(unavailable)?;
            return Err(OnlineMatchError::Session(*error));
        }
        let activity = if result.as_ref().is_ok_and(|(_, changed, _)| *changed) {
            ActivityKind::ProtectedMutationCommitted
        } else {
            ActivityKind::Rejected
        };
        let resource_deadline = result.as_ref().ok().map(|(_, _, deadline)| *deadline);
        finish_operation(&mut tx, &mut authority, activity, now, resource_deadline).await?;
        tx.commit().await.map_err(unavailable)?;
        result.map(|(membership, _, _)| membership)
    }
    /// Resolves an existing subject seat. Rebinding never replaces its occupant.
    pub async fn resolve(
        &self,
        request: CredentialOperation,
        match_id: MatchId,
    ) -> Result<OnlineMembership, OnlineMatchError> {
        let mut tx = begin(&self.pool).await?;
        let mut authority = LockedCredential::lock(&mut tx, request).await?;
        let room = lock_room(&mut tx, match_id).await?;
        let (mut tx, snapshot) = observe_or_commit(tx, &mut authority).await?;
        let now = snapshot.last_observed_at().get();
        let membership = membership_locked(&mut tx, &room, snapshot).await?;
        finish_operation(&mut tx, &mut authority, ActivityKind::Read, now, None).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(membership)
    }
    /// Owns live account→session→room locks through actor apply and one commit.
    pub async fn begin_operation(
        &self,
        request: CredentialOperation,
        match_id: MatchId,
    ) -> Result<PgOnlineOperation, OnlineMatchError> {
        // A committed exclusion survives loss of either backend. Other account
        // operations honor it even after PostgreSQL releases dead-backend locks.
        let publication = PgSessionStore::new(self.pool.clone())
            .begin_publication(request)
            .await?;
        let original = publication
            .snapshot()
            .idle_deadline()
            .min(publication.snapshot().absolute_deadline())
            .get();
        let observed = publication.snapshot().last_observed_at().get();
        let mut deadline_ms = observed
            .checked_add(OPERATION_BUDGET_MS)
            .ok_or(OnlineMatchError::Unavailable)?
            .min(original);
        let mut expires_at = publication.expires_at();
        let mut tx = begin(&self.pool).await?;
        let mut authority =
            LockedCredential::lock_published(&mut tx, request, &publication).await?;
        // Waiting→started is an active-capacity transition. Serialize it with
        // create/join counts, including an in-flight initializer at code expiry.
        let waiting: Option<bool> =
            sqlx::query_scalar("SELECT NOT started FROM online_match_rooms WHERE match_id=$1")
                .bind(Uuid::from_u128(match_id.0))
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        if waiting == Some(true) {
            lock_directory(&mut tx).await?;
        }
        let room = lock_room(&mut tx, match_id).await?;
        let resource_started = Instant::now();
        let (mut tx, snapshot) = observe_or_commit(tx, &mut authority).await?;
        publication.with_current(|_| ())?;
        if !room.started && snapshot.last_observed_at().get() >= unsigned(room.code_deadline_ms)? {
            return Err(OnlineMatchError::JoinUnavailable);
        }
        if !room.started {
            let code_deadline = unsigned(room.code_deadline_ms)?;
            deadline_ms = deadline_ms.min(code_deadline);
            let remaining = code_deadline
                .checked_sub(snapshot.last_observed_at().get())
                .and_then(|n| n.checked_sub(1))
                .filter(|n| *n > 0)
                .ok_or(OnlineMatchError::JoinUnavailable)?;
            expires_at = expires_at.min(resource_started + Duration::from_millis(remaining));
        }
        let membership = membership_locked(&mut tx, &room, snapshot).await?;
        if !membership.ready() {
            return Err(OnlineMatchError::Busy);
        }
        #[cfg(test)]
        let backend_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        Ok(PgOnlineOperation {
            membership: membership.clone(),
            request,
            state: Arc::new(Mutex::new(Some(OnlineTransaction {
                tx,
                authority,
                membership,
                deadline_ms,
                expires_at,
                publication,
                #[cfg(test)]
                backend_pid,
            }))),
        })
    }

    /// Fresh, one-shot account/session/membership authority through a private
    /// first-frame handoff (ADR-0031, I-5). An earlier `resolve` is insufficient.
    /// The exact server-owned scope is checked under the retained room lock.
    /// Membership changes must honor account-publication then room ordering.
    pub async fn begin_publication(
        &self,
        request: CredentialOperation,
        match_id: MatchId,
        expected_scope: OperationScope,
    ) -> Result<PgOnlinePublication, OnlineMatchError> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let operation = self.begin_operation(request, match_id).await?;
            if operation.scope() != expected_scope {
                operation.cancel().await?;
                return Err(OnlineMatchError::Session(SessionError::Unauthenticated));
            }
            operation.with_current(|_| ())?;
            let expires_at = operation
                .state
                .lock()
                .map_err(unavailable)?
                .as_ref()
                .ok_or(OnlineMatchError::Session(SessionError::Unauthenticated))?
                .expires_at;
            Ok(PgOnlinePublication {
                snapshot: operation.snapshot().clone(),
                operation: Some(operation),
                expires_at,
            })
        })
        .await
        .map_err(unavailable)?
    }
}

/// Native private-frame guard retaining current account/session and match
/// membership in the same ordering domain until actual first-frame handoff.
///
/// It owns a live room transaction plus the separately committed bounded
/// session-publication exclusion. Membership changes honoring the account/room
/// ordering remain excluded even after loss of both database backends, until
/// the local monotonic guard is unusable. Drop and every publish result retire
/// the transaction; this is neither a reusable grant nor an apply permit.
pub struct PgOnlinePublication {
    snapshot: SessionSnapshot,
    operation: Option<PgOnlineOperation>,
    expires_at: Instant,
}

impl PgOnlinePublication {
    /// Immutable minimum session/resource monotonic deadline of the fresh
    /// authority operation. Outer publication guards must narrow to this bound
    /// before nested first-frame construction (ADR-0031).
    pub const fn expires_at(&self) -> Instant {
        self.expires_at
    }
}

impl fmt::Debug for PgOnlinePublication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PgOnlinePublication([REDACTED])")
    }
}

impl SessionPublication for PgOnlinePublication {
    fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }

    /// Only bounded pure frame construction/staging is allowed in the callback.
    /// The current membership and monotonic exclusion are checked on both sides;
    /// late results are discarded and repeated calls cannot execute a callback.
    fn publish<R>(
        &mut self,
        publication: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        let operation = self.operation.take().ok_or(SessionError::Unauthenticated)?;
        operation
            .with_current(|member| publication(member.snapshot()))
            .map_err(|error| match error {
                OnlineMatchError::Session(error) => error,
                _ => SessionError::Unavailable,
            })
    }
}
async fn begin(pool: &PgPool) -> Result<Transaction<'static, Postgres>, OnlineMatchError> {
    let mut tx = pool.begin().await.map_err(unavailable)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("SET LOCAL synchronous_commit = on")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("SET LOCAL idle_in_transaction_session_timeout = '10s'")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    Ok(tx)
}
async fn observe_or_commit(
    mut tx: Transaction<'static, Postgres>,
    authority: &mut LockedCredential,
) -> Result<(Transaction<'static, Postgres>, SessionSnapshot), OnlineMatchError> {
    match authority.observe(&mut tx, ActivityKind::Read).await {
        Ok(snapshot) => Ok((tx, snapshot)),
        Err(error) => {
            tx.commit().await.map_err(unavailable)?;
            Err(error.into())
        }
    }
}
async fn lock_directory(tx: &mut Transaction<'_, Postgres>) -> Result<(), OnlineMatchError> {
    // Two-integer resource keys cannot collide with one-bigint account keys.
    sqlx::query(
        "SELECT pg_advisory_xact_lock(541, hashtext('online_match_rooms'::regclass::oid::text))",
    )
    .execute(&mut **tx)
    .await
    .map_err(unavailable)?;
    Ok(())
}
async fn check_user_capacity(
    tx: &mut Transaction<'_, Postgres>,
    user: UserId,
    now: u64,
) -> Result<(), OnlineMatchError> {
    let count:i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_memberships m JOIN online_match_rooms r USING(match_id) WHERE m.user_id=$1 AND NOT r.completed AND (r.started OR r.code_deadline_ms>$2)")
        .bind(Uuid::from_u128(user.0)).bind(signed(now)?).fetch_one(&mut **tx).await.map_err(unavailable)?;
    if count >= MAX_USER_ROOMS {
        return Err(OnlineMatchError::Busy);
    }
    Ok(())
}
/// The caller owns the directory lock, so concurrent creates cannot overbook.
/// Completion and code expiry do not release a resident-owner/dataset slot.
async fn check_dataset_lifetime_capacity(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), OnlineMatchError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_rooms")
        .fetch_one(&mut **tx)
        .await
        .map_err(unavailable)?;
    if count >= MAX_DATASET_LIFETIME_ROOMS {
        return Err(OnlineMatchError::Busy);
    }
    Ok(())
}

async fn lock_room(
    tx: &mut Transaction<'_, Postgres>,
    match_id: MatchId,
) -> Result<RoomRow, OnlineMatchError> {
    let room:RoomRow = sqlx::query_as("SELECT match_id,creator_user_id,game_id,game_version,config,seats,created_at_ms,code_deadline_ms,started,completed FROM online_match_rooms WHERE match_id=$1 FOR UPDATE")
        .bind(Uuid::from_u128(match_id.0)).fetch_optional(&mut **tx).await.map_err(unavailable)?.ok_or(OnlineMatchError::JoinUnavailable)?;
    room.validate()?;
    Ok(room)
}
async fn members(
    tx: &mut Transaction<'_, Postgres>,
    room: &RoomRow,
) -> Result<Vec<MemberRow>, OnlineMatchError> {
    let members:Vec<MemberRow> = sqlx::query_as("SELECT user_id,seat,generation FROM online_match_memberships WHERE match_id=$1 ORDER BY seat").bind(room.match_id).fetch_all(&mut **tx).await.map_err(unavailable)?;
    if members.is_empty()
        || members.len() > usize::try_from(room.seats).map_err(unavailable)?
        || members
            .iter()
            .any(|m| m.user_id.is_nil() || m.seat < 0 || m.seat >= room.seats || m.generation != 1)
        || members[0].seat != 0
        || members[0].user_id != room.creator_user_id
    {
        return Err(OnlineMatchError::Unavailable);
    }
    Ok(members)
}
async fn membership_locked(
    tx: &mut Transaction<'_, Postgres>,
    room: &RoomRow,
    snapshot: SessionSnapshot,
) -> Result<OnlineMembership, OnlineMatchError> {
    // Missing current permission is a caller-safe admission denial, even if
    // removal also makes the remaining room roster structurally incomplete.
    // Every still-present caller must separately pass full roster integrity.
    let current_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM online_match_memberships WHERE match_id=$1 AND user_id=$2)",
    )
    .bind(room.match_id)
    .bind(Uuid::from_u128(snapshot.user_id().0))
    .fetch_one(&mut **tx)
    .await
    .map_err(unavailable)?;
    if !current_member {
        return Err(OnlineMatchError::JoinUnavailable);
    }
    let members = members(tx, room).await?;
    let member = members
        .iter()
        .find(|m| m.user_id.as_u128() == snapshot.user_id().0)
        .ok_or(OnlineMatchError::JoinUnavailable)?;
    let scope = OperationScope {
        record: snapshot.id().get(),
        subject: snapshot.user_id(),
        epoch: snapshot.authorization_epoch().get(),
        seat: SeatId(u8::try_from(member.seat).map_err(unavailable)?),
        generation: unsigned(member.generation)?,
    };
    let old:Option<(Uuid, i64, i16, i64)> = sqlx::query_as("SELECT user_id,authorization_epoch,seat,generation FROM online_match_admissions WHERE match_id=$1 AND session_id=$2")
        .bind(room.match_id).bind(Uuid::from_u128(scope.record)).fetch_optional(&mut **tx).await.map_err(unavailable)?;
    if let Some(old) = old {
        if old
            != (
                Uuid::from_u128(scope.subject.0),
                signed(scope.epoch)?,
                i16::from(scope.seat.0),
                signed(scope.generation)?,
            )
        {
            return Err(OnlineMatchError::Unavailable);
        }
    } else {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM online_match_admissions WHERE match_id=$1")
                .bind(room.match_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(unavailable)?;
        if count >= MAX_ADMISSIONS {
            return Err(OnlineMatchError::Busy);
        }
        sqlx::query("INSERT INTO online_match_admissions(match_id,session_id,user_id,authorization_epoch,seat,generation) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(room.match_id).bind(Uuid::from_u128(scope.record)).bind(Uuid::from_u128(scope.subject.0)).bind(signed(scope.epoch)?).bind(i16::from(scope.seat.0)).bind(signed(scope.generation)?).execute(&mut **tx).await.map_err(unavailable)?;
    }
    let roster = if members.len() == usize::try_from(room.seats).map_err(unavailable)? {
        Some(
            SeatRoster::new(
                members
                    .iter()
                    .map(|m| {
                        Ok(SeatEntry {
                            seat: SeatId(u8::try_from(m.seat).map_err(unavailable)?),
                            occupant: Occupant::Human(UserId(m.user_id.as_u128())),
                            team: None,
                        })
                    })
                    .collect::<Result<_, OnlineMatchError>>()?,
            )
            .map_err(unavailable)?,
        )
    } else {
        None
    };
    Ok(OnlineMembership {
        match_id: MatchId(room.match_id.as_u128()),
        snapshot,
        scope,
        game: GameId::new(room.game_id.clone()).map_err(unavailable)?,
        game_version: GameVersion::new(room.game_version.clone()).map_err(unavailable)?,
        config: room.config.clone(),
        roster,
        started: room.started,
    })
}
async fn take_attempt(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &SessionSnapshot,
    now: u64,
) -> Result<(), OnlineMatchError> {
    let old:Option<(i64, i16)> = sqlx::query_as("SELECT window_started_at_ms,attempts FROM online_match_join_attempts WHERE session_id=$1 FOR UPDATE").bind(Uuid::from_u128(snapshot.id().get())).fetch_optional(&mut **tx).await.map_err(unavailable)?;
    let (window, attempts) = match old {
        Some((window, attempts))
            if now
                .checked_sub(unsigned(window)?)
                .ok_or(OnlineMatchError::Unavailable)?
                < ATTEMPT_WINDOW_MS =>
        {
            if attempts >= MAX_JOIN_ATTEMPTS {
                return Err(OnlineMatchError::RateLimited);
            }
            (
                window,
                attempts
                    .checked_add(1)
                    .ok_or(OnlineMatchError::Unavailable)?,
            )
        }
        _ => (signed(now)?, 1),
    };
    sqlx::query("INSERT INTO online_match_join_attempts(session_id,window_started_at_ms,attempts) VALUES($1,$2,$3) ON CONFLICT(session_id) DO UPDATE SET window_started_at_ms=$2,attempts=$3")
        .bind(Uuid::from_u128(snapshot.id().get())).bind(window).bind(attempts).execute(&mut **tx).await.map_err(unavailable)?;
    Ok(())
}
async fn finish_operation(
    tx: &mut Transaction<'_, Postgres>,
    authority: &mut LockedCredential,
    activity: ActivityKind,
    initial: u64,
    resource_deadline: Option<u64>,
) -> Result<(), OnlineMatchError> {
    let original = authority
        .snapshot()
        .idle_deadline()
        .min(authority.snapshot().absolute_deadline())
        .get();
    let deadline = initial
        .checked_add(OPERATION_BUDGET_MS)
        .ok_or(OnlineMatchError::Unavailable)?
        .min(original)
        .min(resource_deadline.unwrap_or(u64::MAX));
    let snapshot = authority.observe(tx, activity).await?;
    write_session_guard(tx, &snapshot, authority.request(), deadline).await
}
async fn write_session_guard(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &SessionSnapshot,
    request: CredentialOperation,
    deadline: u64,
) -> Result<(), OnlineMatchError> {
    sqlx::query("INSERT INTO online_session_commit_guards(session_id,transaction_id,user_id,authorization_epoch,credential_digest,channel,context_id,deadline_ms) VALUES($1,txid_current(),$2,$3,$4,$5,$6,$7) ON CONFLICT(session_id) DO UPDATE SET transaction_id=txid_current(),user_id=$2,authorization_epoch=$3,credential_digest=$4,channel=$5,context_id=$6,deadline_ms=$7")
        .bind(Uuid::from_u128(snapshot.id().get())).bind(Uuid::from_u128(snapshot.user_id().0)).bind(signed(snapshot.authorization_epoch().get())?).bind(request.digest.as_bytes().as_slice()).bind(request.channel.as_str()).bind(Uuid::from_u128(snapshot.context_id().get())).bind(signed(deadline)?).execute(&mut **tx).await.map_err(unavailable)?;
    Ok(())
}
pub(crate) struct OnlineTransaction {
    pub(crate) tx: Transaction<'static, Postgres>,
    authority: LockedCredential,
    pub(crate) membership: OnlineMembership,
    deadline_ms: u64,
    expires_at: Instant,
    publication: PgSessionPublication,
    #[cfg(test)]
    backend_pid: i32,
}
impl OnlineTransaction {
    pub(crate) fn into_commit_parts(
        self,
    ) -> (Transaction<'static, Postgres>, PgSessionPublication) {
        (self.tx, self.publication)
    }
    pub(crate) async fn authorize_record(
        &mut self,
        record: &JournalRecord,
    ) -> Result<(), OnlineMatchError> {
        if record.match_id != self.membership.match_id {
            return Err(OnlineMatchError::Unavailable);
        }
        let room = lock_room(&mut self.tx, record.match_id).await?;
        if let Some(creation) = &record.creation {
            if room.started
                || self.membership.snapshot.last_observed_at().get()
                    >= unsigned(room.code_deadline_ms)?
                || creation.identity.game != self.membership.game
                || creation.identity.game_version != self.membership.game_version
                || creation.config != self.membership.config
                || canonical_encode(
                    self.membership
                        .roster
                        .as_ref()
                        .ok_or(OnlineMatchError::Unavailable)?,
                )
                .map_err(unavailable)?
                    != canonical_encode(&creation.roster).map_err(unavailable)?
                || record.operation.is_some()
            {
                return Err(OnlineMatchError::Unavailable);
            }
        } else if !room.started
            || room.completed
            || record
                .operation
                .as_ref()
                .is_none_or(|op| op.scope != self.membership.scope)
        {
            return Err(OnlineMatchError::Unavailable);
        }
        self.authority
            .observe(&mut self.tx, ActivityKind::Read)
            .await?;
        Ok(())
    }
    pub(crate) async fn authorize_ledger(
        &mut self,
        match_id: MatchId,
    ) -> Result<(), OnlineMatchError> {
        if match_id != self.membership.match_id {
            return Err(OnlineMatchError::Unavailable);
        }
        let room = lock_room(&mut self.tx, match_id).await?;
        if !room.started {
            return Err(OnlineMatchError::Unavailable);
        }
        self.authority
            .observe(&mut self.tx, ActivityKind::Read)
            .await?;
        Ok(())
    }
    pub(crate) async fn finalize(
        &mut self,
        accepted: bool,
        genesis: bool,
        terminal: bool,
    ) -> Result<(), OnlineMatchError> {
        let snapshot = self
            .authority
            .observe(
                &mut self.tx,
                if accepted {
                    ActivityKind::GameCommandAccepted
                } else {
                    ActivityKind::Read
                },
            )
            .await?;
        write_session_guard(
            &mut self.tx,
            &snapshot,
            self.authority.request(),
            self.deadline_ms,
        )
        .await?;
        let scope = self.membership.scope;
        sqlx::query("INSERT INTO online_match_commit_guards(match_id,transaction_id,session_id,user_id,authorization_epoch,seat,generation) VALUES($1,txid_current(),$2,$3,$4,$5,$6) ON CONFLICT(match_id) DO UPDATE SET transaction_id=txid_current(),session_id=$2,user_id=$3,authorization_epoch=$4,seat=$5,generation=$6")
            .bind(Uuid::from_u128(self.membership.match_id.0)).bind(Uuid::from_u128(scope.record)).bind(Uuid::from_u128(scope.subject.0)).bind(signed(scope.epoch)?).bind(i16::from(scope.seat.0)).bind(signed(scope.generation)?).execute(&mut *self.tx).await.map_err(unavailable)?;
        if genesis || terminal {
            sqlx::query("UPDATE online_match_rooms SET started=started OR $2,completed=completed OR $3 WHERE match_id=$1").bind(Uuid::from_u128(self.membership.match_id.0)).bind(genesis).bind(terminal).execute(&mut *self.tx).await.map_err(unavailable)?;
        }
        Ok(())
    }
}
/// Live one-shot apply/commit permit. Postcommit output requires fresh authority.
/// Its mutex encloses synchronous apply; cancellation drops the owned transaction.
#[derive(Clone)]
pub struct PgOnlineOperation {
    membership: OnlineMembership,
    request: CredentialOperation,
    state: Arc<Mutex<Option<OnlineTransaction>>>,
}
impl fmt::Debug for PgOnlineOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgOnlineOperation").finish_non_exhaustive()
    }
}
impl PgOnlineOperation {
    pub const fn membership(&self) -> &OnlineMembership {
        &self.membership
    }
    pub const fn snapshot(&self) -> &SessionSnapshot {
        self.membership.snapshot()
    }
    pub const fn scope(&self) -> OperationScope {
        self.membership.scope()
    }
    pub const fn credential(&self) -> CredentialOperation {
        self.request
    }
    /// The callback is pure and synchronous; results that exceed the lease are discarded.
    pub fn with_current<R>(
        &self,
        action: impl FnOnce(&OnlineMembership) -> R,
    ) -> Result<R, OnlineMatchError> {
        let state = self.state.lock().map_err(unavailable)?;
        let pending = state
            .as_ref()
            .filter(|pending| Instant::now() < pending.expires_at)
            .ok_or(OnlineMatchError::Session(SessionError::Unauthenticated))?;
        let result = pending
            .publication
            .with_current(|_| action(&self.membership))?;
        if Instant::now() >= pending.expires_at {
            return Err(OnlineMatchError::Session(SessionError::Unauthenticated));
        }
        Ok(result)
    }
    /// Retires a request that produces no journal write before fresh output authority.
    /// Already-consumed operation guards are harmless no-ops.
    pub async fn cancel(&self) -> Result<(), OnlineMatchError> {
        let pending = { self.state.lock().map_err(unavailable)?.take() };
        if let Some(pending) = pending {
            pending.tx.rollback().await.map_err(unavailable)?;
        }
        Ok(())
    }
    pub(crate) fn take(&self) -> Result<OnlineTransaction, OnlineMatchError> {
        self.state
            .lock()
            .map_err(unavailable)?
            .take()
            .filter(|pending| Instant::now() < pending.expires_at)
            .ok_or(OnlineMatchError::Session(SessionError::Unauthenticated))
    }
}
#[cfg(test)]
mod tests;

#[cfg(feature = "online-match-test-support")]
mod test_support;
