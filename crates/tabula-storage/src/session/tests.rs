//! Isolated real-`PostgreSQL` acceptance for the ADR-0036 durable authority slice.
//!
//! These tests are deliberately ignored by ordinary workspace runs. CI must run
//! the non-empty ignored selection with `DATABASE_URL` pointing to its ephemeral
//! `PostgreSQL` 16 service. Missing setup fails; there is no in-memory substitute.
//! Socket/private-outbound fencing and HTTP credential release are outside this
//! storage harness and must not be reported as S09/S08 integration acceptance.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgConnection, PgPool, Row};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AuthSessionId, CredentialDigest, CredentialGeneration, IssueSession,
    ProviderIdentityKey, RotateSession, SessionAuthority, SessionChannel, SessionContextId,
    SessionCredential, SessionError, SessionSnapshot,
};
use uuid::Uuid;

use super::{PgSessionStore, ProtectedOperation, TestControls, TestGate};

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(1);
const WAIT_LIMIT: Duration = Duration::from_secs(15);
const START_MS: u64 = 1_000_000_000;

#[derive(Clone, Copy)]
enum SchemaDdl {
    Create,
    Drop,
}

fn is_owned_identifier(identifier: &str, prefix: &str) -> bool {
    identifier.starts_with(prefix)
        && identifier.len() <= 63
        && identifier.len() > prefix.len()
        && identifier
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// The only dynamic DDL identifiers are generated disposable schema names.
/// Validate the full identifier and choose SQL from this closed action enum
/// before asserting `SQLx`'s test-only string safety boundary.
fn schema_ddl(schema: &str, action: SchemaDdl) -> AssertSqlSafe<String> {
    assert!(is_owned_identifier(schema, "tabula_session_acceptance_"));
    AssertSqlSafe(match action {
        SchemaDdl::Create => format!("CREATE SCHEMA {schema}"),
        SchemaDdl::Drop => format!("DROP SCHEMA {schema} CASCADE"),
    })
}

/// Catalog-derived names still pass a closed identifier check before DDL.
/// The target table and statement shape are fixed in this private test helper.
fn drop_session_check(check: &str) -> AssertSqlSafe<String> {
    assert!(is_owned_identifier(check, "session_auth_sessions_"));
    AssertSqlSafe(format!(
        "ALTER TABLE session_auth_sessions DROP CONSTRAINT {check}"
    ))
}

/// Every test owns a fresh schema and two physically independent one-connection
/// pools. The observer/admin connection never supplies session authority.
struct DatabaseFixture {
    admin: PgPool,
    first: PgPool,
    second: PgPool,
    schema: String,
}

impl DatabaseFixture {
    async fn new() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .expect("ignored PostgreSQL acceptance requires DATABASE_URL; setup cannot be skipped");
        let schema = format!(
            "tabula_session_acceptance_{}_{}",
            std::process::id(),
            NEXT_SCHEMA.fetch_add(1, Ordering::SeqCst)
        );
        let admin = tokio::time::timeout(
            WAIT_LIMIT,
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url),
        )
        .await
        .expect("PostgreSQL acceptance admin connection timed out")
        .unwrap_or_else(|_| panic!("PostgreSQL acceptance admin connection failed"));
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .expect("PostgreSQL acceptance could not establish server version");
        assert!(
            version >= 160_000,
            "acceptance requires PostgreSQL 16 or newer"
        );
        sqlx::raw_sql(schema_ddl(&schema, SchemaDdl::Create))
            .execute(&admin)
            .await
            .expect("PostgreSQL acceptance isolated schema creation failed");
        let first = Self::pool(&database_url, &schema).await;
        let second = Self::pool(&database_url, &schema).await;
        let first_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&first)
            .await
            .expect("first acceptance adapter connection must exist");
        let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&second)
            .await
            .expect("second acceptance adapter connection must exist");
        assert_ne!(
            first_pid, second_pid,
            "race controls require independent backends"
        );
        Self {
            admin,
            first,
            second,
            schema,
        }
    }

    async fn pool(database_url: &str, schema: &str) -> PgPool {
        let schema = schema.to_owned();
        tokio::time::timeout(
            WAIT_LIMIT,
            PgPoolOptions::new()
                .max_connections(1)
                .after_connect(move |connection, _metadata| {
                    Box::pin(configure_connection(connection, schema.clone()))
                })
                .connect(database_url),
        )
        .await
        .expect("isolated PostgreSQL acceptance adapter connection timed out")
        .unwrap_or_else(|_| panic!("isolated PostgreSQL acceptance adapter connection failed"))
    }

    async fn close(self) {
        self.first.close().await;
        self.second.close().await;
        sqlx::raw_sql(schema_ddl(&self.schema, SchemaDdl::Drop))
            .execute(&self.admin)
            .await
            .expect("PostgreSQL acceptance isolated schema cleanup failed");
        self.admin.close().await;
    }
}

/// A concrete connection helper keeps `SQLx`'s after-connect higher-ranked borrow
/// explicit. Parameters carry the generated schema value; no identifier supplied
/// by an external caller is interpolated into connection configuration SQL.
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
        .expect("acceptance adapter backend must be reachable")
}

