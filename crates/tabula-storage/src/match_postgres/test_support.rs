//! Explicit non-production fault injection for disposable ADR-0040 acceptance.
//!
//! This module is absent unless `match-postgres-test-support` is selected.
//! Helpers alter only the caller's selected test database; never use live data.

use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::postgres::PgPoolOptions;
use tabula_core::{InputIndex, MatchId};
use tabula_match_journal::{JournalRecord, ScopeState};
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{
    checksum, encode, strict_decode, unavailable, PgMatchJournal, PgMatchStore, RuntimePortError,
};

/// Transaction barrier after writes are staged, before `PostgreSQL` COMMIT.
#[derive(Debug)]
pub struct CommitPause {
    entered: oneshot::Receiver<()>,
    release: oneshot::Sender<()>,
}

impl CommitPause {
    /// Wait until the transaction owns the head and has staged its changes.
    pub async fn wait_until_entered(&mut self) -> Result<(), RuntimePortError> {
        (&mut self.entered).await.map_err(unavailable)
    }

    /// Allow the staged transaction to commit.
    pub fn release(self) -> Result<(), RuntimePortError> {
        self.release.send(()).map_err(unavailable)
    }
}

/// Deliberate persisted corruption in an explicitly disposable database.
#[derive(Clone, Copy, Debug)]
pub enum Corruption {
    /// Delete the latest non-genesis row without changing the head.
    InputGap,
    /// Drift head counters while retaining all valid SQL constraints.
    HeadVersion,
    /// Damage a record's independent encoded-byte checksum.
    RecordChecksum,
    /// Damage the ledger's independent encoded-byte checksum.
    LedgerChecksum,
}

impl PgMatchJournal {
    /// Roll back the next staged write, simulating a known pre-commit failure.
    pub fn fail_before_commit(&self) {
        self.controls.next.store(1, Ordering::SeqCst);
    }

    /// Commit durably, then report an unknown response to the next write.
    pub fn lose_next_commit_response(&self) {
        self.controls.next.store(2, Ordering::SeqCst);
    }

    /// Pause the next write only after all changes have been staged.
    pub fn pause_before_commit(&self) -> Result<CommitPause, RuntimePortError> {
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let mut current = self.controls.pause.lock().map_err(unavailable)?;
        if current.is_some() {
            return Err(RuntimePortError::Busy);
        }
        *current = Some((entered_tx, release_rx));
        Ok(CommitPause {
            entered: entered_rx,
            release: release_tx,
        })
    }
}

impl PgMatchStore {
    /// Create a fresh disposable schema and its pool, independent of other tests.
    ///
    /// A process crash may leave this test schema behind. Disposable CI destroys
    /// the database itself; no live migrations or persistent credentials are used.
    pub async fn isolated_test_pool(
        url: &str,
        label: &str,
    ) -> Result<sqlx::PgPool, RuntimePortError> {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        if label.is_empty()
            || label.len() > 24
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(RuntimePortError::Unavailable);
        }
        let bootstrap = PgPoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(unavailable)?;
        let schema = format!(
            "match_test_{}_{}_{}",
            label,
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        if schema.len() > 63 {
            return Err(RuntimePortError::Unavailable);
        }
        // Identifier is restricted above to ASCII letters, digits and underscores.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&bootstrap)
            .await
            .map_err(unavailable)?;
        bootstrap.close().await;
        Self::test_pool_in_schema(url, &schema).await
    }

