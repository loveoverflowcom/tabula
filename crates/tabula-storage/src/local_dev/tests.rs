use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::{postgres::PgConnectOptions, AssertSqlSafe, ConnectOptions};

use super::*;

static SCHEMA_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn config_rejects_out_of_budget_values_before_any_io() {
    for (pool, rooms) in [(0, 1), (41, 1), (1, 0), (1, 129), (u32::MAX, u16::MAX)] {
        assert_eq!(
            LocalDevStorageConfig::new(pool, rooms),
            Err(LocalDevStorageError::InvalidConfig)
        );
    }
    for (pool, rooms) in [(1, 1), (8, 16), (40, 128)] {
        let config = LocalDevStorageConfig::new(pool, rooms).unwrap();
        assert_eq!(config.max_connections(), pool);
        assert_eq!(config.lifetime_room_capacity(), rooms);
    }
}

#[test]
fn whole_schema_history_keeps_every_original_version_and_checksum() {
    let combined = migrator().unwrap();
    let versions = combined
        .iter()
        .map(|migration| migration.version)
        .collect::<Vec<_>>();
    assert_eq!(
        versions,
        vec![
            202_610_040_001,
            202_610_050_001,
            202_610_050_040,
            202_610_050_041,
            202_610_060_001,
            20_261_006_004_301,
            20_261_006_004_302,
        ]
    );
    for original in sqlx::migrate!("./session_migrations")
        .iter()
        .chain(sqlx::migrate!("./accounts_migrations").iter())
        .chain(sqlx::migrate!("./social_migrations").iter())
        .chain(sqlx::migrate!("./match_migrations").iter())
        .chain(sqlx::migrate!("./online_match_migrations").iter())
    {
        let retained = combined
            .iter()
            .find(|migration| migration.version == original.version)
            .unwrap();
        assert_eq!(retained.checksum, original.checksum);
        assert_eq!(retained.sql, original.sql);
    }
}

