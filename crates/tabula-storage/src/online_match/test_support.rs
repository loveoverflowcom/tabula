//! Deliberate current-membership loss in the disposable PR3 fixture only.
use super::{
    begin, lock_room, unavailable, LockedCredential, OnlineMatchError, PgOnlineMatchStore,
};
use tabula_core::{MatchId, UserId};
use tabula_session::{ActivityKind, CredentialOperation};
use uuid::Uuid;

impl PgOnlineMatchStore {
    /// Non-locking membership predicate for the disposable committed-prefix oracle.
    /// This returns no scope, seat, receipt or canonical match information.
    pub async fn has_fixture_membership(
        &self,
        subject: UserId,
        match_id: MatchId,
    ) -> Result<bool, OnlineMatchError> {
        let mut tx = begin(&self.pool).await?;
        crate::session::online_test_support::disposable(&mut tx).await?;
        let result: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM online_match_admissions WHERE match_id=$1 AND user_id=$2)",
        )
        .bind(Uuid::from_u128(match_id.0))
        .bind(Uuid::from_u128(subject.0))
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }

    /// Remove the caller's admissions and current membership under normal locks.
    ///
    /// The immutable canonical roster is untouched. Subsequent grant, command
    /// and publication checks must reject this former member (ADR-0031).
    pub async fn invalidate_membership_for_test(
        &self,
        request: CredentialOperation,
        match_id: MatchId,
    ) -> Result<(), OnlineMatchError> {
        let mut tx = begin(&self.pool).await?;
        crate::session::online_test_support::disposable(&mut tx).await?;
        let mut authority = LockedCredential::lock(&mut tx, request).await?;
        let snapshot = authority.observe(&mut tx, ActivityKind::Control).await?;
        let _room = lock_room(&mut tx, match_id).await?;
        let subject = Uuid::from_u128(snapshot.user_id().0);
        let id = Uuid::from_u128(match_id.0);
        sqlx::query("DELETE FROM online_match_admissions WHERE match_id=$1 AND user_id=$2")
            .bind(id)
            .bind(subject)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let removed =
            sqlx::query("DELETE FROM online_match_memberships WHERE match_id=$1 AND user_id=$2")
                .bind(id)
                .bind(subject)
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
        if removed.rows_affected() != 1 {
            return Err(OnlineMatchError::Unavailable);
        }
        tx.commit().await.map_err(unavailable)
    }
}
