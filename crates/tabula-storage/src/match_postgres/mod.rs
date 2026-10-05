//! Isolated atomic match journal (doc 03 §9, ADR-0040).
//!
//! The caller opts into migrations and pool configuration. Claims fence every
//! append, ledger update and repeatable-read recovery; constructors do no I/O.
//! Runtime typed SQLx queries are the documented fallback, not macro-checked
//! queries. Real `PostgreSQL` acceptance owns their schema/type evidence.

use std::fmt;

use serde::{de::DeserializeOwned, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tabula_core::{
    canonical_decode, canonical_encode, InputIndex, MatchId, StateVersion, ENCODING_VERSION,
};
use tabula_match_journal::{
    Journal, JournalRecord, LoadedMatch, MatchCreation, OperationKey, RuntimePortError, ScopeState,
    JOURNAL_FORMAT, MAX_LEDGER_BYTES, MAX_RECOVERY_BYTES, MAX_RECOVERY_RECORDS, MAX_SNAPSHOT_BYTES,
};
use uuid::Uuid;

#[cfg(any(test, feature = "match-postgres-test-support"))]
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex,
};
#[cfg(any(test, feature = "match-postgres-test-support"))]
use tokio::sync::oneshot;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./match_migrations");

/// Explicitly selected pool for the isolated journal; no automatic migration.
#[derive(Clone)]
pub struct PgMatchStore {
    pool: PgPool,
}

impl fmt::Debug for PgMatchStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgMatchStore").finish_non_exhaustive()
    }
}

/// Match-scoped writer with a durable generation fence (ADR-0040).
///
/// Clones share the same owner generation. A new claim invalidates all old
/// clones, including reads. A failed/unknown commit never authorizes output.
#[derive(Clone)]
pub struct PgMatchJournal {
    pool: PgPool,
    match_id: MatchId,
    fence: i64,
    #[cfg(any(test, feature = "match-postgres-test-support"))]
    controls: Arc<TestControls>,
}

impl fmt::Debug for PgMatchJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgMatchJournal")
            .field("match_id", &self.match_id)
            .field("fence", &self.fence)
            .finish_non_exhaustive()
    }
}

#[derive(sqlx::FromRow)]
struct HeadRow {
    match_id: Uuid,
    fence: i64,
    format: i16,
    version: Option<i64>,
    input_index: Option<i64>,
    observed_ms: i64,
    record_count: i64,
    record_bytes: i64,
    creation: Option<Vec<u8>>,
    creation_hash: Option<Vec<u8>>,
    ledger: Option<Vec<u8>>,
    ledger_hash: Option<Vec<u8>>,
    latest_hash: Option<Vec<u8>>,
}

#[derive(sqlx::FromRow)]
struct RecordRow {
    match_id: Uuid,
    input_index: i64,
    state_version: i64,
    logical_ms: i64,
    state_hash: Vec<u8>,
    payload: Vec<u8>,
    payload_bytes: i64,
    payload_hash: Vec<u8>,
}

#[derive(sqlx::FromRow)]
struct TotalsRow {
    count: i64,
    bytes: i64,
}

struct CheckedHead {
    creation: MatchCreation,
    ledger: Vec<ScopeState>,
    version: u64,
    index: u64,
    observed_ms: u64,
    count: usize,
    bytes: usize,
}

fn unavailable<T>(_: T) -> RuntimePortError {
    RuntimePortError::Unavailable
}
fn signed(value: u64) -> Result<i64, RuntimePortError> {
    i64::try_from(value).map_err(unavailable)
}
fn unsigned(value: i64) -> Result<u64, RuntimePortError> {
    u64::try_from(value).map_err(unavailable)
}
fn size(value: i64) -> Result<usize, RuntimePortError> {
    usize::try_from(value).map_err(unavailable)
}
fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, RuntimePortError> {
    canonical_encode(value).map_err(unavailable)
}

fn strict_decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, RuntimePortError> {
    let value: T = canonical_decode(bytes).map_err(unavailable)?;
    if encode(&value)? != bytes {
        return Err(RuntimePortError::Unavailable);
    }
    Ok(value)
}

fn checksum(bytes: &[u8]) -> Vec<u8> {
    blake3::hash(bytes).as_bytes().to_vec()
}
fn checked_blob<'a>(
    blob: Option<&'a [u8]>,
    hash: Option<&[u8]>,
    limit: usize,
) -> Result<&'a [u8], RuntimePortError> {
    let bytes = blob.ok_or(RuntimePortError::Unavailable)?;
    if !(2..=limit).contains(&bytes.len())
        || hash != Some(blake3::hash(bytes).as_bytes().as_slice())
    {
        return Err(RuntimePortError::Unavailable);
    }
    Ok(bytes)
}

