#![cfg(all(feature = "postgres", not(target_arch = "wasm32")))]

//! Real `PostgreSQL` + loopback HTTP acceptance. These ignored cases must be
//! explicitly selected in CI with a disposable `PostgreSQL` 16 `DATABASE_URL`.
//! Missing setup fails; no SQL, direct row edits, or in-memory fallback exists here.
//! TLS, browser/OS credential storage, provider login, and client receipt are outside this evidence.

#[allow(dead_code)]
mod support;

use std::{sync::Arc, time::Duration};

use axum::{body::Body, http::Request};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AccountRecord, AuthSessionId, CredentialOperation, HttpSessionAuthority,
    IssueSession, ProviderIdentityKey, SessionAuthority, SessionChannel, SessionContextId,
    SessionCredential, SessionSnapshot, UnixMillis,
};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::session::PgSessionStore;
use tower::ServiceExt as _;

use support::{assert_session_cookie, WireServer, SESSION_COOKIE, TRUSTED_ORIGIN};

struct DatabaseFixture {
    url: String,
    first_pool: PgPool,
    second_pool: PgPool,
    first: PgSessionStore,
    second: PgSessionStore,
}

impl DatabaseFixture {
    async fn new() -> Self {
        let url = std::env::var("DATABASE_URL").expect(
            "ignored real PostgreSQL HTTP acceptance requires DATABASE_URL; setup cannot be skipped",
        );
        let first_pool = Self::connect(&url).await;
        let second_pool = Self::connect(&url).await;
        let first = PgSessionStore::new(first_pool.clone());
        let second = PgSessionStore::new(second_pool.clone());
        first
            .migrate()
            .await
            .expect("isolated session migrations must succeed");
        Self {
            url,
            first_pool,
            second_pool,
            first,
            second,
        }
    }

    async fn connect(url: &str) -> PgPool {
        PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(15))
            .connect(url)
            .await
            .expect("real PostgreSQL acceptance must connect")
    }

    async fn issue(&self, channel: SessionChannel) -> (String, SessionSnapshot) {
        // All identity/account/session values are synthetic and independent of
        // provider login. Random fixture namespaces avoid cross-test collisions.
        let credential = SessionCredential::generate().unwrap();
        let namespace = random_fixture_id();
        let identity = ProviderIdentityKey::new(
            "https://synthetic-provider.tabula.invalid",
            format!("http-acceptance-{namespace:032x}"),
        )
        .unwrap();
        let account = AccountRecord::new(
            UserId(namespace),
            AccountEpoch::new(0).unwrap(),
            true,
            UnixMillis::new(0).unwrap(),
        )
        .unwrap();
        self.first
            .provision_fixture_identity(identity.clone(), account)
            .await
            .unwrap();
        let snapshot = self
            .first
            .issue_session(IssueSession {
                identity,
                expected_epoch: AccountEpoch::new(0).unwrap(),
                id: AuthSessionId::new(random_fixture_id()).unwrap(),
                channel,
                credential_digest: credential.digest(),
                context_id: SessionContextId::new(random_fixture_id()).unwrap(),
            })
            .await
            .unwrap();
        (credential.expose_encoded(), snapshot)
    }

    async fn listener(store: PgSessionStore) -> Arc<WireServer> {
        Arc::new(
            WireServer::start(
                IsolatedSessionHttp::new(store, TRUSTED_ORIGIN)
                    .unwrap()
                    .router(),
            )
            .await,
        )
    }

    async fn close(self) {
        self.first_pool.close().await;
        self.second_pool.close().await;
    }
}

fn random_fixture_id() -> u128 {
    let random = SessionCredential::generate().unwrap().digest();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&random.as_bytes()[..16]);
    u128::from_be_bytes(bytes) | 1
}

async fn duplicate_browser_refresh(server: &Arc<WireServer>, cookie: &str, csrf: &str) -> String {
    let mut requests = Vec::new();
    for _ in 0..12 {
        let server = server.clone();
        let cookie = cookie.to_owned();
        let csrf = csrf.to_owned();
        requests.push(tokio::spawn(async move {
            server
                .request(
                    "POST",
                    "/api/v1/auth/refresh",
                    &[
                        ("Cookie", &cookie),
                        ("Origin", TRUSTED_ORIGIN),
                        ("Content-Type", "application/json"),
                        ("X-Tabula-CSRF", &csrf),
                    ],
                    "{}",
                )
                .await
        }));
    }
    let mut replacements = Vec::new();
    for request in requests {
        let response = request.await.unwrap();
        response.assert_no_store();
        if response.status == 204 {
            assert_eq!(response.headers("set-cookie").len(), 1);
            assert_session_cookie(response.header("set-cookie").unwrap(), SESSION_COOKIE);
            replacements.push(response.cookie(SESSION_COOKIE).unwrap());
        } else {
            assert!(matches!(response.status, 401 | 409), "{response:?}");
            response.assert_no_cookie();
            response.assert_excludes(&[cookie.split_once('=').unwrap().1]);
        }
    }
    assert_eq!(replacements.len(), 1);
    let replacement = replacements.pop().unwrap();
    assert_ne!(replacement, cookie);
    replacement
}

