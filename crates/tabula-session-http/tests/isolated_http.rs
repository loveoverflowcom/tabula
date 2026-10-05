#![cfg(all(feature = "isolated", not(target_arch = "wasm32")))]

//! Raw loopback HTTP proves the isolated wire boundary, not HTTPS or browser behavior.

#[path = "support/authority.rs"]
mod authority;
mod support;

use std::{sync::Arc, time::Duration};

use axum::{body::Body, http::Request};
use proptest::prelude::*;
use serde_json::Value;
use tabula_session::{
    HttpSessionAuthority, SessionAuthority, SessionChannel, SessionCredential, SessionError,
    SessionSnapshot,
};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tower::ServiceExt as _;

use authority::TestAuthority;
use support::{
    assert_session_cookie, WireResponse, WireServer, PREAUTH_COOKIE, SESSION_COOKIE, TRUSTED_ORIGIN,
};

async fn server(authority: TestAuthority) -> Arc<WireServer> {
    Arc::new(
        WireServer::start(
            IsolatedSessionHttp::new(authority, TRUSTED_ORIGIN)
                .unwrap()
                .router(),
        )
        .await,
    )
}

async fn browser_fixture() -> (
    TestAuthority,
    Arc<WireServer>,
    String,
    String,
    SessionSnapshot,
) {
    let authority = TestAuthority::new();
    let (credential, snapshot) = authority.fixture(SessionChannel::BrowserCookie);
    let server = server(authority.clone()).await;
    let cookie = format!("{SESSION_COOKIE}={credential}");
    let response = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(response.status, 200);
    let csrf = response.json()["csrf_token"].as_str().unwrap().to_owned();
    (authority, server, cookie, csrf, snapshot)
}

fn assert_capabilities(value: &Value, authenticated: bool) {
    assert_eq!(value["version"], 1);
    assert_eq!(value["capabilities"]["login"], false);
    assert_eq!(value["capabilities"]["register"], false);
    assert_eq!(value["capabilities"]["friends"], false);
    assert_eq!(value["capabilities"]["read_self_profile"], authenticated);
    assert_eq!(value["capabilities"].as_object().unwrap().len(), 4);
}

fn assert_problem(response: &WireResponse, code: &str) {
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(
        response.header("content-type"),
        Some("application/problem+json")
    );
    let value = response.json();
    assert_eq!(value["version"], 1);
    assert_eq!(value["status"], response.status);
    assert_eq!(value["code"], code);
    assert_eq!(value.as_object().unwrap().len(), 4);
}

// Literal WHATWG Origin partitions, independent of the constructor's parser.
const CANONICAL_HTTPS_ORIGINS: &[&str] = &[
    TRUSTED_ORIGIN,
    "https://accounts-fixture.tabula.invalid:8443",
    "https://accounts-fixture.tabula.invalid:0",
    "https://accounts-fixture.tabula.invalid:65535",
    "https://accounts-fixture.tabula.invalid.",
    "https://xn--bcher-kva.tabula.invalid",
    "https://127.0.0.1",
    "https://127.0.0.1:8443",
    "https://[::1]",
    "https://[2001:db8::1]:8443",
];

#[test]
fn configured_origin_rejects_review_port_and_case_regressions() {
    let unexpected: Vec<_> = [
        "https://accounts-fixture.tabula.invalid:abc",
        "https://accounts-fixture.tabula.invalid:999999",
        "https://accounts-fixture.tabula.invalid:",
        "https://accounts-fixture.tabula.invalid:443",
        "https://ACCOUNTS-FIXTURE.tabula.invalid",
    ]
    .into_iter()
    .filter(|origin| {
        !matches!(
            IsolatedSessionHttp::new(TestAuthority::new(), origin),
            Err(SessionError::InvalidInput)
        )
    })
    .collect();
    assert!(
        unexpected.is_empty(),
        "accepted review regressions: {unexpected:?}"
    );
}

