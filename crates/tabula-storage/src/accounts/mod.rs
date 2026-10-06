//! Opt-in durable Tabula enrollment/profile authority (ADR-0044).
//! Fixed SQL with hand-mapped checked rows follows doc 01's explicit query fallback;
//! disposable `PostgreSQL` acceptance validates the separate migrated schema.
mod publication;
use crate::session::{LockedCredential, PgSessionStore};
pub use publication::{AccountsPrincipal, AccountsPublication};
use sqlx::{Postgres, Row, Transaction};
use tabula_core::UserId;
use tabula_session::{
    AccountDisplayName, AccountEpoch, AccountHandle, AccountOperationId, AccountProfile,
    AccountProfileAuthority, AccountProfileVisibility, ActivityKind, CredentialDigest,
    CredentialOperation, EnrollmentAuthority, EnrollmentContext, EnrollmentPolicy,
    EnrollmentStatus, ProviderIdentityKey, SessionError, SessionPublication, UnixMillis,
    VerifiedEnrollment,
};
use uuid::Uuid;
const GRANT_LIFETIME_MS: u64 = 300_000;
fn unavailable(_: impl std::fmt::Debug) -> SessionError {
    SessionError::Unavailable
}
fn signed(value: u64) -> Result<i64, SessionError> {
    i64::try_from(value).map_err(unavailable)
}
fn unsigned(value: i64) -> Result<u64, SessionError> {
    u64::try_from(value).map_err(unavailable)
}
fn policy(row: &sqlx::postgres::PgRow) -> Result<EnrollmentPolicy, SessionError> {
    Ok(EnrollmentPolicy {
        epoch: AccountEpoch::new(unsigned(row.try_get("policy_epoch").map_err(unavailable)?)?)
            .map_err(unavailable)?,
        enabled: row.try_get("enabled").map_err(unavailable)?,
    })
}
fn context(row: &sqlx::postgres::PgRow) -> Result<EnrollmentContext, SessionError> {
    Ok(EnrollmentContext {
        operation_id: AccountOperationId::parse(&format!(
            "{:032x}",
            row.try_get::<Uuid, _>("operation_id")
                .map_err(unavailable)?
                .as_u128()
        ))
        .map_err(unavailable)?,
        policy_epoch: AccountEpoch::new(unsigned(
            row.try_get("policy_epoch").map_err(unavailable)?,
        )?)
        .map_err(unavailable)?,
        status: match row.try_get::<&str, _>("disposition").map_err(unavailable)? {
            "ready" => EnrollmentStatus::Ready,
            "accepted" => EnrollmentStatus::AcceptedWithoutSession,
            "rejected" => EnrollmentStatus::Rejected,
            _ => return Err(SessionError::Unavailable),
        },
    })
}
fn profile(row: &sqlx::postgres::PgRow, now: UnixMillis) -> Result<AccountProfile, SessionError> {
    let revision = unsigned(row.try_get("revision").map_err(unavailable)?)?;
    if revision == 0 {
        return Err(SessionError::Unavailable);
    }
    Ok(AccountProfile {
        user_id: UserId(
            row.try_get::<Uuid, _>("user_id")
                .map_err(unavailable)?
                .as_u128(),
        ),
        handle: AccountHandle::new(row.try_get("handle").map_err(unavailable)?)
            .map_err(unavailable)?,
        display_name: AccountDisplayName::new(row.try_get("display_name").map_err(unavailable)?)
            .map_err(unavailable)?,
        visibility: AccountProfileVisibility::parse(
            row.try_get("visibility").map_err(unavailable)?,
        )?,
        revision,
        as_of: now,
    })
}
async fn locked_policy(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(EnrollmentPolicy, UnixMillis), SessionError> {
    let row=sqlx::query("SELECT enabled,policy_epoch,last_observed_at_ms FROM account_enrollment_policy WHERE singleton=TRUE FOR UPDATE").fetch_one(&mut **tx).await.map_err(unavailable)?;
    let now = PgSessionStore::database_clock(tx).await?;
    if now.get() < unsigned(row.try_get("last_observed_at_ms").map_err(unavailable)?)? {
        return Err(SessionError::Unavailable);
    }
    sqlx::query("UPDATE account_enrollment_policy SET last_observed_at_ms=$1 WHERE singleton=TRUE")
        .bind(signed(now.get())?)
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
    Ok((policy(&row)?, now))
}
impl PgSessionStore {
    /// Explicit isolated schema setup; never called by production startup.
    pub async fn migrate_accounts(&self) -> Result<(), SessionError> {
        {
            let mut migrations = sqlx::migrate!("./session_migrations")
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            migrations.extend(sqlx::migrate!("./accounts_migrations").iter().cloned());
            migrations.sort_by_key(|m| m.version);
            let mut versions = std::collections::BTreeSet::new();
            if migrations.iter().any(|m| !versions.insert(m.version)) {
                return Err(SessionError::Unavailable);
            }
            let migrator = sqlx::migrate::Migrator::with_migrations(migrations);
            let mut conn = self.pool.acquire().await.map_err(unavailable)?;
            conn.close_on_drop();
            let result = migrator.run(&mut *conn).await.map_err(unavailable);
            let closed = conn.close().await.map_err(unavailable);
            result.and(closed)
        }
    }
    /// Operator-controlled enrollment CAS. Disabling or reenabling advances epoch,
    /// fencing pending provider attempts and unconsumed grants without reviving accounts.
    pub async fn set_enrollment_enabled(
        &self,
        expected: AccountEpoch,
        enabled: bool,
    ) -> Result<EnrollmentPolicy, SessionError> {
        let mut tx = self.begin().await?;
        let (current, _) = locked_policy(&mut tx).await?;
        if current.epoch != expected {
            return Err(SessionError::Conflict);
        }
        let epoch = current.epoch.checked_next()?;
        sqlx::query(
            "UPDATE account_enrollment_policy SET enabled=$1,policy_epoch=$2 WHERE singleton=TRUE",
        )
        .bind(enabled)
        .bind(signed(epoch.get())?)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        self.commit(tx).await?;
        Ok(EnrollmentPolicy { epoch, enabled })
    }
    #[cfg(feature = "social-postgres")]
    pub(crate) fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}
impl EnrollmentAuthority for PgSessionStore {
    async fn enrollment_policy(&self) -> Result<EnrollmentPolicy, SessionError> {
        let mut tx = self.begin().await?;
        let (result, _) = locked_policy(&mut tx).await?;
        self.commit(tx).await?;
        Ok(result)
    }
    async fn admitted_accounts(
        &self,
        issuer: String,
    ) -> Result<Vec<(ProviderIdentityKey, AccountEpoch)>, SessionError> {
        let rows=sqlx::query("SELECT i.issuer,i.subject,a.authorization_epoch FROM session_provider_identities i JOIN session_accounts a USING(user_id) WHERE i.issuer=$1 AND a.enabled=TRUE ORDER BY i.subject LIMIT 65").bind(issuer).fetch_all(&self.pool).await.map_err(unavailable)?;
        if rows.len() > 64 {
            return Err(SessionError::Unavailable);
        }
        rows.into_iter()
            .map(|row| {
                Ok((
                    ProviderIdentityKey::new(
                        row.try_get::<String, _>("issuer").map_err(unavailable)?,
                        row.try_get::<String, _>("subject").map_err(unavailable)?,
                    )
                    .map_err(unavailable)?,
                    AccountEpoch::new(unsigned(
                        row.try_get("authorization_epoch").map_err(unavailable)?,
                    )?)?,
                ))
            })
            .collect()
    }
    async fn account_profile_ready(&self, user_id: UserId) -> Result<bool, SessionError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles WHERE user_id=$1)")
            .bind(Uuid::from_u128(user_id.0))
            .fetch_one(&self.pool)
            .await
            .map_err(unavailable)
    }
    async fn create_enrollment(
        &self,
        verified: VerifiedEnrollment,
        digest: CredentialDigest,
        operation_id: AccountOperationId,
    ) -> Result<EnrollmentContext, SessionError> {
        let mut tx = self.begin().await?;
        let (policy, now) = locked_policy(&mut tx).await?;
        if !policy.enabled || policy.epoch != verified.expected_policy_epoch {
            return Err(SessionError::Unauthenticated);
        }
        if let Some(expected) = verified.expected_account_epoch {
            let user_id = mapped_user(
                &mut tx,
                verified.identity.issuer(),
                verified.identity.subject(),
            )
            .await?
            .ok_or(SessionError::Unauthenticated)?;
            let mut account = Self::lock_account(&mut tx, user_id).await?;
            let current_time = Self::database_clock(&mut tx).await?;
            account.observe(current_time)?;
            if !account.enabled() || account.authorization_epoch() != expected {
                return Err(SessionError::Unauthenticated);
            }
            Self::save_account(&mut tx, &account).await?;
        }
        sqlx::query("DELETE FROM account_enrollment_grants WHERE expires_at_ms<=$1")
            .bind(signed(now.get())?)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM account_enrollment_grants")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        if count >= 128 {
            return Err(SessionError::Unavailable);
        }
        let created_at = Self::database_clock(&mut tx).await?;
        if created_at < now {
            return Err(SessionError::Unavailable);
        }
        sqlx::query("INSERT INTO account_enrollment_grants(digest,operation_id,issuer,subject,policy_epoch,expected_account_epoch,created_at_ms,expires_at_ms,disposition) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'ready')").bind(digest.as_bytes().as_slice()).bind(Uuid::from_u128(operation_id.get())).bind(verified.identity.issuer()).bind(verified.identity.subject()).bind(signed(policy.epoch.get())?).bind(verified.expected_account_epoch.map(|epoch| signed(epoch.get())).transpose()?).bind(signed(created_at.get())?).bind(signed(created_at.get().checked_add(GRANT_LIFETIME_MS).ok_or(SessionError::Unavailable)?)?).execute(&mut *tx).await.map_err(unavailable)?;
        self.commit(tx).await?;
        Ok(EnrollmentContext {
            operation_id,
            status: EnrollmentStatus::Ready,
            policy_epoch: policy.epoch,
        })
    }
    async fn read_enrollment(
        &self,
        digest: CredentialDigest,
    ) -> Result<EnrollmentContext, SessionError> {
        let mut tx = self.begin().await?;
        let (policy, now) = locked_policy(&mut tx).await?;
        let row = sqlx::query("SELECT * FROM account_enrollment_grants WHERE digest=$1 FOR UPDATE")
            .bind(digest.as_bytes().as_slice())
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?
            .ok_or(SessionError::Unauthenticated)?;
        let ctx = context(&row)?;
        if !policy.enabled
            || policy.epoch != ctx.policy_epoch
            || now.get() < unsigned(row.try_get("created_at_ms").map_err(unavailable)?)?
            || now.get() >= unsigned(row.try_get("expires_at_ms").map_err(unavailable)?)?
        {
            return Err(SessionError::Unauthenticated);
        }
        self.commit(tx).await?;
        Ok(ctx)
    }
    async fn register_account(
        &self,
        digest: CredentialDigest,
        operation_id: AccountOperationId,
        handle: AccountHandle,
        name: AccountDisplayName,
    ) -> Result<EnrollmentStatus, SessionError> {
        let mut tx = self.begin().await?;
        let (policy, now) = locked_policy(&mut tx).await?;
        let row = sqlx::query("SELECT * FROM account_enrollment_grants WHERE digest=$1 FOR UPDATE")
            .bind(digest.as_bytes().as_slice())
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?
            .ok_or(SessionError::Unauthenticated)?;
        let ctx = context(&row)?;
        if !policy.enabled
            || policy.epoch != ctx.policy_epoch
            || ctx.operation_id != operation_id
            || now.get() < unsigned(row.try_get("created_at_ms").map_err(unavailable)?)?
            || now.get() >= unsigned(row.try_get("expires_at_ms").map_err(unavailable)?)?
        {
            return Err(SessionError::Unauthenticated);
        }
        if ctx.status != EnrollmentStatus::Ready {
            if row
                .try_get::<&str, _>("requested_handle")
                .map_err(unavailable)?
                != handle.as_str()
                || row
                    .try_get::<&str, _>("requested_display_name")
                    .map_err(unavailable)?
                    != name.as_str()
            {
                self.commit(tx).await?;
                return Err(SessionError::InvalidInput);
            }
            self.commit(tx).await?;
            return Ok(ctx.status);
        }
        let issuer: String = row.try_get("issuer").map_err(unavailable)?;
        let subject: String = row.try_get("subject").map_err(unavailable)?;
        let existing = mapped_user(&mut tx, &issuer, &subject).await?;
        let taken: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles WHERE handle=$1)")
                .bind(handle.as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(unavailable)?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_accounts")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let status = if let Some(user_id) = existing {
            complete_legacy_profile(&mut tx, user_id, &row, &handle, &name, taken).await?
        } else if taken || count >= 64 {
            EnrollmentStatus::Rejected
        } else {
            let user_id = Uuid::from_u128(AccountOperationId::generate()?.get());
            sqlx::query("INSERT INTO session_accounts(user_id,authorization_epoch,enabled,last_observed_at_ms) VALUES($1,0,TRUE,$2)").bind(user_id).bind(signed(now.get())?).execute(&mut *tx).await.map_err(unavailable)?;
            sqlx::query(
                "INSERT INTO session_provider_identities(issuer,subject,user_id) VALUES($1,$2,$3)",
            )
            .bind(&issuer)
            .bind(&subject)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
            sqlx::query("INSERT INTO account_profiles(user_id,handle,display_name,visibility,revision) VALUES($1,$2,$3,'public',1)").bind(user_id).bind(handle.as_str()).bind(name.as_str()).execute(&mut *tx).await.map_err(unavailable)?;
            EnrollmentStatus::AcceptedWithoutSession
        };
        // Policy lock serializes all enrollment writes. Recheck database time after resource waits.
        let final_now = Self::database_clock(&mut tx).await?;
        if final_now < now
            || final_now.get() >= unsigned(row.try_get("expires_at_ms").map_err(unavailable)?)?
        {
            return Err(SessionError::Unauthenticated);
        }
        sqlx::query("UPDATE account_enrollment_grants SET disposition=$2,requested_handle=$3,requested_display_name=$4 WHERE digest=$1").bind(digest.as_bytes().as_slice()).bind(if status==EnrollmentStatus::AcceptedWithoutSession{"accepted"}else{"rejected"}).bind(handle.as_str()).bind(name.as_str()).execute(&mut *tx).await.map_err(unavailable)?;
        self.commit(tx).await?;
        Ok(status)
    }
}
async fn mapped_user(
    tx: &mut Transaction<'_, Postgres>,
    issuer: &str,
    subject: &str,
) -> Result<Option<Uuid>, SessionError> {
    sqlx::query_scalar(
        "SELECT user_id FROM session_provider_identities WHERE issuer=$1 AND subject=$2",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(unavailable)
}
async fn complete_legacy_profile(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    grant: &sqlx::postgres::PgRow,
    handle: &AccountHandle,
    name: &AccountDisplayName,
    taken: bool,
) -> Result<EnrollmentStatus, SessionError> {
    // Same account exclusion as disable/revocation/profile/social output. The
    // captured admission is never replaced with callback-time authority.
    let mut account = PgSessionStore::lock_account(tx, user_id).await?;
    let now = PgSessionStore::database_clock(tx).await?;
    account.observe(now)?;
    PgSessionStore::save_account(tx, &account).await?;
    let has_profile: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles WHERE user_id=$1)")
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(unavailable)?;
    let captured = grant
        .try_get::<Option<i64>, _>("expected_account_epoch")
        .map_err(unavailable)?;
    let current_epoch = signed(account.authorization_epoch().get())?;
    if !account.enabled() || captured.is_some_and(|epoch| epoch != current_epoch) {
        return Ok(EnrollmentStatus::Rejected);
    }
    if has_profile {
        return Ok(EnrollmentStatus::AcceptedWithoutSession);
    }
    if captured.is_none() || taken {
        return Ok(EnrollmentStatus::Rejected);
    }
    sqlx::query("INSERT INTO account_profiles(user_id,handle,display_name,visibility,revision) VALUES($1,$2,$3,'public',1)").bind(user_id).bind(handle.as_str()).bind(name.as_str()).execute(&mut **tx).await.map_err(unavailable)?;
    Ok(EnrollmentStatus::AcceptedWithoutSession)
}
impl AccountProfileAuthority for PgSessionStore {
    type ProfilePublication = AccountsPublication;
    async fn self_account_profile(
        &self,
        operation: CredentialOperation,
    ) -> Result<(Self::ProfilePublication, Option<AccountProfile>), SessionError> {
        let guard = self
            .begin_accounts_publication(AccountsPrincipal::Credential(operation), vec![])
            .await?;
        let mut tx = self.begin().await?;
        let now = Self::database_clock(&mut tx).await?;
        let row = sqlx::query("SELECT * FROM account_profiles WHERE user_id=$1")
            .bind(Uuid::from_u128(guard.snapshot().user_id().0))
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?;
        let result = row.as_ref().map(|row| profile(row, now)).transpose()?;
        if result
            .as_ref()
            .is_some_and(|profile| !guard.includes(profile.user_id))
        {
            return Err(SessionError::Unavailable);
        }
        self.commit(tx).await?;
        guard.with_current(|_| ())?;
        Ok((guard, result))
    }
    async fn other_account_profile(
        &self,
        operation: CredentialOperation,
        handle: AccountHandle,
    ) -> Result<(Self::ProfilePublication, AccountProfile), SessionError> {
        let target: Option<Uuid> =
            sqlx::query_scalar("SELECT user_id FROM account_profiles WHERE handle=$1")
                .bind(handle.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(unavailable)?;
        let target = target.ok_or(SessionError::Unauthenticated)?;
        let guard = self
            .begin_accounts_publication(
                AccountsPrincipal::Credential(operation),
                vec![UserId(target.as_u128())],
            )
            .await?;
        let mut tx = self.begin().await?;
        let now = Self::database_clock(&mut tx).await?;
        let row=sqlx::query("SELECT p.* FROM account_profiles p JOIN session_accounts a USING(user_id) WHERE p.user_id=$1 AND p.handle=$2 AND a.enabled=TRUE").bind(target).bind(handle.as_str()).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(SessionError::Unauthenticated)?;
        let result = profile(&row, now)?;
        if !guard.includes(result.user_id) {
            return Err(SessionError::Unavailable);
        }
        let permitted = result.user_id == guard.snapshot().user_id()
            || match result.visibility {
                AccountProfileVisibility::Public => true,
                AccountProfileVisibility::Private => false,
                AccountProfileVisibility::Friends => {
                    #[cfg(feature = "social-postgres")]
                    {
                        crate::social::are_friends(
                            &mut tx,
                            guard.snapshot().user_id(),
                            result.user_id,
                        )
                        .await?
                    }
                    #[cfg(not(feature = "social-postgres"))]
                    {
                        false
                    }
                }
            };
        if !permitted {
            return Err(SessionError::Unauthenticated);
        }
        self.commit(tx).await?;
        guard.with_current(|_| ())?;
        Ok((guard, result))
    }
    async fn update_account_profile(
        &self,
        operation: CredentialOperation,
        id: AccountOperationId,
        expected: u64,
        name: AccountDisplayName,
        visibility: AccountProfileVisibility,
    ) -> Result<(), SessionError> {
        if expected == 0 || expected >= i64::MAX as u64 {
            return Err(SessionError::InvalidInput);
        }
        let mut tx = self.begin().await?;
        let mut credential = LockedCredential::lock_accounts(&mut tx, &[], operation).await?;
        let snapshot = match credential.observe(&mut tx, ActivityKind::Read).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.commit(tx).await?;
                return Err(error);
            }
        };
        let existing=sqlx::query("SELECT expected_revision,display_name,visibility FROM account_profile_receipts WHERE user_id=$1 AND operation_id=$2").bind(Uuid::from_u128(snapshot.user_id().0)).bind(Uuid::from_u128(id.get())).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = existing {
            if unsigned(row.try_get("expected_revision").map_err(unavailable)?)? != expected
                || row
                    .try_get::<&str, _>("display_name")
                    .map_err(unavailable)?
                    != name.as_str()
                || row.try_get::<&str, _>("visibility").map_err(unavailable)? != visibility.as_str()
            {
                let checked = credential.observe(&mut tx, ActivityKind::Read).await;
                self.commit(tx).await?;
                checked?;
                return Err(SessionError::InvalidInput);
            }
            let checked = credential.observe(&mut tx, ActivityKind::Read).await;
            self.commit(tx).await?;
            return checked.map(|_| ());
        }
        let receipts: i64 =
            sqlx::query_scalar("SELECT count(*) FROM account_profile_receipts WHERE user_id=$1")
                .bind(Uuid::from_u128(snapshot.user_id().0))
                .fetch_one(&mut *tx)
                .await
                .map_err(unavailable)?;
        if receipts >= 1024 {
            self.commit(tx).await?;
            return Err(SessionError::Unavailable);
        }
        sqlx::query("SAVEPOINT account_profile_edit")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let written=sqlx::query("UPDATE account_profiles SET display_name=$2,visibility=$3,revision=revision+1 WHERE user_id=$1 AND revision=$4").bind(Uuid::from_u128(snapshot.user_id().0)).bind(name.as_str()).bind(visibility.as_str()).bind(signed(expected)?).execute(&mut *tx).await.map_err(unavailable)?;
        if written.rows_affected() != 1 {
            let checked = credential.observe(&mut tx, ActivityKind::Read).await;
            self.commit(tx).await?;
            checked?;
            return Err(SessionError::Conflict);
        }
        sqlx::query("INSERT INTO account_profile_receipts(user_id,operation_id,expected_revision,display_name,visibility,resulting_revision) VALUES($1,$2,$3,$4,$5,$3+1)").bind(Uuid::from_u128(snapshot.user_id().0)).bind(Uuid::from_u128(id.get())).bind(signed(expected)?).bind(name.as_str()).bind(visibility.as_str()).execute(&mut *tx).await.map_err(unavailable)?;
        let checked = credential
            .observe(&mut tx, ActivityKind::ProtectedMutationCommitted)
            .await;
        if checked.is_err() {
            sqlx::query("ROLLBACK TO SAVEPOINT account_profile_edit")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            credential.save_observation(&mut tx).await?;
        }
        self.commit(tx).await?;
        checked.map(|_| ())
    }
}

#[cfg(test)]
mod tests;