fn canonical_blob(bytes: &[u8], limit: usize) -> Result<(), RuntimePortError> {
    if !(2..=limit).contains(&bytes.len()) || bytes[..2] != ENCODING_VERSION.to_le_bytes() {
        return Err(RuntimePortError::Unavailable);
    }
    Ok(())
}

fn validate_creation(creation: &MatchCreation) -> Result<(), RuntimePortError> {
    if creation.identity.rules_hash == [0; 32]
        || creation.format != JOURNAL_FORMAT
        || !(1..=256).contains(&creation.limits.scopes)
        || !(1..=64).contains(&creation.limits.receipts_per_scope)
        || !(1..=3_600_000).contains(&creation.limits.receipt_ttl_ms)
        || creation.roster.is_empty()
    {
        return Err(RuntimePortError::Unavailable);
    }
    signed(creation.started_at_unix_ms)?;
    canonical_blob(&creation.config, MAX_SNAPSHOT_BYTES)?;
    if encode(creation)?.len() > MAX_SNAPSHOT_BYTES {
        return Err(RuntimePortError::Unavailable);
    }
    Ok(())
}

fn validate_ledger(
    match_id: MatchId,
    creation: &MatchCreation,
    ledger: &[ScopeState],
    now: u64,
) -> Result<(), RuntimePortError> {
    if ledger.len() > usize::from(creation.limits.scopes)
        || encode(ledger)?.len() > MAX_LEDGER_BYTES
    {
        return Err(RuntimePortError::Unavailable);
    }
    for (i, state) in ledger.iter().enumerate() {
        if state.scope.record == 0
            || state.scope.subject.0 == 0
            || creation.roster.get(state.scope.seat).is_none()
            || i > 0 && ledger[i - 1].scope >= state.scope
            || state.recent.len() > usize::from(creation.limits.receipts_per_scope)
        {
            return Err(RuntimePortError::Unavailable);
        }
        let mut previous = 0;
        for receipt in &state.recent {
            if receipt.seq == 0
                || receipt.seq <= previous
                || receipt.seq > state.highest
                || receipt.at > now
                || receipt.result.is_ok()
                    && (receipt.command.match_id() != match_id
                        || receipt.command.game() != &creation.identity.game
                        || receipt.command.game_version() != &creation.identity.game_version)
                || receipt.result.is_ok() != receipt.committed_index.is_some()
            {
                return Err(RuntimePortError::Unavailable);
            }
            previous = receipt.seq;
        }
    }
    Ok(())
}

fn validate_record(
    record: &JournalRecord,
    creation: &MatchCreation,
) -> Result<(), RuntimePortError> {
    if record.match_id.0 == 0
        || record.index.0 != record.version.0
        || record.index.0 >= u64::try_from(MAX_RECOVERY_RECORDS).map_err(unavailable)?
    {
        return Err(RuntimePortError::Unavailable);
    }
    signed(record.now.0)?;
    let genesis = record.index.0 == 0;
    if genesis {
        if record.creation.is_none()
            || record.expected_version.is_some()
            || !record.input.is_empty()
            || record.operation.is_some()
            || record.now.0 != 0
            || record
                .ledger
                .iter()
                .any(|state| state.highest != 0 || !state.recent.is_empty())
        {
            return Err(RuntimePortError::Unavailable);
        }
        if encode(
            record
                .creation
                .as_ref()
                .ok_or(RuntimePortError::Unavailable)?,
        )? != encode(creation)?
        {
            return Err(RuntimePortError::Unavailable);
        }
    } else {
        if record.creation.is_some()
            || record.expected_version != Some(StateVersion(record.version.0 - 1))
        {
            return Err(RuntimePortError::Unavailable);
        }
        canonical_blob(&record.input, MAX_RECOVERY_BYTES)?;
    }
    for event in &record.events {
        canonical_blob(event, MAX_RECOVERY_BYTES)?;
    }
    let required = genesis || record.index.0.is_multiple_of(20) || record.terminal;
    if required != record.snapshot.is_some() {
        return Err(RuntimePortError::Unavailable);
    }
    if let Some(snapshot) = &record.snapshot {
        canonical_blob(snapshot, MAX_SNAPSHOT_BYTES)?;
    }
    if let Some(operation) = &record.operation {
        if operation.seq == 0
            || operation.command.match_id() != record.match_id
            || operation.command.game() != &creation.identity.game
            || operation.command.game_version() != &creation.identity.game_version
            || creation.roster.get(operation.scope.seat).is_none()
        {
            return Err(RuntimePortError::Unavailable);
        }
    }
    Ok(())
}