#[test]
fn configured_origin_requires_one_canonical_https_origin() {
    for origin in [
        "http://accounts-fixture.tabula.invalid",
        "https://accounts-fixture.tabula.invalid/",
        "https://accounts-fixture.tabula.invalid/path",
        "https://user@accounts-fixture.tabula.invalid",
        "https://user:password@accounts-fixture.tabula.invalid",
        "https://@accounts-fixture.tabula.invalid",
        "https://accounts-fixture.tabula.invalid?query=1",
        "https://accounts-fixture.tabula.invalid?",
        "https://accounts-fixture.tabula.invalid#fragment",
        "https://accounts-fixture.tabula.invalid#",
        "https://",
        "https:///accounts-fixture.tabula.invalid",
        "https:accounts-fixture.tabula.invalid",
        "https://:8443",
        "https://accounts-fixture.tabula.invalid:abc",
        "https://accounts-fixture.tabula.invalid:-1",
        "https://accounts-fixture.tabula.invalid:+443",
        "https://accounts-fixture.tabula.invalid:65536",
        "https://accounts-fixture.tabula.invalid:999999",
        "https://accounts-fixture.tabula.invalid:",
        "https://accounts-fixture.tabula.invalid:8443:9",
        "https://accounts-fixture.tabula.invalid:443",
        "https://accounts-fixture.tabula.invalid:0443",
        "https://accounts-fixture.tabula.invalid:08443",
        "https://ACCOUNTS-FIXTURE.tabula.invalid",
        "HTTPS://accounts-fixture.tabula.invalid",
        "https://%61ccounts-fixture.tabula.invalid",
        "https://bücher.tabula.invalid",
        "https://xn--.tabula.invalid",
        "https://127.1",
        "https://0177.0.0.1",
        "https://0x7f000001",
        "https://2130706433",
        "https://127.0.0.1.",
        "https://256.0.0.1",
        "https://::1",
        "https://[::1",
        "https://[::1]junk",
        "https://[2001:DB8::1]",
        "https://[0:0:0:0:0:0:0:1]",
        "https://[::ffff:192.0.2.1]",
        "https://[fe80::1%25eth0]",
        "https://accounts-fixture.tabula.invalid\\evil.invalid",
        "https://accounts-fixture.tabula.invalid%2fevil.invalid",
        "https://accounts-fixture.tabula.invalid%00",
        "https://accounts-fixture.tabula.invalid,https://evil.invalid",
        "https://accounts-fixture.tabula.invalid https://evil.invalid",
        " https://accounts-fixture.tabula.invalid",
        "https://accounts-fixture.tabula.invalid ",
        "https://accounts-fixture.tabula.invalid\t",
        "https://accounts-fixture.tabula.invalid\n",
        "https://accounts-fixture.tabula.invalid\0",
        "blob:https://accounts-fixture.tabula.invalid/id",
        "data:text/plain,https://accounts-fixture.tabula.invalid",
        "file://accounts-fixture.tabula.invalid",
        "null",
        "*",
        "",
    ] {
        assert!(
            matches!(
                IsolatedSessionHttp::new(TestAuthority::new(), origin),
                Err(SessionError::InvalidInput)
            ),
            "configuration must fail before serving: {origin:?}"
        );
    }
}

#[test]
fn configured_origin_accepts_canonical_https_browser_origins() {
    for origin in CANONICAL_HTTPS_ORIGINS {
        IsolatedSessionHttp::new(TestAuthority::new(), origin)
            .unwrap_or_else(|error| panic!("canonical configuration {origin}: {error:?}"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(74),
        ..ProptestConfig::default()
    })]

    // Independent semantic generator: lowercase DNS labels, shortest decimal
    // u16 ports; default HTTPS port is omitted. No parser-derived acceptance.
    #[test]
    fn generated_canonical_origins_round_trip_without_rewriting(
        label in "[a-z][a-z0-9]{0,15}",
        port in any::<u16>(),
        trailing_dot in any::<bool>(),
    ) {
        let dot = if trailing_dot { "." } else { "" };
        let suffix = if port == 443 { String::new() } else { format!(":{port}") };
        let origin = format!("https://{label}.tabula.invalid{dot}{suffix}");
        let adapter = IsolatedSessionHttp::new(TestAuthority::new(), &origin);
        prop_assert!(adapter.is_ok(), "canonical origin rejected: {origin}");
        let reparsed = url::Url::parse(&origin).unwrap();
        prop_assert_eq!(reparsed.origin().ascii_serialization(), origin);
    }

    #[test]
    fn generated_authority_aliases_and_malformed_ports_fail_fast(
        label in "[a-z][a-z0-9]{0,15}",
        port in 65_536_u32..=u32::MAX,
        port_text in "[a-z]{1,12}",
    ) {
        let host = format!("{label}.tabula.invalid");
        for raw in [
            format!("https://{host}:{port}"),
            format!("https://{host}:{port_text}"),
            format!("https://{host}:"),
            format!("https://{host}:443"),
            format!("https://{host}:08443"),
            format!("https://{}", host.to_ascii_uppercase()),
            format!("https://@{host}"),
            format!("https://{host}\\evil.invalid"),
            format!("https://{host}\t"),
        ] {
            prop_assert!(
                matches!(IsolatedSessionHttp::new(TestAuthority::new(), &raw), Err(SessionError::InvalidInput)),
                "noncanonical/malformed configuration accepted: {raw:?}"
            );
        }
    }
}

async fn assert_origin_rejections(server: &WireServer, origin: &str, cookie: &str, csrf: &str) {
    let url_with_path = format!("{origin}/");
    let case_alias = origin.to_ascii_uppercase();
    let mut peer = url::Url::parse(origin).unwrap();
    peer.set_port(Some(8444)).unwrap();
    let other_port = peer.origin().ascii_serialization();
    let dot_peer = if origin.contains(".invalid.") {
        origin.replacen(".invalid.", ".invalid", 1)
    } else {
        origin.replacen(".invalid", ".invalid.", 1)
    };
    let mut aliases = vec![
        None,
        Some("null"),
        Some("https://foreign.tabula.invalid"),
        Some(url_with_path.as_str()),
        Some(case_alias.as_str()),
        Some(other_port.as_str()),
    ];
    if dot_peer != origin {
        aliases.push(Some(dot_peer.as_str()));
    }
    for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        for supplied_origin in &aliases {
            let mut headers = vec![
                ("Cookie", cookie),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", csrf),
            ];
            if let Some(value) = supplied_origin {
                headers.push(("Origin", value));
            }
            let rejected = server.request("POST", path, &headers, "{}").await;
            assert_eq!(rejected.status, 403, "{origin} {supplied_origin:?}");
            assert_problem(&rejected, "request_rejected");
        }
        let rejected = server
            .request(
                "POST",
                path,
                &[
                    ("Cookie", cookie),
                    ("Origin", origin),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &"a".repeat(43)),
                ],
                "{}",
            )
            .await;
        assert_eq!(rejected.status, 403, "{origin}");
        assert_problem(&rejected, "request_rejected");
    }
}

