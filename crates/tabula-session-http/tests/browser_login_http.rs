#![cfg(all(feature = "isolated", not(target_arch = "wasm32")))]
//! Actual loopback HTTP with explicit provider/authority doubles. This proves
//! transport/order/failure behavior, not Kanidm, TLS or browser cookie handling.
#[path = "support/authority.rs"]
#[allow(dead_code)]
mod authority;
#[allow(dead_code)]
mod support;

use authority::{OperationGate, TestAuthority};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use support::{WireResponse, WireServer, PREAUTH_COOKIE, SESSION_COOKIE, TRUSTED_ORIGIN};
use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, BrowserLoginCallback, BrowserLoginProvider, BrowserLoginStart,
    CompletedBrowserLogin, ProviderIdentityKey, SessionAuthority, SessionChannel, SessionError,
};
use tabula_session_http::isolated::IsolatedSessionHttp;

#[derive(Default)]
struct ProviderState {
    pending: BTreeMap<String, String>,
    serial: usize,
    begin_gate: Option<Arc<OperationGate>>,
    complete_gate: Option<Arc<OperationGate>>,
    failure: Option<SessionError>,
}
#[derive(Clone)]
struct SyntheticProvider {
    state: Arc<Mutex<ProviderState>>,
    cancelled: Arc<tokio::sync::Semaphore>,
    completions: Arc<AtomicUsize>,
}
impl Default for SyntheticProvider {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProviderState::default())),
            cancelled: Arc::new(tokio::sync::Semaphore::new(0)),
            completions: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl SyntheticProvider {
    fn gate(&self, begin: bool) -> Arc<OperationGate> {
        let gate = OperationGate::new();
        if begin {
            self.state.lock().unwrap().begin_gate = Some(gate.clone());
        } else {
            self.state.lock().unwrap().complete_gate = Some(gate.clone());
        }
        gate
    }
    async fn wait_cancelled(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), self.cancelled.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
    }
}
async fn pause(gate: Option<Arc<OperationGate>>) {
    if let Some(gate) = gate {
        gate.entered.add_permits(1);
        gate.release.acquire().await.unwrap().forget();
    }
}
impl BrowserLoginProvider for SyntheticProvider {
    async fn begin(&self, binding: String) -> Result<BrowserLoginStart, SessionError> {
        let (state, gate) = {
            let mut inner = self.state.lock().unwrap();
            if let Some(error) = inner.failure {
                return Err(error);
            }
            inner.serial += 1;
            let state = format!("synthetic-state-{}", inner.serial);
            inner.pending.insert(binding, state.clone());
            (state, inner.begin_gate.take())
        };
        pause(gate).await;
        Ok(BrowserLoginStart {
            authorization_url: format!("https://kanidm.invalid/oauth2/authorise?state={state}"),
        })
    }
    async fn complete(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
    ) -> Result<CompletedBrowserLogin, SessionError> {
        self.completions.fetch_add(1, Ordering::SeqCst);
        let gate = {
            let mut inner = self.state.lock().unwrap();
            let state = inner
                .pending
                .remove(&binding)
                .ok_or(SessionError::InvalidInput)?;
            if callback.state() != state
                || callback.code() != "synthetic-code"
                || callback
                    .issuer()
                    .is_some_and(|issuer| issuer != "https://kanidm.invalid")
            {
                return Err(SessionError::InvalidInput);
            }
            if let Some(error) = inner.failure {
                return Err(error);
            }
            inner.complete_gate.take()
        };
        pause(gate).await;
        Ok(CompletedBrowserLogin {
            identity: ProviderIdentityKey::new("https://kanidm.invalid", "synthetic-subject")
                .unwrap(),
            expected_epoch: AccountEpoch::new(0).unwrap(),
        })
    }
    fn matches_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .pending
            .get(binding)
            .is_some_and(|state| state == callback.state()))
    }
    fn cancel(&self, binding: &str) {
        self.state.lock().unwrap().pending.remove(binding);
        self.cancelled.add_permits(1);
    }
}
async fn fixture() -> (
    TestAuthority,
    SyntheticProvider,
    Arc<WireServer>,
    String,
    String,
) {
    let authority = TestAuthority::new();
    authority.enable_browser_login();
    let provider = SyntheticProvider::default();
    let server = Arc::new(
        WireServer::start(
            IsolatedSessionHttp::new(authority.clone(), TRUSTED_ORIGIN)
                .unwrap()
                .router_with_login(provider.clone()),
        )
        .await,
    );
    let response = server.request("GET", "/api/v1/auth/context", &[], "").await;
    assert_eq!(response.status, 200);
    assert_eq!(response.json()["capabilities"]["login"], true);
    (
        authority,
        provider,
        server,
        response.cookie(PREAUTH_COOKIE).unwrap(),
        response.json()["csrf_token"].as_str().unwrap().to_owned(),
    )
}
async fn start(server: &WireServer, cookie: &str, csrf: &str) -> WireResponse {
    server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("Cookie", cookie),
                ("X-Tabula-CSRF", csrf),
            ],
            "{}",
        )
        .await
}
fn callback_url(response: &WireResponse) -> String {
    assert_eq!(response.status, 200, "{response:?}");
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(response.json().as_object().unwrap().len(), 2);
    assert_eq!(response.json()["version"], 1);
    let value = response.json();
    let url = url::Url::parse(value["authorization_url"].as_str().unwrap()).unwrap();
    let state = url.query_pairs().find(|(key, _)| key == "state").unwrap().1;
    format!("/api/v1/auth/oidc/callback?state={state}&code=synthetic-code")
}
async fn complete(server: &WireServer, cookie: &str, target: &str) -> WireResponse {
    server
        .request(
            "GET",
            target,
            &[
                ("Cookie", cookie),
                ("Sec-Fetch-Site", "cross-site"),
                ("Sec-Fetch-Mode", "navigate"),
                ("Sec-Fetch-Dest", "document"),
            ],
            "",
        )
        .await
}
async fn cancel(server: &WireServer, cookie: &str, csrf: &str) -> WireResponse {
    server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("Cookie", cookie),
                ("X-Tabula-CSRF", csrf),
            ],
            "{}",
        )
        .await
}
fn rejected(response: &WireResponse) {
    assert!(response.status >= 400, "{response:?}");
    response.assert_no_store();
    response.assert_no_cookie();
    assert_eq!(response.json().as_object().unwrap().len(), 4);
}

