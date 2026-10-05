//! Isolated real-PostgreSQL acceptance for the ADR-0040 journal boundary.
//!
//! Ordinary workspace runs deliberately ignore the database cases. The explicit
//! `real_postgres_` selection requires `TABULA_MATCH_DATABASE_URL`; missing or
//! failed setup is a failure, never a skipped or in-memory substitute check.
//! Every case migrates its own disposable schema and uses independent backends.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgConnection, PgPool, Row};
use tabula_core::{
    canonical_decode, canonical_encode, state_hash, GameId, GameVersion, InputIndex, LogicalTime,
    MatchId, Occupant, RulesVersion, SeatEntry, SeatId, SeatRoster, StateVersion, TimerId, UserId,
};
use tabula_game_api::Effect;
use tabula_match_journal::{
    Journal, JournalRecord, LedgerLimits, MatchCreation, MatchIdentity, OperationKey,
    OperationReceipt, OperationScope, RuntimePortError, ScopeState, JOURNAL_FORMAT,
    MAX_SNAPSHOT_BYTES,
};
use tabula_protocol::{ErrorCode, GameCommandFrame};
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{PgMatchJournal, PgMatchStore};

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(1);
const WAIT_LIMIT: Duration = Duration::from_secs(15);
const MATCH: MatchId = MatchId(101);

#[derive(Clone, Copy)]
enum SchemaDdl {
    Create,
    Drop,
}

fn schema_ddl(schema: &str, action: SchemaDdl) -> AssertSqlSafe<String> {
    assert!(schema.starts_with("tabula_match_acceptance_"));
    assert!(schema.len() <= 63);
    assert!(schema
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
    AssertSqlSafe(match action {
        SchemaDdl::Create => format!("CREATE SCHEMA {schema}"),
        SchemaDdl::Drop => format!("DROP SCHEMA {schema} CASCADE"),
    })
}

struct DatabaseFixture {
    admin: PgPool,
    first: PgPool,
    second: PgPool,
    observer: PgPool,
    url: String,
    schema: String,
}

impl DatabaseFixture {
    async fn new() -> Self {
        let url = std::env::var("TABULA_MATCH_DATABASE_URL").expect(
            "ignored match PostgreSQL acceptance requires TABULA_MATCH_DATABASE_URL; setup cannot be skipped",
        );
        let schema = format!(
            "tabula_match_acceptance_{}_{}",
            std::process::id(),
            NEXT_SCHEMA.fetch_add(1, Ordering::SeqCst)
        );
        let admin = tokio::time::timeout(
            WAIT_LIMIT,
            PgPoolOptions::new().max_connections(1).connect(&url),
        )
        .await
        .expect("match acceptance admin connection timed out")
        .unwrap_or_else(|_| panic!("match acceptance admin connection failed"));
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .expect("match acceptance must establish real database version");
        assert_eq!(
            version / 10_000,
            16,
            "match acceptance requires PostgreSQL 16"
        );
        sqlx::raw_sql(schema_ddl(&schema, SchemaDdl::Create))
            .execute(&admin)
            .await
            .expect("match acceptance disposable schema creation failed");
        let first = Self::pool(&url, &schema).await;
        let second = Self::pool(&url, &schema).await;
        let observer = Self::pool(&url, &schema).await;
        assert_ne!(backend_pid(&first).await, backend_pid(&second).await);
        assert_ne!(backend_pid(&first).await, backend_pid(&observer).await);
        PgMatchStore::migrate(&first)
            .await
            .expect("isolated match migration must succeed");
        Self {
            admin,
            first,
            second,
            observer,
            url,
            schema,
        }
    }

    async fn pool(url: &str, schema: &str) -> PgPool {
        let schema = schema.to_owned();
        tokio::time::timeout(
            WAIT_LIMIT,
            PgPoolOptions::new()
                .max_connections(1)
                .after_connect(move |connection, _metadata| {
                    Box::pin(configure_connection(connection, schema.clone()))
                })
                .connect(url),
        )
        .await
        .expect("match acceptance independent pool connection timed out")
        .unwrap_or_else(|_| panic!("match acceptance independent pool connection failed"))
    }

    async fn reopen(&self) -> PgPool {
        Self::pool(&self.url, &self.schema).await
    }

    async fn close(self) {
        self.first.close().await;
        self.second.close().await;
        self.observer.close().await;
        sqlx::raw_sql(schema_ddl(&self.schema, SchemaDdl::Drop))
            .execute(&self.admin)
            .await
            .expect("match acceptance disposable schema cleanup failed");
        self.admin.close().await;
    }
}

async fn configure_connection(
    connection: &mut PgConnection,
    schema: String,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('search_path', $1, false)")
        .bind(schema)
        .execute(&mut *connection)
        .await?;
    sqlx::query("SET statement_timeout = '10s'")
        .execute(&mut *connection)
        .await?;
    sqlx::query("SET lock_timeout = '10s'")
        .execute(&mut *connection)
        .await?;
    sqlx::query("SET idle_in_transaction_session_timeout = '10s'")
        .execute(&mut *connection)
        .await?;
    Ok(())
}

async fn backend_pid(pool: &PgPool) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .expect("acceptance adapter must have a real backend")
}