/// Preservation law for complete ledgers: scopes/watermarks never disappear.
fn validate_transition(
    old: &[ScopeState],
    new: &[ScopeState],
    accepted: Option<&OperationKey>,
    index: InputIndex,
) -> Result<(), RuntimePortError> {
    for prior in old {
        let next = new
            .iter()
            .find(|state| state.scope == prior.scope)
            .ok_or(RuntimePortError::Unavailable)?;
        if next.highest < prior.highest {
            return Err(RuntimePortError::Unavailable);
        }
        for receipt in &next.recent {
            if let Some(previous) = prior
                .recent
                .iter()
                .find(|previous| previous.seq == receipt.seq)
            {
                if receipt != previous {
                    return Err(RuntimePortError::Unavailable);
                }
            } else if receipt.seq <= prior.highest {
                return Err(RuntimePortError::Unavailable);
            }
        }
    }
    for next in new {
        let prior = old.iter().find(|state| state.scope == next.scope);
        let highest = prior.map_or(0, |state| state.highest);
        for receipt in &next.recent {
            let retained =
                prior.is_some_and(|state| state.recent.iter().any(|previous| previous == receipt));
            if receipt.result.is_ok()
                && !retained
                && !accepted.is_some_and(|operation| {
                    operation.scope == next.scope
                        && operation.seq == receipt.seq
                        && operation.command == receipt.command
                        && receipt.committed_index == Some(index)
                })
            {
                return Err(RuntimePortError::Unavailable);
            }
        }
        if next.highest > highest {
            let latest = next
                .recent
                .iter()
                .find(|receipt| receipt.seq == next.highest)
                .ok_or(RuntimePortError::Unavailable)?;
            if latest.result.is_ok()
                != accepted.is_some_and(|operation| {
                    operation.scope == next.scope && operation.seq == next.highest
                })
            {
                return Err(RuntimePortError::Unavailable);
            }
        }
    }
    if let Some(operation) = accepted {
        let prior = old
            .iter()
            .find(|state| state.scope == operation.scope)
            .ok_or(RuntimePortError::Unavailable)?;
        let next = new
            .iter()
            .find(|state| state.scope == operation.scope)
            .ok_or(RuntimePortError::Unavailable)?;
        if operation.seq <= prior.highest
            || next.highest != operation.seq
            || !next.recent.iter().any(|receipt| {
                receipt.seq == operation.seq
                    && receipt.command == operation.command
                    && receipt.result.is_ok()
                    && receipt.committed_index == Some(index)
            })
        {
            return Err(RuntimePortError::Unavailable);
        }
    }
    Ok(())
}

impl HeadRow {
    fn check_owner(&self, match_id: MatchId, fence: i64) -> Result<(), RuntimePortError> {
        if self.match_id.as_u128() != match_id.0
            || self.format != i16::try_from(JOURNAL_FORMAT).map_err(unavailable)?
            || self.fence <= 0
        {
            return Err(RuntimePortError::Unavailable);
        }
        if self.fence != fence {
            return Err(RuntimePortError::Busy);
        }
        Ok(())
    }

    fn is_uninitialized(&self) -> bool {
        self.version.is_none()
            && self.input_index.is_none()
            && self.observed_ms == 0
            && self.record_count == 0
            && self.record_bytes == 0
            && self.creation.is_none()
            && self.creation_hash.is_none()
            && self.ledger.is_none()
            && self.ledger_hash.is_none()
            && self.latest_hash.is_none()
    }

