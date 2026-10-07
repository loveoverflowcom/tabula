//! Explicit local/dev process composition, doc 03 §1 and ADR-0047.
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
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
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tabula_match_http::isolated::{IsolatedMatchHttp, MatchHttpConfig};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::local_dev::{
    LocalDevStorageConfig, LocalDevStorageError, LocalDevStore, SchemaPolicy,
};

/// Validated operator configuration; credentials never implement Debug (ADR-0031).
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
    request_capacity: usize,
    work_capacity: usize,
    live_match_capacity: usize,
    drain_seconds: u64,
}
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .finish_non_exhaustive()
    }
}
impl Config {
    /// Read bounded TOML plus explicit `TABULA_SERVER_` environment overrides.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| "config file unavailable")?;
        if !metadata.is_file() || metadata.len() > 16_384 {
            return Err("config must be a regular file of at most 16 KiB");
        }
        let config: Self = Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("TABULA_SERVER_"))
            .extract()
            .map_err(|_| "invalid configuration; check required fields and types")?;
        config.validate()?;
        Ok(config)
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.mode != "local-dev" || !self.bind.ip().is_loopback() || self.bind.port() == 0 {
            return Err("local-dev mode and a nonzero loopback bind are required");
        }
        let origin = url::Url::parse(&self.browser_origin).map_err(|_| "invalid browser_origin")?;
        if origin.scheme() != "https"
            || origin.origin().ascii_serialization() != self.browser_origin
            || !origin
                .host_str()
                .is_some_and(|h| h == "localhost" || h.ends_with(".localhost"))
        {
            return Err("browser_origin must be an exact HTTPS localhost origin");
        }
        self.policy()?;
        if self.pool_connections < 2 {
            return Err("pool_connections must be in 2..40 for nested authority operations");
        }
        self.storage_config()?;
        tabula_session::SessionCredential::parse(&self.csrf_key)
            .map_err(|_| "csrf_key must be a canonical 32-byte base64url secret")?;
        self.gateway_config()?;
        if !(1..=30).contains(&self.drain_seconds) || self.database_url.is_empty() {
            return Err("database_url and drain_seconds in 1..30 are required");
        }
        Ok(())
    }
    fn policy(&self) -> Result<SchemaPolicy, &'static str> {
        match self.schema_policy.as_str() {
            "check" => Ok(SchemaPolicy::Check),
            "apply" => Ok(SchemaPolicy::Apply),
            _ => Err("schema_policy must explicitly be check or apply"),
        }
    }
    fn storage_config(&self) -> Result<LocalDevStorageConfig, &'static str> {
        LocalDevStorageConfig::new(self.pool_connections, self.lifetime_room_capacity)
            .map_err(|_| "pool_connections or lifetime_room_capacity out of bounds")
    }
    fn gateway_config(&self) -> Result<MatchHttpConfig, &'static str> {
        MatchHttpConfig::new(
            self.request_capacity,
            self.work_capacity,
            self.live_match_capacity,
        )
        .map_err(|_| "request, work or live match capacity out of bounds")
    }
}

/// Serve current profile/social/session authority and the existing match gateway.
/// Migrations require explicit operator policy; no fixture/test routes are linked.
pub async fn serve(config: Config) -> Result<(), &'static str> {
    config.validate()?;
    let store = LocalDevStore::connect(
        &config.database_url,
        config.storage_config()?,
        config.policy()?,
    )
    .await
    .map_err(storage_failure)?;
    let sessions = store.session_store();
    let session_http = IsolatedSessionHttp::new(sessions.clone(), &config.browser_origin)
        .map_err(|_| "session origin configuration rejected")?
        .with_csrf_key(&config.csrf_key)
        .map_err(|_| "CSRF key configuration rejected")?
        .with_account_capabilities(true, true)
        .map_err(|_| "account capabilities rejected")?
        .with_account_readiness()
        .map_err(|_| "account readiness configuration rejected")?;
    let games = tabula_registry::registered_games();
    if games.is_empty() || !games.iter().any(|g| g.direct_host_supported()) {
        return Err("registry contains no supported direct-match module");
    }
    let gateway = IsolatedMatchHttp::with_config(
        session_http.clone(),
        sessions,
        store.online_match_store(),
        store.match_store(),
        games,
        config.gateway_config()?,
    )
    .map_err(|_| "match gateway configuration rejected")?;
    let stopping = Arc::new(AtomicBool::new(false));
    let ops = operations_router(store.clone(), gateway.clone(), stopping.clone());
    let (social_stop, social_stopped) = tokio::sync::watch::channel(false);
    let (social_router, social_shutdown) = session_http.social_router_with_shutdown(social_stopped);
    let router = ops
        .merge(session_http.profile_router())
        .merge(social_router)
        .merge(session_http.router())
        .merge(gateway.clone().router());
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|_| "loopback bind failed")?;
    eprintln!("tabula-server local-dev ready on {}", config.bind);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => {
            result.map_err(|_| "HTTP listener failed")?;
            return Err("HTTP listener stopped before shutdown");
        }
        result = shutdown_signal() => { result?; }
    }
    stopping.store(true, Ordering::Release);
    gateway.begin_shutdown();
    let _ = social_stop.send(true);
    let _ = stop.send(());
    let budget = Duration::from_secs(config.drain_seconds);
    let shutdown = async {
        let (http, social) = tokio::join!(&mut server, social_shutdown.finish(budget));
        if http.is_err() || !social {
            return Err("HTTP or social drain incomplete");
        }
        if !gateway.shutdown(budget).await.is_complete() {
            return Err("actor drain incomplete");
        }
        store.close().await;
        Ok(())
    };
    tokio::time::timeout(budget, shutdown)
        .await
        .map_err(|_| "shutdown deadline exhausted; durable recovery required")??;
    eprintln!("tabula-server local-dev drained");
    Ok(())
}

