//! Storage-owned local/dev startup and readiness (doc 03 §19.3, doc 06 §11.2).
//!
//! Issue #110 PR01 composes the existing isolated schemas without enabling the
//! general production schema. One strict migration history preserves every
//! reviewed migration version and checksum. Check-only startup never migrates.

use std::{fmt, time::Duration};

use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Transaction};

use crate::{
    match_postgres::PgMatchStore, online_match::PgOnlineMatchStore, session::PgSessionStore,
};

const DATABASE_BUDGET: Duration = Duration::from_secs(5);
const STARTUP_BUDGET: Duration = Duration::from_secs(30);

/// Explicit operator choice for the existing additive local/dev migrations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaPolicy {
    /// Apply the complete reviewed history, then verify readiness.
    Apply,
    /// Verify the complete history and required objects without changing schema.
    Check,
}

/// Checked bounds for one local/dev process (doc 03 §19.3).
///
/// Online owners retain detached physical backends outside the `SQLx` pool.
/// The total database budget therefore includes both service pool maxima plus
/// the selected historical room capacity, not merely `max_connections`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalDevStorageConfig {
    max_connections: u32,
    lifetime_room_capacity: u16,
}
impl LocalDevStorageConfig {
    /// Rejects unbounded pools or any widening of the reviewed 128-room ceiling.
    pub fn new(
        max_connections: u32,
        lifetime_room_capacity: u16,
    ) -> Result<Self, LocalDevStorageError> {
        if !(1..=40).contains(&max_connections) || !(1..=128).contains(&lifetime_room_capacity) {
            return Err(LocalDevStorageError::InvalidConfig);
        }
        Ok(Self {
            max_connections,
            lifetime_room_capacity,
        })
    }
    /// Maximum pooled backends, excluding retained native match-owner backends.
    pub const fn max_connections(self) -> u32 {
        self.max_connections
    }
    /// Maximum historical room IDs, including expired and completed rooms.
    pub const fn lifetime_room_capacity(self) -> u16 {
        self.lifetime_room_capacity
    }
}

/// Redacted operational failures; never includes a URL, SQL or database detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LocalDevStorageError {
    #[error("invalid local/dev storage configuration")]
    InvalidConfig,
    #[error("local/dev database unavailable")]
    DatabaseUnavailable,
    #[error("local/dev schema is absent, stale or incompatible")]
    SchemaUnavailable,
    #[error("local/dev migration failed")]
    MigrationFailed,
}

/// Single shared local/dev pool with no service-owned SQL (I-15).
#[derive(Clone)]
pub struct LocalDevStore {
    pool: PgPool,
    config: LocalDevStorageConfig,
}
impl fmt::Debug for LocalDevStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalDevStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

fn migrator() -> Result<sqlx::migrate::Migrator, LocalDevStorageError> {
    let mut migrations = sqlx::migrate!("./session_migrations")
        .iter()
        .chain(sqlx::migrate!("./accounts_migrations").iter())
        .chain(sqlx::migrate!("./social_migrations").iter())
        .chain(sqlx::migrate!("./match_migrations").iter())
        .chain(sqlx::migrate!("./online_match_migrations").iter())
        .cloned()
        .collect::<Vec<_>>();
    migrations.sort_by_key(|migration| migration.version);
    if migrations
        .windows(2)
        .any(|pair| pair[0].version == pair[1].version)
    {
        return Err(LocalDevStorageError::SchemaUnavailable);
    }
    Ok(sqlx::migrate::Migrator::with_migrations(migrations))
}