    fn checked(&self, match_id: MatchId) -> Result<CheckedHead, RuntimePortError> {
        let creation_bytes = checked_blob(
            self.creation.as_deref(),
            self.creation_hash.as_deref(),
            MAX_SNAPSHOT_BYTES,
        )?;
        let ledger_bytes = checked_blob(
            self.ledger.as_deref(),
            self.ledger_hash.as_deref(),
            MAX_LEDGER_BYTES,
        )?;
        let creation: MatchCreation = strict_decode(creation_bytes)?;
        validate_creation(&creation)?;
        let ledger: Vec<ScopeState> = strict_decode(ledger_bytes)?;
        let version = unsigned(self.version.ok_or(RuntimePortError::Unavailable)?)?;
        let index = unsigned(self.input_index.ok_or(RuntimePortError::Unavailable)?)?;
        let observed_ms = unsigned(self.observed_ms)?;
        let count = size(self.record_count)?;
        let bytes = size(self.record_bytes)?;
        if index != version
            || count
                != usize::try_from(index)
                    .map_err(unavailable)?
                    .checked_add(1)
                    .ok_or(RuntimePortError::Unavailable)?
            || count > MAX_RECOVERY_RECORDS
            || bytes == 0
            || bytes
                .checked_add(creation_bytes.len())
                .and_then(|n| n.checked_add(ledger_bytes.len()))
                .is_none_or(|n| n > MAX_RECOVERY_BYTES)
            || self
                .latest_hash
                .as_ref()
                .is_none_or(|hash| hash.len() != 32)
        {
            return Err(RuntimePortError::Unavailable);
        }
        validate_ledger(match_id, &creation, &ledger, observed_ms)?;
        Ok(CheckedHead {
            creation,
            ledger,
            version,
            index,
            observed_ms,
            count,
            bytes,
        })
    }
}

impl PgMatchStore {
    /// Construct a store using an explicitly configured pool; performs no I/O.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Apply only the isolated additive migrations to the selected database.
    pub async fn migrate(pool: &PgPool) -> Result<(), RuntimePortError> {
        let mut connection = pool.acquire().await.map_err(unavailable)?;
        // SQLx's session-level migration lock survives early validation errors.
        // Closing this backend also releases it if the migration is canceled;
        // it must never return to the pool with a retained advisory lock.
        connection.close_on_drop();
        let result = MIGRATIONS.run(&mut *connection).await.map_err(unavailable);
        let closed = connection.close().await.map_err(unavailable);
        result.and(closed)
    }