async fn wait_blocked(admin: &PgPool, waiting: i32, blocking: i32) {
    tokio::time::timeout(WAIT_LIMIT, async {
        loop {
            let blocked: bool =
                sqlx::query_scalar("SELECT $2::integer = ANY(pg_blocking_pids($1::integer))")
                    .bind(waiting)
                    .bind(blocking)
                    .fetch_one(admin)
                    .await
                    .expect("match acceptance must observe PostgreSQL lock graph");
            if blocked {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second adapter must demonstrably wait for the held match transaction");
}

async fn finish<T>(task: tokio::task::JoinHandle<T>) -> T {
    tokio::time::timeout(WAIT_LIMIT, task)
        .await
        .expect("match acceptance operation timed out")
        .expect("match acceptance operation panicked")
}

fn pause(journal: &PgMatchJournal) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (arrived_tx, arrived_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    *journal
        .controls
        .pause
        .lock()
        .expect("test control mutex must be healthy") = Some((arrived_tx, release_rx));
    (arrived_rx, release_tx)
}

async fn arrived(signal: oneshot::Receiver<()>) {
    tokio::time::timeout(WAIT_LIMIT, signal)
        .await
        .expect("match transaction must reach the before-commit gate")
        .expect("match transaction closed its before-commit gate unexpectedly");
}

fn identity() -> MatchIdentity {
    MatchIdentity {
        game: GameId::new("org.example.storagefixture").unwrap(),
        game_version: GameVersion::new("1.0.0").unwrap(),
        rules_version: RulesVersion(1),
        rules_hash: [7; 32],
    }
}

fn creation() -> MatchCreation {
    MatchCreation {
        format: JOURNAL_FORMAT,
        identity: identity(),
        config: canonical_encode(&5_u64).unwrap(),
        roster: SeatRoster::new(
            vec![SeatEntry {
                seat: SeatId(0),
                occupant: Occupant::Human(UserId(42)),
                team: None,
            }]
            .into(),
        )
        .unwrap(),
        seed: [9; 32],
        started_at_unix_ms: 1_000_000,
        limits: LedgerLimits {
            scopes: 4,
            receipts_per_scope: 4,
            receipt_ttl_ms: 1_000,
        },
    }
}

fn scope() -> OperationScope {
    OperationScope {
        record: 501,
        subject: UserId(42),
        epoch: 1,
        seat: SeatId(0),
        generation: 1,
    }
}

fn command(match_id: MatchId, seq: u8) -> GameCommandFrame {
    GameCommandFrame::new(
        match_id,
        identity().game,
        identity().game_version,
        canonical_encode(&seq).unwrap(),
    )
    .unwrap()
}

fn reserved_ledger() -> Vec<ScopeState> {
    vec![ScopeState {
        scope: scope(),
        highest: 0,
        recent: vec![],
    }]
}

fn accepted_ledger(match_id: MatchId, seq: u64, index: u64, at: u64) -> Vec<ScopeState> {
    vec![ScopeState {
        scope: scope(),
        highest: seq,
        recent: vec![OperationReceipt {
            seq,
            command: command(match_id, u8::try_from(seq).unwrap()),
            result: Ok(()),
            at,
            committed_index: Some(InputIndex(index)),
        }],
    }]
}

fn rejected_ledger(match_id: MatchId, at: u64) -> Vec<ScopeState> {
    let mut ledger = accepted_ledger(match_id, 1, 1, 10);
    ledger[0].highest = 2;
    ledger[0].recent.push(OperationReceipt {
        seq: 2,
        command: command(match_id, 2),
        result: Err(ErrorCode::RuleRejected),
        at,
        committed_index: None,
    });
    ledger
}

fn genesis(match_id: MatchId) -> JournalRecord {
    JournalRecord {
        match_id,
        index: InputIndex(0),
        version: StateVersion(0),
        now: LogicalTime::ZERO,
        input: vec![],
        events: vec![canonical_encode(&90_u8).unwrap()],
        hash: state_hash(RulesVersion(1), &0_u64),
        effects: vec![Effect::CancelTimer { id: TimerId(7) }],
        operation: None,
        terminal: false,
        snapshot: Some(canonical_encode(&0_u64).unwrap()),
        creation: Some(creation()),
        ledger: reserved_ledger(),
        expected_version: None,
    }
}

fn transition(match_id: MatchId, index: u64) -> JournalRecord {
    JournalRecord {
        match_id,
        index: InputIndex(index),
        version: StateVersion(index),
        now: LogicalTime(index * 10),
        input: canonical_encode(&1_u8).unwrap(),
        events: vec![
            canonical_encode(&91_u8).unwrap(),
            canonical_encode(&92_u8).unwrap(),
        ],
        hash: state_hash(RulesVersion(1), &index),
        effects: vec![Effect::CancelTimer { id: TimerId(8) }],
        operation: None,
        terminal: false,
        snapshot: index
            .is_multiple_of(20)
            .then(|| canonical_encode(&index).unwrap()),
        creation: None,
        ledger: reserved_ledger(),
        expected_version: Some(StateVersion(index - 1)),
    }
}

fn accepted_transition(match_id: MatchId) -> JournalRecord {
    let mut record = transition(match_id, 1);
    record.operation = Some(OperationKey {
        scope: scope(),
        seq: 1,
        command: command(match_id, 1),
    });
    record.ledger = accepted_ledger(match_id, 1, 1, 10);
    record
}

#[derive(PartialEq, Eq)]
struct HeadImage {
    fence: i64,
    format: i16,
    version: Option<i64>,
    input_index: Option<i64>,
    observed_ms: i64,
    record_count: i64,
    record_bytes: i64,
    ledger: Option<Vec<u8>>,
    creation: Option<Vec<u8>>,
    latest_hash: Option<Vec<u8>>,
    ledger_hash: Option<Vec<u8>>,
    creation_hash: Option<Vec<u8>>,
}

#[derive(PartialEq, Eq)]
struct RecordImage {
    index: i64,
    version: i64,
    now: i64,
    hash: Vec<u8>,
    payload: Vec<u8>,
    bytes: i64,
    payload_hash: Vec<u8>,
}

impl std::fmt::Debug for HeadImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeadImage")
            .field("version", &self.version)
            .field("input_index", &self.input_index)
            .field("record_count", &self.record_count)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for RecordImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordImage")
            .field("index", &self.index)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

async fn persisted(pool: &PgPool, match_id: MatchId) -> (HeadImage, Vec<RecordImage>) {
    let row = sqlx::query(
        "SELECT fence, format, version, input_index, observed_ms, record_count, record_bytes, ledger, creation, latest_hash, ledger_hash, creation_hash FROM match_journal_heads WHERE match_id = $1",
    ).bind(Uuid::from_u128(match_id.0)).fetch_one(pool).await.expect("claimed head must exist");
    let head = HeadImage {
        fence: row.get("fence"),
        format: row.get("format"),
        version: row.get("version"),
        input_index: row.get("input_index"),
        observed_ms: row.get("observed_ms"),
        record_count: row.get("record_count"),
        record_bytes: row.get("record_bytes"),
        ledger: row.get("ledger"),
        creation: row.get("creation"),
        latest_hash: row.get("latest_hash"),
        ledger_hash: row.get("ledger_hash"),
        creation_hash: row.get("creation_hash"),
    };
    let rows = sqlx::query(
        "SELECT input_index, state_version, logical_ms, state_hash, payload, payload_bytes, payload_hash FROM match_journal_records WHERE match_id = $1 ORDER BY input_index",
    ).bind(Uuid::from_u128(match_id.0)).fetch_all(pool).await.expect("committed records must be readable");
    let records = rows
        .into_iter()
        .map(|row| RecordImage {
            index: row.get("input_index"),
            version: row.get("state_version"),
            now: row.get("logical_ms"),
            hash: row.get("state_hash"),
            payload: row.get("payload"),
            bytes: row.get("payload_bytes"),
            payload_hash: row.get("payload_hash"),
        })
        .collect();
    (head, records)
}

async fn claim(pool: &PgPool) -> PgMatchJournal {
    PgMatchStore::new(pool.clone())
        .claim(MATCH)
        .await
        .expect("test match claim must succeed")
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_creation_and_append_are_atomic_with_ledger_snapshot_and_effects() {
    let db = DatabaseFixture::new().await;
    let journal = claim(&db.first).await;
    let (signal, release) = pause(&journal);
    let writer = journal.clone();
    let creating = tokio::spawn(async move { writer.append(genesis(MATCH)).await });
    arrived(signal).await;
    let (before, rows) = persisted(&db.observer, MATCH).await;
    assert_eq!(before.version, None);
    assert_eq!(before.record_count, 0);
    assert_eq!(before.record_bytes, 0);
    assert!(before.creation.is_none() && before.ledger.is_none() && before.latest_hash.is_none());
    assert!(
        rows.is_empty(),
        "uncommitted genesis must have no visible input row"
    );
    release.send(()).unwrap();
    assert_eq!(finish(creating).await, Ok(()));
    let initialized = persisted(&db.observer, MATCH).await;
    assert_eq!(initialized.0.version, Some(0));
    assert_eq!(initialized.0.record_count, 1);
    assert_eq!(initialized.1.len(), 1);
    let initial: JournalRecord = canonical_decode(&initialized.1[0].payload).unwrap();
    assert!(initial.snapshot.is_some() && initial.creation.is_some());
    assert_eq!(
        canonical_encode(&initial.effects).unwrap(),
        canonical_encode(&genesis(MATCH).effects).unwrap()
    );

    let record = accepted_transition(MATCH);
    let (signal, release) = pause(&journal);
    let writer = journal.clone();
    let committed = record.clone();
    let appending = tokio::spawn(async move { writer.append(committed).await });
    arrived(signal).await;
    assert_eq!(
        persisted(&db.observer, MATCH).await,
        initialized,
        "input, head, ledger and snapshot must stay at the same committed prefix"
    );
    release.send(()).unwrap();
    assert_eq!(finish(appending).await, Ok(()));
    let (head, rows) = persisted(&db.observer, MATCH).await;
    assert_eq!(head.version, Some(1));
    assert_eq!(head.input_index, Some(1));
    assert_eq!(head.record_count, 2);
    assert_eq!(head.latest_hash, Some(record.hash.0.to_vec()));
    assert_eq!(
        canonical_decode::<Vec<ScopeState>>(head.ledger.as_ref().unwrap()).unwrap(),
        record.ledger
    );
    let stored: JournalRecord = canonical_decode(&rows[1].payload).unwrap();
    assert!(
        stored.ledger.is_empty(),
        "head is authoritative for full receipt ledger"
    );
    assert_eq!(stored.input, record.input);
    assert_eq!(stored.events, record.events);
    assert_eq!(stored.hash, record.hash);
    assert_eq!(stored.snapshot, record.snapshot);
    assert_eq!(stored.operation, record.operation);
    assert_eq!(
        canonical_encode(&stored.effects).unwrap(),
        canonical_encode(&record.effects).unwrap()
    );
    assert_eq!(
        head.record_bytes,
        rows.iter().map(|row| row.bytes).sum::<i64>()
    );
    assert_eq!(journal.load(MATCH).await.unwrap().ledger, record.ledger);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_rollback_preserves_entire_visible_creation_append_and_ledger() {
    let db = DatabaseFixture::new().await;
    let journal = claim(&db.first).await;
    let blank = persisted(&db.observer, MATCH).await;
    journal.controls.next.store(1, Ordering::SeqCst);
    assert_eq!(
        journal.append(genesis(MATCH)).await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, blank);
    assert!(matches!(
        journal.load(MATCH).await,
        Err(RuntimePortError::Unavailable)
    ));
    assert_eq!(journal.append(genesis(MATCH)).await, Ok(()));
    let initial = persisted(&db.observer, MATCH).await;
    journal.controls.next.store(1, Ordering::SeqCst);
    assert_eq!(
        journal.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, initial);
    let reserved = vec![ScopeState {
        scope: scope(),
        highest: 0,
        recent: vec![],
    }];
    journal.controls.next.store(1, Ordering::SeqCst);
    assert_eq!(
        journal
            .update_ledger(MATCH, StateVersion(0), 5, reserved)
            .await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, initial);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_response_loss_is_real_commit_and_reopened_owner_recovers_prefix() {
    let db = DatabaseFixture::new().await;
    let first = claim(&db.first).await;
    first.controls.next.store(2, Ordering::SeqCst);
    assert_eq!(
        first.append(genesis(MATCH)).await,
        Err(RuntimePortError::Indeterminate)
    );
    let second = claim(&db.second).await;
    let loaded = second.load(MATCH).await.unwrap();
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.version, StateVersion(0));
    second.controls.next.store(2, Ordering::SeqCst);
    assert_eq!(
        second.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Indeterminate)
    );
    let reopened = db.reopen().await;
    assert_ne!(backend_pid(&reopened).await, backend_pid(&db.second).await);
    let replacement = claim(&reopened).await;
    let loaded = replacement.load(MATCH).await.unwrap();
    assert_eq!(loaded.records.len(), 2);
    assert_eq!(loaded.version, StateVersion(1));
    assert_eq!(loaded.index, InputIndex(1));
    assert_eq!(loaded.ledger, accepted_ledger(MATCH, 1, 1, 10));
    assert_eq!(
        loaded.records[1].snapshot,
        accepted_transition(MATCH).snapshot
    );
    replacement.controls.next.store(2, Ordering::SeqCst);
    assert_eq!(
        replacement
            .update_ledger(MATCH, StateVersion(1), 20, rejected_ledger(MATCH, 20))
            .await,
        Err(RuntimePortError::Indeterminate)
    );
    reopened.close().await;
    let reopened_again = db.reopen().await;
    let after_loss = claim(&reopened_again).await.load(MATCH).await.unwrap();
    assert_eq!(
        after_loss.records.len(),
        2,
        "rejection consumes no input or version"
    );
    assert_eq!(after_loss.version, StateVersion(1));
    assert_eq!(after_loss.observed_ms, 20);
    assert_eq!(after_loss.ledger, rejected_ledger(MATCH, 20));
    reopened_again.close().await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_second_claim_fences_stale_append_ledger_and_load() {
    let db = DatabaseFixture::new().await;
    let stale = claim(&db.first).await;
    stale.append(genesis(MATCH)).await.unwrap();
    let current = claim(&db.second).await;
    let before = persisted(&db.observer, MATCH).await;
    assert_eq!(
        stale.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Busy)
    );
    assert_eq!(
        stale.update_ledger(MATCH, StateVersion(0), 5, vec![]).await,
        Err(RuntimePortError::Busy)
    );
    assert!(matches!(
        stale.load(MATCH).await,
        Err(RuntimePortError::Busy)
    ));
    assert_eq!(persisted(&db.observer, MATCH).await, before);
    current.append(accepted_transition(MATCH)).await.unwrap();
    assert_eq!(current.load(MATCH).await.unwrap().version, StateVersion(1));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_conflicting_expected_version_and_time_rollback_change_nothing() {
    let db = DatabaseFixture::new().await;
    let journal = claim(&db.first).await;
    journal.append(genesis(MATCH)).await.unwrap();
    journal.append(accepted_transition(MATCH)).await.unwrap();
    let before = persisted(&db.observer, MATCH).await;
    assert_eq!(
        journal.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Busy)
    );
    assert_eq!(
        journal
            .update_ledger(MATCH, StateVersion(0), 20, rejected_ledger(MATCH, 20))
            .await,
        Err(RuntimePortError::Busy)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, before);
    journal
        .update_ledger(MATCH, StateVersion(1), 20, rejected_ledger(MATCH, 20))
        .await
        .unwrap();
    let observed = persisted(&db.observer, MATCH).await;
    assert_eq!(
        journal
            .update_ledger(MATCH, StateVersion(1), 19, rejected_ledger(MATCH, 20))
            .await,
        Err(RuntimePortError::Unavailable)
    );
    let mut backwards = transition(MATCH, 2);
    backwards.now = LogicalTime(19);
    backwards.ledger = rejected_ledger(MATCH, 20);
    assert_eq!(
        journal.append(backwards).await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, observed);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_same_fence_concurrent_expected_version_has_exactly_one_winner() {
    let db = DatabaseFixture::new().await;
    let first = claim(&db.first).await;
    first.append(genesis(MATCH)).await.unwrap();
    let mut second = first.clone();
    second.pool = db.second.clone();
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    let (signal, release) = pause(&first);
    let first_task = tokio::spawn(async move { first.append(accepted_transition(MATCH)).await });
    arrived(signal).await;
    let second_task = tokio::spawn(async move { second.append(accepted_transition(MATCH)).await });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    let held = persisted(&db.observer, MATCH).await;
    assert_eq!(held.0.version, Some(0));
    assert_eq!(held.1.len(), 1);
    release.send(()).unwrap();
    assert_eq!(finish(first_task).await, Ok(()));
    assert_eq!(finish(second_task).await, Err(RuntimePortError::Busy));
    let committed = persisted(&db.observer, MATCH).await;
    assert_eq!(committed.0.version, Some(1));
    assert_eq!(committed.0.record_count, 2);
    assert_eq!(committed.1.len(), 2);
    let recovered = claim(&db.second).await.load(MATCH).await.unwrap();
    assert_eq!(recovered.ledger, accepted_ledger(MATCH, 1, 1, 10));
    db.close().await;
}

#[derive(Clone, Copy)]
enum MatchTable {
    Heads,
    Records,
}

/// Only this test's isolated tables and catalog-checked constraint names can be
/// changed. Removing checks admits deliberately corrupt storage fixtures; it
/// must never turn an otherwise failed UPDATE into an unexercised test branch.
async fn drop_checks(pool: &PgPool, table: MatchTable) {
    let (relation, prefix) = match table {
        MatchTable::Heads => ("match_journal_heads", "match_journal_heads_"),
        MatchTable::Records => ("match_journal_records", "match_journal_records_"),
    };
    let checks: Vec<String> = sqlx::query_scalar(
        "SELECT conname FROM pg_constraint WHERE conrelid = $1::regclass AND contype = 'c' ORDER BY conname",
    ).bind(relation).fetch_all(pool).await.expect("isolated check constraint catalog must be readable");
    assert!(
        !checks.is_empty(),
        "corruption fixture must actually remove existing checks"
    );
    for check in checks {
        assert!(check.starts_with(prefix));
        assert!(check.len() <= 63);
        assert!(check
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
        sqlx::raw_sql(AssertSqlSafe(format!(
            "ALTER TABLE {relation} DROP CONSTRAINT {check}"
        )))
        .execute(pool)
        .await
        .expect("isolated corruption fixture check removal must succeed");
    }
}

async fn initialized(pool: &PgPool, match_id: MatchId, transitions: u64) -> PgMatchJournal {
    let journal = PgMatchStore::new(pool.clone())
        .claim(match_id)
        .await
        .unwrap();
    journal.append(genesis(match_id)).await.unwrap();
    for index in 1..=transitions {
        journal.append(transition(match_id, index)).await.unwrap();
    }
    journal
}

async fn replace_payload(pool: &PgPool, match_id: MatchId, index: u64, payload: Vec<u8>) {
    let old_bytes: i64 = sqlx::query_scalar(
        "SELECT payload_bytes FROM match_journal_records WHERE match_id = $1 AND input_index = $2",
    )
    .bind(Uuid::from_u128(match_id.0))
    .bind(i64::try_from(index).unwrap())
    .fetch_one(pool)
    .await
    .unwrap();
    let bytes = i64::try_from(payload.len()).unwrap();
    let checksum = blake3::hash(&payload).as_bytes().to_vec();
    let result = sqlx::query(
        "UPDATE match_journal_records SET payload = $3, payload_bytes = $4, payload_hash = $5 WHERE match_id = $1 AND input_index = $2",
    ).bind(Uuid::from_u128(match_id.0)).bind(i64::try_from(index).unwrap())
        .bind(payload).bind(bytes).bind(checksum).execute(pool).await.unwrap();
    assert_eq!(result.rows_affected(), 1);
    let result = sqlx::query(
        "UPDATE match_journal_heads SET record_bytes = record_bytes + $2 WHERE match_id = $1",
    )
    .bind(Uuid::from_u128(match_id.0))
    .bind(bytes - old_bytes)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
}

async fn assert_corrupt(journal: &PgMatchJournal, match_id: MatchId) {
    assert!(
        matches!(
            journal.load(match_id).await,
            Err(RuntimePortError::Unavailable)
        ),
        "corrupt storage must fail closed rather than return a partial or plausible prefix"
    );
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_head_mismatch_gap_and_negative_fields_fail_closed() {
    let db = DatabaseFixture::new().await;
    let mismatched = initialized(&db.first, MatchId(201), 1).await;
    let result = sqlx::query("UPDATE match_journal_heads SET latest_hash = $2 WHERE match_id = $1")
        .bind(Uuid::from_u128(201))
        .bind(vec![0x55_u8; 32])
        .execute(&db.observer)
        .await
        .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&mismatched, MatchId(201)).await;

    let missing = initialized(&db.first, MatchId(202), 2).await;
    let result =
        sqlx::query("DELETE FROM match_journal_records WHERE match_id = $1 AND input_index = 1")
            .bind(Uuid::from_u128(202))
            .execute(&db.observer)
            .await
            .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&missing, MatchId(202)).await;

    let negative_row = initialized(&db.first, MatchId(203), 1).await;
    drop_checks(&db.observer, MatchTable::Records).await;
    let result = sqlx::query(
        "UPDATE match_journal_records SET logical_ms = -1 WHERE match_id = $1 AND input_index = 1",
    )
    .bind(Uuid::from_u128(203))
    .execute(&db.observer)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&negative_row, MatchId(203)).await;

    let negative_head = initialized(&db.first, MatchId(204), 1).await;
    drop_checks(&db.observer, MatchTable::Heads).await;
    let result = sqlx::query("UPDATE match_journal_heads SET observed_ms = -1 WHERE match_id = $1")
        .bind(Uuid::from_u128(204))
        .execute(&db.observer)
        .await
        .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&negative_head, MatchId(204)).await;

    let counter_head = initialized(&db.first, MatchId(205), 1).await;
    let result =
        sqlx::query("UPDATE match_journal_heads SET record_count = -1 WHERE match_id = $1")
            .bind(Uuid::from_u128(205))
            .execute(&db.observer)
            .await
            .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&counter_head, MatchId(205)).await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_canonical_blob_corruption_fails_with_matching_integrity_hashes() {
    let db = DatabaseFixture::new().await;
    for (offset, bytes) in [vec![0, 0, 0], vec![1, 0, 255], {
        let mut bytes = canonical_encode(&genesis(MatchId(303))).unwrap();
        bytes.push(0);
        bytes
    }]
    .into_iter()
    .enumerate()
    {
        let match_id = MatchId(301 + u128::try_from(offset).unwrap());
        let journal = initialized(&db.first, match_id, 0).await;
        replace_payload(&db.observer, match_id, 0, bytes).await;
        assert_corrupt(&journal, match_id).await;
    }
    let no_snapshot = initialized(&db.first, MatchId(304), 0).await;
    let mut altered = genesis(MatchId(304));
    altered.snapshot = None;
    altered.ledger.clear();
    replace_payload(
        &db.observer,
        MatchId(304),
        0,
        canonical_encode(&altered).unwrap(),
    )
    .await;
    assert_corrupt(&no_snapshot, MatchId(304)).await;

    let creation_corrupt = initialized(&db.first, MatchId(305), 0).await;
    let bytes = vec![0, 0, 0];
    let checksum = blake3::hash(&bytes).as_bytes().to_vec();
    let result = sqlx::query(
        "UPDATE match_journal_heads SET creation = $2, creation_hash = $3 WHERE match_id = $1",
    )
    .bind(Uuid::from_u128(305))
    .bind(bytes)
    .bind(checksum)
    .execute(&db.observer)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&creation_corrupt, MatchId(305)).await;

    let ledger_corrupt = initialized(&db.first, MatchId(306), 0).await;
    let mut bytes = canonical_encode(&Vec::<ScopeState>::new()).unwrap();
    bytes.push(0);
    let checksum = blake3::hash(&bytes).as_bytes().to_vec();
    let result = sqlx::query(
        "UPDATE match_journal_heads SET ledger = $2, ledger_hash = $3 WHERE match_id = $1",
    )
    .bind(Uuid::from_u128(306))
    .bind(bytes)
    .bind(checksum)
    .execute(&db.observer)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&ledger_corrupt, MatchId(306)).await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_integrity_hashes_detect_valid_canonical_payload_tampering() {
    let db = DatabaseFixture::new().await;
    let journal = initialized(&db.first, MATCH, 1).await;
    let mut record = transition(MATCH, 1);
    record.events[0] = canonical_encode(&99_u8).unwrap();
    record.ledger.clear();
    let bytes = canonical_encode(&record).unwrap();
    let count = i64::try_from(bytes.len()).unwrap();
    let result = sqlx::query(
        "UPDATE match_journal_records SET payload = $2, payload_bytes = $3 WHERE match_id = $1 AND input_index = 1",
    ).bind(Uuid::from_u128(MATCH.0)).bind(bytes).bind(count).execute(&db.observer).await.unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_corrupt(&journal, MATCH).await;
    db.close().await;
}

#[test]
fn strict_canonical_decode_rejects_unknown_truncated_trailing_and_nonminimal_bytes() {
    assert_eq!(
        super::strict_decode::<u64>(&canonical_encode(&1_u64).unwrap()),
        Ok(1)
    );
    for bytes in [
        vec![],
        vec![1],
        vec![0, 0, 1],
        vec![1, 0, 128],
        vec![1, 0, 1, 0],
        vec![1, 0, 129, 0],
    ] {
        assert_eq!(
            super::strict_decode::<u64>(&bytes),
            Err(RuntimePortError::Unavailable),
            "{bytes:?}"
        );
    }
}

#[test]
fn disposable_schema_identifiers_are_scoped_and_generated() {
    let schema = "tabula_match_acceptance_1_1";
    assert!(schema_ddl(schema, SchemaDdl::Create)
        .0
        .starts_with("CREATE SCHEMA tabula_match_acceptance_"));
    assert!(schema_ddl(schema, SchemaDdl::Drop)
        .0
        .starts_with("DROP SCHEMA tabula_match_acceptance_"));
}

#[test]
fn creation_validation_rejects_bad_format_clock_config_and_zero_bounds() {
    assert_eq!(super::validate_creation(&creation()), Ok(()));
    let mut maximum = creation();
    maximum.limits = LedgerLimits {
        scopes: 256,
        receipts_per_scope: 64,
        receipt_ttl_ms: 3_600_000,
    };
    assert_eq!(super::validate_creation(&maximum), Ok(()));
    let mut cases = Vec::new();
    let mut altered = creation();
    altered.identity.rules_hash = [0; 32];
    cases.push(altered);
    let mut altered = creation();
    altered.format = JOURNAL_FORMAT + 1;
    cases.push(altered);
    let mut altered = creation();
    altered.started_at_unix_ms = u64::MAX;
    cases.push(altered);
    let mut altered = creation();
    altered.config = vec![0, 0];
    cases.push(altered);
    let mut altered = creation();
    altered.config = vec![1];
    cases.push(altered);
    let mut altered = creation();
    altered.limits.scopes = 0;
    cases.push(altered);
    let mut altered = creation();
    altered.limits.receipts_per_scope = 0;
    cases.push(altered);
    let mut altered = creation();
    altered.limits.receipt_ttl_ms = 0;
    cases.push(altered);
    let mut altered = creation();
    altered.roster = SeatRoster::new(Vec::new().into()).unwrap();
    cases.push(altered);
    let mut altered = creation();
    altered.limits.scopes = 257;
    cases.push(altered);
    let mut altered = creation();
    altered.limits.receipts_per_scope = 65;
    cases.push(altered);
    let mut altered = creation();
    altered.limits.receipt_ttl_ms = 3_600_001;
    cases.push(altered);
    for altered in cases {
        assert_eq!(
            super::validate_creation(&altered),
            Err(RuntimePortError::Unavailable)
        );
    }
}

#[test]
fn record_validation_requires_genesis_periodic_and_terminal_snapshots() {
    let creation = creation();
    assert_eq!(super::validate_record(&genesis(MATCH), &creation), Ok(()));
    for index in [1, 19, 20, 21] {
        assert_eq!(
            super::validate_record(&transition(MATCH, index), &creation),
            Ok(())
        );
    }
    let mut cases = Vec::new();
    let mut altered = genesis(MATCH);
    altered.snapshot = None;
    cases.push(altered);
    let mut altered = transition(MATCH, 20);
    altered.snapshot = None;
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.terminal = true;
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.snapshot = Some(canonical_encode(&1_u64).unwrap());
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.snapshot = Some(vec![1]);
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.snapshot = Some(vec![0, 0, 1]);
    cases.push(altered);
    let mut altered = genesis(MATCH);
    let mut oversized = vec![0; MAX_SNAPSHOT_BYTES + 1];
    oversized[0] = 1;
    altered.snapshot = Some(oversized);
    cases.push(altered);
    for altered in cases {
        assert_eq!(
            super::validate_record(&altered, &creation),
            Err(RuntimePortError::Unavailable)
        );
    }
    let mut terminal = transition(MATCH, 1);
    terminal.terminal = true;
    terminal.snapshot = Some(canonical_encode(&1_u64).unwrap());
    assert_eq!(super::validate_record(&terminal, &creation), Ok(()));
}

#[test]
fn record_validation_rejects_wrong_resource_counter_and_initialization_shape() {
    let creation = creation();
    let mut cases = Vec::new();
    let altered = genesis(MatchId(0));
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.creation = None;
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.creation.as_mut().unwrap().seed = [10; 32];
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.now = LogicalTime(1);
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.input = canonical_encode(&1_u8).unwrap();
    cases.push(altered);
    let mut altered = genesis(MATCH);
    altered.expected_version = Some(StateVersion(0));
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.creation = Some(creation.clone());
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.expected_version = None;
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.expected_version = Some(StateVersion(1));
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.version = StateVersion(2);
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.input.clear();
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.events = vec![vec![0, 0]];
    cases.push(altered);
    let mut altered = transition(MATCH, 1);
    altered.now = LogicalTime(u64::MAX);
    cases.push(altered);
    let altered = transition(MATCH, 10_001);
    cases.push(altered);
    let mut altered = accepted_transition(MATCH);
    altered.operation.as_mut().unwrap().command = command(MatchId(999), 1);
    cases.push(altered);
    for altered in cases {
        assert_eq!(
            super::validate_record(&altered, &creation),
            Err(RuntimePortError::Unavailable)
        );
    }
}

#[test]
fn ledger_validation_rejects_duplicate_scopes_invalid_receipts_and_future_times() {
    let creation = creation();
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &reserved_ledger(), 0),
        Ok(())
    );
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &accepted_ledger(MATCH, 1, 1, 10), 10),
        Ok(())
    );
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &rejected_ledger(MATCH, 20), 20),
        Ok(())
    );
    let mut sorted = reserved_ledger();
    let mut next = sorted[0].clone();
    next.scope.record += 1;
    sorted.push(next);
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &sorted, 10),
        Ok(())
    );
    let mut cases = Vec::new();
    let mut full = accepted_ledger(MATCH, 1, 1, 10);
    full[0].highest = 4;
    full[0].recent = (1..=4)
        .map(|seq| {
            accepted_ledger(MATCH, seq, seq, 10)
                .remove(0)
                .recent
                .remove(0)
        })
        .collect();
    assert_eq!(super::validate_ledger(MATCH, &creation, &full, 10), Ok(()));
    full[0].highest = 5;
    full[0]
        .recent
        .push(accepted_ledger(MATCH, 5, 5, 10).remove(0).recent.remove(0));
    cases.push((full, 10));
    sorted.reverse();
    cases.push((sorted, 10));
    let too_many = (0..5)
        .map(|offset| {
            let mut state = reserved_ledger().remove(0);
            state.scope.record += offset;
            state
        })
        .collect();
    cases.push((too_many, 10));
    let mut altered = reserved_ledger();
    altered.push(altered[0].clone());
    cases.push((altered, 10));
    let mut altered = reserved_ledger();
    altered[0].scope.record = 0;
    cases.push((altered, 10));
    let mut altered = reserved_ledger();
    altered[0].scope.subject = UserId(0);
    cases.push((altered, 10));
    let mut altered = reserved_ledger();
    altered[0].scope.seat = SeatId(99);
    cases.push((altered, 10));
    let mut altered = accepted_ledger(MATCH, 1, 1, 10);
    altered[0].highest = 0;
    cases.push((altered, 10));
    let mut altered = accepted_ledger(MATCH, 1, 1, 10);
    altered[0].recent[0].seq = 0;
    cases.push((altered, 10));
    let mut altered = accepted_ledger(MATCH, 1, 1, 10);
    let duplicate = altered[0].recent[0].clone();
    altered[0].recent.push(duplicate);
    cases.push((altered, 10));
    let mut altered = accepted_ledger(MATCH, 1, 1, 10);
    altered[0].recent[0].committed_index = None;
    cases.push((altered, 10));
    let mut altered = accepted_ledger(MATCH, 1, 1, 10);
    altered[0].recent[0].result = Err(ErrorCode::RuleRejected);
    cases.push((altered, 10));
    cases.push((accepted_ledger(MatchId(999), 1, 1, 10), 10));
    cases.push((accepted_ledger(MATCH, 1, 1, 10), 9));
    for (altered, now) in cases {
        assert_eq!(
            super::validate_ledger(MATCH, &creation, &altered, now),
            Err(RuntimePortError::Unavailable)
        );
    }
}

