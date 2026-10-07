//! Local/dev provider process; Kanidm keeps credential ownership (ADR-0047).
use crate::{config::KanidmConfig, oidc::KanidmOidc};
use axum::{
    http::{header, StatusCode},
    routing::get,
    Router,
};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;
use std::{
    future::IntoFuture,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tabula_session::EnrollmentAuthority;
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::local_dev::{
    LocalDevStorageConfig, LocalDevStorageError, LocalDevStore, SchemaPolicy,
};

/// Exact local/dev trust configuration. Secret values are never Debug-formatted.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    mode: String,
    bind: SocketAddr,
    browser_origin: String,
    database_url: String,
    csrf_key: String,
    schema_policy: String,
    pool_connections: u32,
    lifetime_room_capacity: u16,
    drain_seconds: u64,
    provider_origin: String,
    client_id: String,
    client_secret: String,
    root_certificate: Option<PathBuf>,
}
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .finish_non_exhaustive()
    }
}
impl Config {
    /// Load bounded TOML and explicit `TABULA_AUTH_` environment overrides.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| "config file unavailable")?;
        if !metadata.is_file() || metadata.len() > 16_384 {
            return Err("config must be a regular file of at most 16 KiB");
        }
        let c: Self = Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("TABULA_AUTH_"))
            .extract()
            .map_err(|_| "invalid configuration; check required fields and types")?;
        c.validate()?;
        Ok(c)
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.mode != "local-dev" || !self.bind.ip().is_loopback() || self.bind.port() == 0 {
            return Err("local-dev mode and a nonzero loopback bind are required");
        }
        for raw in [&self.browser_origin, &self.provider_origin] {
            let u = url::Url::parse(raw).map_err(|_| "invalid HTTPS origin")?;
            if u.scheme() != "https"
                || u.origin().ascii_serialization() != *raw
                || !u
                    .host_str()
                    .is_some_and(|h| h == "localhost" || h.ends_with(".localhost"))
            {
                return Err("provider and browser must be exact HTTPS localhost origins");
            }
        }
        self.policy()?;
        if self.pool_connections < 2 {
            return Err("pool_connections must be in 2..40 for nested authority operations");
        }
        self.storage_config()?;
        tabula_session::SessionCredential::parse(&self.csrf_key)
            .map_err(|_| "csrf_key must be a canonical 32-byte base64url secret")?;
        if !(1..=30).contains(&self.drain_seconds) || self.database_url.is_empty() {
            return Err("database_url and drain_seconds in 1..30 are required");
        }
        KanidmConfig::new_with_enrollment(
            &self.provider_origin,
            &self.browser_origin,
            &self.client_id,
            self.client_secret.clone(),
            vec![],
        )
        .map_err(|_| "provider client configuration rejected")?;
        Ok(())
    }
    fn storage_config(&self) -> Result<LocalDevStorageConfig, &'static str> {
        LocalDevStorageConfig::new(self.pool_connections, self.lifetime_room_capacity)
            .map_err(|_| "pool_connections or lifetime_room_capacity out of bounds")
    }
    fn policy(&self) -> Result<SchemaPolicy, &'static str> {
        match self.schema_policy.as_str() {
            "check" => Ok(SchemaPolicy::Check),
            "apply" => Ok(SchemaPolicy::Apply),
            _ => Err("schema_policy must explicitly be check or apply"),
        }
    }
    fn provider(&self) -> Result<KanidmConfig, &'static str> {
        let mut c = KanidmConfig::new_with_enrollment(
            &self.provider_origin,
            &self.browser_origin,
            &self.client_id,
            self.client_secret.clone(),
            vec![],
        )
        .map_err(|_| "provider client configuration rejected")?;
        if let Some(path) = &self.root_certificate {
            let metadata =
                std::fs::symlink_metadata(path).map_err(|_| "provider CA unavailable")?;
            if !metadata.is_file() || metadata.len() > 16_384 {
                return Err("provider CA must be a bounded regular file");
            }
            c = c
                .with_root_certificate(std::fs::read(path).map_err(|_| "provider CA unreadable")?)
                .map_err(|_| "provider CA rejected")?;
        }
        Ok(c)
    }
}