    /// Acquire a new durable owner generation; never initializes game state.
    pub async fn claim(&self, match_id: MatchId) -> Result<PgMatchJournal, RuntimePortError> {
        if match_id.0 == 0 {
            return Err(RuntimePortError::Unavailable);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        write_settings(&mut tx).await?;
        let fence: Option<i64> = sqlx::query_scalar("INSERT INTO match_journal_heads (match_id, fence, format) VALUES ($1, 1, 1) ON CONFLICT (match_id) DO UPDATE SET fence = match_journal_heads.fence + 1 WHERE match_journal_heads.fence < 9223372036854775807 RETURNING fence")
            .bind(Uuid::from_u128(match_id.0)).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let fence = fence
            .filter(|value| *value > 0)
            .ok_or(RuntimePortError::Unavailable)?;
        tx.commit()
            .await
            .map_err(|_| RuntimePortError::Indeterminate)?;
        Ok(PgMatchJournal {
            pool: self.pool.clone(),
            match_id,
            fence,
            #[cfg(any(test, feature = "match-postgres-test-support"))]
            controls: Arc::new(TestControls::default()),
        })
    }
}

async fn write_settings(tx: &mut Transaction<'_, Postgres>) -> Result<(), RuntimePortError> {
    sqlx::query("SET LOCAL synchronous_commit = on")
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
    Ok(())
}

impl PgMatchJournal {
    async fn lock_head(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<HeadRow, RuntimePortError> {
        let head = sqlx::query_as::<_, HeadRow>("SELECT match_id, fence, format, version, input_index, observed_ms, record_count, record_bytes, creation, creation_hash, ledger, ledger_hash, latest_hash FROM match_journal_heads WHERE match_id = $1 FOR UPDATE")
            .bind(Uuid::from_u128(self.match_id.0)).fetch_optional(&mut **tx).await.map_err(unavailable)?.ok_or(RuntimePortError::Unavailable)?;
        head.check_owner(self.match_id, self.fence)?;
        Ok(head)
    }

    async fn check_latest(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        head: &HeadRow,
        checked: &CheckedHead,
    ) -> Result<(), RuntimePortError> {
        let latest: RecordRow = sqlx::query_as("SELECT match_id, input_index, state_version, logical_ms, state_hash, payload, payload_bytes, payload_hash FROM match_journal_records WHERE match_id = $1 AND input_index = $2")
                .bind(Uuid::from_u128(self.match_id.0)).bind(signed(checked.index)?).fetch_one(&mut **tx).await.map_err(unavailable)?;
        if latest.payload_hash != checksum(&latest.payload)
            || latest.state_hash.as_slice()
                != head
                    .latest_hash
                    .as_deref()
                    .ok_or(RuntimePortError::Unavailable)?
        {
            return Err(RuntimePortError::Unavailable);
        }
        let previous: JournalRecord = strict_decode(&latest.payload)?;
        validate_record(&previous, &checked.creation)?;
        if latest.match_id.as_u128() != self.match_id.0
            || unsigned(latest.input_index)? != checked.index
            || unsigned(latest.state_version)? != checked.version
            || size(latest.payload_bytes)? != latest.payload.len()
            || previous.match_id != self.match_id
            || previous.index.0 != checked.index
            || previous.version.0 != checked.version
            || previous.now.0 != unsigned(latest.logical_ms)?
            || previous.now.0 > checked.observed_ms
            || previous.hash.0.as_slice() != latest.state_hash
            || !previous.ledger.is_empty()
        {
            return Err(RuntimePortError::Unavailable);
        }
        if previous.terminal {
            return Err(RuntimePortError::Unavailable);
        }
        Ok(())
    }

    async fn finish(&self, tx: Transaction<'_, Postgres>) -> Result<(), RuntimePortError> {
        #[cfg(any(test, feature = "match-postgres-test-support"))]
        {
            let pause = self.controls.pause.lock().map_err(unavailable)?.take();
            if let Some((entered, release)) = pause {
                let _ = entered.send(());
                release.await.map_err(unavailable)?;
            }
            if self.controls.next.load(Ordering::SeqCst) == 1 {
                self.controls.next.store(0, Ordering::SeqCst);
                tx.rollback().await.map_err(unavailable)?;
                return Err(RuntimePortError::Unavailable);
            }
        }
        tx.commit()
            .await
            .map_err(|_| RuntimePortError::Indeterminate)?;
        #[cfg(any(test, feature = "match-postgres-test-support"))]
        if self.controls.next.swap(0, Ordering::SeqCst) == 2 {
            return Err(RuntimePortError::Indeterminate);
        }
        Ok(())
    }
}

impl PgMatchJournal {
    async fn append_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        mut record: JournalRecord,
    ) -> Result<(), RuntimePortError> {
        if record.match_id != self.match_id {
            return Err(RuntimePortError::Unavailable);
        }
        let head = self.lock_head(tx).await?;
        let (creation, old_ledger, previous_bytes, previous_count) = if record
            .expected_version
            .is_none()
        {
            if !head.is_uninitialized() {
                return Err(RuntimePortError::Busy);
            }
            let total: TotalsRow = sqlx::query_as("SELECT COUNT(*)::bigint AS count, COALESCE(SUM(octet_length(payload)), 0)::bigint AS bytes FROM match_journal_records WHERE match_id = $1")
                .bind(Uuid::from_u128(self.match_id.0)).fetch_one(&mut **tx).await.map_err(unavailable)?;
            if total.count != 0 || total.bytes != 0 {
                return Err(RuntimePortError::Unavailable);
            }
            let creation = record
                .creation
                .clone()
                .ok_or(RuntimePortError::Unavailable)?;
            validate_creation(&creation)?;
            (creation, Vec::new(), 0, 0)
        } else {
            let checked = head.checked(self.match_id)?;
            if record.expected_version != Some(StateVersion(checked.version))
                || record.version.0
                    != checked
                        .version
                        .checked_add(1)
                        .ok_or(RuntimePortError::Unavailable)?
                || record.index.0
                    != checked
                        .index
                        .checked_add(1)
                        .ok_or(RuntimePortError::Unavailable)?
            {
                return Err(RuntimePortError::Busy);
            }
            self.check_latest(tx, &head, &checked).await?;
            (
                checked.creation,
                checked.ledger,
                checked.bytes,
                checked.count,
            )
        };
        validate_record(&record, &creation)?;
        if record.now.0 < unsigned(head.observed_ms)? {
            return Err(RuntimePortError::Unavailable);
        }
        validate_ledger(self.match_id, &creation, &record.ledger, record.now.0)?;
        validate_transition(
            &old_ledger,
            &record.ledger,
            record.operation.as_ref(),
            record.index,
        )?;
        if let Some(operation) = &record.operation {
            if !record
                .ledger
                .iter()
                .find(|state| state.scope == operation.scope)
                .is_some_and(|state| {
                    state
                        .recent
                        .iter()
                        .any(|receipt| receipt.seq == operation.seq && receipt.at == record.now.0)
                })
            {
                return Err(RuntimePortError::Unavailable);
            }
        }
        let ledger = encode(&record.ledger)?;
        let creation_bytes = encode(&creation)?;
        record.ledger.clear();
        let payload = encode(&record)?;
        let bytes = previous_bytes
            .checked_add(payload.len())
            .ok_or(RuntimePortError::Unavailable)?;
        let count = previous_count
            .checked_add(1)
            .ok_or(RuntimePortError::Unavailable)?;
        if count > MAX_RECOVERY_RECORDS
            || bytes
                .checked_add(creation_bytes.len())
                .and_then(|n| n.checked_add(ledger.len()))
                .is_none_or(|n| n > MAX_RECOVERY_BYTES)
        {
            return Err(RuntimePortError::Unavailable);
        }
        sqlx::query("INSERT INTO match_journal_records (match_id, input_index, state_version, logical_ms, state_hash, payload, payload_bytes, payload_hash) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
            .bind(Uuid::from_u128(self.match_id.0)).bind(signed(record.index.0)?).bind(signed(record.version.0)?).bind(signed(record.now.0)?).bind(record.hash.0.as_slice()).bind(&payload).bind(i64::try_from(payload.len()).map_err(unavailable)?).bind(checksum(&payload)).execute(&mut **tx).await.map_err(unavailable)?;
        sqlx::query("UPDATE match_journal_heads SET version = $2, input_index = $3, observed_ms = $4, record_count = $5, record_bytes = $6, creation = $7, creation_hash = $8, ledger = $9, ledger_hash = $10, latest_hash = $11 WHERE match_id = $1")
            .bind(Uuid::from_u128(self.match_id.0)).bind(signed(record.version.0)?).bind(signed(record.index.0)?).bind(signed(record.now.0)?).bind(i64::try_from(count).map_err(unavailable)?).bind(i64::try_from(bytes).map_err(unavailable)?).bind(&creation_bytes).bind(checksum(&creation_bytes)).bind(&ledger).bind(checksum(&ledger)).bind(record.hash.0.as_slice()).execute(&mut **tx).await.map_err(unavailable)?;
        Ok(())
    }

    async fn ledger_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        match_id: MatchId,
        expected_version: StateVersion,
        observed_ms: u64,
        ledger: Vec<ScopeState>,
    ) -> Result<(), RuntimePortError> {
        if match_id != self.match_id {
            return Err(RuntimePortError::Unavailable);
        }
        signed(observed_ms)?;
        let head = self.lock_head(tx).await?;
        let checked = head.checked(match_id)?;
        if checked.version != expected_version.0 {
            return Err(RuntimePortError::Busy);
        }
        if observed_ms < checked.observed_ms {
            return Err(RuntimePortError::Unavailable);
        }
        validate_ledger(match_id, &checked.creation, &ledger, observed_ms)?;
        validate_transition(&checked.ledger, &ledger, None, InputIndex(checked.index))?;
        let bytes = encode(&ledger)?;
        if checked
            .bytes
            .checked_add(encode(&checked.creation)?.len())
            .and_then(|n| n.checked_add(bytes.len()))
            .is_none_or(|n| n > MAX_RECOVERY_BYTES)
        {
            return Err(RuntimePortError::Unavailable);
        }
        sqlx::query("UPDATE match_journal_heads SET ledger = $2, ledger_hash = $3, observed_ms = $4 WHERE match_id = $1")
            .bind(Uuid::from_u128(match_id.0)).bind(&bytes).bind(checksum(&bytes)).bind(signed(observed_ms)?).execute(&mut **tx).await.map_err(unavailable)?;
        Ok(())
    }
}

#[cfg(feature = "online-match-postgres")]
impl PgMatchJournal {
    /// One actual current-session transaction encloses actor apply and this append.
    /// The live permit is consumed; all later output reacquires fresh authority.
    pub async fn append_authenticated(
        &self,
        record: JournalRecord,
        operation: &crate::online_match::PgOnlineOperation,
    ) -> Result<(), RuntimePortError> {
        let mut pending = operation.take().map_err(unavailable)?;
        let staged = async {
            pending
                .authorize_record(&record)
                .await
                .map_err(unavailable)?;
            let genesis = record.creation.is_some();
            let accepted = record.operation.is_some();
            let terminal = record.terminal;
            if genesis {
                if !record.ledger.is_empty() {
                    return Err(RuntimePortError::Unavailable);
                }
            } else {
                let head = self.lock_head(&mut pending.tx).await?;
                let checked = head.checked(self.match_id)?;
                validate_authenticated_scopes(
                    &checked.ledger,
                    &record.ledger,
                    pending.membership.scope(),
                )?;
            }
            self.append_transaction(&mut pending.tx, record).await?;
            pending
                .finalize(accepted, genesis, terminal)
                .await
                .map_err(unavailable)?;
            Ok(())
        }
        .await;
        self.finish_authenticated(pending, operation.credential(), staged)
            .await
    }
    /// Reserve/reject within the same live credential/session/seat transaction.
    /// Only the requesting admission may create or advance an operation scope;
    /// receipt expiration for other retained scopes does not grant new authority.
    pub async fn update_ledger_authenticated(
        &self,
        match_id: MatchId,
        expected_version: StateVersion,
        observed_ms: u64,
        ledger: Vec<ScopeState>,
        operation: &crate::online_match::PgOnlineOperation,
    ) -> Result<(), RuntimePortError> {
        let mut pending = operation.take().map_err(unavailable)?;
        let staged = async {
            pending
                .authorize_ledger(match_id)
                .await
                .map_err(unavailable)?;
            let head = self.lock_head(&mut pending.tx).await?;
            let checked = head.checked(match_id)?;
            let authorized = pending.membership.scope();
            if !ledger.iter().any(|state| state.scope == authorized) {
                return Err(RuntimePortError::Unavailable);
            }
            validate_authenticated_scopes(&checked.ledger, &ledger, authorized)?;
            self.ledger_transaction(
                &mut pending.tx,
                match_id,
                expected_version,
                observed_ms,
                ledger,
            )
            .await?;
            pending
                .finalize(false, false, false)
                .await
                .map_err(unavailable)?;
            Ok(())
        }
        .await;
        self.finish_authenticated(pending, operation.credential(), staged)
            .await
    }
    async fn finish_authenticated(
        &self,
        pending: crate::online_match::OnlineTransaction,
        credential: tabula_session::CredentialOperation,
        staged: Result<(), RuntimePortError>,
    ) -> Result<(), RuntimePortError> {
        use tabula_session::HttpSessionAuthority;
        let (tx, exclusion) = pending.into_commit_parts();
        let result = match staged {
            Ok(()) => self.finish(tx).await,
            Err(error) => match tx.rollback().await {
                Ok(()) => Err(error),
                Err(_) => Err(RuntimePortError::Indeterminate),
            },
        };
        drop(exclusion);
        if result.is_err() {
            // A failed transaction cannot preserve its observed terminal expiry.
            // Re-observe the actual committed credential without activity before
            // returning failure. This cannot reconstruct a rolled-back clock
            // sample if the deployment clock regresses before this observation,
            // or if this independent observation itself cannot commit (ADR-0036).
            let _ = crate::session::PgSessionStore::new(self.pool.clone())
                .read_session(credential)
                .await;
        }
        result
    }
}
#[cfg(feature = "online-match-postgres")]
fn validate_authenticated_scopes(
    old: &[ScopeState],
    new: &[ScopeState],
    authorized: tabula_match_journal::OperationScope,
) -> Result<(), RuntimePortError> {
    for state in new {
        if state.scope == authorized {
            continue;
        }
        let prior = old
            .iter()
            .find(|prior| prior.scope == state.scope)
            .ok_or(RuntimePortError::Unavailable)?;
        if state.highest != prior.highest
            || state
                .recent
                .iter()
                .any(|receipt| !prior.recent.contains(receipt))
        {
            return Err(RuntimePortError::Unavailable);
        }
    }
    Ok(())
}

impl Journal for PgMatchJournal {
    async fn append(&self, record: JournalRecord) -> Result<(), RuntimePortError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        write_settings(&mut tx).await?;
        self.append_transaction(&mut tx, record).await?;
        self.finish(tx).await
    }