/// Observe the database's lock graph before releasing a held operation. Yielding
/// is scheduling assistance, never the oracle or an elapsed-time ordering rule.
async fn wait_blocked(admin: &PgPool, waiting_pid: i32, blocking_pid: i32) {
    tokio::time::timeout(WAIT_LIMIT, async {
        loop {
            let blocked: bool =
                sqlx::query_scalar("SELECT $2::integer = ANY(pg_blocking_pids($1::integer))")
                    .bind(waiting_pid)
                    .bind(blocking_pid)
                    .fetch_one(admin)
                    .await
                    .expect("acceptance must observe PostgreSQL lock ownership");
            if blocked {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second independent adapter must demonstrably wait on the first lock");
}

async fn finish<T>(task: tokio::task::JoinHandle<T>) -> T {
    tokio::time::timeout(WAIT_LIMIT, task)
        .await
        .expect("acceptance operation timed out")
        .expect("acceptance operation panicked")
}

fn store(pool: &PgPool, now: u64) -> (PgSessionStore, std::sync::Arc<TestControls>) {
    let controls = std::sync::Arc::new(TestControls::new(now));
    (
        PgSessionStore::with_test_controls(pool.clone(), controls.clone()),
        controls,
    )
}

async fn seed_identity(
    pool: &PgPool,
    user_id: u128,
    issuer: &str,
    subject: &str,
) -> ProviderIdentityKey {
    sqlx::query(
        "INSERT INTO session_accounts \
         (user_id, authorization_epoch, enabled, last_observed_at_ms) \
         VALUES ($1, 0, true, 1000000000)",
    )
    .bind(Uuid::from_u128(user_id))
    .execute(pool)
    .await
    .expect("acceptance account fixture must be valid");
    sqlx::query(
        "INSERT INTO session_provider_identities (issuer, subject, user_id) VALUES ($1, $2, $3)",
    )
    .bind(issuer)
    .bind(subject)
    .bind(Uuid::from_u128(user_id))
    .execute(pool)
    .await
    .expect("acceptance issuer+subject fixture must be valid");
    ProviderIdentityKey::new(issuer, subject).expect("acceptance identity key must be valid")
}

async fn issue(
    store: &PgSessionStore,
    identity: &ProviderIdentityKey,
    id: u128,
    channel: SessionChannel,
    digest: CredentialDigest,
) -> SessionSnapshot {
    store
        .issue_session(IssueSession {
            identity: identity.clone(),
            expected_epoch: AccountEpoch::new(0).unwrap(),
            id: AuthSessionId::new(id).unwrap(),
            channel,
            credential_digest: digest,
            context_id: SessionContextId::new(id + 100_000).unwrap(),
        })
        .await
        .expect("advertised valid initial session must be issued")
}

#[derive(PartialEq, Eq)]
struct StoredSession {
    user_id: Uuid,
    epoch: i64,
    generation: i64,
    digest: Vec<u8>,
    context_id: Uuid,
    created: i64,
    activity: i64,
    observed: i64,
    idle: i64,
    absolute: i64,
    revoked: Option<i64>,
    expired: Option<i64>,
}

impl std::fmt::Debug for StoredSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StoredSession([REDACTED])")
    }
}

/// High-variety printable fixtures avoid B-tree prefix compression concealing
/// the maximum exact-key storage boundary. These are test data, never entropy.
fn maximum_component(mut seed: u64) -> String {
    (0..1024)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            char::from(u8::try_from(33 + seed % 94).unwrap())
        })
        .collect()
}

async fn stored(pool: &PgPool, id: u128) -> StoredSession {
    let row = sqlx::query(
        "SELECT user_id, authorization_epoch, credential_generation, credential_digest, \
         context_id, created_at_ms, last_activity_at_ms, last_observed_at_ms, \
         idle_deadline_ms, absolute_deadline_ms, revoked_at_ms, expired_at_ms \
         FROM session_auth_sessions WHERE id = $1",
    )
    .bind(Uuid::from_u128(id))
    .fetch_one(pool)
    .await
    .expect("acceptance must inspect a persisted session row");
    StoredSession {
        user_id: row.get("user_id"),
        epoch: row.get("authorization_epoch"),
        generation: row.get("credential_generation"),
        digest: row.get("credential_digest"),
        context_id: row.get("context_id"),
        created: row.get("created_at_ms"),
        activity: row.get("last_activity_at_ms"),
        observed: row.get("last_observed_at_ms"),
        idle: row.get("idle_deadline_ms"),
        absolute: row.get("absolute_deadline_ms"),
        revoked: row.get("revoked_at_ms"),
        expired: row.get("expired_at_ms"),
    }
}

fn install_gate(controls: &TestControls) -> std::sync::Arc<TestGate> {
    let gate = std::sync::Arc::new(TestGate::new());
    *controls
        .gate
        .lock()
        .expect("acceptance test hook mutex must be intact") = Some(gate.clone());
    gate
}

async fn entered(gate: &TestGate) {
    tokio::time::timeout(WAIT_LIMIT, gate.entered.acquire())
        .await
        .expect("first acceptance operation must reach the locked authority boundary")
        .expect("acceptance gate must remain open")
        .forget();
}

fn assert_initial(row: &StoredSession, id: u128, digest: CredentialDigest) {
    // Literal expectations, deliberately independent of the policy's constants
    // and deadline-calculation helpers.
    assert_eq!(row.user_id, Uuid::from_u128(1));
    assert_eq!(row.epoch, 0);
    assert_eq!(row.generation, 0);
    assert_eq!(row.digest.as_slice(), digest.as_bytes());
    assert_eq!(row.context_id, Uuid::from_u128(id + 100_000));
    assert_eq!(row.created, 1_000_000_000);
    assert_eq!(row.activity, 1_000_000_000);
    assert_eq!(row.idle, 1_001_800_000);
    assert_eq!(row.absolute, 1_086_400_000);
    assert_eq!(row.revoked, None);
    assert_eq!(row.expired, None);
}

async fn marker_count(pool: &PgPool, marker: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM tabula_session_acceptance_markers WHERE marker = $1")
        .bind(marker)
        .fetch_one(pool)
        .await
        .expect("acceptance must inspect committed protected markers")
}

async fn marker_table(pool: &PgPool) {
    sqlx::raw_sql("CREATE TABLE tabula_session_acceptance_markers (marker TEXT PRIMARY KEY)")
        .execute(pool)
        .await
        .expect("test-only protected marker table must be created");
}

