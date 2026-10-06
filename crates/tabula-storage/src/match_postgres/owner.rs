//! Native process-lifetime owner exclusion and bounded first-frame fencing.
//! SQL and deployment time remain here, outside the deterministic kernel (I-1).

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    time::Duration,
};

use sqlx::{Connection, PgConnection, Postgres, Transaction};
use tabula_core::MatchId;
use tokio::{sync::oneshot, time::Instant};
use uuid::Uuid;

use super::{signed, unavailable, PgMatchJournal, RuntimePortError};

const PUBLICATION_MS: u64 = 2_000;
pub(super) const OWNER_BUDGET: Duration = Duration::from_secs(5);

/// Retained only by the online writer's clones, never by its cleanup worker.
pub(super) struct OwnerLock {
    active: Arc<AtomicBool>,
    release: Option<oneshot::Sender<()>>,
    #[cfg(any(test, feature = "match-postgres-test-support"))]
    pub(super) backend_pid: i32,
}

impl OwnerLock {
    pub(super) fn retain(
        mut connection: PgConnection,
        #[cfg(any(test, feature = "match-postgres-test-support"))] backend_pid: i32,
    ) -> Arc<Self> {
        let active = Arc::new(AtomicBool::new(true));
        let worker_active = active.clone();
        let (release, mut cancelled) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut cancelled => break,
                    () = tokio::time::sleep(Duration::from_millis(250)) => {},
                }
                let result = tokio::select! {
                    _ = &mut cancelled => break,
                    result = tokio::time::timeout(Duration::from_secs(1), sqlx::query("SELECT 1").execute(&mut connection)) => result,
                };
                if !matches!(result, Ok(Ok(_))) {
                    break;
                }
            }
            worker_active.store(false, Ordering::Release);
            // Never return a physical backend retaining a session advisory lock
            // to a pool, including cancellation and failure paths.
            let _ = connection.close().await;
        });
        Arc::new(Self {
            active,
            release: Some(release),
            #[cfg(any(test, feature = "match-postgres-test-support"))]
            backend_pid,
        })
    }

    pub(super) fn check(&self) -> Result<(), RuntimePortError> {
        if self.active.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(RuntimePortError::Unavailable)
        }
    }
}

impl Drop for OwnerLock {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

/// Same relation-resolved key for online lifetime and offline transaction locks.
/// The two-integer namespace does not collide with account one-bigint keys.
pub(super) async fn lock_transaction_owner(
    tx: &mut Transaction<'_, Postgres>,
    id: MatchId,
) -> Result<(), RuntimePortError> {
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(542, hashtext('match_journal_heads'::regclass::oid::text || ':' || $1::uuid::text))")
        .bind(Uuid::from_u128(id.0)).fetch_one(&mut **tx).await.map_err(unavailable)?;
    if acquired {
        Ok(())
    } else {
        Err(RuntimePortError::Busy)
    }
}

pub(super) async fn check_publication_window(
    tx: &mut Transaction<'_, Postgres>,
    id: MatchId,
) -> Result<(), RuntimePortError> {
    let window: Option<(i64, i64)> = sqlx::query_as(
        "SELECT started_at_ms, until_ms FROM match_journal_publications WHERE match_id = $1",
    )
    .bind(Uuid::from_u128(id.0))
    .fetch_optional(&mut **tx)
    .await
    .map_err(unavailable)?;
    if let Some((started, until)) = window {
        if started < 0
            || until
                .checked_sub(started)
                .is_none_or(|n| !(1..=2000).contains(&n))
        {
            return Err(RuntimePortError::Unavailable);
        }
        let now = database_time(tx).await?;
        if now < until {
            return Err(RuntimePortError::Busy);
        }
    }
    Ok(())
}

async fn database_time(tx: &mut Transaction<'_, Postgres>) -> Result<i64, RuntimePortError> {
    let now: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
            .fetch_one(&mut **tx)
            .await
            .map_err(unavailable)?;
    if now >= 0 {
        Ok(now)
    } else {
        Err(RuntimePortError::Unavailable)
    }
}

/// One-shot current-owner first-frame guard (doc 03 §10, I-14).
///
/// Its committed exclusion prevents a newer fence even if the ownership
/// backend dies. The local monotonic deadline expires one millisecond earlier
/// than database exclusion, under ADR-0036's trusted database-clock assumption.
/// Callbacks must only construct/stage bounded output, never perform I/O; a late
/// callback result is discarded. This is not a distributed ownership lease.
pub struct PgMatchPublication {
    owner: Option<Weak<OwnerLock>>,
    expires_at: Instant,
    published: bool,
}

impl fmt::Debug for PgMatchPublication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PgMatchPublication([REDACTED])")
    }
}