fn assert_same_session_after_rotation(before: &SessionSnapshot, after: &SessionSnapshot) {
    assert_eq!(after.id(), before.id());
    assert_eq!(after.user_id(), before.user_id());
    assert_eq!(after.context_id(), before.context_id());
    assert_eq!(after.authorization_epoch(), before.authorization_epoch());
    assert_eq!(after.credential_generation().get(), 1);
    assert_eq!(after.idle_deadline(), before.idle_deadline());
    assert_eq!(after.absolute_deadline(), before.absolute_deadline());
    assert_eq!(after.last_activity_at(), before.last_activity_at());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit disposable real PostgreSQL 16 DATABASE_URL"]
async fn pg_http_browser_duplicate_refresh_logout_and_adapter_restart_are_durable() {
    let db = DatabaseFixture::new().await;
    let (credential, before) = db.issue(SessionChannel::BrowserCookie).await;
    let first = DatabaseFixture::listener(db.first.clone()).await;
    let second = DatabaseFixture::listener(db.second.clone()).await;
    let cookie = format!("{SESSION_COOKIE}={credential}");
    let context = first
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(context.status, 200);
    context.assert_no_store();
    context.assert_no_cookie();
    let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
    let replacement_cookie = duplicate_browser_refresh(&first, &cookie, &csrf).await;
    let replacement = &replacement_cookie;
    let current = SessionCredential::parse(replacement.split_once('=').unwrap().1).unwrap();
    let after = db
        .second
        .read_session(CredentialOperation {
            digest: current.digest(),
            channel: SessionChannel::BrowserCookie,
            context: None,
        })
        .await
        .unwrap();
    assert_same_session_after_rotation(&before, &after);
    let stale = second
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(stale.status, 401);
    stale.assert_no_cookie();
    let restarted_context = second
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Cookie", replacement)],
            "",
        )
        .await;
    assert_eq!(restarted_context.status, 200);
    assert_eq!(
        restarted_context.json()["account_id"],
        format!("{:032x}", before.user_id().0)
    );
    let live_context = first
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Cookie", replacement)],
            "",
        )
        .await;
    assert_eq!(live_context.json()["csrf_token"], csrf);
    for _ in 0..2 {
        let logout = first
            .request(
                "POST",
                "/api/v1/auth/logout",
                &[
                    ("Cookie", replacement),
                    ("Origin", TRUSTED_ORIGIN),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(logout.status, 204);
        logout.assert_no_store();
        assert!(logout.header("set-cookie").unwrap().contains("Max-Age=0"));
    }
    let profile = second
        .request("GET", "/api/v1/me", &[("Cookie", replacement)], "")
        .await;
    assert_eq!(profile.status, 401);
    profile.assert_no_store();
    profile.assert_no_cookie();
    profile.assert_excludes(&[&format!("{:032x}", before.user_id().0)]);
    drop(first);
    drop(second);
    db.first_pool.close().await;
    let restarted_pool = DatabaseFixture::connect(&db.url).await;
    let restarted = DatabaseFixture::listener(PgSessionStore::new(restarted_pool.clone())).await;
    let profile = restarted
        .request("GET", "/api/v1/me", &[("Cookie", replacement)], "")
        .await;
    assert_eq!(profile.status, 401);
    profile.assert_no_cookie();
    drop(restarted);
    restarted_pool.close().await;
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit disposable real PostgreSQL 16 DATABASE_URL"]
async fn pg_http_native_rotation_channel_and_epoch_are_checked_by_current_store() {
    let db = DatabaseFixture::new().await;
    let (credential, before) = db.issue(SessionChannel::NativeBearer).await;
    let server = DatabaseFixture::listener(db.first.clone()).await;
    let bearer = format!("Bearer {credential}");
    let ambient = format!("{SESSION_COOKIE}={credential}");
    let wrong_channel = server
        .request("GET", "/api/v1/me", &[("Cookie", &ambient)], "")
        .await;
    assert_eq!(wrong_channel.status, 401);
    wrong_channel.assert_no_cookie();
    let mixed = server
        .request(
            "GET",
            "/api/v1/me",
            &[("Authorization", &bearer), ("Cookie", "unrelated=ambient")],
            "",
        )
        .await;
    assert_eq!(mixed.status, 400);
    mixed.assert_no_store();
    mixed.assert_no_cookie();
    for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        let invalid = server
            .request(
                "POST",
                path,
                &[
                    ("Authorization", &bearer),
                    ("Content-Type", "application/json"),
                ],
                "[]",
            )
            .await;
        assert_eq!(invalid.status, 400);
        invalid.assert_no_store();
        invalid.assert_no_cookie();
    }
    let refreshed = server
        .request(
            "POST",
            "/api/v1/auth/refresh",
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
            "{}",
        )
        .await;
    assert_eq!(refreshed.status, 200);
    refreshed.assert_no_store();
    refreshed.assert_no_cookie();
    let replacement = refreshed.json()["credential"].as_str().unwrap().to_owned();
    assert_ne!(replacement, credential);
    let stale = server
        .request(
            "POST",
            "/api/v1/auth/refresh",
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
            "{}",
        )
        .await;
    assert!(matches!(stale.status, 401 | 409));
    stale.assert_no_cookie();
    stale.assert_excludes(&[&replacement]);
    let current_bearer = format!("Bearer {replacement}");
    let profile = server
        .request(
            "GET",
            "/api/v1/me",
            &[("Authorization", &current_bearer)],
            "",
        )
        .await;
    assert_eq!(profile.status, 200);
    assert_eq!(
        profile.json()["account_id"],
        format!("{:032x}", before.user_id().0)
    );
    db.second
        .invalidate_account_epoch(before.user_id(), before.authorization_epoch())
        .await
        .unwrap();
    let profile = server
        .request(
            "GET",
            "/api/v1/me",
            &[("Authorization", &current_bearer)],
            "",
        )
        .await;
    assert_eq!(profile.status, 401);
    profile.assert_no_store();
    profile.assert_no_cookie();
    drop(server);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit disposable real PostgreSQL 16 DATABASE_URL"]
async fn pg_http_database_failure_is_unavailable_and_never_deletes_cookie() {
    let db = DatabaseFixture::new().await;
    let (credential, before) = db.issue(SessionChannel::BrowserCookie).await;
    let first = DatabaseFixture::listener(db.first.clone()).await;
    let second = DatabaseFixture::listener(db.second.clone()).await;
    let cookie = format!("{SESSION_COOKIE}={credential}");
    let context = first
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
    db.first_pool.close().await;
    let context = first
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(context.status, 503);
    assert_eq!(context.json()["disposition"], "unavailable");
    assert!(context.json()["account_id"].is_null());
    context.assert_no_store();
    context.assert_no_cookie();
    for path in ["/api/v1/me", "/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        let method = if path == "/api/v1/me" { "GET" } else { "POST" };
        let response = first
            .request(
                method,
                path,
                &[
                    ("Cookie", &cookie),
                    ("Origin", TRUSTED_ORIGIN),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(response.status, 503);
        response.assert_no_store();
        response.assert_no_cookie();
        assert_eq!(response.json()["code"], "unavailable");
        response.assert_excludes(&["postgres", "SELECT", &credential, &csrf, &db.url]);
    }
    let still_live = second
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(still_live.status, 200);
    assert_eq!(
        still_live.json()["account_id"],
        format!("{:032x}", before.user_id().0)
    );
    drop(first);
    drop(second);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit disposable real PostgreSQL 16 DATABASE_URL"]
async fn pg_http_private_body_publication_fences_independent_pool_logout_and_expires_closed() {
    for path in ["/api/v1/auth/context", "/api/v1/me"] {
        let db = DatabaseFixture::new().await;
        let (credential, before) = db.issue(SessionChannel::NativeBearer).await;
        let router = IsolatedSessionHttp::new(db.first.clone(), TRUSTED_ORIGIN)
            .unwrap()
            .router();
        let response = router
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("Authorization", format!("Bearer {credential}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let current = SessionCredential::parse(&credential).unwrap();
        let request = CredentialOperation {
            digest: current.digest(),
            channel: SessionChannel::NativeBearer,
            context: None,
        };
        let logout = {
            let second = db.second.clone();
            tokio::spawn(async move { second.revoke_credential(request).await })
        };
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(
            !logout.is_finished(),
            "logout overtook a retained real-DB publication lease"
        );
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(std::str::from_utf8(&bytes)
            .unwrap()
            .contains(&format!("{:032x}", before.user_id().0)));
        tokio::time::timeout(Duration::from_secs(15), logout)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(db.first.read_session(request).await.is_err());
        let (credential, before) = db.issue(SessionChannel::NativeBearer).await;
        let router = IsolatedSessionHttp::new(db.first.clone(), TRUSTED_ORIGIN)
            .unwrap()
            .router();
        let response = router
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("Authorization", format!("Bearer {credential}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let current = SessionCredential::parse(&credential).unwrap();
        let request = CredentialOperation {
            digest: current.digest(),
            channel: SessionChannel::NativeBearer,
            context: None,
        };
        tokio::time::sleep(Duration::from_millis(2_150)).await;
        db.second.revoke_credential(request).await.unwrap();
        if let Ok(bytes) = axum::body::to_bytes(response.into_body(), 4096).await {
            assert!(!std::str::from_utf8(&bytes)
                .unwrap()
                .contains(&format!("{:032x}", before.user_id().0)));
        }
        db.close().await;
    }
}