/// Start verified provider login/enrollment and the existing session lifecycle.
pub async fn serve(config: Config) -> Result<(), &'static str> {
    config.validate()?;
    let provider = config.provider()?;
    let store = LocalDevStore::connect(
        &config.database_url,
        config.storage_config()?,
        config.policy()?,
    )
    .await
    .map_err(storage_failure)?;
    let sessions = store.session_store();
    let provider = KanidmOidc::discover_enrollment(provider, sessions.clone())
        .await
        .map_err(|_| "verified provider discovery failed")?;
    let http = IsolatedSessionHttp::new(sessions, &config.browser_origin)
        .map_err(|_| "session origin configuration rejected")?
        .with_csrf_key(&config.csrf_key)
        .map_err(|_| "CSRF key configuration rejected")?
        .with_account_capabilities(true, true)
        .map_err(|_| "account capabilities rejected")?
        .with_account_readiness()
        .map_err(|_| "account readiness configuration rejected")?;
    let router = http.router_with_accounts(provider);
    let stopping = Arc::new(AtomicBool::new(false));
    let ready_store = store.clone();
    let ready_stopping = stopping.clone();
    let ops = Router::new()
        .route(
            "/healthz",
            get(|| async { ([(header::CACHE_CONTROL, "no-store")], "alive") }),
        )
        .route(
            "/readyz",
            get(move || {
                let store = ready_store.clone();
                let stopping = ready_stopping.clone();
                async move {
                    let ready = !stopping.load(Ordering::Acquire)
                        && store.check_ready().await.is_ok()
                        && !stopping.load(Ordering::Acquire);
                    (
                        if ready {
                            StatusCode::OK
                        } else {
                            StatusCode::SERVICE_UNAVAILABLE
                        },
                        [(header::CACHE_CONTROL, "no-store")],
                        if ready { "ready" } else { "unavailable" },
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|_| "loopback bind failed")?;
    eprintln!("tabula-auth local-dev ready on {}", config.bind);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, ops.merge(router))
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => { result.map_err(|_| "HTTP listener failed")?; return Err("HTTP listener stopped before shutdown"); }
        result = shutdown_signal() => { result?; }
    }
    stopping.store(true, Ordering::Release);
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(config.drain_seconds), async {
        server.await.map_err(|_| "HTTP drain failed")?;
        store.close().await;
        Ok::<(), &'static str>(())
    })
    .await
    .map_err(|_| "shutdown deadline exhausted; durable recovery required")??;
    eprintln!("tabula-auth local-dev drained");
    Ok(())
}

fn storage_failure(error: LocalDevStorageError) -> &'static str {
    match error {
        LocalDevStorageError::InvalidConfig => "invalid storage configuration",
        LocalDevStorageError::DatabaseUnavailable => "database connection unavailable",
        LocalDevStorageError::SchemaUnavailable => {
            "schema absent, stale or incompatible; choose explicit apply for a development database"
        }
        LocalDevStorageError::MigrationFailed => "explicit development migration failed",
    }
}

async fn shutdown_signal() -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|_| "SIGTERM handler unavailable")?;
        tokio::select! { r = tokio::signal::ctrl_c() => r.map_err(|_| "SIGINT handler unavailable"), _ = term.recv() => Ok(()) }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .map_err(|_| "shutdown handler unavailable")
}

async fn enrollment(config: Config, enable: bool) -> Result<(), &'static str> {
    let store = LocalDevStore::connect(
        &config.database_url,
        config.storage_config()?,
        config.policy()?,
    )
    .await
    .map_err(storage_failure)?;
    let sessions = store.session_store();
    let policy = sessions
        .enrollment_policy()
        .await
        .map_err(|_| "enrollment policy unavailable")?;
    if policy.enabled != enable {
        sessions
            .set_enrollment_enabled(policy.epoch, enable)
            .await
            .map_err(|_| "enrollment policy changed concurrently or update failed")?;
    }
    store.close().await;
    eprintln!("local-dev enrollment policy updated");
    Ok(())
}

/// The local/dev CLI; enrollment changes are explicit operator commands.
pub fn run() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command, flag, path] if matches!(command.as_str(), "serve" | "enrollment-enable" | "enrollment-disable") && flag == "--config" => {
            Config::load(Path::new(path)).and_then(|config| {
                let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()
                    .map_err(|_| "runtime initialization failed")?;
                if command == "serve" { runtime.block_on(serve(config)) }
                else { runtime.block_on(enrollment(config, command == "enrollment-enable")) }
            })
        }
        _ => Err("usage: tabula-auth <serve|enrollment-enable|enrollment-disable> --config <local-dev.toml>"),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(label) => {
            eprintln!("tabula-auth: {label}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            mode: "local-dev".into(),
            bind: "127.0.0.1:3001".parse().unwrap(),
            browser_origin: "https://app.localhost:8444".into(),
            database_url: "postgres://localhost/tabula_dev".into(),
            csrf_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            schema_policy: "check".into(),
            pool_connections: 8,
            lifetime_room_capacity: 16,
            drain_seconds: 15,
            provider_origin: "https://localhost:8443".into(),
            client_id: "tabula".into(),
            client_secret: "local-test-secret-value".into(),
            root_certificate: None,
        }
    }
    #[test]
    fn config_requires_local_trust_and_explicit_bounded_policy() {
        assert!(config().validate().is_ok());
        let mut c = config();
        c.mode = "production".into();
        assert!(c.validate().is_err());
        for origin in [
            "https://idm.example",
            "http://localhost",
            "https://localhost/",
            "https://localhost:443",
        ] {
            let mut c = config();
            c.provider_origin = origin.into();
            assert!(c.validate().is_err());
        }
        let mut c = config();
        c.bind = "0.0.0.0:3001".parse().unwrap();
        assert!(c.validate().is_err());
        let mut c = config();
        c.schema_policy = "automatic".into();
        assert!(c.validate().is_err());
        let mut c = config();
        c.drain_seconds = 31;
        assert!(c.validate().is_err());
    }
    #[tokio::test]
    async fn public_composition_revalidates_config_before_provider_or_database_io() {
        let mut c = config();
        c.mode = "production".into();
        assert_eq!(
            serve(c).await,
            Err("local-dev mode and a nonzero loopback bind are required")
        );
        let c = config();
        let debug = format!("{c:?}");
        assert!(!debug.contains(&c.client_secret));
        assert!(!debug.contains(&c.csrf_key));
    }
}