    /// Open an independent pool in a verified disposable test schema.
    pub async fn test_pool_in_schema(
        url: &str,
        schema: &str,
    ) -> Result<sqlx::PgPool, RuntimePortError> {
        if !schema.starts_with("match_test_")
            || schema.len() > 63
            || !schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(RuntimePortError::Unavailable);
        }
        let search_path = schema.to_owned();
        PgPoolOptions::new()
            .max_connections(8)
            .after_connect(move |connection, _| {
                let sql = search_path.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(&sql)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(url)
            .await
            .map_err(unavailable)
    }

    /// Exact backend version for the `PostgreSQL` 16 acceptance claim.
    pub async fn server_version_num(&self) -> Result<u32, RuntimePortError> {
        let version: String = sqlx::query_scalar("SHOW server_version_num")
            .fetch_one(&self.pool)
            .await
            .map_err(unavailable)?;
        version.parse().map_err(unavailable)
    }

    async fn check_disposable_schema(&self) -> Result<(), RuntimePortError> {
        let schema = self.test_schema().await?;
        if !(schema.starts_with("match_test_") || schema.starts_with("tabula_match_acceptance_")) {
            return Err(RuntimePortError::Unavailable);
        }
        Ok(())
    }

    /// Install a test-only deferred constraint trigger that rejects non-genesis
    /// INSERTs at `PostgreSQL` COMMIT, after every ordinary statement succeeded.
    /// The selected schema must be a generated disposable acceptance schema.
    pub async fn install_commit_failure_for_test(&self) -> Result<(), RuntimePortError> {
        self.check_disposable_schema().await?;
        sqlx::raw_sql("CREATE FUNCTION match_journal_test_fail_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'disposable match journal commit failure'; RETURN NEW; END; $$; CREATE CONSTRAINT TRIGGER match_journal_test_commit_failure AFTER INSERT ON match_journal_records DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (NEW.input_index > 0) EXECUTE FUNCTION match_journal_test_fail_commit();")
            .execute(&self.pool).await.map_err(unavailable)?;
        Ok(())
    }

    /// Remove the disposable deferred-COMMIT fixture, allowing a recovery retry.
    pub async fn clear_commit_failure_for_test(&self) -> Result<(), RuntimePortError> {
        self.check_disposable_schema().await?;
        sqlx::raw_sql("DROP TRIGGER match_journal_test_commit_failure ON match_journal_records; DROP FUNCTION match_journal_test_fail_commit();")
            .execute(&self.pool).await.map_err(unavailable)?;
        Ok(())
    }

    /// Observe the committed prefix through one non-locking MVCC statement.
    /// Staged writes remain invisible while a pre-commit test gate is held.
    pub async fn committed_prefix_for_test(
        &self,
        match_id: MatchId,
    ) -> Result<(tabula_core::StateVersion, InputIndex, u64), RuntimePortError> {
        let (version, index, count): (i64, i64, i64) = sqlx::query_as("SELECT version, input_index, (SELECT COUNT(*)::bigint FROM match_journal_records WHERE match_id = $1) FROM match_journal_heads WHERE match_id = $1")
            .bind(Uuid::from_u128(match_id.0)).fetch_one(&self.pool).await.map_err(unavailable)?;
        Ok((
            tabula_core::StateVersion(super::unsigned(version)?),
            InputIndex(super::unsigned(index)?),
            super::unsigned(count)?,
        ))
    }

    /// Name of this pool's fresh schema, for independent reopened test pools.
    pub async fn test_schema(&self) -> Result<String, RuntimePortError> {
        sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&self.pool)
            .await
            .map_err(unavailable)
    }