#[test]
fn ledger_validation_preserves_expired_receipts_and_rejected_foreign_envelopes() {
    let creation = creation();
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &accepted_ledger(MATCH, 1, 1, 10), 1_010),
        Ok(()),
        "receipt is retained at its exact TTL boundary"
    );
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &accepted_ledger(MATCH, 1, 1, 10), 1_011),
        Ok(()),
        "expired bytes remain valid until actor access prunes them; the watermark survives"
    );
    let mut foreign_reject = rejected_ledger(MATCH, 20);
    foreign_reject[0].recent[1].command = command(MatchId(999), 2);
    assert_eq!(
        super::validate_ledger(MATCH, &creation, &foreign_reject, 20),
        Ok(()),
        "rejection preserves the original malformed-envelope identity"
    );
}

#[test]
fn ledger_transition_preserves_watermarks_and_receipts_and_requires_matching_acceptance() {
    let reserved = reserved_ledger();
    let accepted = accepted_ledger(MATCH, 1, 1, 10);
    let record = accepted_transition(MATCH);
    assert_eq!(
        super::validate_transition(
            &reserved,
            &accepted,
            record.operation.as_ref(),
            InputIndex(1)
        ),
        Ok(())
    );
    assert_eq!(
        super::validate_transition(&reserved, &accepted, None, InputIndex(1)),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::validate_transition(&[], &accepted, record.operation.as_ref(), InputIndex(1)),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::validate_transition(
            &reserved,
            &accepted,
            record.operation.as_ref(),
            InputIndex(2)
        ),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::validate_transition(&accepted, &rejected_ledger(MATCH, 20), None, InputIndex(1)),
        Ok(())
    );
    assert_eq!(
        super::validate_transition(&accepted, &[], None, InputIndex(1)),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::validate_transition(&accepted, &reserved, None, InputIndex(1)),
        Err(RuntimePortError::Unavailable)
    );
    let mut mutated = accepted.clone();
    mutated[0].recent[0].command = command(MATCH, 9);
    assert_eq!(
        super::validate_transition(&accepted, &mutated, None, InputIndex(1)),
        Err(RuntimePortError::Unavailable)
    );
    let evicted = vec![ScopeState {
        scope: scope(),
        highest: 1,
        recent: vec![],
    }];
    assert_eq!(
        super::validate_transition(&accepted, &evicted, None, InputIndex(1)),
        Ok(()),
        "receipt expiry may remove bytes but cannot lower the watermark"
    );
    assert_eq!(
        super::validate_transition(&evicted, &accepted, None, InputIndex(1)),
        Err(RuntimePortError::Unavailable),
        "an expired sequence can never be recreated as a new acceptance"
    );
}

