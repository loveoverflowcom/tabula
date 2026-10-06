//! Disposable account/social browser acceptance fixture (ADR-0043).
//! No production startup, synthetic login, provider provisioning or debug routes.
#![forbid(unsafe_code)]

use std::{path::PathBuf, time::Duration};

use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use tabula_auth::{config::KanidmConfig, http::isolated_accounts_router};
use tabula_session::{AccountEpoch, EnrollmentAuthority};
use tabula_storage::session::PgSessionStore;

#[derive(Deserialize)]
struct ProviderConfig {
    provider_origin: String,
    client_id: String,
    client_secret: String,
    callback_url: String,
    ca_path: String,
}

fn config() -> Result<ProviderConfig, &'static str> {
    let path = PathBuf::from(
        std::env::var("TABULA_KANIDM_TEST_CONFIG").map_err(|_| "provider config required")?,
    );
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| "provider config absent")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 16_384 {
        return Err("provider config must be bounded private regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err("provider config must be private");
        }
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| "provider config unreadable")?)
        .map_err(|_| "provider config schema invalid")
}

async fn run() -> Result<(), &'static str> {
    if std::env::var("TABULA_ACCOUNTS_DISPOSABLE").as_deref() != Ok("1")
        || std::env::var("TABULA_KANIDM_DISPOSABLE").as_deref() != Ok("1")
    {
        return Err("explicit disposable account/provider acceptance scope required");
    }
    let database = std::env::var("DATABASE_URL").map_err(|_| "disposable database required")?;
    if !database.starts_with("postgres://tabula_test@127.0.0.1:")
        || !database.ends_with("/tabula_accounts_browser_acceptance")
    {
        return Err("only dedicated loopback acceptance database is permitted");
    }
    let c = config()?;
    if c.provider_origin != "https://localhost:8443"
        || c.callback_url != "https://app.localhost:8444/api/v1/auth/oidc/callback"
    {
        return Err("only disposable pinned provider/callback is permitted");
    }
    let provider = KanidmConfig::new_with_enrollment(
        &c.provider_origin,
        "https://app.localhost:8444",
        &c.client_id,
        c.client_secret,
        vec![],
    )
    .map_err(|_| "provider configuration rejected")?
    .with_root_certificate(std::fs::read(c.ca_path).map_err(|_| "private provider CA absent")?)
    .map_err(|_| "provider CA rejected")?;
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database)
        .await
        .map_err(|_| "real PostgreSQL must connect")?;
    let store = PgSessionStore::new(pool.clone());
    store
        .migrate_social()
        .await
        .map_err(|_| "isolated migrations failed")?;
    let policy = store
        .enrollment_policy()
        .await
        .map_err(|_| "enrollment policy unavailable")?;
    if policy.epoch != AccountEpoch::new(0).map_err(|_| "initial epoch invalid")? || policy.enabled
    {
        return Err("fresh disposable account database required");
    }
    store
        .set_enrollment_enabled(policy.epoch, true)
        .await
        .map_err(|_| "explicit enrollment policy enable failed")?;
    let router = isolated_accounts_router(provider, store)
        .await
        .map_err(|_| "actual provider/account composition failed")?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .map_err(|_| "loopback acceptance listener failed")?;
    // The supervising harness terminates this job-only process and deletes all
    // provider/DB/browser state. No API can mint a fixture session or bypass OIDC.
    axum::serve(listener, router)
        .await
        .map_err(|_| "acceptance listener failed")?;
    pool.close().await;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("Account/social acceptance setup failed: {message}");
        std::process::exit(1);
    }
}
