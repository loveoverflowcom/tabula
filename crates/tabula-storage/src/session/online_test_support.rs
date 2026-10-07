//! Storage-owned fault controls for an explicitly disposable PR3 database.
//! These controls do not exist in ordinary application builds (ADR-0031).

use super::{signed, unavailable, PgSessionStore};
use tabula_session::{CredentialOperation, SessionError};

impl PgSessionStore {
    /// Establish a valid short remaining idle lifetime for a current fixture record.
    /// Only bounded disposable expiry races may use this control (ADR-0031).
    pub async fn shorten_credential_lifetime_for_test(
        &self,
        request: CredentialOperation,
        remaining_ms: u64,
    ) -> Result<(), SessionError> {
        if !(1..=5_000).contains(&remaining_ms) {
            return Err(SessionError::InvalidInput);
        }
        let mut tx = self.begin().await?;
        disposable(&mut tx).await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        let now = Self::database_clock(&mut tx).await?;
        account.observe(now)?;
        session.observe_operation(&account, request, now)?;
        let raw = session.into_raw();
        let now = signed(now.get())?;
        let remaining = signed(remaining_ms)?;
        let written=sqlx::query("UPDATE session_auth_sessions SET created_at_ms=$2-1800000+$3, last_activity_at_ms=$2-1800000+$3, last_observed_at_ms=$2, idle_deadline_ms=$2+$3, absolute_deadline_ms=$2-1800000+$3+86400000 WHERE id=$1 AND credential_digest=$4")
            .bind(uuid::Uuid::from_u128(raw.id)).bind(now).bind(remaining).bind(request.digest.as_bytes().as_slice())
            .execute(&mut *tx).await.map_err(unavailable)?;
        if written.rows_affected() != 1 {
            return Err(SessionError::Unavailable);
        }
        Self::save_account(&mut tx, &account).await?;
        self.commit(tx).await
    }

    /// Move only the selected current fixture record to a valid, expired history.
    ///
    /// The account/session/publication ordering is the ordinary ordering. This
    /// control never grants authority or alters a canonical game input.
    pub async fn expire_credential_for_test(
        &self,
        request: CredentialOperation,
    ) -> Result<(), SessionError> {
        let mut tx = self.begin().await?;
        disposable(&mut tx).await?;
        let (mut account, mut session) = Self::lock_digest(&mut tx, request.digest).await?;
        let now = Self::database_clock(&mut tx).await?;
        account.observe(now)?;
        session.observe_operation(&account, request, now)?;
        let raw = session.into_raw();
        if raw.channel != request.channel.as_str()
            || raw.revoked_at_ms.is_some()
            || raw.expired_at_ms.is_some()
        {
            return Err(SessionError::Unauthenticated);
        }
        let now = signed(now.get())?;
        let written = sqlx::query("UPDATE session_auth_sessions SET created_at_ms=$2-86400000, last_activity_at_ms=$2-1800000, last_observed_at_ms=$2, idle_deadline_ms=$2, absolute_deadline_ms=$2, expired_at_ms=$2 WHERE id=$1 AND credential_digest=$3")
            .bind(uuid::Uuid::from_u128(raw.id))
            .bind(now)
            .bind(request.digest.as_bytes().as_slice())
            .execute(&mut *tx).await.map_err(unavailable)?;
        if written.rows_affected() != 1 {
            return Err(SessionError::Unavailable);
        }
        Self::save_account(&mut tx, &account).await?;
        self.commit(tx).await
    }
}

pub(crate) async fn disposable(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), SessionError> {
    if std::env::var("TABULA_ONLINE_MATCH_DISPOSABLE").as_deref() != Ok("1")
        || !matches!(std::env::var("CI").as_deref(), Ok("true" | "1"))
    {
        return Err(SessionError::Unavailable);
    }
    let name: String = sqlx::query_scalar("SELECT current_database()::text")
        .fetch_one(&mut **tx)
        .await
        .map_err(unavailable)?;
    if name != "tabula_online_acceptance" && !name.starts_with("tabula_match_acceptance_") {
        return Err(SessionError::Unavailable);
    }
    Ok(())
}