fn operations_router(
    store: LocalDevStore,
    gateway: IsolatedMatchHttp,
    stopping: Arc<AtomicBool>,
) -> Router {
    let ready_store = store;
    let ready_gateway = gateway;
    let ready_stopping = stopping;
    Router::new()
        .route(
            "/healthz",
            get(|| async { ([(header::CACHE_CONTROL, "no-store")], "alive") }),
        )
        .route(
            "/readyz",
            get(move || {
                let store = ready_store.clone();
                let gateway = ready_gateway.clone();
                let stopping = ready_stopping.clone();
                async move {
                    let ready = !stopping.load(Ordering::Acquire)
                        && !gateway.is_draining()
                        && store.check_ready().await.is_ok()
                        && !stopping.load(Ordering::Acquire)
                        && !gateway.is_draining();
                    (
                        if ready {
                            StatusCode::OK
                        } else {
                            StatusCode::SERVICE_UNAVAILABLE
                        },
                        [(header::CACHE_CONTROL, "no-store")],
                        if ready { "ready" } else { "unavailable" },
                    )
                        .into_response()
                }
            }),
        )
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

/// Execute the explicit local/dev CLI, preserving the default production gate.
pub fn run() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command, flag, path] if command == "serve" && flag == "--config" => {
            Config::load(Path::new(path)).and_then(|config| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "runtime initialization failed")?
                    .block_on(serve(config))
            })
        }
        _ => Err("usage: tabula-server serve --config <local-dev.toml>"),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(label) => {
            eprintln!("tabula-server: {label}");
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
            bind: "127.0.0.1:3002".parse().unwrap(),
            browser_origin: "https://app.localhost:8444".into(),
            database_url: "postgres://localhost/tabula_dev".into(),
            csrf_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            schema_policy: "check".into(),
            pool_connections: 8,
            lifetime_room_capacity: 16,
            request_capacity: 16,
            work_capacity: 16,
            live_match_capacity: 16,
            drain_seconds: 15,
        }
    }
    #[test]
    fn configuration_rejects_public_binds_alias_origins_and_unbounded_shutdown() {
        assert!(config().validate().is_ok());
        for bind in ["0.0.0.0:3002", "127.0.0.1:0", "[::]:3002"] {
            let mut c = config();
            c.bind = bind.parse().unwrap();
            assert!(c.validate().is_err());
        }
        for origin in [
            "http://app.localhost",
            "https://app.localhost/",
            "https://app.localhost:443",
            "https://app.example",
            "https://APP.localhost",
        ] {
            let mut c = config();
            c.browser_origin = origin.into();
            assert!(c.validate().is_err());
        }
        for seconds in [0, 31, u64::MAX] {
            let mut c = config();
            c.drain_seconds = seconds;
            assert!(c.validate().is_err());
        }
        let mut c = config();
        c.schema_policy = "automatic".into();
        assert!(c.validate().is_err());
    }
    #[tokio::test]
    async fn public_composition_revalidates_config_before_opening_storage_or_listener() {
        let mut c = config();
        c.bind = "0.0.0.0:3002".parse().unwrap();
        assert_eq!(
            serve(c).await,
            Err("local-dev mode and a nonzero loopback bind are required")
        );
        let c = config();
        let debug = format!("{c:?}");
        assert!(!debug.contains(&c.database_url));
        assert!(!debug.contains(&c.csrf_key));
    }
}