#[tokio::test]
async fn canonical_configurations_allow_only_exact_browser_origin_with_current_csrf() {
    for origin in CANONICAL_HTTPS_ORIGINS {
        let authority = TestAuthority::new();
        let (credential, snapshot) = authority.fixture(SessionChannel::BrowserCookie);
        let server = WireServer::start(
            IsolatedSessionHttp::new(authority.clone(), origin)
                .unwrap()
                .router(),
        )
        .await;
        let cookie = format!("{SESSION_COOKIE}={credential}");
        let context = server
            .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
            .await;
        assert_eq!(context.status, 200, "{origin}");
        context.assert_no_store();
        let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
        assert_origin_rejections(&server, origin, &cookie, &csrf).await;
        assert_eq!(authority.mutations(), 0);
        assert_eq!(
            authority.snapshot(snapshot.id()).credential_generation(),
            snapshot.credential_generation()
        );
        let refreshed = server
            .request(
                "POST",
                "/api/v1/auth/refresh",
                &[
                    ("Cookie", &cookie),
                    ("Origin", origin),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(refreshed.status, 204, "{origin}");
        refreshed.assert_no_store();
        assert_session_cookie(refreshed.header("set-cookie").unwrap(), SESSION_COOKIE);
        assert_eq!(authority.mutations(), 1);
        let replacement_cookie = refreshed.cookie(SESSION_COOKIE).unwrap();
        let logged_out = server
            .request(
                "POST",
                "/api/v1/auth/logout",
                &[
                    ("Cookie", &replacement_cookie),
                    ("Origin", origin),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(logged_out.status, 204, "{origin}");
        logged_out.assert_no_store();
        assert_eq!(authority.mutations(), 2);
    }
}

#[tokio::test]
async fn signed_out_bootstrap_is_independent_random_and_cannot_authorize_profile() {
    let authority = TestAuthority::new();
    let server = server(authority.clone()).await;
    let first = server.request("GET", "/api/v1/auth/context", &[], "").await;
    let second = server.request("GET", "/api/v1/auth/context", &[], "").await;
    for response in [&first, &second] {
        assert_eq!(response.status, 200);
        response.assert_no_store();
        assert_capabilities(&response.json(), false);
        assert_eq!(response.json()["disposition"], "signed_out");
        assert!(response.json()["account_id"].is_null());
        assert_session_cookie(response.header("set-cookie").unwrap(), PREAUTH_COOKIE);
        assert!(response.cookie(SESSION_COOKIE).is_none());
    }
    let cookie = first.cookie(PREAUTH_COOKIE).unwrap();
    let csrf = first.json()["csrf_token"].as_str().unwrap().to_owned();
    let cookie_value = cookie.split_once('=').unwrap().1;
    assert_ne!(cookie_value, csrf);
    assert_ne!(first.cookie(PREAUTH_COOKIE), second.cookie(PREAUTH_COOKIE));
    assert_ne!(first.json()["csrf_token"], second.json()["csrf_token"]);
    let reused = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(reused.json()["csrf_token"], csrf);
    let profile = server
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(profile.status, 401);
    assert_problem(&profile, "unauthenticated");
    let forged = format!("Bearer {cookie_value}");
    let profile = server
        .request("GET", "/api/v1/me", &[("Authorization", &forged)], "")
        .await;
    assert_ne!(profile.status, 200);
    profile.assert_no_store();
    profile.assert_no_cookie();
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn browser_context_and_profile_expose_only_current_self_identity() {
    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    let response = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(response.status, 200);
    response.assert_no_store();
    response.assert_no_cookie();
    let value = response.json();
    assert_eq!(value["disposition"], "authenticated");
    assert_eq!(value["account_id"], authority.account_id());
    assert_eq!(value["csrf_token"], csrf);
    assert_capabilities(&value, true);
    assert_eq!(value.as_object().unwrap().len(), 5);
    response.assert_excludes(&[
        cookie.split_once('=').unwrap().1,
        "credential_digest",
        "provider",
    ]);
    let hinted = server
        .request(
            "GET",
            "/api/v1/me?account_id=someone-else&subject=other",
            &[("Cookie", &cookie)],
            "",
        )
        .await;
    assert_eq!(hinted.status, 400);
    hinted.assert_no_store();
    hinted.assert_no_cookie();
    let profile = server
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(profile.status, 200);
    profile.assert_no_store();
    profile.assert_no_cookie();
    assert_eq!(profile.json()["account_id"], authority.account_id());
    assert_eq!(profile.json().as_object().unwrap().len(), 2);
    profile.assert_excludes(&[
        &csrf,
        cookie.split_once('=').unwrap().1,
        "subject",
        "generation",
    ]);
    assert_eq!(
        authority.snapshot(snapshot.id()).last_activity_at(),
        snapshot.last_activity_at()
    );
}

#[tokio::test]
async fn native_context_has_no_browser_token_or_cookie_and_self_profile_is_current() {
    let authority = TestAuthority::new();
    let (credential, _) = authority.fixture(SessionChannel::NativeBearer);
    let server = server(authority.clone()).await;
    let bearer = format!("Bearer {credential}");
    let context = server
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Authorization", &bearer)],
            "",
        )
        .await;
    assert_eq!(context.status, 200);
    context.assert_no_store();
    context.assert_no_cookie();
    assert_eq!(context.json()["disposition"], "authenticated");
    assert_eq!(context.json()["account_id"], authority.account_id());
    assert!(context.json()["csrf_token"].is_null());
    assert_capabilities(&context.json(), true);
    context.assert_excludes(&[&credential]);
    let profile = server
        .request("GET", "/api/v1/me", &[("Authorization", &bearer)], "")
        .await;
    assert_eq!(profile.status, 200);
    assert_eq!(profile.json()["account_id"], authority.account_id());
    profile.assert_no_store();
    profile.assert_no_cookie();
}

#[tokio::test]
async fn credentials_are_channel_bound_and_ambiguous_transport_never_selects_identity() {
    let (authority, server, browser_cookie, _, snapshot) = browser_fixture().await;
    let browser_credential = browser_cookie.split_once('=').unwrap().1;
    let (native, _) = authority.fixture(SessionChannel::NativeBearer);
    let native_bearer = format!("Bearer {native}");
    let browser_bearer = format!("Bearer {browser_credential}");
    let native_cookie = format!("{SESSION_COOKIE}={native}");
    let duplicate_cookie = format!("{browser_cookie}; {browser_cookie}");
    for headers in [
        vec![("Authorization", browser_bearer.as_str())],
        vec![("Cookie", native_cookie.as_str())],
        vec![
            ("Cookie", browser_cookie.as_str()),
            ("Authorization", native_bearer.as_str()),
        ],
        vec![
            ("Cookie", "analytics=ambient"),
            ("Authorization", native_bearer.as_str()),
        ],
        vec![("Cookie", duplicate_cookie.as_str())],
        vec![
            ("Cookie", browser_cookie.as_str()),
            ("Cookie", browser_cookie.as_str()),
        ],
        vec![
            ("Authorization", native_bearer.as_str()),
            ("Authorization", native_bearer.as_str()),
        ],
        vec![("Authorization", "Basic fixture")],
        vec![("Authorization", "Bearer malformed")],
        vec![("Authorization", "")],
    ] {
        let response = server.request("GET", "/api/v1/me", &headers, "").await;
        assert!(matches!(response.status, 400 | 401 | 403), "{response:?}");
        response.assert_no_store();
        response.assert_no_cookie();
        response.assert_excludes(&[browser_credential, &native, &authority.account_id()]);
    }
    assert_eq!(authority.mutations(), 0);
    assert_eq!(
        authority.snapshot(snapshot.id()).credential_generation(),
        snapshot.credential_generation()
    );
}

#[tokio::test]
async fn browser_unsafe_requests_require_exact_origin_json_csrf_and_fetch_metadata() {
    let (authority, server, cookie, csrf, _) = browser_fixture().await;
    let cases = [
        (None, Some("application/json"), Some(csrf.as_str()), None),
        (
            Some("null"),
            Some("application/json"),
            Some(csrf.as_str()),
            None,
        ),
        (
            Some("https://foreign.tabula.invalid"),
            Some("application/json"),
            Some(csrf.as_str()),
            None,
        ),
        (
            Some("http://accounts-fixture.tabula.invalid"),
            Some("application/json"),
            Some(csrf.as_str()),
            None,
        ),
        (
            Some("https://accounts-fixture.tabula.invalid:443"),
            Some("application/json"),
            Some(csrf.as_str()),
            None,
        ),
        (Some(TRUSTED_ORIGIN), None, Some(csrf.as_str()), None),
        (
            Some(TRUSTED_ORIGIN),
            Some("text/plain"),
            Some(csrf.as_str()),
            None,
        ),
        (
            Some(TRUSTED_ORIGIN),
            Some("application/x-www-form-urlencoded"),
            Some(csrf.as_str()),
            None,
        ),
        (Some(TRUSTED_ORIGIN), Some("application/json"), None, None),
        (
            Some(TRUSTED_ORIGIN),
            Some("application/json"),
            Some("not-the-token"),
            None,
        ),
        (
            Some(TRUSTED_ORIGIN),
            Some("application/json"),
            Some(csrf.as_str()),
            Some("cross-site"),
        ),
    ];
    for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        for (origin, content_type, token, fetch_site) in cases {
            let mut headers = vec![("Cookie", cookie.as_str())];
            if let Some(value) = origin {
                headers.push(("Origin", value));
            }
            if let Some(value) = content_type {
                headers.push(("Content-Type", value));
            }
            if let Some(value) = token {
                headers.push(("X-Tabula-CSRF", value));
            }
            if let Some(value) = fetch_site {
                headers.push(("Sec-Fetch-Site", value));
            }
            let response = server.request("POST", path, &headers, "{}").await;
            assert!(
                matches!(response.status, 400 | 403 | 415),
                "{path} {response:?}"
            );
            response.assert_no_store();
            response.assert_no_cookie();
        }
    }
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn unsafe_empty_object_contract_rejects_hostile_and_oversized_json_before_effects() {
    let (authority, server, cookie, csrf, _) = browser_fixture().await;
    let oversized = format!("{{{} }}", " ".repeat(1024));
    for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        for body in [
            "",
            "null",
            "[]",
            "true",
            "0",
            "\"\"",
            "{",
            "{}{}",
            "{} trailing",
            "{\"account_id\":\"other\"}",
            "{\"subject\":\"other\"}",
            "{\"session_id\":1}",
            "{\"x\":1,\"x\":2}",
            oversized.as_str(),
        ] {
            let response = server
                .request(
                    "POST",
                    path,
                    &[
                        ("Cookie", &cookie),
                        ("Origin", TRUSTED_ORIGIN),
                        ("Content-Type", "application/json"),
                        ("X-Tabula-CSRF", &csrf),
                    ],
                    body,
                )
                .await;
            assert!(matches!(response.status, 400 | 413), "{path} {response:?}");
            response.assert_no_store();
            response.assert_no_cookie();
        }
    }
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn duplicate_security_headers_and_wrong_context_tokens_are_rejected() {
    let (authority, server, cookie, csrf, _) = browser_fixture().await;
    let (other_credential, _) = authority.fixture(SessionChannel::BrowserCookie);
    let other_cookie = format!("{SESSION_COOKIE}={other_credential}");
    let other_context = server
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Cookie", &other_cookie)],
            "",
        )
        .await;
    let other_csrf = other_context.json()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    for extra in [
        ("Origin", TRUSTED_ORIGIN),
        ("Content-Type", "application/json"),
        ("X-Tabula-CSRF", csrf.as_str()),
        ("Sec-Fetch-Site", "same-origin"),
    ] {
        let response = server
            .request(
                "POST",
                "/api/v1/auth/logout",
                &[
                    ("Cookie", &cookie),
                    ("Origin", TRUSTED_ORIGIN),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                    ("Sec-Fetch-Site", "same-origin"),
                    extra,
                ],
                "{}",
            )
            .await;
        assert!(matches!(response.status, 400 | 403), "{response:?}");
        response.assert_no_store();
        response.assert_no_cookie();
    }
    let response = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", &other_csrf),
            ],
            "{}",
        )
        .await;
    assert!(matches!(response.status, 400 | 403), "{response:?}");
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn supplied_native_origins_and_all_preflight_requests_fail_closed() {
    let authority = TestAuthority::new();
    let (credential, _) = authority.fixture(SessionChannel::NativeBearer);
    let server = server(authority.clone()).await;
    let bearer = format!("Bearer {credential}");
    for origin in [
        "null",
        "https://foreign.tabula.invalid",
        "http://accounts-fixture.tabula.invalid",
    ] {
        for path in ["/api/v1/me", "/api/v1/auth/refresh", "/api/v1/auth/logout"] {
            let method = if path == "/api/v1/me" { "GET" } else { "POST" };
            let response = server
                .request(
                    method,
                    path,
                    &[
                        ("Authorization", &bearer),
                        ("Origin", origin),
                        ("Content-Type", "application/json"),
                        ("X-Forwarded-Host", "accounts-fixture.tabula.invalid"),
                        ("X-Forwarded-Proto", "https"),
                    ],
                    "{}",
                )
                .await;
            assert!(matches!(response.status, 400 | 403));
            response.assert_no_store();
            response.assert_no_cookie();
        }
    }
    for origin in [TRUSTED_ORIGIN, "https://foreign.tabula.invalid"] {
        let response = server
            .request(
                "OPTIONS",
                "/api/v1/auth/refresh",
                &[
                    ("Origin", origin),
                    ("Access-Control-Request-Method", "POST"),
                    (
                        "Access-Control-Request-Headers",
                        "authorization,x-tabula-csrf",
                    ),
                ],
                "",
            )
            .await;
        assert!(matches!(response.status, 403 | 405));
        response.assert_no_store();
        response.assert_no_cookie();
    }
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn browser_refresh_rotates_only_verifier_preserves_csrf_and_stale_response_sets_no_cookie() {
    let (authority, server, cookie, csrf, before) = browser_fixture().await;
    authority.set_time(before.created_at().get() + 100);
    let response = server
        .request(
            "POST",
            "/api/v1/auth/refresh",
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json; charset=utf-8"),
                ("X-Tabula-CSRF", &csrf),
            ],
            "{}",
        )
        .await;
    assert_eq!(response.status, 204, "{response:?}");
    response.assert_no_store();
    assert!(response.body.is_empty());
    assert_eq!(response.headers("set-cookie").len(), 1);
    assert_session_cookie(response.header("set-cookie").unwrap(), SESSION_COOKIE);
    let replacement = response.cookie(SESSION_COOKIE).unwrap();
    assert_ne!(replacement, cookie);
    let after = authority.snapshot(before.id());
    assert_eq!(after.id(), before.id());
    assert_eq!(after.user_id(), before.user_id());
    assert_eq!(after.context_id(), before.context_id());
    assert_eq!(after.authorization_epoch(), before.authorization_epoch());
    assert_eq!(
        after.credential_generation().get(),
        before.credential_generation().get() + 1
    );
    assert_eq!(after.last_activity_at(), before.last_activity_at());
    assert_eq!(after.idle_deadline(), before.idle_deadline());
    assert_eq!(after.absolute_deadline(), before.absolute_deadline());
    let stale = server
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
        .await;
    assert!(matches!(stale.status, 401 | 409));
    stale.assert_no_store();
    stale.assert_no_cookie();
    let context = server
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Cookie", &replacement)],
            "",
        )
        .await;
    assert_eq!(context.json()["csrf_token"], csrf);
    assert_eq!(context.json()["account_id"], authority.account_id());
    assert_eq!(authority.mutations(), 1);
}

#[tokio::test]
async fn native_refresh_needs_only_explicit_bearer_and_releases_credential_only_on_success() {
    let authority = TestAuthority::new();
    let (credential, before) = authority.fixture(SessionChannel::NativeBearer);
    let server = server(authority.clone()).await;
    let bearer = format!("Bearer {credential}");
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
    assert_eq!(refreshed.status, 200, "{refreshed:?}");
    refreshed.assert_no_store();
    refreshed.assert_no_cookie();
    let replacement = refreshed.json()["credential"].as_str().unwrap().to_owned();
    assert_ne!(replacement, credential);
    assert!(SessionCredential::parse(&replacement).is_ok());
    assert_eq!(refreshed.json().as_object().unwrap().len(), 2);
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
    stale.assert_no_store();
    stale.assert_no_cookie();
    stale.assert_excludes(&[&replacement, &credential, "credential"]);
    let new_bearer = format!("Bearer {replacement}");
    let profile = server
        .request("GET", "/api/v1/me", &[("Authorization", &new_bearer)], "")
        .await;
    assert_eq!(profile.status, 200);
    assert_eq!(profile.json()["account_id"], authority.account_id());
    let after = authority.snapshot(before.id());
    assert_eq!(after.idle_deadline(), before.idle_deadline());
    assert_eq!(after.absolute_deadline(), before.absolute_deadline());
    assert_eq!(after.last_activity_at(), before.last_activity_at());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_duplicate_refresh_has_one_wire_winner_and_no_stale_cookie_deletion() {
    let (authority, server, cookie, csrf, before) = browser_fixture().await;
    let mut requests = Vec::new();
    for _ in 0..12 {
        let server = server.clone();
        let cookie = cookie.clone();
        let csrf = csrf.clone();
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
    let mut winners = 0;
    for request in requests {
        let response = request.await.unwrap();
        response.assert_no_store();
        if response.status == 204 {
            winners += 1;
            assert_eq!(response.headers("set-cookie").len(), 1);
            assert_session_cookie(response.header("set-cookie").unwrap(), SESSION_COOKIE);
        } else {
            assert!(matches!(response.status, 401 | 409), "{response:?}");
            response.assert_no_cookie();
        }
    }
    assert_eq!(winners, 1);
    assert_eq!(authority.mutations(), 1);
    assert_eq!(
        authority
            .snapshot(before.id())
            .credential_generation()
            .get(),
        1
    );
}

#[tokio::test]
async fn logout_reports_success_after_revocation_and_clears_only_browser_cookie() {
    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    let response = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", &csrf),
            ],
            "{}",
        )
        .await;
    assert_eq!(response.status, 204, "{response:?}");
    response.assert_no_store();
    assert!(authority.snapshot(snapshot.id()).revoked_at().is_some());
    let deleted = response.header("set-cookie").unwrap();
    assert!(deleted.starts_with(&format!("{SESSION_COOKIE}=;")));
    for attribute in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/", "Max-Age=0"] {
        assert!(deleted.contains(attribute), "{deleted}");
    }
    assert!(!deleted.to_ascii_lowercase().contains("domain"));
    let profile = server
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(profile.status, 401);
    assert_problem(&profile, "unauthenticated");
    let context = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(context.json()["disposition"], "signed_out");
    assert_capabilities(&context.json(), false);
}

