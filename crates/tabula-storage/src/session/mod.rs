//! Durable PostgreSQL session authority for isolated ADR-0036 validation.
//!
//! Every state-changing boundary uses READ COMMITTED and the order account
//! advisory lock → `FOR UPDATE` → session `FOR UPDATE` → protected resource.
//! Candidate digest lookups confer no authority: checked records are reread after waiting. Time
//! is sampled with `clock_timestamp()` only after those locks. Known success
//! is returned only after durable commit. SQL/storage details never escape.
//!
//! Neither production service enables this module. Observations/bindings remain
//! snapshots. The explicit bounded publication guard fences only isolated server
//! body/frame handoff, not client receipt, buffered bytes or WebSocket delivery.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::{Connection, PgPool, Postgres, Transaction};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AccountRecord, CredentialDigest, CredentialOperation, HttpSessionAuthority,
    IssueSession, ProviderIdentityKey, RawAccountRecord, RawSessionRecord, RotateCredential,
    RotateSession, SelfProfileSnapshot, SessionAuthority, SessionBinding, SessionChannel,
    SessionError, SessionPublication, SessionRecord, SessionSnapshot, UnixMillis,
};
use tokio::sync::oneshot;
use tokio::time::Instant;
use uuid::Uuid;

#[cfg(test)]
use std::sync::{
    atomic::{AtomicU64, AtomicU8},
    Mutex,
};

/// `PostgreSQL` implementation of the internal durable authority port (ADR-0036).
///
/// The caller owns pool configuration and explicit migration opt-in. Neither
/// constructing this adapter nor observing it activates provider login, HTTP,
/// WebSocket enforcement, CSRF handling or private outbound delivery.
#[derive(Clone)]
pub struct PgSessionStore {
    pool: PgPool,
    #[cfg(test)]
    controls: Option<Arc<TestControls>>,
}

impl fmt::Debug for PgSessionStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PgSessionStore")
            .finish_non_exhaustive()
    }
}

struct AccountRow {
    user_id: Uuid,
    authorization_epoch: i64,
    enabled: bool,
    last_observed_at_ms: i64,
    publication_lease_started_at_ms: Option<i64>,
    publication_lease_until_ms: Option<i64>,
}

#[derive(Clone, Copy)]
struct PublicationLease {
    started_at: UnixMillis,
    until: UnixMillis,
}

impl PublicationLease {
    fn checked(started: Option<i64>, until: Option<i64>) -> Result<Option<Self>, SessionError> {
        match (started, until) {
            (None, None) => Ok(None),
            (Some(started), Some(until)) => {
                let started_at = UnixMillis::new(unsigned(started)?).map_err(unavailable)?;
                let until = UnixMillis::new(unsigned(until)?).map_err(unavailable)?;
                until
                    .get()
                    .checked_sub(started_at.get())
                    .filter(|duration| (1..=PUBLICATION_LEASE_MS).contains(duration))
                    .ok_or(SessionError::Unavailable)?;
                Ok(Some(Self { started_at, until }))
            }
            _ => Err(SessionError::Unavailable),
        }
    }
}

struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    authorization_epoch: i64,
    channel: String,
    credential_generation: i64,
    credential_digest: Vec<u8>,
    context_id: Uuid,
    created_at_ms: i64,
    last_activity_at_ms: i64,
    last_observed_at_ms: i64,
    idle_deadline_ms: i64,
    absolute_deadline_ms: i64,
    revoked_at_ms: Option<i64>,
    expired_at_ms: Option<i64>,
}

fn unavailable<T>(_: T) -> SessionError {
    SessionError::Unavailable
}
fn signed(value: u64) -> Result<i64, SessionError> {
    i64::try_from(value).map_err(unavailable)
}
fn unsigned(value: i64) -> Result<u64, SessionError> {
    u64::try_from(value).map_err(unavailable)
}

impl TryFrom<AccountRow> for AccountRecord {
    type Error = SessionError;
    fn try_from(row: AccountRow) -> Result<Self, Self::Error> {
        PublicationLease::checked(
            row.publication_lease_started_at_ms,
            row.publication_lease_until_ms,
        )?;
        Self::try_from(RawAccountRecord {
            user_id: row.user_id.as_u128(),
            authorization_epoch: unsigned(row.authorization_epoch)?,
            enabled: row.enabled,
            last_observed_at_ms: unsigned(row.last_observed_at_ms)?,
        })
        .map_err(unavailable)
    }
}

