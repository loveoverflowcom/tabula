//! Bounded multi-account publication exclusions shared by profiles and social output.
use crate::session::{
    local_publication_budget, PgSessionPublication, PgSessionStore, PublicationLease,
    PUBLICATION_LEASE_MS,
};
use sqlx::{Acquire, Postgres, Transaction};
use std::{
    collections::BTreeSet,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tabula_core::UserId;
use tabula_session::{
    AccountRecord, CredentialOperation, SessionBinding, SessionError, SessionPublication,
    SessionSnapshot, UnixMillis,
};
use tokio::{sync::oneshot, time::Instant};
use uuid::Uuid;

/// Caller provenance is current HTTP credential or an established socket binding.
#[derive(Clone, Copy, Debug)]
pub enum AccountsPrincipal {
    Credential(CredentialOperation),
    Binding(SessionBinding),
}
/// One-use first-frame guard over caller and every disclosed account (ADR-0043).
pub struct AccountsPublication<K = ()> {
    inner: PgSessionPublication,
    data: K,
    accounts: BTreeSet<UserId>,
}
impl<K> fmt::Debug for AccountsPublication<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountsPublication([REDACTED])")
    }
}
impl<K> AccountsPublication<K> {
    #[cfg(test)]
    pub(crate) fn backend_pid(&self) -> i32 {
        self.inner.backend_pid
    }
    pub fn data(&self) -> &K {
        &self.data
    }
    pub fn with_data<T>(self, data: T) -> AccountsPublication<T> {
        AccountsPublication {
            inner: self.inner,
            data,
            accounts: self.accounts,
        }
    }
    pub(crate) fn includes(&self, id: UserId) -> bool {
        self.accounts.contains(&id)
    }
    pub(crate) fn with_current<R>(
        &self,
        action: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        self.inner.with_current(action)
    }
}
impl<K: Send> SessionPublication for AccountsPublication<K> {
    fn snapshot(&self) -> &SessionSnapshot {
        self.inner.snapshot()
    }
    fn publish<R>(
        &mut self,
        action: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        self.inner.publish(action)
    }
}
impl<K: Send> tabula_session::SocketFramePublication for AccountsPublication<K> {
    fn handoff<R>(
        &mut self,
        frame: tabula_session::BoundedSocketFrame,
        transport: impl FnOnce(tabula_session::BoundedSocketFrame) -> R,
    ) -> Result<R, SessionError> {
        tabula_session::SocketFramePublication::handoff(&mut self.inner, frame, transport)
    }
}
impl PgSessionStore {
    /// Locks account exclusions in canonical order, before session/pair/resource rows.
    pub(crate) async fn lock_accounts(
        tx: &mut Transaction<'_, Postgres>,
        ids: &[UserId],
    ) -> Result<Vec<AccountRecord>, SessionError> {
        if ids.len() > 201 {
            return Err(SessionError::InvalidInput);
        }
        let ordered: BTreeSet<_> = ids.iter().map(|id| id.0).collect();
        if ordered.len() > 201 || ordered.contains(&0) {
            return Err(SessionError::InvalidInput);
        }
        let mut records = Vec::with_capacity(ordered.len());
        for id in ordered {
            records.push(Self::lock_account(tx, Uuid::from_u128(id)).await?);
        }
        Ok(records)
    }
    /// Starts a bounded multi-account fence; candidate resources must be reselected
    /// under this guard, and a newly appearing peer requires another guard.
    #[allow(clippy::too_many_lines)] // Keep one ordered authority/lease transaction auditable.
    pub async fn begin_accounts_publication(
        &self,
        principal: AccountsPrincipal,
        peers: Vec<UserId>,
    ) -> Result<AccountsPublication, SessionError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        connection.close_on_drop();
        #[cfg(test)]
        let backend_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| SessionError::Unavailable)?;
        let mut tx = connection
            .begin()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        Self::configure_transaction(&mut tx).await?;
        let (caller, id) = match principal {
            AccountsPrincipal::Credential(request) => {
                let row = sqlx::query_file!(
                    "src/session/sql/locate_digest.sql",
                    request.digest.as_bytes().as_slice()
                )
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| SessionError::Unavailable)?
                .ok_or(SessionError::Unauthenticated)?;
                (UserId(row.user_id.as_u128()), row.id)
            }
            AccountsPrincipal::Binding(binding) => {
                (binding.user_id(), Uuid::from_u128(binding.id().get()))
            }
        };
        let mut ids = peers;
        ids.push(caller);
        let mut accounts = Self::lock_accounts(&mut tx, &ids).await?;
        let mut session = Self::lock_session(&mut tx, id, Uuid::from_u128(caller.0)).await?;
        for account in &accounts {
            sqlx::query_file!(
                "src/session/sql/account_advisory_publication.sql",
                Uuid::from_u128(account.user_id().0)
            )
            .execute(&mut *tx)
            .await
            .map_err(|_| SessionError::Unavailable)?;
        }
        let lease_started = Instant::now();
        let now = Self::database_clock(&mut tx).await?;
        for account in &mut accounts {
            account.observe(now)?;
        }
        let account = accounts
            .iter()
            .find(|record| record.user_id() == caller)
            .ok_or(SessionError::Unavailable)?;
        let result = match principal {
            AccountsPrincipal::Credential(request) => {
                session.observe_operation(account, request, now)
            }
            AccountsPrincipal::Binding(binding) => {
                let observed = session.observe(account, now);
                if observed.is_ok() && !session.matches_binding(binding) {
                    Err(SessionError::Unauthenticated)
                } else {
                    observed
                }
            }
        };
        let lease_ms = result.as_ref().map_or(0, |s| {
            s.idle_deadline()
                .min(s.absolute_deadline())
                .get()
                .saturating_sub(now.get())
                .min(PUBLICATION_LEASE_MS)
        });
        let mut owned_lease = None;
        if lease_ms > 0 {
            let database_now = Self::database_clock(&mut tx).await?;
            let until = UnixMillis::new(
                database_now
                    .get()
                    .checked_add(lease_ms)
                    .ok_or(SessionError::Unavailable)?,
            )
            .map_err(|_| SessionError::Unavailable)?;
            owned_lease = Some(PublicationLease {
                started_at: database_now,
                until,
            });
            for account in &accounts {
                Self::save_publication_lease(
                    &mut tx,
                    Uuid::from_u128(account.user_id().0),
                    Some(PublicationLease {
                        started_at: database_now,
                        until,
                    }),
                )
                .await?;
            }
        }
        for account in &accounts {
            Self::save_account(&mut tx, account).await?;
        }
        Self::save_session(&mut tx, &session).await?;
        self.commit(tx).await?;
        let snapshot = result?;
        let expires_at = lease_started + local_publication_budget(lease_ms)?;
        if Instant::now() >= expires_at {
            return Err(SessionError::Unauthenticated);
        }
        let active = Arc::new(AtomicBool::new(true));
        let cleanup = active.clone();
        let transfer_fence = Arc::new(crate::session::SocketTransferFence::default());
        let cleanup_transfer = Arc::clone(&transfer_fence);
        let (release, cancelled) = oneshot::channel();
        let cleanup_accounts = accounts
            .iter()
            .map(|a| Uuid::from_u128(a.user_id().0))
            .collect::<Vec<_>>();
        tokio::spawn(async move {
            tokio::select! {_ = cancelled=>{},()=tokio::time::sleep_until(expires_at)=>{},}
            cleanup.store(false, Ordering::Release);
            cleanup_transfer.wait().await;
            if let Some(lease) = owned_lease {
                Self::clear_owned_publication_leases(&mut connection, &cleanup_accounts, lease)
                    .await;
            }
            let _ = connection.close().await;
        });
        Ok(AccountsPublication {
            inner: PgSessionPublication {
                snapshot,
                active,
                published: false,
                expires_at,
                release: Some(release),
                transfer_fence,
                #[cfg(test)]
                backend_pid,
            },
            data: (),
            accounts: accounts
                .into_iter()
                .map(|account| account.user_id())
                .collect(),
        })
    }
}