    async fn update_ledger(
        &self,
        match_id: MatchId,
        expected_version: StateVersion,
        observed_ms: u64,
        ledger: Vec<ScopeState>,
    ) -> Result<(), RuntimePortError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        write_settings(&mut tx).await?;
        self.ledger_transaction(&mut tx, match_id, expected_version, observed_ms, ledger)
            .await?;
        self.finish(tx).await
    }

    async fn load(&self, match_id: MatchId) -> Result<LoadedMatch, RuntimePortError> {
        if match_id != self.match_id {
            return Err(RuntimePortError::Unavailable);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        // SHARE pins the fence until this consistent recovery has finished.
        let head: HeadRow = sqlx::query_as("SELECT match_id, fence, format, version, input_index, observed_ms, record_count, record_bytes, creation, creation_hash, ledger, ledger_hash, latest_hash FROM match_journal_heads WHERE match_id = $1 FOR SHARE")
            .bind(Uuid::from_u128(match_id.0)).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(RuntimePortError::Unavailable)?;
        head.check_owner(match_id, self.fence)?;
        let checked = head.checked(match_id)?;
        let total: TotalsRow = sqlx::query_as("SELECT COUNT(*)::bigint AS count, COALESCE(SUM(octet_length(payload)), 0)::bigint AS bytes FROM match_journal_records WHERE match_id = $1")
            .bind(Uuid::from_u128(match_id.0)).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if size(total.count)? != checked.count || size(total.bytes)? != checked.bytes {
            return Err(RuntimePortError::Unavailable);
        }
        let rows: Vec<RecordRow> = sqlx::query_as("SELECT match_id, input_index, state_version, logical_ms, state_hash, payload, payload_bytes, payload_hash FROM match_journal_records WHERE match_id = $1 ORDER BY input_index ASC LIMIT 10002")
            .bind(Uuid::from_u128(match_id.0)).fetch_all(&mut *tx).await.map_err(unavailable)?;
        if rows.len() != checked.count {
            return Err(RuntimePortError::Unavailable);
        }
        let mut records = Vec::with_capacity(rows.len());
        let mut previous_time = 0;
        let mut terminal = false;
        for (ordinal, row) in rows.into_iter().enumerate() {
            if terminal
                || row.match_id.as_u128() != match_id.0
                || size(row.input_index)? != ordinal
                || size(row.state_version)? != ordinal
                || size(row.payload_bytes)? != row.payload.len()
                || row.payload_hash != checksum(&row.payload)
                || row.state_hash.len() != 32
            {
                return Err(RuntimePortError::Unavailable);
            }
            let record: JournalRecord = strict_decode(&row.payload)?;
            validate_record(&record, &checked.creation)?;
            if record.match_id != match_id
                || record.index.0 != unsigned(row.input_index)?
                || record.version.0 != unsigned(row.state_version)?
                || record.now.0 != unsigned(row.logical_ms)?
                || record.now.0 < previous_time
                || record.now.0 > checked.observed_ms
                || record.hash.0.as_slice() != row.state_hash
                || !record.ledger.is_empty()
            {
                return Err(RuntimePortError::Unavailable);
            }
            previous_time = record.now.0;
            terminal = record.terminal;
            records.push(record);
        }
        if records
            .last()
            .is_none_or(|record| Some(record.hash.0.as_slice()) != head.latest_hash.as_deref())
        {
            return Err(RuntimePortError::Unavailable);
        }
        validate_loaded_operations(&records, &checked.ledger)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(LoadedMatch {
            creation: checked.creation,
            records,
            ledger: checked.ledger,
            version: StateVersion(checked.version),
            index: InputIndex(checked.index),
            observed_ms: checked.observed_ms,
        })
    }
}