async fn assert_identity_key_boundaries(pool: &PgPool, store: &PgSessionStore) {
    let maximum_issuer = maximum_component(0x1456_789a_bcde_f013);
    let maximum_subject = maximum_component(0xa51c_e72d_9860_431f);
    assert_ne!(maximum_issuer, maximum_subject);
    assert_eq!(maximum_issuer.len(), 1024);
    assert_eq!(maximum_subject.len(), 1024);
    let maximum_key = seed_identity(pool, 4, &maximum_issuer, &maximum_subject).await;
    assert_eq!(
        ProviderIdentityKey::new(format!("{maximum_issuer}x"), &maximum_subject),
        Err(SessionError::InvalidInput)
    );
    assert_eq!(
        ProviderIdentityKey::new(&maximum_issuer, format!("{maximum_subject}x")),
        Err(SessionError::InvalidInput)
    );
    assert_eq!(
        store.account_snapshot(maximum_key).await.unwrap().user_id(),
        UserId(4)
    );
    let composed = seed_identity(pool, 5, "unicode-issuer", "é").await;
    let decomposed = seed_identity(pool, 6, "unicode-issuer", "e\u{301}").await;
    assert_eq!(
        store.account_snapshot(composed).await.unwrap().user_id(),
        UserId(5)
    );
    assert_eq!(
        store.account_snapshot(decomposed).await.unwrap().user_id(),
        UserId(6)
    );
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_migrations_repeat_and_provider_linkage_is_exact_pair() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first
        .migrate()
        .await
        .expect("additive session migrations must apply");
    let identity = seed_identity(&db.first, 1, "https://issuer.example/", "same@example.com").await;
    let other_issuer =
        seed_identity(&db.first, 2, "https://other.example/", "same@example.com").await;
    let other_subject =
        seed_identity(&db.first, 3, "https://issuer.example/", "other-subject").await;
    first
        .migrate()
        .await
        .expect("repeated migration application must succeed");
    let (second, _) = store(&db.second, START_MS);
    assert_eq!(
        second
            .account_snapshot(identity.clone())
            .await
            .unwrap()
            .user_id(),
        UserId(1)
    );
    assert_eq!(
        second
            .account_snapshot(other_issuer)
            .await
            .unwrap()
            .user_id(),
        UserId(2)
    );
    assert_eq!(
        second
            .account_snapshot(other_subject)
            .await
            .unwrap()
            .user_id(),
        UserId(3)
    );
    for key in [
        ProviderIdentityKey::new("https://issuer.example", "same@example.com").unwrap(),
        ProviderIdentityKey::new("https://ISSUER.example/", "same@example.com").unwrap(),
        ProviderIdentityKey::new("https://issuer.example/", "SAME@example.com").unwrap(),
    ] {
        assert_eq!(
            second.account_snapshot(key).await,
            Err(SessionError::Unauthenticated)
        );
    }
    assert_identity_key_boundaries(&db.first, &second).await;
    let duplicate = sqlx::query(
        "INSERT INTO session_provider_identities (issuer, subject, user_id) VALUES ($1, $2, $3)",
    )
    .bind(identity.issuer())
    .bind(identity.subject())
    .bind(Uuid::from_u128(2))
    .execute(&db.first)
    .await
    .expect_err("the database must reject duplicate exact issuer+subject linkage");
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    let identities: i64 = sqlx::query_scalar("SELECT count(*) FROM session_provider_identities")
        .fetch_one(&db.first)
        .await
        .unwrap();
    assert_eq!(identities, 6);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_contains_only_digests_and_debug_boundaries_are_redacted() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let credential = SessionCredential::generate().unwrap();
    let other_credential = SessionCredential::generate().unwrap();
    assert_ne!(
        credential.expose_encoded(),
        other_credential.expose_encoded()
    );
    let snapshot = issue(
        &first,
        &identity,
        10,
        SessionChannel::BrowserCookie,
        credential.digest(),
    )
    .await;
    issue(
        &first,
        &identity,
        11,
        SessionChannel::NativeBearer,
        other_credential.digest(),
    )
    .await;
    assert_initial(&stored(&db.first, 10).await, 10, credential.digest());
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(s)::text FROM session_auth_sessions s")
            .fetch_all(&db.first)
            .await
            .unwrap();
    for row in rows {
        assert!(!row.contains(&credential.expose_encoded()));
        assert!(!row.contains(&other_credential.expose_encoded()));
    }
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = current_schema() ORDER BY table_name, ordinal_position",
    )
    .fetch_all(&db.first)
    .await
    .unwrap();
    for forbidden in [
        "email",
        "password",
        "password_hash",
        "credential",
        "bearer",
        "refresh_token",
        "provider_access_token",
    ] {
        assert!(!columns.iter().any(|column| column == forbidden));
    }
    assert_eq!(format!("{credential:?}"), "SessionCredential([REDACTED])");
    assert_eq!(
        format!("{:?}", credential.digest()),
        "CredentialDigest([REDACTED])"
    );
    assert_eq!(format!("{snapshot:?}"), "SessionSnapshot([REDACTED])");
    assert_eq!(
        format!("{:?}", snapshot.binding()),
        "SessionBinding([REDACTED])"
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_rejects_wrong_channel_unknown_digest_and_public_record_ids() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let browser = SessionCredential::generate().unwrap();
    let native = SessionCredential::generate().unwrap();
    issue(
        &first,
        &identity,
        10,
        SessionChannel::BrowserCookie,
        browser.digest(),
    )
    .await;
    issue(
        &first,
        &identity,
        11,
        SessionChannel::NativeBearer,
        native.digest(),
    )
    .await;
    let (second, _) = store(&db.second, START_MS);
    assert!(second
        .observe_credential(browser.digest(), SessionChannel::BrowserCookie)
        .await
        .is_ok());
    assert!(second
        .observe_credential(native.digest(), SessionChannel::NativeBearer)
        .await
        .is_ok());
    assert_eq!(
        second
            .observe_credential(browser.digest(), SessionChannel::NativeBearer)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        second
            .observe_credential(native.digest(), SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        second
            .observe_credential(
                CredentialDigest::from_bytes([0x77; 32]),
                SessionChannel::BrowserCookie
            )
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        SessionCredential::parse(&Uuid::from_u128(10).to_string()),
        Err(SessionError::InvalidInput)
    );
    // A correctly encoded arbitrary 32-byte value still establishes no authority.
    let public_like =
        SessionCredential::parse("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    assert_eq!(
        second
            .observe_credential(public_like.digest(), SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_initial(&stored(&db.first, 10).await, 10, browser.digest());
    assert_initial(&stored(&db.first, 11).await, 11, native.digest());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_checked_rows_reject_corruption_instead_of_restoring_authority() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let (second, _) = store(&db.second, START_MS);
    let mut bindings = Vec::new();
    for id in 10..18 {
        bindings.push(
            issue(
                &first,
                &identity,
                id,
                SessionChannel::BrowserCookie,
                SessionCredential::generate().unwrap().digest(),
            )
            .await
            .binding(),
        );
    }
    // SQL CHECKs are independently useful, but legacy/corrupt rows must also pass
    // checked domain decoding. Disable only this disposable schema's CHECKs.
    let checks: Vec<String> = sqlx::query_scalar(
        "SELECT conname::text FROM pg_constraint WHERE conrelid = 'session_auth_sessions'::regclass AND contype = 'c'",
    ).fetch_all(&db.first).await.unwrap();
    assert!(!checks.is_empty());
    for check in checks {
        sqlx::raw_sql(drop_session_check(&check))
            .execute(&db.first)
            .await
            .unwrap();
    }
    let corruptions: [&'static str; 8] = [
        "UPDATE session_auth_sessions SET credential_digest = decode('00', 'hex') WHERE id = $1",
        "UPDATE session_auth_sessions SET credential_generation = -1 WHERE id = $1",
        "UPDATE session_auth_sessions SET channel = 'unknown-channel' WHERE id = $1",
        "UPDATE session_auth_sessions SET context_id = '00000000-0000-0000-0000-000000000000'::uuid WHERE id = $1",
        "UPDATE session_auth_sessions SET last_activity_at_ms = 999999999 WHERE id = $1",
        "UPDATE session_auth_sessions SET last_observed_at_ms = 999999999 WHERE id = $1",
        "UPDATE session_auth_sessions SET idle_deadline_ms = 1086400001 WHERE id = $1",
        "UPDATE session_auth_sessions SET absolute_deadline_ms = 1086400001 WHERE id = $1",
    ];
    for (binding, corruption) in bindings.into_iter().zip(corruptions) {
        sqlx::query(corruption)
            .bind(Uuid::from_u128(binding.id().get()))
            .execute(&db.first)
            .await
            .unwrap();
        assert_eq!(
            second.observe_binding(binding).await,
            Err(SessionError::Unavailable)
        );
    }
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_idle_and_absolute_boundaries_use_literal_equality_oracle() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let mut cases = Vec::new();
    for (id, now, valid, absolute_case) in [
        (10, 1_001_799_999, true, false),
        (11, 1_001_800_000, false, false),
        (12, 1_001_800_001, false, false),
        (13, 1_086_399_999, true, true),
        (14, 1_086_400_000, false, true),
        (15, 1_086_400_001, false, true),
    ] {
        let digest = SessionCredential::generate().unwrap().digest();
        issue(&first, &identity, id, SessionChannel::BrowserCookie, digest).await;
        if absolute_case {
            sqlx::query(
                "UPDATE session_auth_sessions SET last_activity_at_ms = 1084600000, \
                 last_observed_at_ms = 1084600000, idle_deadline_ms = 1086400000 WHERE id = $1",
            )
            .bind(Uuid::from_u128(id))
            .execute(&db.first)
            .await
            .unwrap();
        }
        cases.push((id, digest, now, valid, absolute_case));
    }
    for (id, digest, now, valid, absolute_case) in cases {
        controls.clock.store(now, Ordering::SeqCst);
        let result = first
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await;
        if valid {
            let snapshot = result.expect("literal just-before-deadline control must remain valid");
            assert_eq!(snapshot.absolute_deadline().get(), 1_086_400_000);
            assert_eq!(
                snapshot.idle_deadline().get(),
                if absolute_case {
                    1_086_400_000
                } else {
                    1_001_800_000
                }
            );
        } else {
            assert_eq!(result, Err(SessionError::Unauthenticated));
        }
        let row = stored(&db.first, id).await;
        assert_eq!(
            row.expired,
            if valid {
                None
            } else {
                Some(i64::try_from(now).unwrap())
            }
        );
        assert_eq!(row.absolute, 1_086_400_000);
        assert_eq!(
            row.activity,
            if absolute_case {
                1_084_600_000
            } else {
                1_000_000_000
            }
        );
    }
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_only_committed_new_protected_writes_extend_idle_bounded_by_absolute() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    marker_table(&db.first).await;
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let old = SessionCredential::generate().unwrap().digest();
    let replacement = SessionCredential::generate().unwrap().digest();
    issue(&first, &identity, 10, SessionChannel::BrowserCookie, old).await;
    let bounded = SessionCredential::generate().unwrap().digest();
    issue(
        &first,
        &identity,
        11,
        SessionChannel::BrowserCookie,
        bounded,
    )
    .await;
    for (now, operation, marker) in [
        (1_000_100_000, ProtectedOperation::Read, "read"),
        (1_000_200_000, ProtectedOperation::Rejected, "rejected"),
        (1_000_300_000, ProtectedOperation::Duplicate, "duplicate"),
    ] {
        controls.clock.store(now, Ordering::SeqCst);
        first
            .protected_marker(old, SessionChannel::BrowserCookie, marker, operation)
            .await
            .unwrap();
        assert_initial(&stored(&db.first, 10).await, 10, old);
        assert_eq!(marker_count(&db.first, marker).await, 0);
    }
    controls.clock.store(1_000_400_000, Ordering::SeqCst);
    first
        .rotate_session(RotateSession {
            current_digest: old,
            channel: SessionChannel::BrowserCookie,
            expected_generation: CredentialGeneration::new(0).unwrap(),
            replacement_digest: replacement,
        })
        .await
        .unwrap();
    let rotated = stored(&db.first, 10).await;
    assert_eq!(rotated.activity, 1_000_000_000);
    assert_eq!(rotated.idle, 1_001_800_000);
    assert_eq!(rotated.absolute, 1_086_400_000);
    controls.clock.store(1_000_500_000, Ordering::SeqCst);
    first
        .protected_marker(
            replacement,
            SessionChannel::BrowserCookie,
            "accepted",
            ProtectedOperation::Accepted,
        )
        .await
        .unwrap();
    let accepted = stored(&db.first, 10).await;
    assert_eq!(accepted.activity, 1_000_500_000);
    assert_eq!(accepted.idle, 1_002_300_000);
    assert_eq!(accepted.absolute, 1_086_400_000);
    assert_eq!(marker_count(&db.first, "accepted").await, 1);
    controls.clock.store(1_000_600_000, Ordering::SeqCst);
    first
        .protected_marker(
            replacement,
            SessionChannel::BrowserCookie,
            "accepted",
            ProtectedOperation::Accepted,
        )
        .await
        .unwrap();
    let duplicate = stored(&db.first, 10).await;
    assert_eq!(duplicate.activity, 1_000_500_000);
    assert_eq!(duplicate.idle, 1_002_300_000);
    assert_eq!(marker_count(&db.first, "accepted").await, 1);
    sqlx::query(
        "UPDATE session_auth_sessions SET last_activity_at_ms = 1084600000, \
         last_observed_at_ms = 1084600000, idle_deadline_ms = 1086400000 WHERE id = $1",
    )
    .bind(Uuid::from_u128(11))
    .execute(&db.first)
    .await
    .unwrap();
    controls.clock.store(1_085_500_000, Ordering::SeqCst);
    first
        .protected_marker(
            bounded,
            SessionChannel::BrowserCookie,
            "bounded",
            ProtectedOperation::Accepted,
        )
        .await
        .unwrap();
    let bounded_row = stored(&db.first, 11).await;
    assert_eq!(bounded_row.activity, 1_085_500_000);
    assert_eq!(bounded_row.idle, 1_086_400_000);
    assert_eq!(bounded_row.absolute, 1_086_400_000);
    assert_eq!(marker_count(&db.first, "bounded").await, 1);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_terminal_expiry_and_clock_high_water_survive_a_new_adapter() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let digest = SessionCredential::generate().unwrap().digest();
    let issued = issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest).await;
    controls.clock.store(1_001_800_000, Ordering::SeqCst);
    assert_eq!(
        first
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    let expired = stored(&db.first, 10).await;
    assert_eq!(expired.expired, Some(1_001_800_000));
    assert_eq!(expired.observed, 1_001_800_000);
    let (restarted, restarted_controls) = store(&db.second, 1_001_000_000);
    assert_eq!(
        restarted.observe_binding(issued.binding()).await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(stored(&db.first, 10).await, expired);
    restarted_controls
        .clock
        .store(1_001_800_001, Ordering::SeqCst);
    assert_eq!(
        restarted
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(stored(&db.first, 10).await.expired, Some(1_001_800_000));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_waiting_on_authority_lock_resamples_time_after_expiry() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let digest = SessionCredential::generate().unwrap().digest();
    issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest).await;
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    let mut held = db.first.begin().await.unwrap();
    sqlx::query("SELECT user_id FROM session_accounts WHERE user_id = $1 FOR UPDATE")
        .bind(Uuid::from_u128(1))
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let (second, controls) = store(&db.second, 1_001_799_999);
    let waiting = tokio::spawn(async move {
        second
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await
    });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    controls.clock.store(1_001_800_000, Ordering::SeqCst);
    held.commit().await.unwrap();
    assert_eq!(finish(waiting).await, Err(SessionError::Unauthenticated));
    let row = stored(&db.first, 10).await;
    assert_eq!(row.expired, Some(1_001_800_000));
    assert_eq!(row.observed, 1_001_800_000);
    assert_eq!(row.activity, 1_000_000_000);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_rotation_has_one_winner_and_immediately_invalidates_old_verifier() {
    let db = DatabaseFixture::new().await;
    let (first, first_controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let old = SessionCredential::generate().unwrap().digest();
    let replacement_a = SessionCredential::generate().unwrap().digest();
    let replacement_b = SessionCredential::generate().unwrap().digest();
    let initial = issue(&first, &identity, 10, SessionChannel::BrowserCookie, old).await;
    first_controls.clock.store(1_000_100_000, Ordering::SeqCst);
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    let gate = install_gate(&first_controls);
    let first_request = RotateSession {
        current_digest: old,
        channel: SessionChannel::BrowserCookie,
        expected_generation: CredentialGeneration::new(0).unwrap(),
        replacement_digest: replacement_a,
    };
    let first_runner = first.clone();
    let winner = tokio::spawn(async move { first_runner.rotate_session(first_request).await });
    entered(&gate).await;
    let (second, _) = store(&db.second, 1_000_100_000);
    let second_runner = second.clone();
    let loser = tokio::spawn(async move {
        second_runner
            .rotate_session(RotateSession {
                current_digest: old,
                channel: SessionChannel::BrowserCookie,
                expected_generation: CredentialGeneration::new(0).unwrap(),
                replacement_digest: replacement_b,
            })
            .await
    });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    gate.release.add_permits(1);
    let rotated = finish(winner)
        .await
        .expect("first lock owner must win rotation");
    assert_eq!(finish(loser).await, Err(SessionError::Unauthenticated));
    assert_eq!(rotated.credential_generation().get(), 1);
    assert_eq!(rotated.id(), initial.id());
    assert_eq!(rotated.user_id(), initial.user_id());
    assert_eq!(rotated.context_id(), initial.context_id());
    assert_eq!(rotated.binding(), initial.binding());
    assert_eq!(rotated.last_activity_at().get(), 1_000_000_000);
    assert_eq!(rotated.idle_deadline().get(), 1_001_800_000);
    assert_eq!(rotated.absolute_deadline().get(), 1_086_400_000);
    assert_eq!(
        second
            .observe_credential(old, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert!(second
        .observe_credential(replacement_a, SessionChannel::BrowserCookie)
        .await
        .is_ok());
    assert_eq!(
        second
            .observe_credential(replacement_b, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert!(second.observe_binding(initial.binding()).await.is_ok());
    let row = stored(&db.first, 10).await;
    assert_eq!(row.generation, 1);
    assert_eq!(row.digest.as_slice(), replacement_a.as_bytes());
    assert_eq!(row.idle, 1_001_800_000);
    assert_eq!(row.absolute, 1_086_400_000);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_refresh_and_logout_in_both_lock_orders_never_resurrect_a_record() {
    for refresh_first in [true, false] {
        let db = DatabaseFixture::new().await;
        let (first, first_controls) = store(&db.first, START_MS);
        first.migrate().await.unwrap();
        let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
        let old = SessionCredential::generate().unwrap().digest();
        let replacement = SessionCredential::generate().unwrap().digest();
        let binding = issue(&first, &identity, 10, SessionChannel::BrowserCookie, old)
            .await
            .binding();
        let request = RotateSession {
            current_digest: old,
            channel: SessionChannel::BrowserCookie,
            expected_generation: CredentialGeneration::new(0).unwrap(),
            replacement_digest: replacement,
        };
        first_controls.clock.store(1_000_100_000, Ordering::SeqCst);
        let first_pid = backend_pid(&db.first).await;
        let second_pid = backend_pid(&db.second).await;
        let gate = install_gate(&first_controls);
        let first_runner = first.clone();
        let first_request = request.clone();
        let first_task = tokio::spawn(async move {
            if refresh_first {
                first_runner
                    .rotate_session(first_request)
                    .await
                    .map(|snapshot| snapshot.credential_generation().get())
            } else {
                first_runner.revoke_session(binding).await.map(|()| 0)
            }
        });
        entered(&gate).await;
        let (second, _) = store(&db.second, 1_000_100_000);
        let second_runner = second.clone();
        let second_task = tokio::spawn(async move {
            if refresh_first {
                second_runner.revoke_session(binding).await.map(|()| 0)
            } else {
                second_runner
                    .rotate_session(request)
                    .await
                    .map(|snapshot| snapshot.credential_generation().get())
            }
        });
        wait_blocked(&db.admin, second_pid, first_pid).await;
        gate.release.add_permits(1);
        assert_eq!(finish(first_task).await, Ok(u64::from(refresh_first)));
        assert_eq!(
            finish(second_task).await,
            if refresh_first {
                Ok(0)
            } else {
                Err(SessionError::Unauthenticated)
            }
        );
        let row = stored(&db.first, 10).await;
        assert_eq!(row.revoked, Some(1_000_100_000));
        assert_eq!(row.generation, i64::from(refresh_first));
        assert_eq!(row.activity, 1_000_000_000);
        assert_eq!(row.idle, 1_001_800_000);
        assert_eq!(row.absolute, 1_086_400_000);
        assert_eq!(
            second
                .observe_credential(old, SessionChannel::BrowserCookie)
                .await,
            Err(SessionError::Unauthenticated)
        );
        assert_eq!(
            second
                .observe_credential(replacement, SessionChannel::BrowserCookie)
                .await,
            Err(SessionError::Unauthenticated)
        );
        assert_eq!(
            second.observe_binding(binding).await,
            Err(SessionError::Unauthenticated)
        );
        db.close().await;
    }
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_logout_is_idempotent_for_one_device_and_epoch_fences_all_devices() {
    let db = DatabaseFixture::new().await;
    let (first, first_controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let browser = SessionCredential::generate().unwrap().digest();
    let native = SessionCredential::generate().unwrap().digest();
    let first_device = issue(
        &first,
        &identity,
        10,
        SessionChannel::BrowserCookie,
        browser,
    )
    .await;
    let other_device = issue(&first, &identity, 11, SessionChannel::NativeBearer, native).await;
    first_controls.clock.store(1_000_100_000, Ordering::SeqCst);
    first.revoke_session(first_device.binding()).await.unwrap();
    first_controls.clock.store(1_000_200_000, Ordering::SeqCst);
    first.revoke_session(first_device.binding()).await.unwrap();
    assert_eq!(stored(&db.first, 10).await.revoked, Some(1_000_100_000));
    let (second, second_controls) = store(&db.second, 1_000_200_000);
    assert_eq!(
        second.observe_binding(first_device.binding()).await,
        Err(SessionError::Unauthenticated)
    );
    assert!(second.observe_binding(other_device.binding()).await.is_ok());
    second_controls.clock.store(1_000_300_000, Ordering::SeqCst);
    let changed = second
        .invalidate_account_epoch(UserId(1), AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    assert_eq!(changed.authorization_epoch().get(), 1);
    assert_eq!(
        second.observe_binding(first_device.binding()).await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        second.observe_binding(other_device.binding()).await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        second
            .observe_credential(native, SessionChannel::NativeBearer)
            .await,
        Err(SessionError::Unauthenticated)
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_epoch_commit_fences_stale_inflight_issuance() {
    let db = DatabaseFixture::new().await;
    let (first, first_controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let stale_epoch = first
        .account_snapshot(identity.clone())
        .await
        .unwrap()
        .authorization_epoch();
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    first_controls.clock.store(1_000_100_000, Ordering::SeqCst);
    let gate = install_gate(&first_controls);
    let first_runner = first.clone();
    let epoch_task = tokio::spawn(async move {
        first_runner
            .invalidate_account_epoch(UserId(1), stale_epoch)
            .await
    });
    entered(&gate).await;
    let (second, _) = store(&db.second, 1_000_100_000);
    let second_runner = second.clone();
    let stale_digest = SessionCredential::generate().unwrap().digest();
    let stale_issue = tokio::spawn(async move {
        second_runner
            .issue_session(IssueSession {
                identity,
                expected_epoch: stale_epoch,
                id: AuthSessionId::new(10).unwrap(),
                channel: SessionChannel::BrowserCookie,
                credential_digest: stale_digest,
                context_id: SessionContextId::new(100_010).unwrap(),
            })
            .await
    });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    gate.release.add_permits(1);
    assert_eq!(
        finish(epoch_task)
            .await
            .unwrap()
            .authorization_epoch()
            .get(),
        1
    );
    assert_eq!(
        finish(stale_issue).await,
        Err(SessionError::Unauthenticated)
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM session_auth_sessions")
        .fetch_one(&db.first)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(
        second
            .observe_credential(stale_digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_rejected_epoch_cas_persists_clock_floor_for_independent_adapters() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let digest = SessionCredential::generate().unwrap().digest();
    issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest).await;
    controls.clock.store(1_000_200_000, Ordering::SeqCst);
    assert_eq!(
        first
            .invalidate_account_epoch(UserId(1), AccountEpoch::new(1).unwrap())
            .await,
        Err(SessionError::Conflict)
    );
    let account = first.account_snapshot(identity).await.unwrap();
    assert_eq!(account.authorization_epoch().get(), 0);
    assert_eq!(account.last_observed_at().get(), 1_000_200_000);
    let (second, _) = store(&db.second, 1_000_100_000);
    assert_eq!(
        second
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unavailable)
    );
    let row = stored(&db.first, 10).await;
    assert_eq!(row.activity, 1_000_000_000);
    assert_eq!(row.observed, 1_000_000_000);
    assert_eq!(row.revoked, None);
    assert_eq!(row.expired, None);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
#[allow(clippy::too_many_lines)]
async fn postgres_protected_writes_and_logout_or_epoch_share_both_commit_orders() {
    for invalidate_epoch in [false, true] {
        for write_first in [false, true] {
            let db = DatabaseFixture::new().await;
            let (first, first_controls) = store(&db.first, START_MS);
            first.migrate().await.unwrap();
            marker_table(&db.first).await;
            let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
            let digest = SessionCredential::generate().unwrap().digest();
            let binding = issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest)
                .await
                .binding();
            first_controls.clock.store(1_000_100_000, Ordering::SeqCst);
            let first_pid = backend_pid(&db.first).await;
            let second_pid = backend_pid(&db.second).await;
            let gate = install_gate(&first_controls);
            let first_runner = first.clone();
            let first_task = tokio::spawn(async move {
                if write_first {
                    first_runner
                        .protected_marker(
                            digest,
                            SessionChannel::BrowserCookie,
                            "ordered",
                            ProtectedOperation::Accepted,
                        )
                        .await
                        .map(|snapshot| snapshot.last_activity_at().get())
                } else if invalidate_epoch {
                    first_runner
                        .invalidate_account_epoch(UserId(1), AccountEpoch::new(0).unwrap())
                        .await
                        .map(|account| account.authorization_epoch().get())
                } else {
                    first_runner.revoke_session(binding).await.map(|()| 0)
                }
            });
            entered(&gate).await;
            let (second, _) = store(&db.second, 1_000_100_000);
            let second_runner = second.clone();
            let second_task = tokio::spawn(async move {
                if !write_first {
                    second_runner
                        .protected_marker(
                            digest,
                            SessionChannel::BrowserCookie,
                            "ordered",
                            ProtectedOperation::Accepted,
                        )
                        .await
                        .map(|snapshot| snapshot.last_activity_at().get())
                } else if invalidate_epoch {
                    second_runner
                        .invalidate_account_epoch(UserId(1), AccountEpoch::new(0).unwrap())
                        .await
                        .map(|account| account.authorization_epoch().get())
                } else {
                    second_runner.revoke_session(binding).await.map(|()| 0)
                }
            });
            wait_blocked(&db.admin, second_pid, first_pid).await;
            gate.release.add_permits(1);
            assert_eq!(
                finish(first_task).await,
                Ok(if write_first {
                    1_000_100_000
                } else {
                    u64::from(invalidate_epoch)
                })
            );
            assert_eq!(
                finish(second_task).await,
                if write_first {
                    Ok(u64::from(invalidate_epoch))
                } else {
                    Err(SessionError::Unauthenticated)
                }
            );
            assert_eq!(
                marker_count(&db.first, "ordered").await,
                i64::from(write_first)
            );
            let row = stored(&db.first, 10).await;
            assert_eq!(
                row.activity,
                if write_first {
                    1_000_100_000
                } else {
                    1_000_000_000
                }
            );
            assert_eq!(
                row.idle,
                if write_first {
                    1_001_900_000
                } else {
                    1_001_800_000
                }
            );
            assert_eq!(row.absolute, 1_086_400_000);
            assert_eq!(
                row.revoked,
                if invalidate_epoch {
                    None
                } else {
                    Some(1_000_100_000)
                }
            );
            assert_eq!(
                second.observe_binding(binding).await,
                Err(SessionError::Unauthenticated)
            );
            if invalidate_epoch {
                assert_eq!(
                    second
                        .account_snapshot(identity)
                        .await
                        .unwrap()
                        .authorization_epoch()
                        .get(),
                    1
                );
            }
            db.close().await;
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
#[allow(clippy::too_many_lines)]
async fn postgres_rollback_and_uncertain_commit_never_report_known_success() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    marker_table(&db.first).await;
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let old = SessionCredential::generate().unwrap().digest();
    let rolled_back_digest = SessionCredential::generate().unwrap().digest();
    let uncertain_digest = SessionCredential::generate().unwrap().digest();
    let binding = issue(&first, &identity, 10, SessionChannel::BrowserCookie, old)
        .await
        .binding();
    let before = stored(&db.first, 10).await;
    controls.clock.store(1_000_100_000, Ordering::SeqCst);
    controls.fault.store(1, Ordering::SeqCst);
    assert_eq!(
        first
            .protected_marker(
                old,
                SessionChannel::BrowserCookie,
                "rolled-back",
                ProtectedOperation::Accepted
            )
            .await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(marker_count(&db.first, "rolled-back").await, 0);
    assert_eq!(stored(&db.first, 10).await, before);
    assert_eq!(
        first
            .account_snapshot(identity.clone())
            .await
            .unwrap()
            .last_observed_at()
            .get(),
        1_000_000_000
    );
    controls.fault.store(1, Ordering::SeqCst);
    assert_eq!(
        first
            .rotate_session(RotateSession {
                current_digest: old,
                channel: SessionChannel::BrowserCookie,
                expected_generation: CredentialGeneration::new(0).unwrap(),
                replacement_digest: rolled_back_digest,
            })
            .await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(stored(&db.first, 10).await, before);
    let (second, second_controls) = store(&db.second, 1_000_100_000);
    assert!(second
        .observe_credential(old, SessionChannel::BrowserCookie)
        .await
        .is_ok());
    assert_eq!(
        second
            .observe_credential(rolled_back_digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    controls.fault.store(2, Ordering::SeqCst);
    assert_eq!(
        first
            .rotate_session(RotateSession {
                current_digest: old,
                channel: SessionChannel::BrowserCookie,
                expected_generation: CredentialGeneration::new(0).unwrap(),
                replacement_digest: uncertain_digest,
            })
            .await,
        Err(SessionError::Unavailable)
    );
    // The actual COMMIT succeeded while its acknowledgement was lost. The port
    // emitted no successful snapshot; HTTP bearer/Set-Cookie release is gated.
    let committed = stored(&db.first, 10).await;
    assert_eq!(committed.generation, 1);
    assert_eq!(committed.digest.as_slice(), uncertain_digest.as_bytes());
    assert_eq!(
        second
            .observe_credential(old, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert!(second
        .observe_credential(uncertain_digest, SessionChannel::BrowserCookie)
        .await
        .is_ok());
    controls.clock.store(1_000_200_000, Ordering::SeqCst);
    controls.fault.store(2, Ordering::SeqCst);
    assert_eq!(
        first
            .protected_marker(
                uncertain_digest,
                SessionChannel::BrowserCookie,
                "uncertain-write",
                ProtectedOperation::Accepted
            )
            .await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(marker_count(&db.first, "uncertain-write").await, 1);
    let uncertain_write = stored(&db.first, 10).await;
    assert_eq!(uncertain_write.activity, 1_000_200_000);
    assert_eq!(uncertain_write.idle, 1_002_000_000);
    controls.fault.store(1, Ordering::SeqCst);
    assert_eq!(
        first.revoke_session(binding).await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(stored(&db.first, 10).await.revoked, None);
    controls.fault.store(2, Ordering::SeqCst);
    assert_eq!(
        first.revoke_session(binding).await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(stored(&db.first, 10).await.revoked, Some(1_000_200_000));
    second_controls.clock.store(1_000_200_000, Ordering::SeqCst);
    assert_eq!(
        second.observe_binding(binding).await,
        Err(SessionError::Unauthenticated)
    );
    controls.fault.store(2, Ordering::SeqCst);
    assert_eq!(
        first
            .issue_session(IssueSession {
                identity,
                expected_epoch: AccountEpoch::new(0).unwrap(),
                id: AuthSessionId::new(11).unwrap(),
                channel: SessionChannel::NativeBearer,
                credential_digest: SessionCredential::generate().unwrap().digest(),
                context_id: SessionContextId::new(100_011).unwrap(),
            })
            .await,
        Err(SessionError::Unavailable)
    );
    assert_eq!(stored(&db.first, 11).await.created, 1_000_200_000);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_fixture_provisioning_is_idempotent_and_cannot_reassign_an_identity() {
    let db = DatabaseFixture::new().await;
    let (first, _) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    let identity = ProviderIdentityKey::new("issuer", "subject").unwrap();
    let account = tabula_session::AccountRecord::new(
        UserId(1),
        AccountEpoch::new(0).unwrap(),
        true,
        tabula_session::UnixMillis::new(START_MS).unwrap(),
    )
    .unwrap();
    first
        .provision_fixture_identity(identity.clone(), account.clone())
        .await
        .unwrap();
    first
        .provision_fixture_identity(identity.clone(), account.clone())
        .await
        .unwrap();
    let (second, _) = store(&db.second, START_MS);
    assert_eq!(
        second
            .account_snapshot(identity.clone())
            .await
            .unwrap()
            .user_id(),
        UserId(1)
    );
    let other = tabula_session::AccountRecord::new(
        UserId(2),
        AccountEpoch::new(0).unwrap(),
        true,
        tabula_session::UnixMillis::new(START_MS).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first
            .provision_fixture_identity(identity.clone(), other)
            .await,
        Err(SessionError::Conflict)
    );
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM session_accounts")
        .fetch_one(&db.first)
        .await
        .unwrap();
    assert_eq!(
        accounts, 1,
        "reassignment rejection must roll back the attempted account"
    );
    let another_key = ProviderIdentityKey::new("issuer", "other-subject").unwrap();
    first
        .provision_fixture_identity(another_key.clone(), account)
        .await
        .unwrap();
    assert_eq!(
        second
            .account_snapshot(another_key)
            .await
            .unwrap()
            .user_id(),
        UserId(1)
    );
    assert_eq!(
        second.account_snapshot(identity).await.unwrap().user_id(),
        UserId(1)
    );
    db.close().await;
}

async fn database_clock(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
        .fetch_one(pool)
        .await
        .expect("independent acceptance database clock must be reachable")
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
#[allow(clippy::too_many_lines)]
async fn postgres_production_clock_is_sampled_after_waiting_on_authority_locks() {
    let db = DatabaseFixture::new().await;
    // No TestControls: this reaches the production clock_timestamp query.
    let first = PgSessionStore::new(db.first.clone());
    first.migrate().await.unwrap();
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let digest = SessionCredential::generate().unwrap().digest();
    let before_issue = database_clock(&db.admin).await;
    issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest).await;
    let after_issue = database_clock(&db.admin).await;
    let issued = stored(&db.first, 10).await;
    assert!((before_issue..=after_issue).contains(&issued.created));
    assert_eq!(issued.activity, issued.created);
    assert_eq!(issued.idle, issued.created + 1_800_000);
    assert_eq!(issued.absolute, issued.created + 86_400_000);
    let first_pid = backend_pid(&db.first).await;
    let second_pid = backend_pid(&db.second).await;
    let deadline = database_clock(&db.admin).await + 2_000;
    let created = deadline - 1_800_000;
    sqlx::query(
        "UPDATE session_auth_sessions SET created_at_ms = $2, last_activity_at_ms = $2, \
         last_observed_at_ms = $2, idle_deadline_ms = $3, absolute_deadline_ms = $4 WHERE id = $1",
    )
    .bind(Uuid::from_u128(10))
    .bind(created)
    .bind(deadline)
    .bind(created + 86_400_000)
    .execute(&db.first)
    .await
    .unwrap();
    let mut held = db.first.begin().await.unwrap();
    sqlx::query("SELECT user_id FROM session_accounts WHERE user_id = $1 FOR UPDATE")
        .bind(Uuid::from_u128(1))
        .fetch_one(&mut *held)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM session_auth_sessions WHERE id = $1 FOR UPDATE")
        .bind(Uuid::from_u128(10))
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let second = PgSessionStore::new(db.second.clone());
    let waiting = tokio::spawn(async move {
        second
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await
    });
    wait_blocked(&db.admin, second_pid, first_pid).await;
    assert!(
        database_clock(&db.admin).await < deadline,
        "production-clock fixture must establish the transaction wait before expiry"
    );
    // The independent database clock is the ordering oracle. No duration or
    // scheduler sleep is treated as proof that the deadline has passed.
    tokio::time::timeout(WAIT_LIMIT, async {
        while database_clock(&db.admin).await < deadline {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("database clock must reach the fixture deadline");
    held.commit().await.unwrap();
    assert_eq!(finish(waiting).await, Err(SessionError::Unauthenticated));
    let row = stored(&db.first, 10).await;
    assert!(row.expired.is_some_and(|at| at >= deadline));
    assert_eq!(row.activity, created);
    assert_eq!(row.idle, deadline);
    assert_eq!(row.absolute, created + 86_400_000);
    db.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_resource_unique_lock_wait_rechecks_expiry_for_new_and_duplicate_effects() {
    for resource_commits in [false, true] {
        let db = DatabaseFixture::new().await;
        let (first, _) = store(&db.first, START_MS);
        first.migrate().await.unwrap();
        marker_table(&db.first).await;
        let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
        let digest = SessionCredential::generate().unwrap().digest();
        issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest).await;
        let first_pid = backend_pid(&db.first).await;
        let second_pid = backend_pid(&db.second).await;
        // An independent resource owner holds only the unique marker key. The
        // session operation can acquire authority locks before this resource wait.
        let mut resource_owner = db.first.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO tabula_session_acceptance_markers (marker) VALUES ('resource-locked')",
        )
        .execute(&mut *resource_owner)
        .await
        .unwrap();
        let (second, controls) = store(&db.second, 1_001_799_999);
        let waiting = tokio::spawn(async move {
            second
                .protected_marker(
                    digest,
                    SessionChannel::BrowserCookie,
                    "resource-locked",
                    ProtectedOperation::Accepted,
                )
                .await
        });
        wait_blocked(&db.admin, second_pid, first_pid).await;
        controls.clock.store(1_001_800_000, Ordering::SeqCst);
        if resource_commits {
            resource_owner.commit().await.unwrap();
        } else {
            resource_owner.rollback().await.unwrap();
        }
        assert_eq!(finish(waiting).await, Err(SessionError::Unauthenticated));
        assert_eq!(
            marker_count(&db.first, "resource-locked").await,
            i64::from(resource_commits)
        );
        let row = stored(&db.first, 10).await;
        assert_eq!(row.expired, Some(1_001_800_000));
        assert_eq!(row.observed, 1_001_800_000);
        assert_eq!(row.activity, 1_000_000_000);
        assert_eq!(row.idle, 1_001_800_000);
        assert_eq!(row.absolute, 1_086_400_000);
        db.close().await;
    }
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL 16 via DATABASE_URL"]
async fn postgres_impossible_future_session_epoch_is_retired_before_account_catches_up() {
    let db = DatabaseFixture::new().await;
    let (first, controls) = store(&db.first, START_MS);
    first.migrate().await.unwrap();
    marker_table(&db.first).await;
    let identity = seed_identity(&db.first, 1, "issuer", "subject").await;
    let digest = SessionCredential::generate().unwrap().digest();
    let binding = issue(&first, &identity, 10, SessionChannel::BrowserCookie, digest)
        .await
        .binding();
    sqlx::query("UPDATE session_auth_sessions SET authorization_epoch = 1 WHERE id = $1")
        .bind(Uuid::from_u128(10))
        .execute(&db.first)
        .await
        .unwrap();
    controls.clock.store(1_000_100_000, Ordering::SeqCst);
    assert_eq!(
        first.observe_binding(binding).await,
        Err(SessionError::Unavailable)
    );
    let retired = stored(&db.first, 10).await;
    assert_eq!(retired.epoch, 1);
    assert_eq!(retired.revoked, Some(1_000_100_000));
    assert_eq!(retired.observed, 1_000_100_000);
    assert_eq!(retired.activity, 1_000_000_000);
    let (second, _) = store(&db.second, 1_000_200_000);
    assert_eq!(
        second
            .invalidate_account_epoch(UserId(1), AccountEpoch::new(0).unwrap())
            .await
            .unwrap()
            .authorization_epoch()
            .get(),
        1
    );
    assert_eq!(
        second
            .observe_credential(digest, SessionChannel::BrowserCookie)
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        second
            .protected_marker(
                digest,
                SessionChannel::BrowserCookie,
                "future-epoch",
                ProtectedOperation::Accepted
            )
            .await,
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(marker_count(&db.first, "future-epoch").await, 0);
    assert_eq!(stored(&db.first, 10).await.revoked, Some(1_000_100_000));
    db.close().await;
}
