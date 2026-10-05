//! Actual `PostgreSQL` 16 admission/apply/commit boundaries. Missing setup fails.
//! These fixtures are synthetic accounts and journal bytes, never browser or
//! provider evidence. Explicit ignored selection is required in isolated CI.
use super::{OnlineMatchError, OnlineMembership, PgOnlineMatchStore};
use crate::match_postgres::{PgMatchJournal, PgMatchStore};
use crate::session::PgSessionStore;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tabula_core::{
    canonical_encode, state_hash, GameId, GameVersion, InputIndex, LogicalTime, MatchId,
    RulesVersion, StateVersion, UserId,
};
use tabula_match_journal::{
    Journal, JournalRecord, LedgerLimits, MatchCreation, MatchIdentity, OperationKey,
    OperationReceipt, RuntimePortError, ScopeState, JOURNAL_FORMAT,
};
use tabula_protocol::GameCommandFrame;
use tabula_session::{
    AccountEpoch, AccountRecord, AuthSessionId, CredentialOperation, HttpSessionAuthority,
    IssueSession, ProviderIdentityKey, SessionAuthority, SessionChannel, SessionContextBinding,
    SessionContextId, SessionCredential, SessionError, SessionSnapshot, UnixMillis,
};
use uuid::Uuid;

static COUNTER: AtomicU64 = AtomicU64::new(1);
const WAIT: Duration = Duration::from_secs(15);
const MATCH: MatchId = MatchId(101);
fn ddl(schema: &str, create: bool) -> AssertSqlSafe<String> {
    assert!(schema.starts_with("tabula_online_acceptance_") && schema.len() <= 63);
    assert!(schema
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'));
    AssertSqlSafe(if create {
        format!("CREATE SCHEMA {schema}")
    } else {
        format!("DROP SCHEMA {schema} CASCADE")
    })
}
struct Fixture {
    admin: PgPool,
    first: PgPool,
    second: PgPool,
    observer: PgPool,
    schema: String,
}
async fn configure(connection: &mut PgConnection, schema: String) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('search_path',$1,false)")
        .bind(schema)
        .execute(&mut *connection)
        .await?;
    sqlx::query("SET statement_timeout='10s'")
        .execute(&mut *connection)
        .await?;
    Ok(())
}
async fn fixture_pool(url: &str, schema: &str) -> PgPool {
    let schema = schema.to_owned();
    PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| Box::pin(configure(connection, schema.clone())))
        .connect(url)
        .await
        .unwrap()
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("TABULA_MATCH_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")).expect("real online PostgreSQL acceptance requires a dedicated database URL; setup cannot skip");
        let admin =
            tokio::time::timeout(WAIT, PgPoolOptions::new().max_connections(1).connect(&url))
                .await
                .unwrap()
                .unwrap();
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .unwrap();
        assert_eq!(
            version / 10_000,
            16,
            "online acceptance requires actual PostgreSQL 16"
        );
        let schema = format!(
            "tabula_online_acceptance_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        sqlx::raw_sql(ddl(&schema, true))
            .execute(&admin)
            .await
            .unwrap();
        let first = fixture_pool(&url, &schema).await;
        let second = fixture_pool(&url, &schema).await;
        let observer = fixture_pool(&url, &schema).await;
        PgOnlineMatchStore::migrate(&first)
            .await
            .expect("strict combined migration must execute");
        Self {
            admin,
            first,
            second,
            observer,
            schema,
        }
    }
    async fn close(self) {
        self.first.close().await;
        self.second.close().await;
        self.observer.close().await;
        sqlx::raw_sql(ddl(&self.schema, false))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
#[derive(Clone)]
struct Enrolled {
    identity: ProviderIdentityKey,
    snapshot: SessionSnapshot,
    operation: CredentialOperation,
}
async fn enroll(pool: &PgPool, user: u128, record: u128) -> Enrolled {
    let store = PgSessionStore::new(pool.clone());
    let identity =
        ProviderIdentityKey::new("https://online-fixture.invalid", format!("subject-{user}"))
            .unwrap();
    let now: i64 =
        sqlx::query_scalar("SELECT floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint")
            .fetch_one(pool)
            .await
            .unwrap();
    store
        .provision_fixture_identity(
            identity.clone(),
            AccountRecord::new(
                UserId(user),
                AccountEpoch::new(0).unwrap(),
                true,
                UnixMillis::new(u64::try_from(now).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    issue(pool, identity, record).await
}
async fn issue(pool: &PgPool, identity: ProviderIdentityKey, record: u128) -> Enrolled {
    let credential = SessionCredential::generate().unwrap();
    let digest = credential.digest();
    let snapshot = PgSessionStore::new(pool.clone())
        .issue_session(IssueSession {
            identity: identity.clone(),
            expected_epoch: AccountEpoch::new(0).unwrap(),
            id: AuthSessionId::new(record).unwrap(),
            channel: SessionChannel::BrowserCookie,
            credential_digest: digest,
            context_id: SessionContextId::new(record + 1_000_000).unwrap(),
        })
        .await
        .unwrap();
    let operation = CredentialOperation {
        digest,
        channel: SessionChannel::BrowserCookie,
        context: Some(SessionContextBinding {
            context_id: snapshot.context_id(),
            authorization_epoch: snapshot.authorization_epoch(),
        }),
    };
    Enrolled {
        identity,
        snapshot,
        operation,
    }
}
fn game() -> GameId {
    GameId::new("org.example.onlinefixture").unwrap()
}
fn version() -> GameVersion {
    GameVersion::new("1.0.0").unwrap()
}
async fn room(
    pool: &PgPool,
    owner: &Enrolled,
    match_id: MatchId,
    code: [u8; 32],
    seats: u8,
) -> OnlineMembership {
    PgOnlineMatchStore::new(pool.clone())
        .create(
            owner.operation,
            match_id,
            code,
            game(),
            version(),
            canonical_encode(&5_u64).unwrap(),
            seats,
        )
        .await
        .unwrap()
}
fn genesis(member: &OnlineMembership) -> JournalRecord {
    JournalRecord {
        match_id: member.match_id(),
        index: InputIndex(0),
        version: StateVersion(0),
        now: LogicalTime::ZERO,
        input: vec![],
        events: vec![],
        hash: state_hash(RulesVersion(1), &0_u64),
        effects: vec![],
        operation: None,
        terminal: false,
        snapshot: Some(canonical_encode(&0_u64).unwrap()),
        creation: Some(MatchCreation {
            format: JOURNAL_FORMAT,
            identity: MatchIdentity {
                game: member.game().clone(),
                game_version: member.game_version().clone(),
                rules_version: RulesVersion(1),
                rules_hash: [7; 32],
            },
            config: member.config().to_vec(),
            roster: member.roster().unwrap().clone(),
            seed: [9; 32],
            started_at_unix_ms: member.snapshot().last_observed_at().get(),
            limits: LedgerLimits {
                scopes: 64,
                receipts_per_scope: 4,
                receipt_ttl_ms: 1_000,
            },
        }),
        ledger: vec![],
        expected_version: None,
    }
}
fn accepted(member: &OnlineMembership) -> JournalRecord {
    let command = GameCommandFrame::new(
        member.match_id(),
        member.game().clone(),
        member.game_version().clone(),
        canonical_encode(&1_u8).unwrap(),
    )
    .unwrap();
    JournalRecord {
        match_id: member.match_id(),
        index: InputIndex(1),
        version: StateVersion(1),
        now: LogicalTime(10),
        input: canonical_encode(&1_u8).unwrap(),
        events: vec![],
        hash: state_hash(RulesVersion(1), &1_u64),
        effects: vec![],
        operation: Some(OperationKey {
            scope: member.scope(),
            seq: 1,
            command: command.clone(),
        }),
        terminal: false,
        snapshot: None,
        creation: None,
        ledger: vec![ScopeState {
            scope: member.scope(),
            highest: 1,
            recent: vec![OperationReceipt {
                seq: 1,
                command,
                result: Ok(()),
                at: 10,
                committed_index: Some(InputIndex(1)),
            }],
        }],
        expected_version: Some(StateVersion(0)),
    }
}
async fn ready(f: &Fixture) -> (Enrolled, Enrolled, OnlineMembership) {
    let owner = enroll(&f.first, 42, 501).await;
    let opponent = enroll(&f.second, 43, 502).await;
    room(&f.first, &owner, MATCH, [1; 32], 2).await;
    let member = PgOnlineMatchStore::new(f.second.clone())
        .join(opponent.operation, [1; 32])
        .await
        .unwrap();
    assert!(member.ready());
    assert!(!member.started());
    (owner, opponent, member)
}
async fn started(f: &Fixture) -> (Enrolled, Enrolled, PgMatchJournal) {
    let (owner, opponent, member) = ready(f).await;
    let journal = PgMatchStore::new(f.first.clone())
        .claim(MATCH)
        .await
        .unwrap();
    // Either real ready member may initialize; caller cannot manufacture roster/config.
    let guard = PgOnlineMatchStore::new(f.second.clone())
        .begin_operation(opponent.operation, MATCH)
        .await
        .unwrap();
    journal
        .append_authenticated(genesis(&member), &guard)
        .await
        .unwrap();
    assert!(
        guard.with_current(|_| ()).is_err(),
        "COMMIT invalidates the apply permit"
    );
    let current = PgOnlineMatchStore::new(f.first.clone())
        .resolve(owner.operation, MATCH)
        .await
        .unwrap();
    assert!(
        current.started(),
        "canonical genesis and room-start marker commit together"
    );
    (owner, opponent, journal)
}
async fn pid(pool: &PgPool) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn blocked(admin: &PgPool, waiting: i32, blocking: i32) {
    tokio::time::timeout(WAIT, async {
        loop {
            let is_blocked: bool =
                sqlx::query_scalar("SELECT $2::integer=ANY(pg_blocking_pids($1::integer))")
                    .bind(waiting)
                    .bind(blocking)
                    .fetch_one(admin)
                    .await
                    .unwrap();
            if is_blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("independent current-authority operation must demonstrably wait");
}

#[test]
fn online_composition_has_four_exact_strict_versions() {
    assert_eq!(
        PgOnlineMatchStore::migration_versions(),
        vec![
            202_610_040_001,
            202_610_050_001,
            202_610_050_040,
            202_610_050_041
        ]
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_composition_and_server_seat_roster() {
    let f = Fixture::new().await;
    let recorded: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&f.first)
            .await
            .unwrap();
    assert_eq!(recorded, PgOnlineMatchStore::migration_versions());
    assert!(
        PgSessionStore::new(f.first.clone())
            .migrate()
            .await
            .is_err(),
        "narrow migrator must not ignore combined history"
    );
    assert!(PgMatchStore::migrate(&f.first).await.is_err());
    let checksums: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&f.first)
            .await
            .unwrap();
    let mut expected: Vec<_> = super::SESSION_MIGRATIONS
        .iter()
        .chain(super::MATCH_MIGRATIONS.iter())
        .chain(super::ONLINE_MIGRATIONS.iter())
        .map(|migration| (migration.version, migration.checksum.to_vec()))
        .collect();
    expected.sort_by_key(|(version, _)| *version);
    assert_eq!(
        checksums, expected,
        "strict composition preserves original migration checksums"
    );
    PgOnlineMatchStore::migrate(&f.first).await.unwrap();
    let owner = enroll(&f.first, 42, 501).await;
    let opponent = enroll(&f.second, 43, 502).await;
    let creator = room(&f.first, &owner, MATCH, [1; 32], 2).await;
    assert_eq!(creator.scope().seat.0, 0);
    assert!(!creator.ready());
    assert!(creator.roster().is_none());
    let member = PgOnlineMatchStore::new(f.second.clone())
        .join(opponent.operation, [1; 32])
        .await
        .unwrap();
    assert_eq!(member.scope().seat.0, 1);
    assert_eq!(member.roster().unwrap().len(), 2);
    assert_eq!(
        member
            .roster()
            .unwrap()
            .get(tabula_core::SeatId(0))
            .unwrap()
            .occupant,
        tabula_core::Occupant::Human(UserId(42))
    );
    assert_eq!(
        member
            .roster()
            .unwrap()
            .get(tabula_core::SeatId(1))
            .unwrap()
            .occupant,
        tabula_core::Occupant::Human(UserId(43))
    );
    let third = enroll(&f.first, 44, 503).await;
    assert!(matches!(
        PgOnlineMatchStore::new(f.first.clone())
            .join(third.operation, [1; 32])
            .await,
        Err(OnlineMatchError::JoinUnavailable)
    ));
    let resolved = PgOnlineMatchStore::new(f.first.clone())
        .resolve(owner.operation, MATCH)
        .await
        .unwrap();
    assert!(resolved.ready());
    assert_eq!(resolved.scope(), creator.scope());
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_failed_attempts_expiry_and_room_capacity_are_durable() {
    let f = Fixture::new().await;
    let owner = enroll(&f.first, 42, 501).await;
    let joiner = enroll(&f.second, 43, 502).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    for index in 0..4 {
        room(
            &f.first,
            &owner,
            MatchId(101 + index),
            [u8::try_from(index + 1).unwrap(); 32],
            2,
        )
        .await;
    }
    assert!(matches!(
        store
            .create(
                owner.operation,
                MatchId(999),
                [9; 32],
                game(),
                version(),
                canonical_encode(&5_u64).unwrap(),
                2
            )
            .await,
        Err(OnlineMatchError::Busy)
    ));
    let joining = PgOnlineMatchStore::new(f.second.clone());
    for _ in 0..8 {
        assert!(matches!(
            joining.join(joiner.operation, [99; 32]).await,
            Err(OnlineMatchError::JoinUnavailable)
        ));
    }
    assert!(matches!(
        joining.join(joiner.operation, [1; 32]).await,
        Err(OnlineMatchError::RateLimited)
    ));
    let attempts: i16 =
        sqlx::query_scalar("SELECT attempts FROM online_match_join_attempts WHERE session_id=$1")
            .bind(Uuid::from_u128(joiner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(attempts, 8);
    sqlx::query("UPDATE online_match_join_attempts SET window_started_at_ms=window_started_at_ms-60000 WHERE session_id=$1").bind(Uuid::from_u128(joiner.snapshot.id().get())).execute(&f.observer).await.unwrap();
    sqlx::query("UPDATE online_match_rooms SET created_at_ms=created_at_ms-600001,code_deadline_ms=code_deadline_ms-600001 WHERE match_id=$1").bind(Uuid::from_u128(MATCH.0)).execute(&f.observer).await.unwrap();
    assert!(matches!(
        joining.join(joiner.operation, [1; 32]).await,
        Err(OnlineMatchError::JoinUnavailable)
    ));
    let activity: i64 =
        sqlx::query_scalar("SELECT last_activity_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(joiner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(
        u64::try_from(activity).unwrap(),
        joiner.snapshot.last_activity_at().get(),
        "failed/limited/expired joins do not extend idle"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_rebind_preserves_seat_and_caps_permanent_admissions() {
    let f = Fixture::new().await;
    let (owner, _, member) = ready(&f).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    let old = store.resolve(owner.operation, MATCH).await.unwrap();
    for record in 600..662 {
        let fresh = issue(&f.first, owner.identity.clone(), record).await;
        let rebind = store.resolve(fresh.operation, MATCH).await.unwrap();
        assert_eq!(rebind.scope().seat, old.scope().seat);
        assert_eq!(rebind.scope().generation, old.scope().generation);
        assert_ne!(rebind.scope().record, old.scope().record);
        assert_eq!(
            canonical_encode(rebind.roster().unwrap()).unwrap(),
            canonical_encode(member.roster().unwrap()).unwrap()
        );
    }
    let fresh = issue(&f.first, owner.identity.clone(), 662).await;
    assert!(matches!(
        store.resolve(fresh.operation, MATCH).await,
        Err(OnlineMatchError::Busy)
    ));
    assert_eq!(
        store.resolve(owner.operation, MATCH).await.unwrap().scope(),
        old.scope(),
        "same original record remains usable at capacity"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_admissions WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(count, 64);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_commit_serializes_with_current_session_revocation() {
    let f = Fixture::new().await;
    let (owner, _, journal) = started(&f).await;
    let reservation = PgOnlineMatchStore::new(f.first.clone())
        .begin_operation(owner.operation, MATCH)
        .await
        .unwrap();
    journal
        .update_ledger_authenticated(
            MATCH,
            StateVersion(0),
            0,
            vec![ScopeState {
                scope: reservation.scope(),
                highest: 0,
                recent: vec![],
            }],
            &reservation,
        )
        .await
        .unwrap();
    let blocker = pid(&f.first).await;
    let waiting = pid(&f.second).await;
    let guard = PgOnlineMatchStore::new(f.first.clone())
        .begin_operation(owner.operation, MATCH)
        .await
        .unwrap();
    assert_eq!(
        guard.with_current(OnlineMembership::scope).unwrap(),
        guard.scope()
    );
    let store = PgSessionStore::new(f.second.clone());
    let credential = owner.operation;
    let revoke = tokio::spawn(async move { store.revoke_credential(credential).await });
    blocked(&f.admin, waiting, blocker).await;
    journal
        .append_authenticated(accepted(guard.membership()), &guard)
        .await
        .unwrap();
    assert!(guard.with_current(|_| ()).is_err());
    tokio::time::timeout(WAIT, revoke)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(
        PgOnlineMatchStore::new(f.first.clone())
            .begin_operation(owner.operation, MATCH)
            .await,
        Err(OnlineMatchError::Session(SessionError::Unauthenticated))
    ));
    let loaded = journal.load(MATCH).await.unwrap();
    assert_eq!(loaded.version, StateVersion(1));
    assert_eq!(loaded.ledger[0].scope, guard.scope());
    assert_eq!(loaded.ledger[0].highest, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_accepted_commit_activity_and_forged_scope_are_atomic() {
    let f = Fixture::new().await;
    let (owner, _, journal) = started(&f).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    journal
        .update_ledger_authenticated(
            MATCH,
            StateVersion(0),
            0,
            vec![ScopeState {
                scope: guard.scope(),
                highest: 0,
                recent: vec![],
            }],
            &guard,
        )
        .await
        .unwrap();
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    let mut forged = accepted(guard.membership());
    forged.operation.as_mut().unwrap().scope.seat = tabula_core::SeatId(1);
    assert_eq!(
        journal.append_authenticated(forged, &guard).await,
        Err(RuntimePortError::Unavailable)
    );
    assert_eq!(journal.load(MATCH).await.unwrap().version, StateVersion(0));
    let before: i64 =
        sqlx::query_scalar("SELECT last_activity_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(owner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    let record = accepted(guard.membership());
    journal.append_authenticated(record, &guard).await.unwrap();
    assert!(guard.with_current(|_| ()).is_err());
    let (activity, idle, absolute):(i64, i64, i64) = sqlx::query_as("SELECT last_activity_at_ms,idle_deadline_ms,absolute_deadline_ms FROM session_auth_sessions WHERE id=$1").bind(Uuid::from_u128(owner.snapshot.id().get())).fetch_one(&f.observer).await.unwrap();
    assert!(activity >= before);
    assert_eq!(idle, (activity + 1_800_000).min(absolute));
    let loaded = journal.load(MATCH).await.unwrap();
    assert_eq!(loaded.version, StateVersion(1));
    assert_eq!(loaded.records.len(), 2);
    assert_eq!(loaded.ledger[0].highest, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_expiry_at_actual_commit_rolls_back_input_and_activity() {
    let f = Fixture::new().await;
    let (owner, _, journal) = started(&f).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    journal
        .update_ledger_authenticated(
            MATCH,
            StateVersion(0),
            0,
            vec![ScopeState {
                scope: guard.scope(),
                highest: 0,
                recent: vec![],
            }],
            &guard,
        )
        .await
        .unwrap();
    // Retire the previous bounded publication exclusion before constructing a
    // valid near-idle-deadline session fixture. No pre-expiry idle extension occurs.
    PgSessionStore::new(f.first.clone())
        .read_session(owner.operation)
        .await
        .unwrap();
    let now: i64 =
        sqlx::query_scalar("SELECT floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint")
            .fetch_one(&f.observer)
            .await
            .unwrap();
    sqlx::query("UPDATE session_auth_sessions SET created_at_ms=$2-1800000,last_activity_at_ms=$2-1798500,last_observed_at_ms=$2,idle_deadline_ms=$2+1500,absolute_deadline_ms=$2-1800000+86400000 WHERE id=$1").bind(Uuid::from_u128(owner.snapshot.id().get())).bind(now).execute(&f.observer).await.unwrap();
    let before: i64 =
        sqlx::query_scalar("SELECT last_activity_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(owner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    let record = accepted(guard.membership());
    let mut pause = journal.pause_before_commit().unwrap();
    let writing = journal.clone();
    let write = tokio::spawn(async move { writing.append_authenticated(record, &guard).await });
    pause.wait_until_entered().await.unwrap();
    // Actual server time, not elapsed client scheduling, proves the deferred deadline crossed.
    sqlx::query("SELECT pg_sleep(1.55)")
        .execute(&f.observer)
        .await
        .unwrap();
    pause.release().unwrap();
    assert_eq!(
        tokio::time::timeout(WAIT, write).await.unwrap().unwrap(),
        Err(RuntimePortError::Indeterminate)
    );
    let loaded = journal.load(MATCH).await.unwrap();
    assert_eq!(loaded.version, StateVersion(0));
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.ledger[0].highest, 0);
    let after: i64 =
        sqlx::query_scalar("SELECT last_activity_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(owner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(after, before);
    let expired: Option<i64> =
        sqlx::query_scalar("SELECT expired_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(owner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert!(expired.is_some(), "authenticated journal re-observation persists terminal expiry before returning rejected COMMIT");
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_offline_journal_cannot_bypass_online_commit_fence() {
    let f = Fixture::new().await;
    let (_, opponent, member) = ready(&f).await;
    let journal = PgMatchStore::new(f.first.clone())
        .claim(MATCH)
        .await
        .unwrap();
    assert_eq!(
        journal.append(genesis(&member)).await,
        Err(RuntimePortError::Indeterminate)
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM match_journal_records WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let guard = PgOnlineMatchStore::new(f.second.clone())
        .begin_operation(opponent.operation, MATCH)
        .await
        .unwrap();
    journal
        .append_authenticated(genesis(&member), &guard)
        .await
        .unwrap();
    let offline = PgMatchStore::new(f.first.clone())
        .claim(MatchId(999))
        .await
        .unwrap();
    let mut record = genesis(&member);
    record.match_id = MatchId(999);
    offline.append(record).await.unwrap();
    assert_eq!(offline.load(MatchId(999)).await.unwrap().records.len(), 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_durable_apply_exclusion_survives_loss_of_both_backends() {
    let f = Fixture::new().await;
    let (owner, _, _) = started(&f).await;
    let guard = PgOnlineMatchStore::new(f.first.clone())
        .begin_operation(owner.operation, MATCH)
        .await
        .unwrap();
    let (publication_pid, transaction_pid) = {
        let state = guard.state.lock().unwrap();
        let pending = state.as_ref().unwrap();
        (pending.publication.backend_pid, pending.backend_pid)
    };
    assert_ne!(
        publication_pid, transaction_pid,
        "apply transaction and close-on-drop exclusion backend are independent"
    );
    let deadline: i64 = sqlx::query_scalar(
        "SELECT publication_lease_until_ms FROM session_accounts WHERE user_id=$1",
    )
    .bind(Uuid::from_u128(owner.snapshot.user_id().0))
    .fetch_one(&f.observer)
    .await
    .unwrap();
    for backend in [publication_pid, transaction_pid] {
        let killed: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
            .bind(backend)
            .fetch_one(&f.observer)
            .await
            .unwrap();
        assert!(killed);
    }
    PgSessionStore::new(f.second.clone())
        .revoke_credential(owner.operation)
        .await
        .unwrap();
    let revoked: i64 =
        sqlx::query_scalar("SELECT revoked_at_ms FROM session_auth_sessions WHERE id=$1")
            .bind(Uuid::from_u128(owner.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert!(
        revoked >= deadline,
        "revocation must honor committed exclusion after every backend lock vanished"
    );
    assert!(
        guard.with_current(|_| ()).is_err(),
        "local apply permit cannot survive the durable revocation boundary"
    );
    drop(guard);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_dataset_lifetime_room_bound_is_atomic() {
    let f = Fixture::new().await;
    for user in 1..=32 {
        let owner = enroll(&f.first, 10_000 + user, 20_000 + user).await;
        for index in 0..4 {
            let id = 1_000 + user * 4 + index;
            let code = *blake3::hash(&id.to_le_bytes()).as_bytes();
            room(&f.first, &owner, MatchId(id), code, 2).await;
        }
    }
    let next = enroll(&f.second, 99_999, 88_888).await;
    assert!(matches!(
        PgOnlineMatchStore::new(f.second.clone())
            .create(
                next.operation,
                MatchId(999_999),
                [255; 32],
                game(),
                version(),
                canonical_encode(&5_u64).unwrap(),
                2
            )
            .await,
        Err(OnlineMatchError::Busy)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_rooms")
        .fetch_one(&f.observer)
        .await
        .unwrap();
    assert_eq!(count, 128);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_rotation_retains_scope_but_epoch_invalidation_denies_apply() {
    let f = Fixture::new().await;
    let (owner, _, _) = ready(&f).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    let original = store.resolve(owner.operation, MATCH).await.unwrap();
    let replacement = SessionCredential::generate().unwrap();
    PgSessionStore::new(f.second.clone())
        .rotate_credential(tabula_session::RotateCredential {
            credential: owner.operation,
            expected_generation: owner.snapshot.credential_generation(),
            replacement_digest: replacement.digest(),
        })
        .await
        .unwrap();
    assert!(matches!(
        store.begin_operation(owner.operation, MATCH).await,
        Err(OnlineMatchError::Session(SessionError::Unauthenticated))
    ));
    let current = CredentialOperation {
        digest: replacement.digest(),
        ..owner.operation
    };
    assert_eq!(
        store.resolve(current, MATCH).await.unwrap().scope(),
        original.scope(),
        "same-record verifier rotation does not reset operation identity"
    );
    PgSessionStore::new(f.second.clone())
        .invalidate_account_epoch(
            owner.snapshot.user_id(),
            owner.snapshot.authorization_epoch(),
        )
        .await
        .unwrap();
    assert!(matches!(
        store.begin_operation(current, MATCH).await,
        Err(OnlineMatchError::Session(SessionError::Unauthenticated))
    ));
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_waiting_room_cannot_activate_after_code_deadline() {
    let f = Fixture::new().await;
    let (owner, _, member) = ready(&f).await;
    let journal = PgMatchStore::new(f.first.clone())
        .claim(MATCH)
        .await
        .unwrap();
    let now: i64 =
        sqlx::query_scalar("SELECT floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint")
            .fetch_one(&f.observer)
            .await
            .unwrap();
    sqlx::query("UPDATE online_match_rooms SET created_at_ms=$2-599000,code_deadline_ms=$2+1000 WHERE match_id=$1").bind(Uuid::from_u128(MATCH.0)).bind(now).execute(&f.observer).await.unwrap();
    let store = PgOnlineMatchStore::new(f.first.clone());
    let guard = store.begin_operation(owner.operation, MATCH).await.unwrap();
    let mut pause = journal.pause_before_commit().unwrap();
    let writing = journal.clone();
    let write =
        tokio::spawn(async move { writing.append_authenticated(genesis(&member), &guard).await });
    pause.wait_until_entered().await.unwrap();
    sqlx::query("SELECT pg_sleep(1.05)")
        .execute(&f.observer)
        .await
        .unwrap();
    pause.release().unwrap();
    assert_eq!(
        tokio::time::timeout(WAIT, write).await.unwrap().unwrap(),
        Err(RuntimePortError::Indeterminate)
    );
    let started: bool =
        sqlx::query_scalar("SELECT started FROM online_match_rooms WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert!(!started);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM match_journal_records WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert!(matches!(
        store.begin_operation(owner.operation, MATCH).await,
        Err(OnlineMatchError::JoinUnavailable)
    ));
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_failed_admission_does_not_reserve_a_seat() {
    let f = Fixture::new().await;
    let owner = enroll(&f.first, 42, 501).await;
    let existing = enroll(&f.second, 43, 502).await;
    room(&f.first, &owner, MATCH, [1; 32], 3).await;
    let store = PgOnlineMatchStore::new(f.first.clone());
    store.join(existing.operation, [1; 32]).await.unwrap();
    for record in 600..662 {
        let fresh = issue(&f.first, owner.identity.clone(), record).await;
        store.resolve(fresh.operation, MATCH).await.unwrap();
    }
    let joining = enroll(&f.second, 44, 503).await;
    assert!(matches!(
        store.join(joining.operation, [1; 32]).await,
        Err(OnlineMatchError::Busy)
    ));
    let seats: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_memberships WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(
        seats, 2,
        "savepoint rolls back a rejected seat while preserving the attempt"
    );
    let attempts: i16 =
        sqlx::query_scalar("SELECT attempts FROM online_match_join_attempts WHERE session_id=$1")
            .bind(Uuid::from_u128(joining.snapshot.id().get()))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert!(matches!(
        store.resolve(joining.operation, MATCH).await,
        Err(OnlineMatchError::JoinUnavailable)
    ));
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_join_resamples_code_expiry_after_actual_room_wait() {
    let f = Fixture::new().await;
    let owner = enroll(&f.first, 42, 501).await;
    let joining = enroll(&f.second, 43, 502).await;
    room(&f.first, &owner, MATCH, [1; 32], 2).await;
    let now: i64 =
        sqlx::query_scalar("SELECT floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint")
            .fetch_one(&f.first)
            .await
            .unwrap();
    sqlx::query("UPDATE online_match_rooms SET created_at_ms=$2-599500,code_deadline_ms=$2+500 WHERE match_id=$1").bind(Uuid::from_u128(MATCH.0)).bind(now).execute(&f.first).await.unwrap();
    let blocking = pid(&f.observer).await;
    let waiting = pid(&f.second).await;
    let mut held = f.observer.begin().await.unwrap();
    sqlx::query("SELECT match_id FROM online_match_rooms WHERE match_id=$1 FOR UPDATE")
        .bind(Uuid::from_u128(MATCH.0))
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let store = PgOnlineMatchStore::new(f.second.clone());
    let operation = joining.operation;
    let join = tokio::spawn(async move { store.join(operation, [1; 32]).await });
    blocked(&f.admin, waiting, blocking).await;
    sqlx::query("SELECT pg_sleep(0.55)")
        .execute(&f.first)
        .await
        .unwrap();
    let expired:bool = sqlx::query_scalar("SELECT floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint >= code_deadline_ms FROM online_match_rooms WHERE match_id=$1").bind(Uuid::from_u128(MATCH.0)).fetch_one(&f.first).await.unwrap();
    assert!(expired);
    held.commit().await.unwrap();
    assert!(matches!(
        tokio::time::timeout(WAIT, join).await.unwrap().unwrap(),
        Err(OnlineMatchError::JoinUnavailable)
    ));
    let seats: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_memberships WHERE match_id=$1")
            .bind(Uuid::from_u128(MATCH.0))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert_eq!(seats, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_online_dataset_lifetime_capacity_retains_completed_and_expired_ids() {
    let f = Fixture::new().await;
    let owner = enroll(&f.first, 77_777, 88_888).await;
    for index in 0_u128..128 {
        let id = MatchId(10_000 + index);
        let code = *blake3::hash(&id.0.to_le_bytes()).as_bytes();
        room(&f.first, &owner, id, code, 2).await;
        // Synthetic lifecycle rows exercise the real admission transaction,
        // not gameplay/completion acceptance or a reusable-owner claim.
        if index < 64 {
            sqlx::query(
                "UPDATE online_match_rooms SET started=true,completed=true WHERE match_id=$1",
            )
            .bind(Uuid::from_u128(id.0))
            .execute(&f.observer)
            .await
            .unwrap();
        } else {
            sqlx::query("UPDATE online_match_rooms SET created_at_ms=created_at_ms-600001,code_deadline_ms=code_deadline_ms-600001 WHERE match_id=$1")
                .bind(Uuid::from_u128(id.0)).execute(&f.observer).await.unwrap();
        }
    }
    let lifecycle: (i64, i64, i64) = sqlx::query_as("SELECT count(*) FILTER (WHERE completed), count(*) FILTER (WHERE NOT started AND code_deadline_ms<=floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint), count(*) FILTER (WHERE NOT completed AND (started OR code_deadline_ms>floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint)) FROM online_match_rooms")
        .fetch_one(&f.observer).await.unwrap();
    assert_eq!(lifecycle, (64, 64, 0), "per-user active capacity is free");
    let counts_sql = "SELECT (SELECT count(*) FROM online_match_rooms), (SELECT count(*) FROM online_match_memberships), (SELECT count(*) FROM online_match_admissions), (SELECT count(*) FROM online_session_commit_guards), (SELECT count(*) FROM online_match_commit_guards)";
    let before_counts: (i64, i64, i64, i64, i64) = sqlx::query_as(counts_sql)
        .fetch_one(&f.observer)
        .await
        .unwrap();
    let times_sql = "SELECT s.last_activity_at_ms,s.last_observed_at_ms,a.last_observed_at_ms FROM session_auth_sessions s JOIN session_accounts a USING(user_id) WHERE s.id=$1";
    let before_times: (i64, i64, i64) = sqlx::query_as(times_sql)
        .bind(Uuid::from_u128(owner.snapshot.id().get()))
        .fetch_one(&f.observer)
        .await
        .unwrap();
    assert!(matches!(
        PgOnlineMatchStore::new(f.second.clone())
            .create(
                owner.operation,
                MatchId(999_999),
                [255; 32],
                game(),
                version(),
                canonical_encode(&5_u64).unwrap(),
                2,
            )
            .await,
        Err(OnlineMatchError::Busy)
    ));
    let after_counts: (i64, i64, i64, i64, i64) = sqlx::query_as(counts_sql)
        .fetch_one(&f.observer)
        .await
        .unwrap();
    let after_times: (i64, i64, i64) = sqlx::query_as(times_sql)
        .bind(Uuid::from_u128(owner.snapshot.id().get()))
        .fetch_one(&f.observer)
        .await
        .unwrap();
    assert_eq!(before_counts.0, 128);
    assert_eq!(
        after_counts, before_counts,
        "129th admission creates no durable rows or witnesses"
    );
    assert_eq!(
        after_times, before_times,
        "capacity rejection commits no authority mutation or activity"
    );
    let absent: bool =
        sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM online_match_rooms WHERE match_id=$1)")
            .bind(Uuid::from_u128(999_999))
            .fetch_one(&f.observer)
            .await
            .unwrap();
    assert!(absent);
    f.close().await;
}