impl LocalDevStore {
    /// Connects with bounded acquisition and statement/idle-transaction deadlines.
    /// Schema changes occur only under the caller's explicit `Apply` policy.
    pub async fn connect(
        database_url: &str,
        config: LocalDevStorageConfig,
        policy: SchemaPolicy,
    ) -> Result<Self, LocalDevStorageError> {
        let pool = tokio::time::timeout(
            DATABASE_BUDGET,
            PgPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(DATABASE_BUDGET)
                .after_connect(|connection, _| {
                    Box::pin(async move {
                        sqlx::query("SET statement_timeout = '5s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("SET idle_in_transaction_session_timeout = '10s'")
                            .execute(connection)
                            .await?;
                        Ok(())
                    })
                })
                .connect(database_url),
        )
        .await
        .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?
        .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?;
        let store = Self { pool, config };
        let result = if policy == SchemaPolicy::Apply {
            tokio::time::timeout(STARTUP_BUDGET, store.apply_schema())
                .await
                .map_err(|_| LocalDevStorageError::MigrationFailed)
                .and_then(|result| result)
        } else {
            Ok(())
        };
        let result = match result {
            Ok(()) => store.check_ready().await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            store.pool.close().await;
            return Err(error);
        }
        Ok(store)
    }

    async fn apply_schema(&self) -> Result<(), LocalDevStorageError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?;
        // SQLx's session migration lock can survive validation failure/cancel.
        // This physical backend must close instead of returning to the pool.
        connection.close_on_drop();
        let result = migrator()?
            .run(&mut *connection)
            .await
            .map_err(|_| LocalDevStorageError::MigrationFailed);
        let closed = connection
            .close()
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable);
        result.and(closed)
    }

    /// Rechecks connectivity, exact history and the required authority objects.
    /// Read-only verification does not apply migrations or issue any session.
    pub async fn check_ready(&self) -> Result<(), LocalDevStorageError> {
        tokio::time::timeout(DATABASE_BUDGET, self.check_schema())
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?
    }

    async fn check_schema(&self) -> Result<(), LocalDevStorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable)?;
        check_history(&mut tx).await?;
        // This zero-row query validates all required columns and access without
        // fetching authority rows, canonical state or credentials.
        sqlx::query(include_str!("schema_probe.sql"))
            .execute(&mut *tx)
            .await
            .map_err(|_| LocalDevStorageError::SchemaUnavailable)?;
        let fences: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid \
             JOIN pg_namespace n ON n.oid=c.relnamespace \
             JOIN pg_proc p ON p.oid=t.tgfoid \
             WHERE n.nspname=current_schema() AND t.tgenabled='O' \
             AND t.tgdeferrable AND t.tginitdeferred AND NOT t.tgisinternal \
             AND ((c.relname='online_session_commit_guards' \
             AND t.tgname='online_session_commit_fence' \
             AND p.proname='online_validate_session_commit') \
             OR (c.relname='match_journal_heads' \
             AND t.tgname='online_match_commit_fence' \
             AND p.proname='online_validate_match_commit'))",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| LocalDevStorageError::SchemaUnavailable)?;
        if fences != 2 {
            return Err(LocalDevStorageError::SchemaUnavailable);
        }
        tx.commit()
            .await
            .map_err(|_| LocalDevStorageError::DatabaseUnavailable)
    }

    /// Existing current durable session/account/social authority.
    pub fn session_store(&self) -> PgSessionStore {
        PgSessionStore::new(self.pool.clone())
    }
    /// Existing generic canonical journal and physical owner authority.
    pub fn match_store(&self) -> PgMatchStore {
        PgMatchStore::new(self.pool.clone())
    }
    /// Existing join-code authority with the explicit historical room limit.
    pub fn online_match_store(&self) -> PgOnlineMatchStore {
        // Config's private checked fields establish the constructor's bounds.
        PgOnlineMatchStore::with_lifetime_room_capacity(
            self.pool.clone(),
            self.config.lifetime_room_capacity,
        )
        .expect("checked local/dev room capacity")
    }
    /// Closes the shared `SQLx` pool during local/dev shutdown.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

async fn check_history(tx: &mut Transaction<'_, Postgres>) -> Result<(), LocalDevStorageError> {
    let applied: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut **tx)
            .await
            .map_err(|_| LocalDevStorageError::SchemaUnavailable)?;
    let expected = migrator()?;
    if applied.len() != expected.iter().count()
        || applied
            .iter()
            .zip(expected.iter())
            .any(|((version, checksum, success), migration)| {
                !success
                    || *version != migration.version
                    || checksum.as_slice() != migration.checksum.as_ref()
            })
    {
        return Err(LocalDevStorageError::SchemaUnavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
