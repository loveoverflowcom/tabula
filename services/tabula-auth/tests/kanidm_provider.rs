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
// Failure output is an untrusted subprocess boundary. Only these fixed IDs
// may appear in a public failure; callback stdout and arbitrary stderr stay private.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HelperStage {
    Input,
    Config,
    AuthorizationRequest,
    LoginBegin,
    LoginMechanism,
    Totp,
    Password,
    Resume,
    Consent,
    Callback,
    Flow,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HelperCategory {
    InputInvalid,
    ConfigInvalid,
    AuthorizationContract,
    FormContract,
    RedirectContract,
    CallbackContract,
    MissingFlowEvidence,
    TotpParameters,
    FlowBound,
    TlsCertificateVerification,
    TlsHandshake,
    TransportTimeout,
    ConnectionRefused,
    NetworkTransport,
    HttpStatus,
    BodyBound,
    InternalError,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperFailure {
    error: HelperError,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperError {
    stage: HelperStage,
    category: HelperCategory,
}
fn helper_failure(bytes: &[u8]) -> String {
    if bytes.len() > 1024 {
        return "helper_failure_unclassified".into();
    }
    let Ok(failure) = serde_json::from_slice::<HelperFailure>(bytes) else {
        return "helper_failure_unclassified".into();
    };
    format!(
        "stage={:?}; category={:?}",
        failure.error.stage, failure.error.category
    )
}
#[test]
fn helper_failure_accepts_only_closed_stage_category_ids() {
    assert_eq!(
        helper_failure(br#"{"error":{"stage":"totp","category":"form_contract"}}"#),
        "stage=Totp; category=FormContract"
    );
    assert_eq!(helper_failure(br#"{"error":{"stage":"authorization_request","category":"tls_certificate_verification"}}"#), "stage=AuthorizationRequest; category=TlsCertificateVerification");
}
#[test]
fn helper_failure_redacts_callback_extra_fields_unknown_ids_and_oversized_bytes() {
    for input in [
        br#"{"callback_url":"https://app.example/callback?code=secret"}"#.as_slice(),
        br#"{"error":{"stage":"callback","category":"callback_contract","detail":"secret"}}"#,
        br#"{"error":{"stage":"secret","category":"form_contract"}}"#,
        br#"{"error":{"stage":"totp","category":"secret"}}"#,
        br#"{"error":{"stage":"totp","category":"form_contract"},"token":"secret"}"#,
        b"secret raw stderr",
        &[0xff, 0xfe],
    ] {
        assert_eq!(helper_failure(input), "helper_failure_unclassified");
    }
    assert_eq!(
        helper_failure(&vec![b's'; 1025]),
        "helper_failure_unclassified"
    );
}
async fn authorize(url: String) -> String {
    tokio::task::spawn_blocking(move || {
        let helper =
            std::env::var("TABULA_KANIDM_TEST_HELPER").expect("real-provider helper is required");
        let config = std::env::var("TABULA_KANIDM_TEST_CONFIG").unwrap();
        assert!(Path::new(&helper).is_file());
        let mut child = Command::new("python3")
            .arg(helper)
            .arg("authorize")
            .arg("--config")
            .arg(config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("provider helper must start");
        let input = serde_json::to_vec(&serde_json::json!({"authorization_url":url})).unwrap();
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output = child
            .wait_with_output()
            .expect("provider helper must finish");
        assert!(
            output.status.success(),
            "actual provider login helper failed: {}",
            helper_failure(&output.stdout)
        );
        assert!(output.stdout.len() <= 8192);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .expect("provider helper result schema must be valid");
        value
            .get("callback_url")
            .and_then(serde_json::Value::as_str)
            .expect("actual provider must supply code callback")
            .to_owned()
    })
    .await
    .unwrap()
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

#[cfg(feature = "accounts-acceptance")]
async fn begin_verified_enrollment(router: &Router, origin: &str) -> (String, String) {
    let (signed_out, preauth) = context(router, origin, "").await;
    let preauth = preauth.unwrap();
    let response = request(
        router,
        Method::POST,
        "/api/v2/auth/enrollment/start",
        origin,
        &preauth,
        signed_out.csrf_token.as_deref(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let start: tabula_session_http::accounts::EnrollmentStartResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    start.validate().unwrap();
    (preauth, authorize(start.authorization_url).await)
}
#[cfg(feature = "accounts-acceptance")]
async fn verified_grant(
    router: &Router,
    origin: &str,
) -> (
    String,
    tabula_session_http::accounts::EnrollmentContextResponse,
) {
    let (preauth, callback) = begin_verified_enrollment(router, origin).await;
    let response = request(
        router,
        Method::GET,
        &callback_path(&callback),
        origin,
        &preauth,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_no_session_cookie(&response);
    let grant = cookie(&response, "__Host-tabula_enrollment").unwrap();
    let response = request(
        router,
        Method::GET,
        "/api/v2/auth/enrollment",
        origin,
        &grant,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: tabula_session_http::accounts::EnrollmentContextResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    value.validate().unwrap();
    (grant, value)
}
#[cfg(feature = "accounts-acceptance")]
#[tokio::test]
#[ignore = "requires disposable pinned Kanidm 1.11.2 and PostgreSQL16; setup errors fail"]
#[allow(clippy::too_many_lines)] // One real-provider legacy completion and revocation lifecycle.
async fn real_kanidm_legacy_profile_completion_captured_epoch_and_preservation() {
    use sqlx::{AssertSqlSafe, Row};
    use tabula_auth::http::isolated_accounts_router;
    use tabula_session::EnrollmentAuthority;
    use tabula_session_http::accounts::{
        RegistrationDisposition, RegistrationResponse, SelfAccountProfileResponse,
    };
    let c = config_file();
    let origin = url::Url::parse(&c.callback_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let config = KanidmConfig::new_with_enrollment(
        &c.provider_origin,
        &origin,
        &c.client_id,
        c.client_secret.clone(),
        vec![],
    )
    .unwrap()
    .with_root_certificate(std::fs::read(&c.ca_path).unwrap())
    .unwrap();
    let database = std::env::var("DATABASE_URL").expect("dedicated disposable PostgreSQL required");
    assert!(
        database.starts_with("postgres://tabula_test@127.0.0.1:")
            && database.ends_with("/tabula_oidc_acceptance")
    );
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database)
        .await
        .unwrap();
    let schema = format!("tabula_legacy_acceptance_{:032x}", random_id());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .after_connect(move |connection, _| {
            let path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&database)
        .await
        .unwrap();
    let store = PgSessionStore::new(pool.clone());
    store.migrate_social().await.unwrap();
    store
        .set_enrollment_enabled(AccountEpoch::new(0).unwrap(), true)
        .await
        .unwrap();
    let identity =
        tabula_session::ProviderIdentityKey::new(&c.issuer, &c.admitted_subject).unwrap();
    let legacy_user = UserId(random_id());
    store
        .provision_fixture_identity(
            identity.clone(),
            AccountRecord::new(
                legacy_user,
                AccountEpoch::new(0).unwrap(),
                true,
                UnixMillis::new(0).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let router = isolated_accounts_router(config, store.clone())
        .await
        .unwrap();
    // Actual provider navigation captured epoch zero before later invalidation.
    let (preauth, callback) = begin_verified_enrollment(&router, &origin).await;
    store
        .invalidate_account_epoch(legacy_user, AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    let fenced = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert!(!fenced.status().is_success());
    assert_no_session_cookie(&fenced);
    assert!(cookie(&fenced, "__Host-tabula_enrollment").is_none());
    // A verified grant is also fenced if invalidation wins before registration.
    let (grant, value) = verified_grant(&router, &origin).await;
    let captured: i64 =
        sqlx::query_scalar("SELECT expected_account_epoch FROM account_enrollment_grants")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(captured, 1);
    store
        .invalidate_account_epoch(legacy_user, AccountEpoch::new(1).unwrap())
        .await
        .unwrap();
    let body=serde_json::json!({"operation_id":value.operation_id.unwrap(),"handle":"legacy_handle","display_name":"Ngọc Hà"}).to_string();
    let response = register_json(
        &router,
        &origin,
        &grant,
        value.csrf_token.as_deref().unwrap(),
        &body,
    )
    .await;
    assert_no_session_cookie(&response);
    let value: RegistrationResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(value.disposition, RegistrationDisposition::Rejected);
    assert!(!store.account_profile_ready(legacy_user).await.unwrap());
    // Explicit actual-provider login preserves v1 compatibility while v2 Friends
    // stays unavailable until a durable profile exists.
    let (signed_out, preauth) = context(&router, &origin, "").await;
    let preauth = preauth.unwrap();
    let callback = authorize(
        start(
            &router,
            &origin,
            &preauth,
            signed_out.csrf_token.as_deref().unwrap(),
        )
        .await,
    )
    .await;
    let response = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    let session = cookie(&response, "__Host-tabula_session").unwrap();
    let (before, _) = context(&router, &origin, &session).await;
    assert_eq!(before.disposition, SessionDisposition::Authenticated);
    assert!(!before.capabilities.friends);
    let absent = request(
        &router,
        Method::GET,
        "/api/v2/profiles/me",
        &origin,
        &session,
        None,
    )
    .await;
    assert_eq!(
        absent.status(),
        StatusCode::NOT_FOUND,
        "authorized absence is distinct from credential rejection"
    );
    let absent: tabula_session_http::PublicProblem =
        serde_json::from_slice(&to_bytes(absent.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(absent.status, 404);
    assert_eq!(absent.code, "not_available");
    let (still_current, _) = context(&router, &origin, &session).await;
    assert_eq!(still_current.disposition, SessionDisposition::Authenticated);
    assert!(!still_current.capabilities.friends);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM session_auth_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    for name in ["Ngọc Hà", "Must not replace"] {
        let (grant, value) = verified_grant(&router, &origin).await;
        let body=serde_json::json!({"operation_id":value.operation_id.unwrap(),"handle":if name=="Ngọc Hà" {"legacy_handle"} else {"replacement_handle"},"display_name":name}).to_string();
        let response = register_json(
            &router,
            &origin,
            &grant,
            value.csrf_token.as_deref().unwrap(),
            &body,
        )
        .await;
        assert_no_session_cookie(&response);
        let value: RegistrationResponse =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(
            value.disposition,
            RegistrationDisposition::AcceptedWithoutSession
        );
    }
    let (after, _) = context(&router, &origin, &session).await;
    assert!(after.capabilities.friends);
    assert_eq!(after.account_id, Some(format!("{:032x}", legacy_user.0)));
    let profile = request(
        &router,
        Method::GET,
        "/api/v2/profiles/me",
        &origin,
        &session,
        None,
    )
    .await;
    assert_eq!(profile.status(), StatusCode::OK);
    let profile: SelfAccountProfileResponse =
        serde_json::from_slice(&to_bytes(profile.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(profile.account_id, format!("{:032x}", legacy_user.0));
    assert_eq!(profile.handle, "legacy_handle");
    assert_eq!(profile.display_name, "Ngọc Hà");
    assert_eq!(profile.revision, 1);
    let account = store.account_snapshot(identity).await.unwrap();
    assert_eq!(account.authorization_epoch().get(), 2);
    assert!(account.enabled());
    let counts=sqlx::query("SELECT (SELECT count(*) FROM session_accounts) AS accounts,(SELECT count(*) FROM session_provider_identities) AS identities,(SELECT count(*) FROM account_profiles) AS profiles,(SELECT count(*) FROM session_auth_sessions) AS sessions").fetch_one(&pool).await.unwrap();
    assert_eq!(counts.get::<i64, _>("accounts"), 1);
    assert_eq!(counts.get::<i64, _>("identities"), 1);
    assert_eq!(counts.get::<i64, _>("profiles"), 1);
    assert_eq!(counts.get::<i64, _>("sessions"), sessions);
    drop(router);
    pool.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
#[cfg(feature = "accounts-acceptance")]
async fn register_json(
    router: &Router,
    origin: &str,
    grant: &str,
    token: &str,
    body: &str,
) -> Response {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v2/auth/register")
                .header(header::ORIGIN, origin)
                .header(header::COOKIE, grant)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tabula-csrf", token)
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[cfg(feature = "accounts-acceptance")]
#[tokio::test]
#[ignore = "requires disposable pinned Kanidm 1.11.2 and PostgreSQL16; setup errors fail"]
#[allow(clippy::too_many_lines)]
async fn real_kanidm_unmapped_enrollment_receipt_login_profile_and_policy_fences() {
    use sqlx::{AssertSqlSafe, Row};
    use tabula_auth::http::isolated_accounts_router;
    use tabula_session::EnrollmentAuthority;
    use tabula_session_http::accounts::{
        EnrollmentContextResponse, EnrollmentDisposition, EnrollmentStartResponse,
        RegistrationDisposition, RegistrationResponse, SelfAccountProfileResponse,
    };
    let c = config_file();
    let origin = url::Url::parse(&c.callback_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let make_config = || {
        KanidmConfig::new_with_enrollment(
            &c.provider_origin,
            &origin,
            &c.client_id,
            c.client_secret.clone(),
            vec![],
        )
        .unwrap()
        .with_root_certificate(std::fs::read(&c.ca_path).unwrap())
        .unwrap()
    };
    let database = std::env::var("DATABASE_URL").expect("dedicated disposable PostgreSQL required");
    assert!(
        database.starts_with("postgres://tabula_test@127.0.0.1:")
            && (database.ends_with("/tabula_oidc_acceptance")
                || database.ends_with("/tabula_accounts_browser_acceptance"))
    );
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database)
        .await
        .expect("actual PostgreSQL must connect");
    let schema = format!("tabula_accounts_acceptance_{:032x}", random_id());
    assert!(
        schema.len() <= 63
            && schema
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    );
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .after_connect(move |connection, _| {
            let path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&database)
        .await
        .unwrap();
    let store = PgSessionStore::new(pool.clone());
    store
        .migrate_social()
        .await
        .expect("combined migration checksums and real account/social schema");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_accounts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "real verified subject must start entirely unmapped"
    );
    store
        .set_enrollment_enabled(AccountEpoch::new(0).unwrap(), true)
        .await
        .unwrap();
    let router = isolated_accounts_router(make_config(), store.clone())
        .await
        .unwrap();
    let (signed_out, preauth) = context(&router, &origin, "").await;
    assert!(signed_out.capabilities.register);
    let preauth = preauth.unwrap();
    let enrollment = request(
        &router,
        Method::POST,
        "/api/v2/auth/enrollment/start",
        &origin,
        &preauth,
        signed_out.csrf_token.as_deref(),
    )
    .await;
    assert_eq!(enrollment.status(), StatusCode::OK);
    let enrollment_start: EnrollmentStartResponse =
        serde_json::from_slice(&to_bytes(enrollment.into_body(), 8192).await.unwrap()).unwrap();
    enrollment_start.validate().unwrap();
    let callback = authorize(enrollment_start.authorization_url).await;
    let completed = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert_eq!(completed.status(), StatusCode::SEE_OTHER);
    assert_eq!(completed.headers()[header::LOCATION], "/register");
    assert_no_session_cookie(&completed);
    let grant = cookie(&completed, "__Host-tabula_enrollment")
        .expect("durable cookie-bound grant after real verified provider callback");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_auth_sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "enrollment must never issue a session"
    );
    // New router/process-secret replacement still resolves the committed grant;
    // it issues a fresh synchronizer, never a fresh enrollment or borrowed identity.
    drop(router);
    let router = isolated_accounts_router(make_config(), store.clone())
        .await
        .unwrap();
    let response = request(
        &router,
        Method::GET,
        "/api/v2/auth/enrollment",
        &origin,
        &grant,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let enrollment: EnrollmentContextResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    enrollment.validate().unwrap();
    assert_eq!(enrollment.disposition, EnrollmentDisposition::Ready);
    assert!(enrollment.agreement.is_none());
    let body=serde_json::json!({"operation_id":enrollment.operation_id.unwrap(),"handle":"new_account","display_name":"Ngọc Hà"}).to_string();
    let token = enrollment.csrf_token.unwrap();
    let wrong = register_json(&router, "https://foreign.example", &grant, &token, &body).await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert_no_session_cookie(&wrong);
    let wrong = register_json(&router, &origin, &grant, &"A".repeat(43), &body).await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert_no_session_cookie(&wrong);
    for _ in 0..2 {
        let registered = register_json(&router, &origin, &grant, &token, &body).await;
        assert_eq!(registered.status(), StatusCode::OK);
        assert_no_session_cookie(&registered);
        let receipt: RegistrationResponse =
            serde_json::from_slice(&to_bytes(registered.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(
            receipt.disposition,
            RegistrationDisposition::AcceptedWithoutSession
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_accounts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_auth_sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let changed = serde_json::from_str::<serde_json::Value>(&body).unwrap();
    let mut changed = changed;
    changed["handle"] = "different".into();
    let rejected = register_json(&router, &origin, &grant, &token, &changed.to_string()).await;
    let receipt: RegistrationResponse =
        serde_json::from_slice(&to_bytes(rejected.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(receipt.disposition, RegistrationDisposition::Rejected);
    let (fresh, preauth) = context(&router, &origin, "").await;
    let preauth = preauth.unwrap();
    let login = start(
        &router,
        &origin,
        &preauth,
        fresh.csrf_token.as_deref().unwrap(),
    )
    .await;
    let callback = authorize(login).await;
    let logged_in = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert_eq!(
        logged_in.status(),
        StatusCode::SEE_OTHER,
        "newly registered mapping must be captured by fresh login without static invitation"
    );
    let session = cookie(&logged_in, "__Host-tabula_session").unwrap();
    let profile = request(
        &router,
        Method::GET,
        "/api/v2/profiles/me",
        &origin,
        &session,
        None,
    )
    .await;
    assert_eq!(profile.status(), StatusCode::OK);
    let profile: SelfAccountProfileResponse =
        serde_json::from_slice(&to_bytes(profile.into_body(), 4096).await.unwrap()).unwrap();
    profile.validate().unwrap();
    assert_eq!(profile.handle, "new_account");
    assert_eq!(profile.display_name, "Ngọc Hà");
    assert_eq!(profile.revision, 1);
    let user = UserId(u128::from_str_radix(&profile.account_id, 16).unwrap());
    let identity =
        tabula_session::ProviderIdentityKey::new(&c.issuer, &c.admitted_subject).unwrap();
    assert_eq!(
        store.account_snapshot(identity).await.unwrap().user_id(),
        user
    );
    // Independent viewer fixture exercises PostgreSQL permission and target fences;
    // it supplies trusted test identity facts, not evidence of provider verification.
    let viewer_credential = SessionCredential::generate().unwrap();
    let viewer_grant = SessionCredential::generate().unwrap();
    let viewer_op = tabula_session::AccountOperationId::generate().unwrap();
    let viewer_identity =
        tabula_session::ProviderIdentityKey::new(&c.issuer, "profile-policy-viewer-fixture")
            .unwrap();
    let policy = store.enrollment_policy().await.unwrap();
    store
        .create_enrollment(
            tabula_session::VerifiedEnrollment {
                identity: viewer_identity.clone(),
                expected_policy_epoch: policy.epoch,
                expected_account_epoch: None,
            },
            viewer_grant.digest(),
            viewer_op,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .register_account(
                viewer_grant.digest(),
                viewer_op,
                tabula_session::AccountHandle::new("viewer_fixture".into()).unwrap(),
                tabula_session::AccountDisplayName::new("Viewer".into()).unwrap()
            )
            .await
            .unwrap(),
        tabula_session::EnrollmentStatus::AcceptedWithoutSession
    );
    store
        .issue_session(tabula_session::IssueSession {
            identity: viewer_identity,
            expected_epoch: AccountEpoch::new(0).unwrap(),
            id: tabula_session::AuthSessionId::new(random_id()).unwrap(),
            channel: tabula_session::SessionChannel::BrowserCookie,
            credential_digest: viewer_credential.digest(),
            context_id: tabula_session::SessionContextId::new(random_id()).unwrap(),
        })
        .await
        .unwrap();
    let viewer_cookie = format!(
        "__Host-tabula_session={}",
        viewer_credential.expose_encoded()
    );
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/api/v2/profiles/by-handle/new_account",
            &origin,
            &viewer_cookie,
            None
        )
        .await
        .status(),
        StatusCode::OK,
        "actual Axum0.7 parameter route and current public profile permission"
    );

    let (current, _) = context(&router, &origin, &session).await;
    let csrf = current.csrf_token.unwrap();
    let update=serde_json::json!({"operation_id":format!("{:032x}",random_id()),"expected_revision":1,"display_name":"Updated","visibility":"private"}).to_string();
    let edit = |payload: String| {
        Request::builder()
            .method(Method::PATCH)
            .uri("/api/v2/profiles/me")
            .header(header::ORIGIN, &origin)
            .header(header::COOKIE, &session)
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-tabula-csrf", &csrf)
            .body(Body::from(payload))
            .unwrap()
    };
    for _ in 0..2 {
        assert_eq!(
            router
                .clone()
                .oneshot(edit(update.clone()))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
    }
    let conflict=serde_json::json!({"operation_id":format!("{:032x}",random_id()),"expected_revision":1,"display_name":"Lost update","visibility":"public"}).to_string();
    assert_eq!(
        router
            .clone()
            .oneshot(edit(conflict))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let row = sqlx::query(
        "SELECT display_name,visibility,revision FROM account_profiles WHERE user_id=$1",
    )
    .bind(sqlx::types::Uuid::from_u128(user.0))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("display_name"), "Updated");
    assert_eq!(row.get::<String, _>("visibility"), "private");
    assert_eq!(row.get::<i64, _>("revision"), 2);
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/api/v2/profiles/by-handle/new_account",
            &origin,
            &viewer_cookie,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "private profile denied uniformly to another current account"
    );
    // Test-only SQL simulates an operator-disabled target after it was public.
    sqlx::query("UPDATE account_profiles SET visibility='public' WHERE user_id=$1")
        .bind(sqlx::types::Uuid::from_u128(user.0))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE session_accounts SET enabled=FALSE WHERE user_id=$1")
        .bind(sqlx::types::Uuid::from_u128(user.0))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/api/v2/profiles/by-handle/new_account",
            &origin,
            &viewer_cookie,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "disabled public target must share unavailable response"
    );
    sqlx::query("UPDATE session_accounts SET enabled=TRUE WHERE user_id=$1")
        .bind(sqlx::types::Uuid::from_u128(user.0))
        .execute(&pool)
        .await
        .unwrap();
    store
        .invalidate_account_epoch(user, AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/api/v2/profiles/me",
            &origin,
            &session,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    // A captured policy epoch cannot authorize a later signup after disable/re-enable.
    let (fresh, preauth) = context(&router, &origin, "").await;
    let preauth = preauth.unwrap();
    let response = request(
        &router,
        Method::POST,
        "/api/v2/auth/enrollment/start",
        &origin,
        &preauth,
        fresh.csrf_token.as_deref(),
    )
    .await;
    let enrollment_start: EnrollmentStartResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    let callback = authorize(enrollment_start.authorization_url).await;
    let policy = store.enrollment_policy().await.unwrap();
    store
        .set_enrollment_enabled(policy.epoch, false)
        .await
        .unwrap();
    let fenced = request(
        &router,
        Method::GET,
        &callback_path(&callback),
        &origin,
        &preauth,
        None,
    )
    .await;
    assert!(!fenced.status().is_success());
    assert_no_session_cookie(&fenced);
    assert!(cookie(&fenced, "__Host-tabula_enrollment").is_none());
    assert!(store
        .account_snapshot(
            tabula_session::ProviderIdentityKey::new(&c.issuer, &c.admitted_subject).unwrap()
        )
        .await
        .is_ok());
    drop(router);
    pool.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