impl PgMatchPublication {
    fn check(&self) -> Result<(), RuntimePortError> {
        if Instant::now() >= self.expires_at {
            return Err(RuntimePortError::Unavailable);
        }
        if let Some(owner) = &self.owner {
            owner
                .upgrade()
                .ok_or(RuntimePortError::Unavailable)?
                .check()?;
        }
        Ok(())
    }

    /// Guard bounded pure first-frame construction on both sides of its callback.
    pub fn publish<T>(&mut self, action: impl FnOnce() -> T) -> Result<T, RuntimePortError> {
        self.check()?;
        if self.published {
            return Err(RuntimePortError::Unavailable);
        }
        self.published = true;
        let result = action();
        self.check()?;
        Ok(result)
    }
}

impl PgMatchJournal {
    /// Fresh generation authority before staging or actual first-frame handoff.
    /// Failure cannot authorize old queued output. The whole SQL operation is bounded.
    pub async fn begin_publication(&self) -> Result<PgMatchPublication, RuntimePortError> {
        tokio::time::timeout(OWNER_BUDGET, self.publication_inner())
            .await
            .map_err(unavailable)?
    }

    async fn publication_inner(&self) -> Result<PgMatchPublication, RuntimePortError> {
        self.check_native_owner()?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        super::write_settings(&mut tx).await?;
        let _head = self.lock_head(&mut tx).await?;
        let started = Instant::now();
        let now = database_time(&mut tx).await?;
        let until = now
            .checked_add(signed(PUBLICATION_MS)?)
            .ok_or(RuntimePortError::Unavailable)?;
        sqlx::query("INSERT INTO match_journal_publications(match_id, fence, started_at_ms, until_ms) VALUES ($1,$2,$3,$4) ON CONFLICT(match_id) DO UPDATE SET fence = EXCLUDED.fence, started_at_ms = GREATEST(match_journal_publications.until_ms, EXCLUDED.until_ms) - 2000, until_ms = GREATEST(match_journal_publications.until_ms, EXCLUDED.until_ms)")
            .bind(Uuid::from_u128(self.match_id.0)).bind(self.fence).bind(now).bind(until)
            .execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit()
            .await
            .map_err(|_| RuntimePortError::Indeterminate)?;
        let guard = PgMatchPublication {
            owner: self.owner.as_ref().map(Arc::downgrade),
            expires_at: started + Duration::from_millis(PUBLICATION_MS - 1),
            published: false,
        };
        guard.check()?;
        Ok(guard)
    }

    pub(super) fn check_native_owner(&self) -> Result<(), RuntimePortError> {
        match &self.owner {
            Some(owner) => owner.check(),
            None => Ok(()),
        }
    }

    /// Local lifetime-lock health only; durable generation authority requires
    /// `begin_publication` or a fenced mutation. This is not a recovery cursor.
    pub fn is_owner_active(&self) -> bool {
        self.check_native_owner().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_guard_is_once_and_checks_expiry_after_pure_construction() {
        let mut guard = PgMatchPublication {
            owner: None,
            expires_at: Instant::now() + Duration::from_secs(1),
            published: false,
        };
        assert_eq!(guard.publish(|| 7), Ok(7));
        let mut repeated = false;
        assert_eq!(
            guard.publish(|| repeated = true),
            Err(RuntimePortError::Unavailable)
        );
        assert!(!repeated);
        let mut guard = PgMatchPublication {
            owner: None,
            expires_at: Instant::now() + Duration::from_millis(50),
            published: false,
        };
        let mut constructed = false;
        assert_eq!(
            guard.publish(|| {
                constructed = true;
                std::thread::sleep(Duration::from_millis(100));
                "late private frame"
            }),
            Err(RuntimePortError::Unavailable)
        );
        assert!(
            constructed,
            "post-construction expiry case must reach the callback"
        );
    }
}