impl TryFrom<SessionRow> for SessionRecord {
    type Error = SessionError;
    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Self::try_from(RawSessionRecord {
            id: row.id.as_u128(),
            user_id: row.user_id.as_u128(),
            authorization_epoch: unsigned(row.authorization_epoch)?,
            channel: row.channel,
            credential_generation: unsigned(row.credential_generation)?,
            credential_digest: row.credential_digest,
            context_id: row.context_id.as_u128(),
            created_at_ms: unsigned(row.created_at_ms)?,
            last_activity_at_ms: unsigned(row.last_activity_at_ms)?,
            last_observed_at_ms: unsigned(row.last_observed_at_ms)?,
            idle_deadline_ms: unsigned(row.idle_deadline_ms)?,
            absolute_deadline_ms: unsigned(row.absolute_deadline_ms)?,
            revoked_at_ms: row.revoked_at_ms.map(unsigned).transpose()?,
            expired_at_ms: row.expired_at_ms.map(unsigned).transpose()?,
        })
        .map_err(unavailable)
    }
}

impl PgSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            #[cfg(test)]
            controls: None,
        }
    }

    /// Explicitly applies only the additive, isolated session migrations.
    /// Production bootstrap does not call this (ADR-0036).
    pub async fn migrate(&self) -> Result<(), SessionError> {
        sqlx::migrate!("./session_migrations")
            .run(&self.pool)
            .await
            .map_err(unavailable)
    }

    /// Links a structurally checked identity fixture to a checked account.
    ///
    /// This is deliberately named fixture provisioning: it verifies no Kanidm
    /// provider, OIDC signature, issuer trust, reauthentication or password. An
    /// existing pair cannot be reassigned, including between case variants.
    pub async fn provision_fixture_identity(
        &self,
        identity: ProviderIdentityKey,
        account: AccountRecord,
    ) -> Result<(), SessionError> {
        let raw = account.into_raw();
        let user_id = Uuid::from_u128(raw.user_id);
        let epoch = signed(raw.authorization_epoch)?;
        let observed = signed(raw.last_observed_at_ms)?;
        let mut tx = self.begin().await?;
        Self::lock_account_publication_order(&mut tx, user_id).await?;
        sqlx::query_file!(
            "src/session/sql/insert_fixture_account.sql",
            user_id,
            epoch,
            raw.enabled,
            observed
        )
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        let stored = Self::lock_account(&mut tx, user_id).await?;
        let stored_raw = stored.into_raw();
        if stored_raw.authorization_epoch != raw.authorization_epoch
            || stored_raw.enabled != raw.enabled
        {
            return Err(SessionError::Conflict);
        }
        sqlx::query_file!(
            "src/session/sql/insert_fixture_identity.sql",
            identity.issuer(),
            identity.subject(),
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        let linked = sqlx::query_file!(
            "src/session/sql/identity_user.sql",
            identity.issuer(),
            identity.subject()
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unavailable)?;
        if linked.user_id != user_id {
            return Err(SessionError::Conflict);
        }
        self.commit(tx).await
    }

    async fn begin(&self) -> Result<Transaction<'_, Postgres>, SessionError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        Self::configure_transaction(&mut tx).await?;
        Ok(tx)
    }

    async fn configure_transaction(tx: &mut Transaction<'_, Postgres>) -> Result<(), SessionError> {
        sqlx::query_file!("src/session/sql/isolation.sql")
            .execute(&mut **tx)
            .await
            .map_err(unavailable)?;
        sqlx::query_file!("src/session/sql/synchronous_commit.sql")
            .execute(&mut **tx)
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    async fn commit(&self, tx: Transaction<'_, Postgres>) -> Result<(), SessionError> {
        #[cfg(test)]
        let fault = self
            .controls
            .as_ref()
            .map_or(0, |controls| controls.fault.swap(0, Ordering::SeqCst));
        #[cfg(test)]
        if fault == 1 {
            tx.rollback().await.map_err(unavailable)?;
            return Err(SessionError::Unavailable);
        }
        tx.commit().await.map_err(unavailable)?;
        #[cfg(test)]
        if fault == 2 {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }

    #[cfg(test)]
    async fn after_lock(&self) {
        if let Some(controls) = &self.controls {
            let gate = controls
                .gate
                .lock()
                .expect("acceptance gate mutex poisoned")
                .take();
            if let Some(gate) = gate {
                gate.entered.add_permits(1);
                gate.release
                    .acquire()
                    .await
                    .expect("acceptance gate closed")
                    .forget();
            }
        }
    }

    async fn clock(&self, tx: &mut Transaction<'_, Postgres>) -> Result<UnixMillis, SessionError> {
        #[cfg(test)]
        if let Some(controls) = &self.controls {
            return UnixMillis::new(controls.clock.load(Ordering::SeqCst)).map_err(unavailable);
        }
        Self::database_clock(tx).await
    }

    async fn database_clock(
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<UnixMillis, SessionError> {
        let row = sqlx::query_file!("src/session/sql/clock.sql")
            .fetch_one(&mut **tx)
            .await
            .map_err(unavailable)?;
        UnixMillis::new(unsigned(row.now_ms)?).map_err(unavailable)
    }

    async fn lock_account(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
    ) -> Result<AccountRecord, SessionError> {
        Self::lock_account_publication_order(tx, user_id).await?;
        let row = sqlx::query_file_as!(
            AccountRow,
            "src/session/sql/account_for_update.sql",
            user_id
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?;
        let lease = PublicationLease::checked(
            row.publication_lease_started_at_ms,
            row.publication_lease_until_ms,
        )?;
        let account = AccountRecord::try_from(row)?;
        if let Some(lease) = lease {
            // A dead publication backend releases its advisory lock, but cannot
            // erase this committed exclusion. Hold the account row until its
            // bounded database-clock lease expires. Sample operation time only
            // after this wait, never from the earlier locator or lease check.
            loop {
                let now = Self::database_clock(tx).await?;
                if now < lease.started_at {
                    return Err(SessionError::Unavailable);
                }
                if now >= lease.until {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(lease.until.get() - now.get())).await;
            }
            Self::save_publication_lease(tx, user_id, None).await?;
        }
        Ok(account)
    }

    async fn save_publication_lease(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        lease: Option<PublicationLease>,
    ) -> Result<(), SessionError> {
        let started = lease
            .map(|lease| signed(lease.started_at.get()))
            .transpose()?;
        let until = lease.map(|lease| signed(lease.until.get())).transpose()?;
        let written = sqlx::query_file!(
            "src/session/sql/update_account_publication_lease.sql",
            user_id,
            started,
            until
        )
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
        if written.rows_affected() != 1 {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }

    async fn lock_account_publication_order(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
    ) -> Result<(), SessionError> {
        // A session-level publication guard holds the same key across the
        // durable observation commit and bounded first-frame handoff. Hash
        // collisions merely serialize unrelated accounts. The resolved owning
        // relation OID, not search_path/current_schema, makes independent pool
        // paths resolving the same account table share one authority lock.
        sqlx::query_file!("src/session/sql/account_advisory_transaction.sql", user_id)
            .execute(&mut **tx)
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    async fn lock_session(
        tx: &mut Transaction<'_, Postgres>,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<SessionRecord, SessionError> {
        sqlx::query_file_as!(
            SessionRow,
            "src/session/sql/session_for_update.sql",
            id,
            user_id
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?
        .try_into()
    }

    async fn lock_digest(
        tx: &mut Transaction<'_, Postgres>,
        digest: CredentialDigest,
    ) -> Result<(AccountRecord, SessionRecord), SessionError> {
        // Locator only. The digest, generation, channel, epoch and status are
        // checked again against the locked row, including after lock waits.
        let locator = sqlx::query_file!(
            "src/session/sql/locate_digest.sql",
            digest.as_bytes().as_slice()
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?;
        let account = Self::lock_account(tx, locator.user_id).await?;
        let session = Self::lock_session(tx, locator.id, locator.user_id).await?;
        Ok((account, session))
    }

    async fn save_account(
        tx: &mut Transaction<'_, Postgres>,
        account: &AccountRecord,
    ) -> Result<(), SessionError> {
        let raw = account.clone().into_raw();
        let user_id = Uuid::from_u128(raw.user_id);
        let epoch = signed(raw.authorization_epoch)?;
        let observed = signed(raw.last_observed_at_ms)?;
        let result = sqlx::query_file!(
            "src/session/sql/update_account.sql",
            user_id,
            epoch,
            raw.enabled,
            observed
        )
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
        if result.rows_affected() != 1 {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }

    async fn insert_session(
        tx: &mut Transaction<'_, Postgres>,
        session: &SessionRecord,
    ) -> Result<(), SessionError> {
        let raw = session.clone().into_raw();
        let id = Uuid::from_u128(raw.id);
        let user_id = Uuid::from_u128(raw.user_id);
        let epoch = signed(raw.authorization_epoch)?;
        let generation = signed(raw.credential_generation)?;
        let context_id = Uuid::from_u128(raw.context_id);
        let created = signed(raw.created_at_ms)?;
        let activity = signed(raw.last_activity_at_ms)?;
        let observed = signed(raw.last_observed_at_ms)?;
        let idle = signed(raw.idle_deadline_ms)?;
        let absolute = signed(raw.absolute_deadline_ms)?;
        let revoked = raw.revoked_at_ms.map(signed).transpose()?;
        let expired = raw.expired_at_ms.map(signed).transpose()?;
        sqlx::query_file!(
            "src/session/sql/insert_session.sql",
            id,
            user_id,
            epoch,
            raw.channel,
            generation,
            raw.credential_digest,
            context_id,
            created,
            activity,
            observed,
            idle,
            absolute,
            revoked,
            expired
        )
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
        Ok(())
    }

    async fn save_session(
        tx: &mut Transaction<'_, Postgres>,
        session: &SessionRecord,
    ) -> Result<(), SessionError> {
        let raw = session.clone().into_raw();
        let id = Uuid::from_u128(raw.id);
        let user_id = Uuid::from_u128(raw.user_id);
        let epoch = signed(raw.authorization_epoch)?;
        let generation = signed(raw.credential_generation)?;
        let context_id = Uuid::from_u128(raw.context_id);
        let created = signed(raw.created_at_ms)?;
        let activity = signed(raw.last_activity_at_ms)?;
        let observed = signed(raw.last_observed_at_ms)?;
        let idle = signed(raw.idle_deadline_ms)?;
        let absolute = signed(raw.absolute_deadline_ms)?;
        let revoked = raw.revoked_at_ms.map(signed).transpose()?;
        let expired = raw.expired_at_ms.map(signed).transpose()?;
        let result = sqlx::query_file!(
            "src/session/sql/update_session.sql",
            id,
            user_id,
            epoch,
            raw.channel,
            generation,
            raw.credential_digest,
            context_id,
            created,
            activity,
            observed,
            idle,
            absolute,
            revoked,
            expired
        )
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
        if result.rows_affected() != 1 {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }
}

impl SessionAuthority for PgSessionStore {
    async fn account_snapshot(
        &self,
        identity: ProviderIdentityKey,
    ) -> Result<AccountRecord, SessionError> {
        sqlx::query_file_as!(
            AccountRow,
            "src/session/sql/account_by_identity.sql",
            identity.issuer(),
            identity.subject()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?
        .try_into()
    }

    async fn issue_session(&self, request: IssueSession) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let linked = sqlx::query_file!(
            "src/session/sql/identity_user.sql",
            request.identity.issuer(),
            request.identity.subject()
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?;
        let mut account = Self::lock_account(&mut tx, linked.user_id).await?;
        #[cfg(test)]
        self.after_lock().await;
        // Fixture linkage is immutable through this adapter. Still reread the
        // exact pair after a potential account-lock wait; the earlier lookup
        // cannot substitute for current issuance facts.
        let current_link = sqlx::query_file!(
            "src/session/sql/identity_user.sql",
            request.identity.issuer(),
            request.identity.subject()
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        Self::save_account(&mut tx, &account).await?;
        let result = if current_link.is_none_or(|identity| identity.user_id != linked.user_id)
            || !account.enabled()
            || account.authorization_epoch() != request.expected_epoch
        {
            Err(SessionError::Unauthenticated)
        } else {
            SessionRecord::issue(
                request.id,
                account.user_id(),
                request.expected_epoch,
                request.channel,
                request.credential_digest,
                request.context_id,
                now,
            )
            .map_err(unavailable)
        };
        if let Ok(session) = &result {
            Self::insert_session(&mut tx, session).await?;
        }
        self.commit(tx).await?;
        result.map(|session| session.snapshot())
    }

    async fn observe_credential(
        &self,
        digest: CredentialDigest,
        channel: SessionChannel,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.observe_credential(&account, digest, channel, now);
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn observe_binding(
        &self,
        binding: SessionBinding,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let mut account = Self::lock_account(&mut tx, Uuid::from_u128(binding.user_id().0)).await?;
        let mut session = Self::lock_session(
            &mut tx,
            Uuid::from_u128(binding.id().get()),
            Uuid::from_u128(binding.user_id().0),
        )
        .await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let observed = session.observe(&account, now);
        let result = if !session.matches_binding(binding) && observed.is_ok() {
            Err(SessionError::Unauthenticated)
        } else {
            observed
        };
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn rotate_session(
        &self,
        request: RotateSession,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.current_digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.rotate(
            &account,
            request.current_digest,
            request.channel,
            request.expected_generation,
            request.replacement_digest,
            now,
        );
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn revoke_session(&self, binding: SessionBinding) -> Result<(), SessionError> {
        let mut tx = self.begin().await?;
        let mut account = Self::lock_account(&mut tx, Uuid::from_u128(binding.user_id().0)).await?;
        let mut session = Self::lock_session(
            &mut tx,
            Uuid::from_u128(binding.id().get()),
            Uuid::from_u128(binding.user_id().0),
        )
        .await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let observation = session.observe(&account, now);
        let result = if matches!(observation, Err(SessionError::Unavailable)) {
            // Includes impossible future epochs: the policy terminally fences
            // that corruption, and we must durably persist the fence below.
            Err(SessionError::Unavailable)
        } else if !session.matches_binding(binding) {
            Err(SessionError::Unauthenticated)
        } else {
            // Revocation remains idempotent for an expired, disabled-account,
            // old-epoch or already revoked established connection binding.
            session.revoke(now)
        };
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn invalidate_account_epoch(
        &self,
        user_id: UserId,
        expected_epoch: AccountEpoch,
    ) -> Result<AccountRecord, SessionError> {
        let mut tx = self.begin().await?;
        let mut account = Self::lock_account(&mut tx, Uuid::from_u128(user_id.0)).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        let result = account.invalidate(expected_epoch, now);
        Self::save_account(&mut tx, &account).await?;
        self.commit(tx).await?;
        result.map(|()| account)
    }
}

/// Maximum isolated first-frame authority lease. This is not a session lifetime.
/// The earlier idle/absolute deadline also bounds every publication guard.
const PUBLICATION_LEASE_MS: u64 = 2_000;
// clock_timestamp is floored to integer milliseconds. A local monotonic lease
// must end at least one millisecond before its persisted exclusion/deadline.
const PUBLICATION_QUANTIZATION_MARGIN_MS: u64 = 1;

fn local_publication_budget(lease_ms: u64) -> Result<Duration, SessionError> {
    if lease_ms > PUBLICATION_LEASE_MS {
        return Err(SessionError::Unavailable);
    }
    lease_ms
        .checked_sub(PUBLICATION_QUANTIZATION_MARGIN_MS)
        .filter(|remaining| *remaining > 0)
        .map(Duration::from_millis)
        .ok_or(SessionError::Unauthenticated)
}

/// Owned isolated server-frame guard, never a transferable credential (ADR-0036).
///
/// Its worker owns a dedicated close-on-drop database connection with the account
/// advisory lock. Durable facts are committed before this type is returned.
/// Cancellation, body drop, lease expiry or worker shutdown closes that physical
/// connection, so a pooled connection never retains a publication advisory lock.
/// The separately committed bounded lease still fences all account operations
/// if that backend dies, until this local guard is unusable. This relies on the
/// same trusted database-clock assumption as ADR-0036: an unobserved forward
/// clock correction is not solved by a monotonic local timer.
pub struct PgSessionPublication {
    snapshot: SessionSnapshot,
    active: Arc<AtomicBool>,
    published: bool,
    expires_at: Instant,
    release: Option<oneshot::Sender<()>>,
}

impl fmt::Debug for PgSessionPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PgSessionPublication([REDACTED])")
    }
}

impl SessionPublication for PgSessionPublication {
    fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }

    fn publish<R>(
        &mut self,
        publication: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        if !self.active.load(Ordering::Acquire)
            || self.published
            || Instant::now() >= self.expires_at
        {
            return Err(SessionError::Unauthenticated);
        }
        self.published = true;
        // The body owns this guard through its first frame. Cleanup never
        // blocks on construction: the committed exclusion still fences other
        // operations, and the post-construction check drops any late result.
        let frame = publication(&self.snapshot);
        // Frame construction is pure and must not perform I/O or send data.
        // A paused/scheduled callback can cross the lease, especially after
        // publication-backend loss. Drop its value rather than hand a late
        // frame to the transport. The atomic liveness check also observes
        // independent cleanup without locking or starving Tokio workers.
        if !self.active.load(Ordering::Acquire) || Instant::now() >= self.expires_at {
            return Err(SessionError::Unauthenticated);
        }
        Ok(frame)
    }
}

impl Drop for PgSessionPublication {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

impl HttpSessionAuthority for PgSessionStore {
    type Publication = PgSessionPublication;

    async fn read_session(
        &self,
        request: CredentialOperation,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.observe_operation(&account, request, now);
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn read_logout_context(
        &self,
        request: CredentialOperation,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.observe_logout_context(&account, request, now);
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn read_self_profile(
        &self,
        request: CredentialOperation,
    ) -> Result<SelfProfileSnapshot, SessionError> {
        self.read_session(request)
            .await
            .map(|session| SelfProfileSnapshot::from_session(&session))
    }

    async fn rotate_credential(
        &self,
        request: RotateCredential,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) =
            Self::lock_digest(&mut tx, request.credential.digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.rotate_credential(&account, request, now);
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn revoke_credential(&self, request: CredentialOperation) -> Result<(), SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.revoke_credential(&account, request, now);
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }

    async fn begin_publication(
        &self,
        request: CredentialOperation,
    ) -> Result<Self::Publication, SessionError> {
        let mut connection = self.pool.acquire().await.map_err(unavailable)?;
        // SQLx closes rather than returning this physical connection on every
        // error/cancellation path, including a failed/indeterminate COMMIT.
        connection.close_on_drop();
        let mut tx = connection.begin().await.map_err(unavailable)?;
        Self::configure_transaction(&mut tx).await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let user_id = Uuid::from_u128(account.user_id().0);
        sqlx::query_file!("src/session/sql/account_advisory_publication.sql", user_id)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        // Start the conservative monotonic lease before sampling the database
        // clock. SQL/COMMIT delays consume the lease rather than extending it.
        let lease_started = Instant::now();
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let result = session.observe_operation(&account, request, now);
        let lease_ms = if let Ok(snapshot) = &result {
            snapshot
                .idle_deadline()
                .min(snapshot.absolute_deadline())
                .get()
                .checked_sub(now.get())
                .ok_or(SessionError::Unauthenticated)?
                .min(PUBLICATION_LEASE_MS)
        } else {
            0
        };
        if lease_ms > 0 {
            // Use the actual database clock for durable publication exclusion,
            // including acceptance with independently injected policy clocks.
            // This sample follows the local monotonic start, so persisted
            // exclusion cannot expire before the local guard under the trusted
            // deployment-clock assumption already required by ADR-0036.
            let database_now = Self::database_clock(&mut tx).await?;
            let until = database_now
                .get()
                .checked_add(lease_ms)
                .and_then(|until| UnixMillis::new(until).ok())
                .ok_or(SessionError::Unavailable)?;
            Self::save_publication_lease(
                &mut tx,
                user_id,
                Some(PublicationLease {
                    started_at: database_now,
                    until,
                }),
            )
            .await?;
        }
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        let snapshot = result?;
        let expires_at = lease_started + local_publication_budget(lease_ms)?;
        if Instant::now() >= expires_at {
            return Err(SessionError::Unauthenticated);
        }
        let active = Arc::new(AtomicBool::new(true));
        let cleanup_active = active.clone();
        let (release, cancelled) = oneshot::channel();
        tokio::spawn(async move {
            tokio::select! {
                _ = cancelled => {},
                () = tokio::time::sleep_until(expires_at) => {},
            }
            // Never block a runtime worker on frame construction. Cancellation
            // and expiry independently mark the guard dead and close its backend,
            // even while a callback is paused. Its post-construction check will
            // suppress that result; the committed lease survives backend loss.
            cleanup_active.store(false, Ordering::Release);
            let _ = connection.close().await;
        });
        Ok(PgSessionPublication {
            snapshot,
            active,
            published: false,
            expires_at,
            release: Some(release),
        })
    }
}

#[cfg(test)]
struct TestControls {
    clock: AtomicU64,
    gate: Mutex<Option<Arc<TestGate>>>,
    fault: AtomicU8,
}

#[cfg(test)]
impl TestControls {
    fn new(now: u64) -> Self {
        Self {
            clock: AtomicU64::new(now),
            gate: Mutex::new(None),
            fault: AtomicU8::new(0),
        }
    }
}

#[cfg(test)]
struct TestGate {
    entered: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
}

#[cfg(test)]
impl TestGate {
    fn new() -> Self {
        Self {
            entered: tokio::sync::Semaphore::new(0),
            release: tokio::sync::Semaphore::new(0),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum ProtectedOperation {
    Accepted,
    Rejected,
    Duplicate,
    Read,
}

#[cfg(test)]
impl PgSessionStore {
    fn with_test_controls(pool: PgPool, controls: Arc<TestControls>) -> Self {
        Self {
            pool,
            controls: Some(controls),
        }
    }

    // This fixed storage-private marker is an independent protected-commit
    // oracle. There is no public callback, transferable permit or toy API.
    async fn protected_marker(
        &self,
        digest: CredentialDigest,
        channel: SessionChannel,
        marker: &str,
        operation: ProtectedOperation,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut tx = self.begin().await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, digest).await?;
        #[cfg(test)]
        self.after_lock().await;
        let now = self.clock(&mut tx).await?;
        account.observe(now)?;
        let mut result = session.observe_credential(&account, digest, channel, now);
        if result.is_ok() && matches!(operation, ProtectedOperation::Accepted) {
            // Test-only oracle DDL/DML is dynamic; all production queries use
            // committed, PostgreSQL-generated SQLx compile-time metadata.
            sqlx::query("SAVEPOINT protected_marker")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            let written = sqlx::query("INSERT INTO tabula_session_acceptance_markers (marker) VALUES ($1) ON CONFLICT (marker) DO NOTHING")
                .bind(marker).execute(&mut *tx).await;
            // A unique-index/resource wait can cross expiry even while the
            // account and session locks are held. The first snapshot does not
            // authorize this later effect. Re-sample/recheck after that wait.
            result = match written {
                Ok(written) => self.clock(&mut tx).await.and_then(|final_now| {
                    account.observe(final_now)?;
                    let observed =
                        session.observe_credential(&account, digest, channel, final_now)?;
                    if written.rows_affected() == 1 {
                        session.record_activity(
                            &account,
                            tabula_session::ActivityKind::ProtectedMutationCommitted,
                            final_now,
                        )
                    } else {
                        Ok(observed)
                    }
                }),
                Err(_) => Err(SessionError::Unavailable),
            };
            if result.is_err() {
                // Roll back only the tentative resource effect. Checked
                // terminal expiry and observed floors live in memory and are
                // saved below, outside this savepoint, even on rejection.
                sqlx::query("ROLLBACK TO SAVEPOINT protected_marker")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            }
            sqlx::query("RELEASE SAVEPOINT protected_marker")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
        }
        Self::save_account(&mut tx, &account).await?;
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        result
    }
}

#[cfg(test)]
mod tests;