#[tokio::test]
async fn explicit_login_issues_cookie_only_after_single_success_and_fixed_navigation() {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    let duplicate = start(&server, &cookie, &csrf).await;
    assert_eq!(duplicate.status, 409);
    rejected(&duplicate);
    let response = complete(&server, &cookie, &target).await;
    assert_eq!(response.status, 303, "{response:?}");
    response.assert_no_store();
    assert_eq!(response.header("location"), Some("/account"));
    assert_eq!(response.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(response.body, "");
    let session = response.cookie(SESSION_COOKIE).unwrap();
    support::assert_session_cookie(
        response
            .headers("set-cookie")
            .into_iter()
            .find(|value| value.starts_with(SESSION_COOKIE))
            .unwrap(),
        SESSION_COOKIE,
    );
    assert!(response
        .headers("set-cookie")
        .iter()
        .any(|value| value.starts_with(PREAUTH_COOKIE) && value.contains("Max-Age=0")));
    let current = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &session)], "")
        .await;
    assert_eq!(current.json()["disposition"], "authenticated");
    assert_eq!(current.json()["capabilities"]["login"], false);
    let profile = server
        .request("GET", "/api/v1/me", &[("Cookie", &session)], "")
        .await;
    assert_eq!(profile.status, 200);
    let replay = complete(&server, &cookie, &target).await;
    rejected(&replay);
    rejected(&start(&server, &cookie, &csrf).await);
    assert_eq!(authority.issuance_attempts(), 1);
    assert_eq!(provider.completions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn login_rejects_origin_channel_csrf_body_and_account_switch_before_provider_work() {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let fake = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    for headers in [
        vec![
            ("Cookie", cookie.as_str()),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", csrf.as_str()),
        ],
        vec![
            ("Origin", "https://evil.invalid"),
            ("Cookie", cookie.as_str()),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", csrf.as_str()),
        ],
        vec![
            ("Origin", TRUSTED_ORIGIN),
            ("Cookie", cookie.as_str()),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", fake),
        ],
        vec![
            ("Origin", TRUSTED_ORIGIN),
            ("Cookie", cookie.as_str()),
            ("Content-Type", "text/plain"),
            ("X-Tabula-CSRF", csrf.as_str()),
        ],
        vec![
            (
                "Authorization",
                "Bearer AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
            ("Content-Type", "application/json"),
        ],
        vec![
            ("Origin", TRUSTED_ORIGIN),
            ("Cookie", cookie.as_str()),
            (
                "Authorization",
                "Bearer AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", csrf.as_str()),
        ],
        vec![
            ("Origin", TRUSTED_ORIGIN),
            ("Sec-Fetch-Site", "cross-site"),
            ("Cookie", cookie.as_str()),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", csrf.as_str()),
        ],
    ] {
        rejected(
            &server
                .request("POST", "/api/v1/auth/login", &headers, "{}")
                .await,
        );
    }
    for body in [
        "null",
        "[]",
        "{\"return_url\":\"https://evil.invalid\"}",
        "{",
        &"x".repeat(1025),
    ] {
        rejected(
            &server
                .request(
                    "POST",
                    "/api/v1/auth/login",
                    &[
                        ("Origin", TRUSTED_ORIGIN),
                        ("Cookie", &cookie),
                        ("Content-Type", "application/json"),
                        ("X-Tabula-CSRF", &csrf),
                    ],
                    body,
                )
                .await,
        );
    }
    let (session, _) = authority.fixture(SessionChannel::BrowserCookie);
    let combined = format!("{cookie}; {SESSION_COOKIE}={session}");
    let response = start(&server, &combined, &csrf).await;
    assert_eq!(response.status, 409);
    rejected(&response);
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(provider.state.lock().unwrap().serial, 0);
}

async fn assert_callback_transport_rejections(server: &WireServer, cookie: &str, target: &str) {
    rejected(
        &server
            .request(
                "GET",
                target,
                &[
                    ("Cookie", cookie),
                    ("Sec-Fetch-Mode", "cors"),
                    ("Sec-Fetch-Site", "cross-site"),
                ],
                "",
            )
            .await,
    );
    rejected(
        &server
            .request(
                "GET",
                target,
                &[
                    ("Cookie", cookie),
                    (
                        "Authorization",
                        "Bearer AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    ),
                ],
                "",
            )
            .await,
    );
    rejected(
        &server
            .request("GET", target, &[("Cookie", cookie), ("Cookie", cookie)], "")
            .await,
    );
    rejected(&complete(server, "", target).await);
    let head = server
        .request("HEAD", target, &[("Cookie", cookie)], "")
        .await;
    assert_eq!(head.status, 400);
    head.assert_no_cookie();
    head.assert_no_store();
    // Ordinary read routes remain closed to cross-origin navigation/fetch.
    rejected(
        &server
            .request(
                "GET",
                "/api/v1/auth/context",
                &[("Cookie", cookie), ("Sec-Fetch-Site", "cross-site")],
                "",
            )
            .await,
    );
}

#[tokio::test]
async fn callback_rejects_duplicate_unknown_malformed_and_fetch_inputs_without_consuming_valid_flow(
) {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    for query in [
        "",
        "state=x",
        "state=x&code=y&code=z",
        "state=x&code=y&%63ode=z",
        "state=x&code=y&iss=a&iss=b",
        "state=x&code=y&return_url=/me",
        "state=x&code=%xx",
        "state=x&code=%FF",
        "state=x&code=y&error=denied",
        "state=x&code=y&",
    ] {
        // Empty trailing pair is explicitly rejected below by the HTTP parser.
        rejected(
            &complete(
                &server,
                &cookie,
                &format!("/api/v1/auth/oidc/callback?{query}"),
            )
            .await,
        );
    }
    rejected(
        &complete(
            &server,
            &cookie,
            &format!(
                "/api/v1/auth/oidc/callback?state={}&code=y",
                "x".repeat(257)
            ),
        )
        .await,
    );
    rejected(
        &complete(
            &server,
            &cookie,
            &format!(
                "/api/v1/auth/oidc/callback?state={}&code=y",
                "x".repeat(4100)
            ),
        )
        .await,
    );
    assert_callback_transport_rejections(&server, &cookie, &target).await;
    for bogus in [
        "/api/v1/auth/oidc/callback?state=bogus&code=bogus",
        "/api/v1/auth/oidc/callback?state=bogus&code=bogus&iss=https%3A%2F%2Fevil.invalid",
    ] {
        rejected(&complete(&server, &cookie, bogus).await);
    }
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(provider.completions.load(Ordering::SeqCst), 0);
    assert_eq!(complete(&server, &cookie, &target).await.status, 303);
}

#[tokio::test]
async fn concurrent_callbacks_and_starts_have_one_winner() {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let (first, second) = tokio::join!(
        start(&server, &cookie, &csrf),
        start(&server, &cookie, &csrf)
    );
    assert_eq!(
        [first.status, second.status]
            .iter()
            .filter(|status| **status == 200)
            .count(),
        1
    );
    let target = callback_url(if first.status == 200 { &first } else { &second });
    let (first, second) = tokio::join!(
        complete(&server, &cookie, &target),
        complete(&server, &cookie, &target)
    );
    assert_eq!(
        [first.status, second.status]
            .iter()
            .filter(|status| **status == 303)
            .count(),
        1
    );
    assert_eq!(authority.issuance_attempts(), 1);
    assert_eq!(provider.completions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn logout_invalidates_actual_preauth_and_retained_provider_state() {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    let (session, _) = authority.fixture(SessionChannel::BrowserCookie);
    let combined = format!("{cookie}; {SESSION_COOKIE}={session}");
    let context = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &combined)], "")
        .await;
    let active_csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
    assert_eq!(cancel(&server, &combined, &active_csrf).await.status, 204);
    assert!(provider.state.lock().unwrap().pending.is_empty());
    rejected(&start(&server, &cookie, &csrf).await);
    rejected(&complete(&server, &cookie, &target).await);
    let fresh = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    assert_ne!(
        fresh.cookie(PREAUTH_COOKIE).as_deref(),
        Some(cookie.as_str())
    );
    assert_eq!(authority.issuance_attempts(), 0);
}

#[tokio::test]
async fn signed_out_logout_cancels_begin_and_complete_in_flight_without_issuance() {
    for during_begin in [true, false] {
        let (authority, provider, server, cookie, csrf) = fixture().await;
        let target = if during_begin {
            None
        } else {
            Some(callback_url(&start(&server, &cookie, &csrf).await))
        };
        let gate = provider.gate(during_begin);
        let active_server = server.clone();
        let active_cookie = cookie.clone();
        let active_csrf = csrf.clone();
        let active = tokio::spawn(async move {
            if let Some(target) = target {
                complete(&active_server, &active_cookie, &target).await
            } else {
                start(&active_server, &active_cookie, &active_csrf).await
            }
        });
        gate.wait_entered().await;
        let logout_server = server.clone();
        let logout_cookie = cookie.clone();
        let logout_csrf = csrf.clone();
        let logout =
            tokio::spawn(async move { cancel(&logout_server, &logout_cookie, &logout_csrf).await });
        provider.wait_cancelled().await;
        gate.release.add_permits(1);
        rejected(&active.await.unwrap());
        assert_eq!(logout.await.unwrap().status, 204);
        assert_eq!(authority.issuance_attempts(), 0);
        rejected(&start(&server, &cookie, &csrf).await);
    }
}

#[tokio::test]
async fn logout_during_durable_issuance_suppresses_cookie_and_revokes_known_commit() {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    let gate = authority.pause_next_operation();
    let active_server = server.clone();
    let active_cookie = cookie.clone();
    let active =
        tokio::spawn(async move { complete(&active_server, &active_cookie, &target).await });
    gate.wait_entered().await;
    let logout_server = server.clone();
    let logout_cookie = cookie.clone();
    let logout_csrf = csrf.clone();
    let logout =
        tokio::spawn(async move { cancel(&logout_server, &logout_cookie, &logout_csrf).await });
    provider.wait_cancelled().await;
    gate.release.add_permits(1);
    rejected(&active.await.unwrap());
    assert_eq!(logout.await.unwrap().status, 204);
    assert_eq!(authority.issuance_attempts(), 1);
    assert!(authority.mutations() >= 2);
}

#[tokio::test]
async fn callback_active_cookie_switch_and_captured_epoch_are_rejected() {
    let (authority, _, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    let (active, _) = authority.fixture(SessionChannel::BrowserCookie);
    let response = complete(
        &server,
        &format!("{cookie}; {SESSION_COOKIE}={active}"),
        &target,
    )
    .await;
    assert_eq!(response.status, 409);
    rejected(&response);
    assert_eq!(authority.issuance_attempts(), 0);
    let (authority, _, server, cookie, csrf) = fixture().await;
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    authority
        .invalidate_account_epoch(UserId(0x000a_11ce), AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    rejected(&complete(&server, &cookie, &target).await);
    assert_eq!(authority.issuance_attempts(), 1);
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn provider_authority_and_ambiguous_commit_fail_closed_without_retry_or_cookie_clear() {
    for fault in 0..4 {
        let (authority, provider, server, cookie, csrf) = fixture().await;
        let target = callback_url(&start(&server, &cookie, &csrf).await);
        match fault {
            0 => provider.state.lock().unwrap().failure = Some(SessionError::Unavailable),
            1 => authority.set_failure(Some(SessionError::Unavailable)),
            2 => authority.set_mutation_fault(SessionError::Unavailable, false),
            _ => authority.set_mutation_fault(SessionError::Unavailable, true),
        }
        let response = complete(&server, &cookie, &target).await;
        assert_eq!(response.status, 503);
        rejected(&response);
        let attempts = authority.issuance_attempts();
        rejected(&complete(&server, &cookie, &target).await);
        assert_eq!(authority.issuance_attempts(), attempts);
        assert_eq!(provider.completions.load(Ordering::SeqCst), 1);
        assert_eq!(authority.mutations(), usize::from(fault == 3));
    }
}

#[tokio::test]
async fn terminal_cookie_context_supports_new_login_and_preauth_cancel_without_stale_cookie_clear()
{
    use tabula_session::{
        CredentialOperation, HttpSessionAuthority, SessionContextBinding, SessionCredential,
    };
    for kind in 0..3 {
        let (authority, _, server, _, _) = fixture().await;
        let (old, snapshot) = authority.fixture(SessionChannel::BrowserCookie);
        if kind == 0 {
            authority
                .revoke_credential(CredentialOperation {
                    digest: SessionCredential::parse(&old).unwrap().digest(),
                    channel: SessionChannel::BrowserCookie,
                    context: Some(SessionContextBinding {
                        context_id: snapshot.context_id(),
                        authorization_epoch: snapshot.authorization_epoch(),
                    }),
                })
                .await
                .unwrap();
        } else if kind == 1 {
            authority.set_time(1_801_000);
        }
        let old = if kind == 2 {
            SessionCredential::generate().unwrap().expose_encoded()
        } else {
            old
        };
        let stale = format!("{SESSION_COOKIE}={old}");
        let context = server
            .request("GET", "/api/v1/auth/context", &[("Cookie", &stale)], "")
            .await;
        assert_eq!(context.status, 200);
        assert_eq!(context.json()["disposition"], "signed_out");
        assert_eq!(context.json()["capabilities"]["login"], true);
        assert!(context.cookie(SESSION_COOKIE).is_none());
        let preauth = context.cookie(PREAUTH_COOKIE).unwrap();
        let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
        let combined = format!("{stale}; {preauth}");
        let target = callback_url(&start(&server, &combined, &csrf).await);
        assert_eq!(cancel(&server, &combined, &csrf).await.status, 204);
        rejected(&complete(&server, &combined, &target).await);
        let context = server
            .request("GET", "/api/v1/auth/context", &[("Cookie", &stale)], "")
            .await;
        let preauth = context.cookie(PREAUTH_COOKIE).unwrap();
        let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
        let combined = format!("{stale}; {preauth}");
        let target = callback_url(&start(&server, &combined, &csrf).await);
        assert_eq!(complete(&server, &combined, &target).await.status, 303);
        assert_eq!(authority.issuance_attempts(), 1);
    }
}

#[tokio::test]
async fn discarded_login_start_can_be_explicitly_cancelled_and_retried_but_active_cookie_cannot_be_revoked(
) {
    let (authority, provider, server, cookie, csrf) = fixture().await;
    // The browser received no usable navigation result, but the server started
    // a flow. Only an explicit same-origin CSRF cancellation resets it.
    assert_eq!(start(&server, &cookie, &csrf).await.status, 200);
    let (active, snapshot) = authority.fixture(SessionChannel::BrowserCookie);
    let combined = format!("{cookie}; {SESSION_COOKIE}={active}");
    let response = cancel(&server, &combined, &csrf).await;
    assert_eq!(response.status, 409);
    rejected(&response);
    assert!(authority.snapshot(snapshot.id()).revoked_at().is_none());
    assert_eq!(provider.state.lock().unwrap().pending.len(), 1);
    let cancelled = cancel(&server, &cookie, &csrf).await;
    assert_eq!(cancelled.status, 204);
    assert!(cancelled.cookie(SESSION_COOKIE).is_none());
    let context = server.request("GET", "/api/v1/auth/context", &[], "").await;
    let cookie = context.cookie(PREAUTH_COOKIE).unwrap();
    let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
    let target = callback_url(&start(&server, &cookie, &csrf).await);
    assert_eq!(complete(&server, &cookie, &target).await.status, 303);
    assert_eq!(authority.issuance_attempts(), 1);
}

#[tokio::test]
async fn login_start_budget_is_bounded_without_issuing_any_session() {
    let (authority, provider, server, mut cookie, mut csrf) = fixture().await;
    for index in 0..31 {
        if index > 0 {
            let context = server.request("GET", "/api/v1/auth/context", &[], "").await;
            cookie = context.cookie(PREAUTH_COOKIE).unwrap();
            csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
        }
        let response = start(&server, &cookie, &csrf).await;
        if index < 30 {
            assert_eq!(response.status, 200);
        } else {
            assert_eq!(response.status, 429);
            rejected(&response);
        }
    }
    assert_eq!(provider.state.lock().unwrap().serial, 30);
    assert_eq!(authority.issuance_attempts(), 0);
}

#[tokio::test]
async fn matched_state_bad_code_or_issuer_is_terminal_and_returns_no_cookie_or_provider_details() {
    for bad_issuer in [false, true] {
        let (authority, _, server, cookie, csrf) = fixture().await;
        let target = callback_url(&start(&server, &cookie, &csrf).await);
        let bogus = if bad_issuer {
            format!("{target}&iss=https%3A%2F%2Fevil.invalid")
        } else {
            target.replace("code=synthetic-code", "code=bogus-code")
        };
        let response = complete(&server, &cookie, &bogus).await;
        assert_eq!(response.status, 403);
        rejected(&response);
        response.assert_excludes(&[
            "synthetic-subject",
            "synthetic-state",
            "bogus-code",
            "evil.invalid",
        ]);
        rejected(&complete(&server, &cookie, &target).await);
        let fresh = server
            .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
            .await;
        assert_ne!(
            fresh.cookie(PREAUTH_COOKIE).as_deref(),
            Some(cookie.as_str())
        );
        assert_eq!(authority.issuance_attempts(), 0);
    }
}
