#![cfg(all(feature = "account-http-acceptance", not(target_arch = "wasm32")))]

//! PR3's pure account lifecycle consumes responses from the real PR2 isolated
//! adapter over TCP. Identity/session provisioning is explicitly synthetic.
//! This establishes neither Kanidm authentication, `PostgreSQL` durability,
//! browser Fetch/cookies/TLS, rendered frames nor accessibility behavior.

// Reuse the PR2 authority fixture rather than introducing another authentication
// implementation. Its other transport/race partitions remain in the PR2 suite.
#[allow(dead_code)]
#[path = "../../../crates/tabula-session-http/tests/support/authority.rs"]
mod authority;
// The binary's pure core is shared by path; browser-only trust/listener failure
// helpers have their own focused tests rather than a TCP substitute here.
#[allow(dead_code)]
#[path = "../src/account/core.rs"]
mod core;
#[path = "support/delayed_private.rs"]
mod delayed_private;
#[path = "support/http_wire.rs"]
mod http_wire;

use axum::middleware;
use serde_json::{json, Value};
use tabula_session::{SessionChannel, SessionError, SessionSnapshot};
use tabula_session_http::isolated::IsolatedSessionHttp;

use authority::TestAuthority;
use core::{AccountCore, AccountFailure, AccountRequest, AccountStatus, HttpResponse, RequestKind};
use http_wire::{WireResponse, WireServer, SESSION_COOKIE, TRUSTED_ORIGIN};

async fn server(authority: TestAuthority) -> WireServer {
    WireServer::start(
        IsolatedSessionHttp::new(authority, TRUSTED_ORIGIN)
            .unwrap()
            .router()
            .layer(middleware::from_fn(
                delayed_private::delay_first_private_frame,
            )),
    )
    .await
}

async fn fixture() -> (TestAuthority, WireServer, String, SessionSnapshot) {
    let authority = TestAuthority::new();
    // Direct fixture issuance is not a login endpoint or provider authentication.
    let (credential, snapshot) = authority.fixture(SessionChannel::BrowserCookie);
    let server = server(authority.clone()).await;
    (
        authority,
        server,
        format!("{SESSION_COOKIE}={credential}"),
        snapshot,
    )
}

async fn exchange(server: &WireServer, request: &AccountRequest, cookie: &str) -> WireResponse {
    let mut headers = vec![("Cookie", cookie)];
    if let Some(csrf) = request.csrf_token() {
        headers.extend([
            ("Origin", TRUSTED_ORIGIN),
            ("Content-Type", "application/json"),
            ("X-Tabula-CSRF", csrf),
            ("Sec-Fetch-Site", "same-origin"),
        ]);
    }
    let response = server
        .request(
            request.method(),
            request.path(),
            &headers,
            request.body().unwrap_or(""),
        )
        .await;
    response.assert_no_store();
    response
}

fn completion(response: &WireResponse) -> Result<HttpResponse, AccountFailure> {
    response
        .body
        .as_ref()
        .map(|body| HttpResponse {
            status: response.status,
            body: body.clone(),
        })
        .map_err(|_| AccountFailure::Transport)
}

fn json_body(response: &WireResponse) -> Value {
    serde_json::from_slice(response.body.as_ref().unwrap()).unwrap()
}

async fn authenticate(core: &mut AccountCore, server: &WireServer, cookie: &str, subject: &str) {
    let context = core.recheck().unwrap();
    assert_eq!(context.kind(), RequestKind::Context);
    let response = exchange(server, &context, cookie).await;
    assert_eq!(response.status, 200);
    assert_eq!(json_body(&response)["account_id"], subject);
    let profile = core.complete(&context, completion(&response)).unwrap();
    assert_eq!(profile.kind(), RequestKind::Profile);
    assert_eq!(core.snapshot().status, AccountStatus::Resolving);
    let response = exchange(server, &profile, cookie).await;
    assert_eq!(response.status, 200);
    assert!(core.complete(&profile, completion(&response)).is_none());
    assert_eq!(
        core.snapshot().status,
        AccountStatus::Authenticated {
            account_id: subject.to_owned()
        }
    );
}

