//! Durable PostgreSQL session authority for isolated ADR-0035 validation.
//!
//! Every state-changing boundary uses READ COMMITTED and the order account
//! `FOR UPDATE` → session `FOR UPDATE` → protected resource. Candidate digest
//! lookups confer no authority: checked records are reread after waiting. Time
//! is sampled with `clock_timestamp()` only after those locks. Known success
//! is returned only after durable commit. SQL/storage details never escape.
//!
//! Neither production service enables this module. In particular a returned
//! observation/binding does not fence a later effect or private socket send.

use std::fmt;

use sqlx::{PgPool, Postgres, Transaction};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AccountRecord, CredentialDigest, IssueSession, ProviderIdentityKey,
    RawAccountRecord, RawSessionRecord, RotateSession, SessionAuthority, SessionBinding,
    SessionChannel, SessionError, SessionRecord, SessionSnapshot, UnixMillis,
};
use uuid::Uuid;

#[cfg(test)]
use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc, Mutex,
};

/// `PostgreSQL` implementation of the internal durable authority port (ADR-0035).
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
    /// Production bootstrap does not call this (ADR-0035).
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
        sqlx::query_file!("src/session/sql/isolation.sql")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        sqlx::query_file!("src/session/sql/synchronous_commit.sql")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        Ok(tx)
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
        sqlx::query_file_as!(
            AccountRow,
            "src/session/sql/account_for_update.sql",
            user_id
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Unauthenticated)?
        .try_into()
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
