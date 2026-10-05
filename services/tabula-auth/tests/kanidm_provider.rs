#![cfg(all(feature = "postgres-acceptance", not(target_arch = "wasm32")))]
//! Ignored acceptance against the actual pinned Kanidm process and `PostgreSQL`.
//! Provider TLS, password reauthentication, resume/consent, PKCE code exchange,
//! signed ID token and durable browser session are real. App requests use Router
//! calls, not a rendered browser or app TLS cookie jar: those claims stay open.

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    response::Response,
    Router,
};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tabula_auth::{config::KanidmConfig, http::isolated_router};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AccountRecord, SessionAuthority, SessionCredential, UnixMillis,
};
use tabula_session_http::{ContextResponse, LoginStartResponse, SessionDisposition};
use tabula_storage::session::PgSessionStore;
use tower::ServiceExt as _;

#[derive(Deserialize)]
struct TestConfig {
    provider_origin: String,
    issuer: String,
    client_id: String,
    client_secret: String,
    callback_url: String,
    ca_path: String,
    admitted_subject: String,
}
fn config_file() -> TestConfig {
    assert_eq!(
        std::env::var("TABULA_KANIDM_DISPOSABLE").as_deref(),
        Ok("1"),
        "requires explicit disposable-provider harness"
    );
    let path =
        std::env::var("TABULA_KANIDM_TEST_CONFIG").expect("real provider test config is required");
    let metadata = std::fs::symlink_metadata(&path).expect("private test config must exist");
    assert!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "config must be a regular private file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o600,
            "config must be private"
        );
    }
    let data = std::fs::read(&path).expect("private test config must be readable");
    assert!(data.len() <= 16_384);
    serde_json::from_slice(&data).expect("private test config schema must be valid")
}
fn provider_config(c: &TestConfig) -> KanidmConfig {
    let callback = url::Url::parse(&c.callback_url).unwrap();
    let origin = callback.origin().ascii_serialization();
    let config = KanidmConfig::new(
        &c.provider_origin,
        &origin,
        &c.client_id,
        c.client_secret.clone(),
        vec![c.admitted_subject.clone()],
    )
    .unwrap()
    .with_root_certificate(std::fs::read(&c.ca_path).unwrap())
    .unwrap();
    assert!(
        config.issuer() == c.issuer && config.callback_url() == c.callback_url,
        "provider/callback trust must be exact"
    );
    config
}
fn random_id() -> u128 {
    let c = SessionCredential::generate().unwrap();
    let d = c.digest();
    u128::from_be_bytes(d.as_bytes()[..16].try_into().unwrap())
}
fn cookie(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.split(';')
                .next()
                .filter(|pair| pair.starts_with(&format!("{name}=")))
                .map(str::to_owned)
        })
}
async fn request(
    router: &Router,
    method: Method,
    path: &str,
    origin: &str,
    cookies: &str,
    csrf: Option<&str>,
) -> Response {
    let mut request = Request::builder().method(method.clone()).uri(path);
    if !cookies.is_empty() {
        request = request.header(header::COOKIE, cookies);
    }
    if method == Method::POST {
        request = request
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(token) = csrf {
        request = request.header("x-tabula-csrf", token);
    }
    router
        .clone()
        .oneshot(
            request
                .body(if method == Method::POST {
                    Body::from("{}")
                } else {
                    Body::empty()
                })
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn context(
    router: &Router,
    origin: &str,
    cookies: &str,
) -> (ContextResponse, Option<String>) {
    let response = request(
        router,
        Method::GET,
        "/api/v1/auth/context",
        origin,
        cookies,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK, "context must succeed");
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let preauth = cookie(&response, "__Host-tabula_preauth");
    let value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    (value, preauth)
}
async fn start(router: &Router, origin: &str, cookies: &str, csrf: &str) -> String {
    let response = request(
        router,
        Method::POST,
        "/api/v1/auth/login",
        origin,
        cookies,
        Some(csrf),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "real login start must succeed"
    );
    let value: LoginStartResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    value.validate().unwrap();
    value.authorization_url
}
async fn authorize(url: String) -> String {
    tokio::task::spawn_blocking(move||{
        let helper=std::env::var("TABULA_KANIDM_TEST_HELPER").expect("real-provider helper is required");
        let config=std::env::var("TABULA_KANIDM_TEST_CONFIG").unwrap();
        assert!(Path::new(&helper).is_file());
        let mut child=Command::new("python3").arg(helper).arg("authorize").arg("--config").arg(config).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("provider helper must start");
        let input=serde_json::to_vec(&serde_json::json!({"authorization_url":url})).unwrap();
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output=child.wait_with_output().expect("provider helper must finish");
        assert!(output.status.success(),"actual provider password/resume/consent flow failed; secret-free helper diagnostics are captured");
        assert!(output.stdout.len()<=8192);
        let value:serde_json::Value=serde_json::from_slice(&output.stdout).expect("provider helper result schema must be valid");
        value.get("callback_url").and_then(serde_json::Value::as_str).expect("actual provider must supply code callback").to_owned()
    }).await.unwrap()
}
fn callback_path(url: &str) -> String {
    let u = url::Url::parse(url).unwrap();
    format!("{}?{}", u.path(), u.query().expect("actual callback query"))
}
fn assert_no_session_cookie(response: &Response) {
    assert!(
        cookie(response, "__Host-tabula_session").is_none(),
        "failed flow must not change a session cookie"
    );
}

#[tokio::test]
#[ignore = "requires disposable pinned Kanidm 1.11.2 and PostgreSQL16; setup errors fail"]
#[allow(clippy::too_many_lines)] // One real-process lifecycle; private setup is not repeated.
async fn real_kanidm_invited_web_login_lifecycle_and_stale_epoch() {
    let c = config_file();
    let config = provider_config(&c);
    let origin = config.browser_origin().to_owned();
    let database = std::env::var("DATABASE_URL").expect("disposable real PostgreSQL is required");
    assert!(
        database.starts_with("postgres://tabula_test@127.0.0.1:")
            && database.ends_with("/tabula_oidc_acceptance"),
        "only the dedicated disposable acceptance database is allowed"
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&database)
        .await
        .expect("real PostgreSQL must connect");
    let store = PgSessionStore::new(pool.clone());
    store
        .migrate()
        .await
        .expect("isolated migrations must succeed");
    let identity = config.admitted_identities()[0].clone();
    let user = UserId(random_id());
    let now = UnixMillis::new(
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap(),
    )
    .unwrap();
    // Operator-admitted mapping fixture uses the actual provider's exact subject.
    // This provisioning proves no identity by itself; only the real OIDC callback does.
    store
        .provision_fixture_identity(
            identity.clone(),
            AccountRecord::new(user, AccountEpoch::new(0).unwrap(), true, now).unwrap(),
        )
        .await
        .unwrap();
    let bare = KanidmConfig::new(
        &c.provider_origin,
        &origin,
        &c.client_id,
        c.client_secret.clone(),
        vec![c.admitted_subject.clone()],
    )
    .unwrap();
    assert!(
        matches!(
            isolated_router(bare, store.clone()).await,
            Err(tabula_session::SessionError::Unavailable)
        ),
        "actual private-CA provider must be rejected without explicit CA trust"
    );
    let outage = KanidmConfig::new(
        "https://localhost:8445",
        &origin,
        &c.client_id,
        c.client_secret.clone(),
        vec![c.admitted_subject.clone()],
    )
    .unwrap();
    assert!(
        matches!(
            isolated_router(outage, store.clone()).await,
            Err(tabula_session::SessionError::Unavailable)
        ),
        "unreachable provider discovery must fail closed before any login/session"
    );
    let router = isolated_router(config, store.clone())
        .await
        .expect("actual provider discovery and exact trust must succeed");
    let (signed_out, preauth) = context(&router, &origin, "").await;
    assert_eq!(signed_out.disposition, SessionDisposition::SignedOut);
    assert!(signed_out.capabilities.login && !signed_out.capabilities.register);
    let preauth = preauth.expect("server preauth cookie");
    let csrf = signed_out.csrf_token.unwrap();
    let url = start(&router, &origin, &preauth, &csrf).await;
    let callback = authorize(url).await;
    // A foreign arbitrary callback cannot consume the genuine cookie-bound attempt.
    let wrong = request(
        &router,
        Method::GET,
        "/api/v1/auth/oidc/callback?state=wrong&code=wrong",
        &origin,
        &preauth,
        None,
    )
    .await;
    assert!(!wrong.status().is_success());
    assert_no_session_cookie(&wrong);
    let response = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::SEE_OTHER,
        "actual code/PKCE/JWK/signature callback must issue"
    );
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/account"
    );
    let session = cookie(&response, "__Host-tabula_session")
        .expect("known durable issuance must set session cookie");
    for attribute in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|h| h.to_str().ok())
            .any(|h| h.starts_with("__Host-tabula_session=") && h.contains(attribute)));
    }
    let replay = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert!(!replay.status().is_success());
    assert_no_session_cookie(&replay);
    let (current, _) = context(&router, &origin, &session).await;
    assert_eq!(current.disposition, SessionDisposition::Authenticated);
    assert_eq!(current.account_id, Some(format!("{:032x}", user.0)));
    let profile = request(&router, Method::GET, "/api/v1/me", &origin, &session, None).await;
    assert_eq!(profile.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(profile.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["account_id"], format!("{:032x}", user.0));
    let csrf = current.csrf_token.unwrap();
    let account_switch = request(
        &router,
        Method::POST,
        "/api/v1/auth/login",
        &origin,
        &session,
        Some(&csrf),
    )
    .await;
    assert!(!account_switch.status().is_success());
    assert_no_session_cookie(&account_switch);
    let logout = request(
        &router,
        Method::POST,
        "/api/v1/auth/logout",
        &origin,
        &session,
        Some(&csrf),
    )
    .await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(&router, Method::GET, "/api/v1/me", &origin, &session, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // A stale HttpOnly session cookie does not block an explicit new login.
    let (old, preauth) = context(&router, &origin, &session).await;
    assert_eq!(old.disposition, SessionDisposition::SignedOut);
    assert!(old.capabilities.login);
    let preauth = preauth.unwrap();
    let cookies = format!("{session}; {preauth}");
    let url = start(&router, &origin, &cookies, &old.csrf_token.unwrap()).await;
    store
        .invalidate_account_epoch(user, AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    let callback = authorize(url).await;
    let stale = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &cookies,
        None,
    )
    .await;
    assert!(
        !stale.status().is_success(),
        "captured epoch must reject an attempt invalidated in-flight"
    );
    assert_no_session_cookie(&stale);
    // A newly authenticated attempt captures the new epoch, not callback-time borrowing.
    let (fresh, preauth) = context(&router, &origin, "").await;
    let preauth = preauth.unwrap();
    let url = start(&router, &origin, &preauth, &fresh.csrf_token.unwrap()).await;
    let callback = authorize(url).await;
    let response = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let replacement = cookie(&response, "__Host-tabula_session").unwrap();
    let (restored, _) = context(&router, &origin, &replacement).await;
    assert_eq!(restored.account_id, Some(format!("{:032x}", user.0)));
    store
        .invalidate_account_epoch(user, AccountEpoch::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/api/v1/me",
            &origin,
            &replacement,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    pool.close().await;
}