#[tokio::test]
async fn tcp_signed_out_and_unavailable_contexts_never_provision_or_present_a_subject() {
    let authority = TestAuthority::new();
    let server = server(authority.clone()).await;
    let mut core = AccountCore::default();
    let context = core.recheck().unwrap();
    let response = server
        .request(context.method(), context.path(), &[], "")
        .await;
    assert_eq!(response.status, 200);
    response.assert_no_store();
    assert_eq!(json_body(&response)["disposition"], "signed_out");
    assert!(core.complete(&context, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
    assert!(core.refresh().is_none());
    assert!(core.logout().is_none());
    let (credential, _) = authority.fixture(SessionChannel::BrowserCookie);
    let cookie = format!("{SESSION_COOKIE}={credential}");
    authority.set_failure(Some(SessionError::Unavailable));
    let context = core.recheck().unwrap();
    let response = exchange(&server, &context, &cookie).await;
    assert_eq!(response.status, 503);
    assert_eq!(json_body(&response)["disposition"], "unavailable");
    assert!(core.complete(&context, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Unavailable);
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(authority.mutations(), 0);
}

#[tokio::test]
async fn tcp_context_then_current_self_profile_is_required_to_publish_identity() {
    let (authority, server, cookie, snapshot) = fixture().await;
    let mut core = AccountCore::default();
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    assert!(core.snapshot().busy.is_none());
    assert_eq!(authority.issuance_attempts(), 0);
    assert_eq!(authority.mutations(), 0);
    assert_eq!(
        authority.snapshot(snapshot.id()).last_activity_at(),
        snapshot.last_activity_at()
    );
    let context = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    let value = json_body(&context);
    assert_eq!(
        value["capabilities"],
        json!({"login":false,"register":false,"friends":false,"read_self_profile":true})
    );
    assert_eq!(value.as_object().unwrap().len(), 5);
    let bytes = std::str::from_utf8(context.body.as_ref().unwrap()).unwrap();
    assert!(!bytes.contains(cookie.split_once('=').unwrap().1));
    context.assert_no_store();
}

#[tokio::test]
async fn malformed_unknown_and_incompatible_contexts_never_schedule_a_private_profile() {
    let (_authority, server, cookie, _) = fixture().await;
    let actual = server
        .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
        .await;
    actual.assert_no_store();
    let original = json_body(&actual);
    let mut unknown = original.clone();
    unknown["unexpected_private_field"] = json!("hostile");
    let mut version = original.clone();
    version["version"] = json!(2);
    let mut subject = original.clone();
    subject["account_id"] = json!("not-a-canonical-id");
    let mut capabilities = original.clone();
    capabilities["capabilities"]["login"] = json!(true);
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("csrf_token");
    let positional = json!([
        1,
        "authenticated",
        original["account_id"],
        original["csrf_token"],
        [false, false, false, true]
    ]);
    let mut positional_capabilities = original.clone();
    positional_capabilities["capabilities"] = json!([false, false, false, true]);
    let duplicate_version = std::str::from_utf8(actual.body.as_ref().unwrap())
        .unwrap()
        .replace("\"version\":1", "\"version\":1,\"version\":1");
    assert_ne!(duplicate_version.as_bytes(), actual.body.as_ref().unwrap());
    for body in [
        b"{".to_vec(),
        b"[]".to_vec(),
        serde_json::to_vec(&unknown).unwrap(),
        serde_json::to_vec(&version).unwrap(),
        serde_json::to_vec(&subject).unwrap(),
        serde_json::to_vec(&capabilities).unwrap(),
        serde_json::to_vec(&missing).unwrap(),
        serde_json::to_vec(&positional).unwrap(),
        serde_json::to_vec(&positional_capabilities).unwrap(),
        duplicate_version.into_bytes(),
        vec![b' '; core::MAX_RESPONSE_BYTES + 1],
    ] {
        // Hostile response variants are independently authored decode evidence,
        // not responses falsely attributed to the production adapter.
        let mut core = AccountCore::default();
        let request = core.recheck().unwrap();
        assert!(core
            .complete(&request, Ok(HttpResponse { status: 200, body }))
            .is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Error);
        assert!(core.snapshot().busy.is_none());
    }
}

#[tokio::test]
async fn a_validly_encoded_other_subject_or_unknown_profile_is_rejected_after_real_context() {
    let (authority, server, cookie, _) = fixture().await;
    for body in [
        json!({"version":1,"account_id":"00000000000000000000000000000001"}),
        json!({"version":1,"account_id":authority.account_id(),"name":"invented"}),
        json!({"version":2,"account_id":authority.account_id()}),
        json!([1, authority.account_id()]),
    ] {
        let mut core = AccountCore::default();
        let context = core.recheck().unwrap();
        let context_response = exchange(&server, &context, &cookie).await;
        let profile = core
            .complete(&context, completion(&context_response))
            .unwrap();
        let recheck = core
            .complete(
                &profile,
                Ok(HttpResponse {
                    status: 200,
                    body: serde_json::to_vec(&body).unwrap(),
                }),
            )
            .unwrap();
        assert_eq!(recheck.kind(), RequestKind::Context);
        assert_eq!(core.snapshot().status, AccountStatus::Error);
        let response = exchange(&server, &recheck, &cookie).await;
        assert!(core.complete(&recheck, completion(&response)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Error);
    }
}

#[tokio::test]
async fn actual_401_403_and_503_remain_distinct_and_never_restore_a_stale_profile() {
    let (authority, server, cookie, snapshot) = fixture().await;
    let mut core = AccountCore::default();
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    let refresh = core.refresh().unwrap();
    let rejected = server
        .request(
            "POST",
            refresh.path(),
            &[
                ("Cookie", &cookie),
                ("Origin", TRUSTED_ORIGIN),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", "not-the-current-token"),
            ],
            "{}",
        )
        .await;
    assert_eq!(rejected.status, 403);
    rejected.assert_no_store();
    let diagnose = core.complete(&refresh, completion(&rejected)).unwrap();
    let response = exchange(&server, &diagnose, &cookie).await;
    assert!(core.complete(&diagnose, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Error);
    assert_eq!(authority.mutations(), 0);

    let recheck = core.recheck().unwrap();
    let response = exchange(&server, &recheck, &cookie).await;
    let profile = core.complete(&recheck, completion(&response)).unwrap();
    authority.set_failure(Some(SessionError::Unavailable));
    let unavailable = exchange(&server, &profile, &cookie).await;
    assert_eq!(unavailable.status, 503);
    let diagnose = core.complete(&profile, completion(&unavailable)).unwrap();
    let response = exchange(&server, &diagnose, &cookie).await;
    assert_eq!(response.status, 503);
    assert!(core.complete(&diagnose, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Unavailable);

    authority.set_failure(None);
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    let context = core.recheck().unwrap();
    let response = exchange(&server, &context, &cookie).await;
    let profile = core.complete(&context, completion(&response)).unwrap();
    // Server time is an explicit synthetic control. Advancing to the exact idle
    // deadline makes the real adapter reject the still-attached cookie.
    authority.set_time(snapshot.idle_deadline().get());
    let expired = exchange(&server, &profile, &cookie).await;
    assert_eq!(expired.status, 401);
    let diagnose = core.complete(&profile, completion(&expired)).unwrap();
    let response = exchange(&server, &diagnose, &cookie).await;
    assert_eq!(json_body(&response)["disposition"], "signed_out");
    assert!(core.complete(&diagnose, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Expired);
}

#[tokio::test]
async fn refresh_204_requires_new_context_and_profile_and_uses_the_refetched_token() {
    let (authority, first_server, old_cookie, snapshot) = fixture().await;
    let mut core = AccountCore::default();
    authenticate(
        &mut core,
        &first_server,
        &old_cookie,
        &authority.account_id(),
    )
    .await;
    let refresh = core.refresh().unwrap();
    let old_csrf = refresh.csrf_token().unwrap().to_owned();
    let response = exchange(&first_server, &refresh, &old_cookie).await;
    assert_eq!(response.status, 204);
    assert!(response.body.as_ref().unwrap().is_empty());
    let current_cookie = response.cookie(SESSION_COOKIE).unwrap();
    assert_ne!(current_cookie, old_cookie);
    let context = core.complete(&refresh, completion(&response)).unwrap();
    assert_eq!(context.kind(), RequestKind::Context);
    assert_eq!(core.snapshot().status, AccountStatus::Resolving);
    let stale = first_server
        .request("GET", "/api/v1/me", &[("Cookie", &old_cookie)], "")
        .await;
    assert_eq!(stale.status, 401);
    stale.assert_no_store();
    let restarted = server(authority.clone()).await;
    let response = exchange(&restarted, &context, &current_cookie).await;
    let current_csrf = json_body(&response)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(
        current_csrf, old_csrf,
        "new adapter key requires fresh context"
    );
    let profile = core.complete(&context, completion(&response)).unwrap();
    assert_eq!(core.snapshot().status, AccountStatus::Resolving);
    let response = exchange(&restarted, &profile, &current_cookie).await;
    assert!(core.complete(&profile, completion(&response)).is_none());
    assert_eq!(
        core.snapshot().status,
        AccountStatus::Authenticated {
            account_id: authority.account_id()
        }
    );
    let logout = core.logout().unwrap();
    assert_eq!(logout.csrf_token(), Some(current_csrf.as_str()));
    let response = exchange(&restarted, &logout, &current_cookie).await;
    assert_eq!(response.status, 204);
    assert!(core.complete(&logout, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
    assert_eq!(
        authority.snapshot(snapshot.id()).idle_deadline(),
        snapshot.idle_deadline()
    );
}

#[tokio::test]
async fn ambiguous_logout_ack_requires_explicit_same_route_retry_204() {
    for committed in [false, true] {
        let (authority, server, cookie, snapshot) = fixture().await;
        let mut core = AccountCore::default();
        authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
        authority.set_mutation_fault(SessionError::Unavailable, committed);
        let logout = core.logout().unwrap();
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        let response = exchange(&server, &logout, &cookie).await;
        assert_eq!(response.status, 503);
        assert!(response.cookie(SESSION_COOKIE).is_none());
        let diagnose = core.complete(&logout, completion(&response)).unwrap();
        let response = exchange(&server, &diagnose, &cookie).await;
        assert!(core.complete(&diagnose, completion(&response)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        assert_eq!(
            authority.snapshot(snapshot.id()).revoked_at().is_some(),
            committed
        );
        assert!(core.refresh().is_none());
        authority.clear_mutation_fault();
        // A rejected/lost acknowledgement cannot be reinterpreted as success.
        // The original token is usable only for explicit record-bound revocation.
        let retry = core.logout().unwrap();
        assert_eq!(retry.kind(), RequestKind::Logout);
        assert_eq!(retry.csrf_token(), logout.csrf_token());
        let response = exchange(&server, &retry, &cookie).await;
        assert_eq!(response.status, 204);
        assert!(core.complete(&retry, completion(&response)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
    }
}

#[tokio::test]
async fn cleanup_erases_reusable_logout_token_and_terminal_recheck_does_not_claim_success() {
    let (authority, server, cookie, _) = fixture().await;
    let mut core = AccountCore::default();
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    authority.set_mutation_fault(SessionError::Unavailable, true);
    let logout = core.logout().unwrap();
    let response = exchange(&server, &logout, &cookie).await;
    assert_eq!(response.status, 503);
    let diagnose = core.complete(&logout, completion(&response)).unwrap();
    let response = exchange(&server, &diagnose, &cookie).await;
    assert!(core.complete(&diagnose, completion(&response)).is_none());
    core.route_exit();
    authority.clear_mutation_fault();
    let recheck = core.recheck().unwrap();
    let response = exchange(&server, &recheck, &cookie).await;
    assert_eq!(json_body(&response)["disposition"], "signed_out");
    assert!(core.complete(&recheck, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
    let retry = core.logout().unwrap();
    assert_eq!(retry.kind(), RequestKind::Context);
    assert!(retry.csrf_token().is_none());
    let response = exchange(&server, &retry, &cookie).await;
    assert!(core.complete(&retry, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
}

#[tokio::test]
async fn logout_retry_never_revokes_changed_account_or_replacement_same_account_record() {
    for different_account in [false, true] {
        let (original, original_server, cookie, old_snapshot) = fixture().await;
        let mut core = AccountCore::default();
        authenticate(&mut core, &original_server, &cookie, &original.account_id()).await;
        original.set_mutation_fault(SessionError::Unavailable, false);
        let logout = core.logout().unwrap();
        let response = exchange(&original_server, &logout, &cookie).await;
        let diagnose = core.complete(&logout, completion(&response)).unwrap();
        let response = exchange(&original_server, &diagnose, &cookie).await;
        assert!(core.complete(&diagnose, completion(&response)).is_none());
        let current = if different_account {
            TestAuthority::with_user_id(tabula_core::UserId(2))
        } else {
            original.clone()
        };
        let (new_credential, new_snapshot) = current.fixture_with_context(
            SessionChannel::BrowserCookie,
            (!different_account).then_some(old_snapshot.context_id()),
        );
        let new_cookie = format!("{SESSION_COOKIE}={new_credential}");
        // Same-account replacement deliberately reuses the context ID and the
        // original adapter key: immutable record binding must still differ.
        let replacement_server = if different_account {
            Some(server(current.clone()).await)
        } else {
            None
        };
        let current_server = replacement_server.as_ref().unwrap_or(&original_server);
        let same_route_retry = core.logout().unwrap();
        assert_eq!(same_route_retry.kind(), RequestKind::Logout);
        assert_eq!(same_route_retry.csrf_token(), logout.csrf_token());
        let rejected = exchange(current_server, &same_route_retry, &new_cookie).await;
        assert_eq!(rejected.status, 403);
        let diagnose = core
            .complete(&same_route_retry, completion(&rejected))
            .unwrap();
        let response = exchange(current_server, &diagnose, &new_cookie).await;
        assert!(core.complete(&diagnose, completion(&response)).is_none());
        core.route_exit();
        let retry = core.logout().unwrap();
        assert_eq!(retry.kind(), RequestKind::Context);
        let response = exchange(current_server, &retry, &new_cookie).await;
        assert_eq!(json_body(&response)["account_id"], current.account_id());
        assert!(
            core.complete(&retry, completion(&response)).is_none(),
            "mismatched target token must never generate logout POST"
        );
        assert_eq!(core.snapshot().status, AccountStatus::LogoutContextChanged);
        assert!(current.snapshot(new_snapshot.id()).revoked_at().is_none());
        assert!(original.snapshot(old_snapshot.id()).revoked_at().is_none());
        assert_eq!(current.mutations(), 0);
    }
}

#[tokio::test]
async fn private_body_failure_after_received_200_stays_masked_until_explicit_recheck() {
    let (authority, server, cookie, _) = fixture().await;
    let mut core = AccountCore::default();
    let context = core.recheck().unwrap();
    let response = exchange(&server, &context, &cookie).await;
    let profile = core.complete(&context, completion(&response)).unwrap();
    let response = server
        .request(
            "GET",
            profile.path(),
            &[("Cookie", &cookie), ("X-Test-Delay-Private-Frame", "1")],
            "",
        )
        .await;
    assert_eq!(
        response.status, 200,
        "successful headers precede failed private body"
    );
    response.assert_no_store();
    assert!(
        response.body.is_err(),
        "expired guard must not deliver private bytes"
    );
    response.assert_received_excludes(&authority.account_id());
    let diagnose = core.complete(&profile, completion(&response)).unwrap();
    assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
    let response = exchange(&server, &diagnose, &cookie).await;
    assert!(core.complete(&diagnose, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
}

#[tokio::test]
async fn pagehide_cancel_and_route_exit_retire_successful_tcp_completions() {
    let (authority, server, cookie, snapshot) = fixture().await;
    for cleanup in [
        AccountCore::suspend,
        AccountCore::cancel,
        AccountCore::route_exit,
    ] {
        let mut core = AccountCore::default();
        let context = core.recheck().unwrap();
        let response = exchange(&server, &context, &cookie).await;
        let profile = core.complete(&context, completion(&response)).unwrap();
        let delayed_completion = exchange(&server, &profile, &cookie).await;
        cleanup(&mut core);
        assert!(core
            .complete(&profile, completion(&delayed_completion))
            .is_none());
        assert!(!matches!(
            core.snapshot().status,
            AccountStatus::Authenticated { .. }
        ));
        authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    }
    let mut core = AccountCore::default();
    authenticate(&mut core, &server, &cookie, &authority.account_id()).await;
    core.suspend();
    authority.set_time(snapshot.idle_deadline().get());
    let context = core.recheck().unwrap();
    let response = exchange(&server, &context, &cookie).await;
    assert_eq!(json_body(&response)["disposition"], "signed_out");
    assert!(core.complete(&context, completion(&response)).is_none());
    assert_eq!(core.snapshot().status, AccountStatus::Expired);
    assert!(core.refresh().is_none());
}