fn validate_loaded_operations(
    records: &[JournalRecord],
    ledger: &[ScopeState],
) -> Result<(), RuntimePortError> {
    let mut accepted = std::collections::BTreeMap::new();
    let mut highest = std::collections::BTreeMap::new();
    for record in records {
        if let Some(operation) = &record.operation {
            let scope = ledger
                .iter()
                .find(|state| state.scope == operation.scope)
                .ok_or(RuntimePortError::Unavailable)?;
            if operation.seq <= *highest.get(&operation.scope).unwrap_or(&0)
                || operation.seq > scope.highest
                || accepted
                    .insert(
                        (operation.scope, operation.seq),
                        (record.index, &operation.command, record.now.0),
                    )
                    .is_some()
            {
                return Err(RuntimePortError::Unavailable);
            }
            highest.insert(operation.scope, operation.seq);
        }
    }
    for state in ledger {
        for receipt in &state.recent {
            if let Some(index) = receipt.committed_index {
                if accepted.get(&(state.scope, receipt.seq))
                    != Some(&(index, &receipt.command, receipt.at))
                {
                    return Err(RuntimePortError::Unavailable);
                }
            } else if accepted.contains_key(&(state.scope, receipt.seq)) {
                return Err(RuntimePortError::Unavailable);
            }
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "match-postgres-test-support"))]
#[derive(Default)]
struct TestControls {
    next: AtomicU8,
    pause: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
}

#[cfg(any(test, feature = "match-postgres-test-support"))]
pub mod test_support;

#[cfg(test)]
mod tests;