    /// Change record contents and matching storage checksums, leaving semantic
    /// validation to recovery. The fixture closure must never see production data.
    pub async fn mutate_record_for_test<F>(
        &self,
        match_id: MatchId,
        index: InputIndex,
        mutate: F,
    ) -> Result<(), RuntimePortError>
    where
        F: FnOnce(&mut JournalRecord) + Send,
    {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let payload: Vec<u8> = sqlx::query_scalar("SELECT payload FROM match_journal_records WHERE match_id = $1 AND input_index = $2 FOR UPDATE")
            .bind(Uuid::from_u128(match_id.0)).bind(super::signed(index.0)?).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let mut record: JournalRecord = strict_decode(&payload)?;
        mutate(&mut record);
        let changed = encode(&record)?;
        sqlx::query("UPDATE match_journal_records SET payload = $3, payload_bytes = $4, payload_hash = $5, state_hash = $6 WHERE match_id = $1 AND input_index = $2")
            .bind(Uuid::from_u128(match_id.0)).bind(super::signed(index.0)?).bind(&changed).bind(i64::try_from(changed.len()).map_err(unavailable)?).bind(checksum(&changed)).bind(record.hash.0.as_slice()).execute(&mut *tx).await.map_err(unavailable)?;
        let difference = i64::try_from(changed.len()).map_err(unavailable)?
            - i64::try_from(payload.len()).map_err(unavailable)?;
        sqlx::query("UPDATE match_journal_heads SET record_bytes = record_bytes + $2, latest_hash = CASE WHEN input_index = $3 THEN $4 ELSE latest_hash END WHERE match_id = $1")
            .bind(Uuid::from_u128(match_id.0)).bind(difference).bind(super::signed(index.0)?).bind(record.hash.0.as_slice()).execute(&mut *tx).await.map_err(unavailable)?;
        // A changed genesis creation has exact matching head bytes; this lets the
        // recovery actor reject a foreign package identity rather than a checksum.
        if index.0 == 0 {
            if let Some(creation) = &record.creation {
                let bytes = encode(creation)?;
                sqlx::query("UPDATE match_journal_heads SET creation = $2, creation_hash = $3 WHERE match_id = $1")
                    .bind(Uuid::from_u128(match_id.0)).bind(&bytes).bind(checksum(&bytes)).execute(&mut *tx).await.map_err(unavailable)?;
            }
        }
        tx.commit().await.map_err(unavailable)
    }

    /// Change ledger contents with matching checksums for semantic failure cases.
    pub async fn mutate_ledger_for_test<F>(
        &self,
        match_id: MatchId,
        mutate: F,
    ) -> Result<(), RuntimePortError>
    where
        F: FnOnce(&mut Vec<ScopeState>) + Send,
    {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let bytes: Vec<u8> = sqlx::query_scalar(
            "SELECT ledger FROM match_journal_heads WHERE match_id = $1 FOR UPDATE",
        )
        .bind(Uuid::from_u128(match_id.0))
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let mut ledger: Vec<ScopeState> = strict_decode(&bytes)?;
        mutate(&mut ledger);
        let changed = encode(&ledger)?;
        sqlx::query(
            "UPDATE match_journal_heads SET ledger = $2, ledger_hash = $3 WHERE match_id = $1",
        )
        .bind(Uuid::from_u128(match_id.0))
        .bind(&changed)
        .bind(checksum(&changed))
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }

    /// Inject one raw-row corruption without admitting SQL into runtime crates.
    pub async fn corrupt_for_test(
        &self,
        match_id: MatchId,
        corruption: Corruption,
    ) -> Result<(), RuntimePortError> {
        let sql = match corruption {
            Corruption::InputGap => "DELETE FROM match_journal_records WHERE match_id = $1 AND input_index = (SELECT input_index FROM match_journal_heads WHERE match_id = $1) AND input_index > 0",
            Corruption::HeadVersion => "UPDATE match_journal_heads SET version = version + 1, input_index = input_index + 1, record_count = record_count + 1 WHERE match_id = $1",
            Corruption::RecordChecksum => "UPDATE match_journal_records SET payload_hash = decode(repeat('ff', 32), 'hex') WHERE match_id = $1",
            Corruption::LedgerChecksum => "UPDATE match_journal_heads SET ledger_hash = decode(repeat('ff', 32), 'hex') WHERE match_id = $1",
        };
        let result = sqlx::query(sql)
            .bind(Uuid::from_u128(match_id.0))
            .execute(&self.pool)
            .await
            .map_err(unavailable)?;
        if result.rows_affected() == 0 {
            return Err(RuntimePortError::Unavailable);
        }
        Ok(())
    }
}