#[tokio::test]
async fn unavailable_is_distinct_and_failed_mutations_emit_no_credential_or_cookie_deletion() {
    let (authority, server, cookie, csrf, _) = browser_fixture().await;
    authority.set_failure(Some(SessionError::Unavailable));
    let context = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(context.status, 503);
    context.assert_no_store();
    context.assert_no_cookie();
    assert_eq!(context.json()["disposition"], "unavailable");
    assert!(context.json()["account_id"].is_null());
    assert!(context.json()["csrf_token"].is_null());
    assert_capabilities(&context.json(), false);
    for path in ["/api/v1/me", "/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        let method = if path == "/api/v1/me" { "GET" } else { "POST" };
        let response = server
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
        assert_problem(&response, "unavailable");
        response.assert_excludes(&[
            &authority.account_id(),
            &csrf,
            cookie.split_once('=').unwrap().1,
        ]);
    }
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn provider_and_social_routes_remain_generic_unavailable_and_never_mint_a_fixture_session() {
    let authority = TestAuthority::new();
    let server = server(authority.clone()).await;
    for path in [
        "/api/v1/auth/login",
        "/api/v1/auth/register",
        "/api/v1/friends",
    ] {
        for body in [
            "{}",
            "{\"subject\":\"known-fixture\"}",
            "{\"subject\":\"unknown-fixture\"}",
        ] {
            let method = if path == "/api/v1/friends" {
                "GET"
            } else {
                "POST"
            };
            let response = server
                .request(
                    method,
                    path,
                    &[
                        ("Origin", TRUSTED_ORIGIN),
                        ("Content-Type", "application/json"),
                    ],
                    body,
                )
                .await;
            assert_eq!(response.status, 503, "{path} {response:?}");
            assert_problem(&response, "unavailable");
            response.assert_excludes(&["known-fixture", "unknown-fixture", "provider", "subject"]);
        }
    }
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn reads_refresh_and_clock_regression_cannot_extend_or_resurrect_expired_authority() {
    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    authority.set_time(snapshot.idle_deadline().get());
    let response = server
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
        .await;
    assert_eq!(response.status, 401);
    response.assert_no_store();
    response.assert_no_cookie();
    assert!(authority.snapshot(snapshot.id()).expired_at().is_some());
    authority.set_time(snapshot.created_at().get());
    let response = server
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(response.status, 503);
    assert_problem(&response, "unavailable");
    assert_eq!(
        authority.snapshot(snapshot.id()).idle_deadline(),
        snapshot.idle_deadline()
    );
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_context_and_profile_recheck_credential_after_logout_orders_first() {
    for path in ["/api/v1/auth/context", "/api/v1/me"] {
        let (authority, server, cookie, csrf, _) = browser_fixture().await;
        let gate = authority.pause_next_operation();
        let pending = {
            let server = server.clone();
            let cookie = cookie.clone();
            tokio::spawn(async move {
                server
                    .request("GET", path, &[("Cookie", &cookie)], "")
                    .await
            })
        };
        gate.wait_entered().await;
        let logout = server
            .request(
                "POST",
                "/api/v1/auth/logout",
                &[
                    ("Cookie", &cookie),
                    ("Origin", TRUSTED_ORIGIN),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(logout.status, 204);
        gate.release.add_permits(1);
        let response = pending.await.unwrap();
        response.assert_no_store();
        response.assert_excludes(&[&authority.account_id(), &csrf]);
        if path == "/api/v1/auth/context" {
            assert_eq!(response.json()["disposition"], "signed_out");
        } else {
            assert_eq!(response.status, 401);
            response.assert_no_cookie();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_private_body_fences_logout_until_first_frame_and_expired_lease_never_publishes() {
    for path in ["/api/v1/auth/context", "/api/v1/me"] {
        let authority = TestAuthority::new();
        let (credential, snapshot) = authority.fixture(SessionChannel::NativeBearer);
        let router = IsolatedSessionHttp::new(authority.clone(), TRUSTED_ORIGIN)
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
        let digest = SessionCredential::parse(&credential).unwrap().digest();
        let logout = {
            let authority = authority.clone();
            tokio::spawn(async move {
                authority
                    .revoke_credential(tabula_session::CredentialOperation {
                        digest,
                        channel: SessionChannel::NativeBearer,
                        context: None,
                    })
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(
            !logout.is_finished(),
            "logout overtook a retained publication guard"
        );
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(std::str::from_utf8(&bytes)
            .unwrap()
            .contains(&authority.account_id()));
        logout.await.unwrap().unwrap();
        assert!(authority.snapshot(snapshot.id()).revoked_at().is_some());

        let (credential, _) = authority.fixture(SessionChannel::NativeBearer);
        let router = IsolatedSessionHttp::new(authority.clone(), TRUSTED_ORIGIN)
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
        tokio::time::sleep(Duration::from_millis(2_100)).await;
        let bytes = axum::body::to_bytes(response.into_body(), 4096).await;
        if let Ok(bytes) = bytes {
            assert!(!std::str::from_utf8(&bytes)
                .unwrap()
                .contains(&authority.account_id()));
        }
    }
}

#[tokio::test]
async fn epoch_invalidation_rejects_old_context_and_profile() {
    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    authority
        .invalidate_account_epoch(snapshot.user_id(), snapshot.authorization_epoch())
        .await
        .unwrap();
    let profile = server
        .request("GET", "/api/v1/me", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(profile.status, 401);
    assert_problem(&profile, "unauthenticated");
    let refresh = server
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
        .await;
    assert_eq!(refresh.status, 401);
    refresh.assert_no_cookie();
    refresh.assert_no_store();
}

#[tokio::test]
async fn repeated_current_logout_clears_cookie_but_rotated_old_verifier_cannot_revoke() {
    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    for _ in 0..2 {
        let response = server
            .request(
                "POST",
                "/api/v1/auth/logout",
                &[
                    ("Cookie", &cookie),
                    ("Origin", TRUSTED_ORIGIN),
                    ("Content-Type", "application/json"),
                    ("X-Tabula-CSRF", &csrf),
                ],
                "{}",
            )
            .await;
        assert_eq!(response.status, 204, "{response:?}");
        response.assert_no_store();
        assert!(response.header("set-cookie").unwrap().contains("Max-Age=0"));
    }
    assert!(authority.snapshot(snapshot.id()).revoked_at().is_some());

    let (authority, server, cookie, csrf, snapshot) = browser_fixture().await;
    let refreshed = server
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
        .await;
    assert_eq!(refreshed.status, 204);
    let stale_logout = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", &csrf),
            ],
            "{}",
        )
        .await;
    assert_eq!(stale_logout.status, 401);
    stale_logout.assert_no_store();
    stale_logout.assert_no_cookie();
    assert!(authority.snapshot(snapshot.id()).revoked_at().is_none());
}

#[tokio::test]
async fn repeated_context_id_does_not_reuse_another_sessions_synchronizer_token() {
    let (authority, server, _cookie, csrf, snapshot) = browser_fixture().await;
    let (credential, _) =
        authority.fixture_with_context(SessionChannel::BrowserCookie, Some(snapshot.context_id()));
    let other_cookie = format!("{SESSION_COOKIE}={credential}");
    let other = server
        .request(
            "GET",
            "/api/v1/auth/context",
            &[("Cookie", &other_cookie)],
            "",
        )
        .await;
    assert_eq!(other.status, 200);
    assert_ne!(other.json()["csrf_token"], csrf);
    let response = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Cookie", &other_cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", &csrf),
            ],
            "{}",
        )
        .await;
    assert_eq!(response.status, 403);
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn unauthenticated_bootstrap_is_rate_bounded_without_minting_authority() {
    let authority = TestAuthority::new();
    let server = server(authority.clone()).await;
    for _ in 0..120 {
        let response = server.request("GET", "/api/v1/auth/context", &[], "").await;
        assert_eq!(response.status, 200);
        response.assert_no_store();
        assert_eq!(response.json()["disposition"], "signed_out");
    }
    let limited = server.request("GET", "/api/v1/auth/context", &[], "").await;
    assert_eq!(limited.status, 429);
    assert_problem(&limited, "rate_limited");
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn unfinished_request_body_times_out_before_any_authority_effect() {
    let (authority, server, cookie, csrf, _) = browser_fixture().await;
    let raw = format!(
        "POST /api/v1/auth/refresh HTTP/1.1\r\nHost: unrelated.invalid\r\nConnection: close\r\nCookie: {cookie}\r\nOrigin: {TRUSTED_ORIGIN}\r\nContent-Type: application/json\r\nX-Tabula-CSRF: {csrf}\r\nContent-Length: 2\r\n\r\n{{"
    );
    let response = server.raw_request(&raw).await;
    assert_eq!(response.status, 408);
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn native_unsafe_shape_and_exact_body_bound_apply_without_a_browser_token() {
    let authority = TestAuthority::new();
    let (credential, _) = authority.fixture(SessionChannel::NativeBearer);
    let server = server(authority.clone()).await;
    let bearer = format!("Bearer {credential}");
    for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
        let response = server
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
        assert_eq!(response.status, 400);
        response.assert_no_store();
        response.assert_no_cookie();
    }
    assert_eq!(authority.mutations(), 0);
    let exactly_limit = format!("{}{{}}", " ".repeat(1022));
    assert_eq!(exactly_limit.len(), 1024);
    let response = server
        .request(
            "POST",
            "/api/v1/auth/refresh",
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
                ("Origin", TRUSTED_ORIGIN),
            ],
            &exactly_limit,
        )
        .await;
    assert_eq!(response.status, 200);
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(authority.mutations(), 1);
}

#[tokio::test]
async fn failed_or_indeterminate_mutation_ack_never_publishes_a_replacement_or_deletion() {
    for channel in [SessionChannel::BrowserCookie, SessionChannel::NativeBearer] {
        for path in ["/api/v1/auth/refresh", "/api/v1/auth/logout"] {
            for committed in [false, true] {
                let authority = TestAuthority::new();
                let (credential, snapshot) = authority.fixture(channel);
                let server = server(authority.clone()).await;
                let attached = if channel == SessionChannel::BrowserCookie {
                    format!("{SESSION_COOKIE}={credential}")
                } else {
                    format!("Bearer {credential}")
                };
                let header = if channel == SessionChannel::BrowserCookie {
                    "Cookie"
                } else {
                    "Authorization"
                };
                let context = server
                    .request("GET", "/api/v1/auth/context", &[(header, &attached)], "")
                    .await;
                let token = context.json()["csrf_token"].as_str().map(str::to_owned);
                authority.set_mutation_fault(SessionError::Unavailable, committed);
                let mut headers = vec![
                    (header, attached.as_str()),
                    ("Content-Type", "application/json"),
                ];
                if let Some(token) = token.as_deref() {
                    headers.push(("Origin", TRUSTED_ORIGIN));
                    headers.push(("X-Tabula-CSRF", token));
                }
                let response = server.request("POST", path, &headers, "{}").await;
                assert_eq!(response.status, 503);
                assert_problem(&response, "unavailable");
                response.assert_excludes(&[&credential, "credential", &authority.account_id()]);
                assert_eq!(authority.mutations(), usize::from(committed));
                let after = authority.snapshot(snapshot.id());
                if path == "/api/v1/auth/refresh" {
                    assert_eq!(after.credential_generation().get(), u64::from(committed));
                } else {
                    assert_eq!(after.revoked_at().is_some(), committed);
                }
            }
        }
    }
}

#[tokio::test]
async fn logout_only_revokes_current_device_and_native_logout_never_sets_a_cookie() {
    let (authority, server, cookie, csrf, browser) = browser_fixture().await;
    let (native, native_snapshot) = authority.fixture(SessionChannel::NativeBearer);
    let logout = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", &csrf),
            ],
            "{}",
        )
        .await;
    assert_eq!(logout.status, 204);
    assert!(authority.snapshot(browser.id()).revoked_at().is_some());
    assert!(authority
        .snapshot(native_snapshot.id())
        .revoked_at()
        .is_none());
    let bearer = format!("Bearer {native}");
    let profile = server
        .request("GET", "/api/v1/me", &[("Authorization", &bearer)], "")
        .await;
    assert_eq!(profile.status, 200);
    let logout = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
            "{}",
        )
        .await;
    assert_eq!(logout.status, 204);
    logout.assert_no_store();
    logout.assert_no_cookie();
    assert!(authority
        .snapshot(native_snapshot.id())
        .revoked_at()
        .is_some());
}