struct Fixture {
    admin: PgPool,
    schema: String,
    url: String,
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("TABULA_MATCH_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .expect("local/dev acceptance requires an explicit disposable PostgreSQL URL");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .unwrap();
        assert_eq!(version / 10_000, 16, "actual PostgreSQL 16 is required");
        let schema = format!(
            "tabula_local_dev_{}_{}",
            std::process::id(),
            SCHEMA_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        sqlx::raw_sql(schema_ddl(&schema, true))
            .execute(&admin)
            .await
            .unwrap();
        let mut url = url.parse::<PgConnectOptions>().unwrap().to_url_lossy();
        // SQLx's lossy URL conversion drops connection options; append the
        // search path to the actual URL consumed by public startup instead.
        url.query_pairs_mut()
            .append_pair("options", &format!("-c search_path={schema}"));
        let url = url.to_string();
        let scoped = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let actual: String = sqlx::query_scalar("SELECT current_schema()")
            .fetch_one(&scoped)
            .await
            .unwrap();
        assert_eq!(actual, schema, "test setup must select its isolated schema");
        scoped.close().await;
        Self { admin, schema, url }
    }
    async fn connect(&self, policy: SchemaPolicy) -> Result<LocalDevStore, LocalDevStorageError> {
        LocalDevStore::connect(&self.url, LocalDevStorageConfig::new(4, 2).unwrap(), policy).await
    }
    async fn close(self) {
        sqlx::raw_sql(schema_ddl(&self.schema, false))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

fn schema_ddl(schema: &str, create: bool) -> AssertSqlSafe<String> {
    assert!(schema.starts_with("tabula_local_dev_"));
    assert!(schema
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
    AssertSqlSafe(if create {
        format!("CREATE SCHEMA {schema}")
    } else {
        format!("DROP SCHEMA {schema} CASCADE")
    })
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_local_dev_check_is_read_only_and_apply_is_repeatable() {
    let fixture = Fixture::new().await;
    assert_eq!(
        fixture.connect(SchemaPolicy::Check).await.unwrap_err(),
        LocalDevStorageError::SchemaUnavailable
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace \
         WHERE n.nspname=$1",
    )
    .bind(&fixture.schema)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "check-only startup must not create migration metadata"
    );
    let first = fixture.connect(SchemaPolicy::Apply).await.unwrap();
    first.check_ready().await.unwrap();
    let deadlines: (String, String) = sqlx::query_as(
        "SELECT current_setting('statement_timeout'), \
         current_setting('idle_in_transaction_session_timeout')",
    )
    .fetch_one(&first.pool)
    .await
    .unwrap();
    assert_eq!(deadlines, ("5s".to_owned(), "10s".to_owned()));
    let second = fixture.connect(SchemaPolicy::Apply).await.unwrap();
    let checked = fixture.connect(SchemaPolicy::Check).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_auth_sessions")
        .fetch_one(&checked.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "readiness must not issue fixture sessions");
    first.close().await;
    second.close().await;
    checked.close().await;
    fixture.close().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_local_dev_composes_existing_online_and_social_histories() {
    for online in [true, false] {
        let fixture = Fixture::new().await;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&fixture.url)
            .await
            .unwrap();
        if online {
            PgOnlineMatchStore::migrate(&pool).await.unwrap();
        } else {
            PgSessionStore::new(pool.clone())
                .migrate_social()
                .await
                .unwrap();
        }
        let before: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        sqlx::query("INSERT INTO session_accounts(user_id,authorization_epoch,enabled,last_observed_at_ms) VALUES('00000000-0000-0000-0000-000000000001',0,true,0)")
            .execute(&pool).await.unwrap();
        assert_eq!(
            fixture.connect(SchemaPolicy::Check).await.unwrap_err(),
            LocalDevStorageError::SchemaUnavailable
        );
        let store = fixture.connect(SchemaPolicy::Apply).await.unwrap();
        let after: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&store.pool)
                .await
                .unwrap();
        assert!(before.iter().all(|migration| after.contains(migration)));
        let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM session_accounts")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            accounts, 1,
            "composition must retain the existing authority data"
        );
        store.check_ready().await.unwrap();
        store.close().await;
        pool.close().await;
        fixture.close().await;
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL 16"]
async fn real_postgres_local_dev_readiness_rejects_history_column_and_commit_fence_drift() {
    let fixture = Fixture::new().await;
    let store = fixture.connect(SchemaPolicy::Apply).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=$2")
        .bind(vec![0_u8; 48])
        .bind(202_610_040_001_i64)
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store.check_ready().await,
        Err(LocalDevStorageError::SchemaUnavailable)
    );
    let expected = migrator().unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=$2")
        .bind(expected.iter().next().unwrap().checksum.as_ref())
        .bind(202_610_040_001_i64)
        .execute(&store.pool)
        .await
        .unwrap();
    store.check_ready().await.unwrap();
    sqlx::query("ALTER TABLE session_accounts RENAME COLUMN enabled TO broken_enabled")
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store.check_ready().await,
        Err(LocalDevStorageError::SchemaUnavailable)
    );
    sqlx::query("ALTER TABLE session_accounts RENAME COLUMN broken_enabled TO enabled")
        .execute(&store.pool)
        .await
        .unwrap();
    store.check_ready().await.unwrap();
    sqlx::query("ALTER TABLE match_journal_heads DISABLE TRIGGER online_match_commit_fence")
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store.check_ready().await,
        Err(LocalDevStorageError::SchemaUnavailable)
    );
    sqlx::query("ALTER TABLE match_journal_heads ENABLE TRIGGER online_match_commit_fence")
        .execute(&store.pool)
        .await
        .unwrap();
    store.check_ready().await.unwrap();
    store.close().await;
    assert_eq!(
        store.check_ready().await,
        Err(LocalDevStorageError::DatabaseUnavailable)
    );
    fixture.close().await;
}