#[test]
fn signed_database_conversion_never_wraps_negative_or_overflowing_values() {
    assert_eq!(super::signed(0), Ok(0));
    assert_eq!(
        super::signed(u64::try_from(i64::MAX).unwrap()),
        Ok(i64::MAX)
    );
    assert_eq!(super::signed(u64::MAX), Err(RuntimePortError::Unavailable));
    assert_eq!(super::unsigned(-1), Err(RuntimePortError::Unavailable));
    assert_eq!(super::size(-1), Err(RuntimePortError::Unavailable));
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_periodic_snapshot_and_terminal_effects_commit_in_the_input_transaction() {
    let db = DatabaseFixture::new().await;
    let journal = initialized(&db.first, MATCH, 19).await;
    let before = persisted(&db.observer, MATCH).await;
    assert_eq!(before.0.version, Some(19));
    let periodic = transition(MATCH, 20);
    let (signal, release) = pause(&journal);
    let writer = journal.clone();
    let appending = tokio::spawn(async move { writer.append(periodic).await });
    arrived(signal).await;
    assert_eq!(persisted(&db.observer, MATCH).await, before);
    release.send(()).unwrap();
    assert_eq!(finish(appending).await, Ok(()));
    let mut terminal = transition(MATCH, 21);
    terminal.terminal = true;
    terminal.snapshot = Some(canonical_encode(&21_u64).unwrap());
    let expected_effects = canonical_encode(&terminal.effects).unwrap();
    let (signal, release) = pause(&journal);
    let writer = journal.clone();
    let ending = tokio::spawn(async move { writer.append(terminal).await });
    arrived(signal).await;
    let held = persisted(&db.observer, MATCH).await;
    assert_eq!(held.0.version, Some(20));
    let row: JournalRecord = canonical_decode(&held.1[20].payload).unwrap();
    assert_eq!(row.snapshot, Some(canonical_encode(&20_u64).unwrap()));
    assert!(!row.terminal);
    release.send(()).unwrap();
    assert_eq!(finish(ending).await, Ok(()));
    let loaded = journal.load(MATCH).await.unwrap();
    assert_eq!(loaded.records.len(), 22);
    assert_eq!(loaded.version, StateVersion(21));
    assert!(loaded.records[21].terminal);
    assert_eq!(
        loaded.records[21].snapshot,
        Some(canonical_encode(&21_u64).unwrap())
    );
    assert_eq!(
        canonical_encode(&loaded.records[21].effects).unwrap(),
        expected_effects
    );
    let ended = persisted(&db.observer, MATCH).await;
    assert_eq!(
        journal.append(transition(MATCH, 22)).await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(persisted(&db.observer, MATCH).await, ended);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_reclaim_waits_for_pending_commit_then_fences_prior_generation() {
    let db = DatabaseFixture::new().await;
    let journal = initialized(&db.first, MATCH, 0).await;
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    let (signal, release) = pause(&journal);
    let writer = journal.clone();
    let pending = tokio::spawn(async move { writer.append(accepted_transition(MATCH)).await });
    arrived(signal).await;
    let other_pool = db.second.clone();
    let reclaiming = tokio::spawn(async move { PgMatchStore::new(other_pool).claim(MATCH).await });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    assert_eq!(persisted(&db.observer, MATCH).await.0.version, Some(0));
    release.send(()).unwrap();
    assert_eq!(finish(pending).await, Ok(()));
    let replacement = finish(reclaiming).await.unwrap();
    let loaded = replacement.load(MATCH).await.unwrap();
    assert_eq!(loaded.version, StateVersion(1));
    assert_eq!(loaded.records.len(), 2);
    assert_eq!(loaded.ledger, accepted_ledger(MATCH, 1, 1, 10));
    assert!(matches!(
        journal.load(MATCH).await,
        Err(RuntimePortError::Busy)
    ));
    assert_eq!(
        journal.append(transition(MATCH, 2)).await,
        Err(RuntimePortError::Busy)
    );
    db.close().await;
}

#[test]
fn checked_blob_requires_present_bounded_bytes_and_exact_integrity_digest() {
    let bytes = canonical_encode(&42_u64).unwrap();
    let correct = Some(blake3::hash(&bytes).as_bytes().to_vec());
    let blob = Some(bytes.clone());
    assert_eq!(
        super::checked_blob(blob.as_deref(), correct.as_deref(), bytes.len()),
        Ok(bytes.as_slice())
    );
    assert_eq!(
        super::checked_blob(blob.as_deref(), correct.as_deref(), bytes.len() - 1),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::checked_blob(None, correct.as_deref(), 1_024),
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(
        super::checked_blob(blob.as_deref(), None, 1_024),
        Err(RuntimePortError::Unavailable)
    );
    for invalid in [vec![], vec![0; 31], vec![0; 32], vec![0; 33]] {
        assert_eq!(
            super::checked_blob(blob.as_deref(), Some(invalid.as_slice()), 1_024),
            Err(RuntimePortError::Unavailable)
        );
    }
}

#[tokio::test]
#[ignore = "requires disposable real PostgreSQL 16 via TABULA_MATCH_DATABASE_URL"]
async fn real_postgres_server_failed_commit_preserves_prefix_and_recovered_retry_commits_once() {
    let db = DatabaseFixture::new().await;
    let journal = initialized(&db.first, MATCH, 0).await;
    let before = persisted(&db.observer, MATCH).await;
    let loaded_before = journal.load(MATCH).await.unwrap();
    let fixture_store = PgMatchStore::new(db.observer.clone());
    fixture_store
        .install_commit_failure_for_test()
        .await
        .unwrap();
    assert_eq!(
        journal.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Indeterminate),
        "the server rejects COMMIT itself, so the adapter cannot advertise known success"
    );
    assert_eq!(
        persisted(&db.observer, MATCH).await,
        before,
        "server-failed COMMIT must preserve every head field, receipt and input row"
    );
    let replacement = claim(&db.second).await;
    let recovered = replacement.load(MATCH).await.unwrap();
    assert_eq!(recovered.version, loaded_before.version);
    assert_eq!(recovered.index, loaded_before.index);
    assert_eq!(recovered.observed_ms, loaded_before.observed_ms);
    assert_eq!(recovered.ledger, loaded_before.ledger);
    assert!(
        canonical_encode(&recovered.creation).unwrap()
            == canonical_encode(&loaded_before.creation).unwrap(),
        "immutable creation must match the last committed prefix"
    );
    assert!(
        canonical_encode(&recovered.records).unwrap()
            == canonical_encode(&loaded_before.records).unwrap(),
        "recovery must return exactly the prior committed record prefix"
    );
    fixture_store.clear_commit_failure_for_test().await.unwrap();
    replacement
        .append(accepted_transition(MATCH))
        .await
        .unwrap();
    let retried = replacement.load(MATCH).await.unwrap();
    assert_eq!(retried.records.len(), 2);
    assert_eq!(retried.version, StateVersion(1));
    assert_eq!(retried.index, InputIndex(1));
    assert_eq!(retried.ledger, accepted_ledger(MATCH, 1, 1, 10));
    let committed = persisted(&db.observer, MATCH).await;
    assert_eq!(
        replacement.append(accepted_transition(MATCH)).await,
        Err(RuntimePortError::Busy)
    );
    assert_eq!(
        persisted(&db.observer, MATCH).await,
        committed,
        "retrying an already committed sequence cannot append it twice"
    );
    db.close().await;
}

#[test]
fn genesis_validation_allows_reserved_zero_scopes_and_rejects_consumed_sequences() {
    let creation = creation();
    let reserved = genesis(MATCH);
    assert_eq!(super::validate_record(&reserved, &creation), Ok(()));
    let mut watermark = genesis(MATCH);
    watermark.ledger[0].highest = 1;
    assert_eq!(
        super::validate_record(&watermark, &creation),
        Err(RuntimePortError::Unavailable),
        "genesis cannot invent a previously consumed operation watermark"
    );
    let mut receipt = genesis(MATCH);
    receipt.ledger[0].recent.push(OperationReceipt {
        seq: 1,
        command: command(MATCH, 1),
        result: Err(ErrorCode::RuleRejected),
        at: 0,
        committed_index: None,
    });
    assert_eq!(
        super::validate_record(&receipt, &creation),
        Err(RuntimePortError::Unavailable),
        "genesis reserves scopes but cannot contain earlier operation receipts"
    );
}

#[cfg(feature = "online-match-postgres")]
#[test]
fn online_authenticated_ledger_changes_cannot_create_or_advance_another_admission() {
    let authorized = scope();
    let foreign = OperationScope {
        record: 777,
        subject: UserId(43),
        epoch: 1,
        seat: SeatId(1),
        generation: 1,
    };
    let receipt = OperationReceipt {
        seq: 1,
        command: command(MATCH, 1),
        result: Err(ErrorCode::Malformed),
        at: 0,
        committed_index: None,
    };
    let old = vec![
        ScopeState {
            scope: authorized,
            highest: 0,
            recent: vec![],
        },
        ScopeState {
            scope: foreign,
            highest: 1,
            recent: vec![receipt.clone()],
        },
    ];
    let mut expired = old.clone();
    expired[1].recent.clear();
    assert_eq!(
        super::validate_authenticated_scopes(&old, &expired, authorized),
        Ok(()),
        "foreign receipt TTL eviction changes no watermark or admission"
    );
    let mut forged = expired.clone();
    forged[1].highest = 2;
    assert_eq!(
        super::validate_authenticated_scopes(&old, &forged, authorized),
        Err(RuntimePortError::Unavailable)
    );
    let mut forged = expired;
    forged[1]
        .recent
        .push(OperationReceipt { seq: 2, ..receipt });
    assert_eq!(
        super::validate_authenticated_scopes(&old, &forged, authorized),
        Err(RuntimePortError::Unavailable)
    );
    let new_scope = vec![ScopeState {
        scope: foreign,
        highest: 0,
        recent: vec![],
    }];
    assert_eq!(
        super::validate_authenticated_scopes(&[], &new_scope, authorized),
        Err(RuntimePortError::Unavailable)
    );
}
