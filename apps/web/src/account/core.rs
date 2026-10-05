//! Generation-fenced account decisions for ADR-0031 §6 and ADR-0036 PR3.
//!
//! The core holds only the current document's CSRF context, never credentials.
//! HTTP DTO validation is necessary but cannot replace durable server authority.

use sha2::{Digest, Sha256};
use tabula_session_http::{ContextResponse, SelfProfileResponse, SessionDisposition};

/// Initial authenticated deployment is same-origin HTTPS only (ADR-0031 §2).
#[must_use]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Native fixtures do not Fetch.
pub fn trusted_browser_scheme(protocol: &str) -> bool {
    protocol == "https:"
}

/// Maximum decoded response body accepted by this compact isolated client.
pub const MAX_RESPONSE_BYTES: usize = 4096;

/// What can be presented without exposing unresolved or old-account data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    Resolving,
    SignedOut,
    Authenticated { account_id: String },
    Expired,
    Unavailable,
    Disconnected,
    Error,
    LogoutPending,
    LogoutContextChanged,
    Cancelled,
}

/// One same-document operation; repeated clicks cannot create another request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountOperation {
    Recheck,
    Refresh,
    Logout,
}

/// Public presentation only. Neither a session credential nor CSRF is exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub status: AccountStatus,
    pub busy: Option<AccountOperation>,
    pub presentation_generation: u64,
}

/// Fixed same-origin endpoints, never user-supplied URLs or account overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Context,
    Profile,
    Refresh,
    Logout,
}

/// A completion witness scoped to one operation generation and request step.
/// Debug deliberately redacts the synchronizer token.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountRequest {
    generation: u64,
    sequence: u64,
    kind: RequestKind,
    csrf_token: Option<String>,
}
impl std::fmt::Debug for AccountRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountRequest")
            .field("generation", &self.generation)
            .field("sequence", &self.sequence)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Accessed by WASM Fetch/opt-in TCP tests.
impl AccountRequest {
    #[must_use]
    pub const fn kind(&self) -> RequestKind {
        self.kind
    }
    #[must_use]
    pub const fn path(&self) -> &'static str {
        match self.kind {
            RequestKind::Context => "/api/v1/auth/context",
            RequestKind::Profile => "/api/v1/me",
            RequestKind::Refresh => "/api/v1/auth/refresh",
            RequestKind::Logout => "/api/v1/auth/logout",
        }
    }
    #[must_use]
    pub const fn method(&self) -> &'static str {
        match self.kind {
            RequestKind::Context | RequestKind::Profile => "GET",
            _ => "POST",
        }
    }
    #[must_use]
    pub fn csrf_token(&self) -> Option<&str> {
        self.csrf_token.as_deref()
    }
    #[must_use]
    pub const fn body(&self) -> Option<&'static str> {
        match self.kind {
            RequestKind::Context | RequestKind::Profile => None,
            _ => Some("{}"),
        }
    }
}

/// Bounded literal HTTP response after successful full body consumption.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

/// Safe error classes; no raw server, transport or credential diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountFailure {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Browser/TCP body failures.
    Transport,
    Protocol,
    Unavailable,
    Unauthenticated,
}

#[derive(Clone, Copy)]
enum ContextPurpose {
    ReadProfile,
    Diagnose(AccountFailure),
    RetryLogout,
}

/// Pure operation lifecycle shared by route owners in one application document.
/// Logout suppression survives route cleanup, but never persists account data.
pub struct AccountCore {
    generation: u64,
    sequence: u64,
    pending: Option<AccountRequest>,
    snapshot: AccountSnapshot,
    context_account: Option<String>,
    csrf_token: Option<String>,
    had_authenticated: bool,
    logout_suppressed: bool,
    // Original record-bound token is revocation-only while this route stays active.
    // Cleanup erases it; a one-way fingerprint can compare a newly fetched token.
    logout_retry_token: Option<String>,
    logout_target_fingerprint: Option<[u8; 32]>,
    context_purpose: ContextPurpose,
}
impl Default for AccountCore {
    fn default() -> Self {
        Self {
            generation: 0,
            sequence: 0,
            pending: None,
            snapshot: AccountSnapshot {
                presentation_generation: 0,
                status: AccountStatus::Resolving,
                busy: None,
            },
            context_account: None,
            csrf_token: None,
            had_authenticated: false,
            logout_suppressed: false,
            logout_retry_token: None,
            logout_target_fingerprint: None,
            context_purpose: ContextPurpose::ReadProfile,
        }
    }
}
impl AccountCore {
    #[must_use]
    pub fn snapshot(&self) -> AccountSnapshot {
        self.snapshot.clone()
    }

    fn clear_private(&mut self) {
        self.context_account = None;
        self.csrf_token = None;
    }
    fn advance(&mut self) -> bool {
        self.pending = None;
        self.clear_private();
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
            true
        } else {
            self.snapshot = AccountSnapshot {
                presentation_generation: self.generation,
                status: AccountStatus::Error,
                busy: None,
            };
            false
        }
    }
    fn request(&mut self, kind: RequestKind, csrf_token: Option<String>) -> Option<AccountRequest> {
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.clear_private();
            self.finish(AccountStatus::Error);
            return None;
        };
        self.sequence = sequence;
        let request = AccountRequest {
            generation: self.generation,
            sequence,
            kind,
            csrf_token,
        };
        self.pending = Some(request.clone());
        Some(request)
    }
    fn context(&mut self, purpose: ContextPurpose) -> Option<AccountRequest> {
        self.clear_private();
        self.context_purpose = purpose;
        self.request(RequestKind::Context, None)
    }
    fn finish(&mut self, status: AccountStatus) {
        self.pending = None;
        self.snapshot = AccountSnapshot {
            presentation_generation: self.generation,
            status,
            busy: None,
        };
    }

    /// Explicit bootstrap/recovery. Pending operations are single-flight.
    pub fn recheck(&mut self) -> Option<AccountRequest> {
        if self.pending.is_some() || !self.advance() {
            return None;
        }
        self.snapshot = AccountSnapshot {
            presentation_generation: self.generation,
            status: if self.logout_suppressed {
                AccountStatus::LogoutPending
            } else {
                AccountStatus::Resolving
            },
            busy: Some(AccountOperation::Recheck),
        };
        self.context(ContextPurpose::ReadProfile)
    }

    /// Rotation is available only after validated current context and profile.
    pub fn refresh(&mut self) -> Option<AccountRequest> {
        if self.pending.is_some()
            || self.logout_suppressed
            || !matches!(self.snapshot.status, AccountStatus::Authenticated { .. })
        {
            return None;
        }
        let token = self.csrf_token.clone()?;
        if !self.advance() {
            return None;
        }
        self.snapshot = AccountSnapshot {
            presentation_generation: self.generation,
            status: AccountStatus::Resolving,
            busy: Some(AccountOperation::Refresh),
        };
        self.request(RequestKind::Refresh, Some(token))
    }

    /// Starts/retries current-device revocation. Local cleanup alone is not success.
    pub fn logout(&mut self) -> Option<AccountRequest> {
        if self.pending.is_some() {
            return None;
        }
        let token = if self.logout_suppressed {
            self.logout_retry_token.clone()
        } else {
            self.csrf_token.clone()
        };
        let can_logout = matches!(self.snapshot.status, AccountStatus::Authenticated { .. });
        if !can_logout && !self.logout_suppressed {
            return None;
        }
        if !self.logout_suppressed {
            let token = token.as_ref()?;
            self.logout_target_fingerprint = Some(Sha256::digest(token.as_bytes()).into());
            self.logout_retry_token = Some(token.clone());
        }
        self.logout_suppressed = true;
        if !self.advance() {
            return None;
        }
        self.snapshot = AccountSnapshot {
            presentation_generation: self.generation,
            status: AccountStatus::LogoutPending,
            busy: Some(AccountOperation::Logout),
        };
        if let Some(token) = token {
            self.request(RequestKind::Logout, Some(token))
        } else {
            self.context(ContextPurpose::RetryLogout)
        }
    }

    /// Cancel/Back permanently invalidates late work and clears private presentation.
    pub fn cancel(&mut self) {
        self.logout_retry_token = None;
        self.advance();
        self.finish(if self.logout_suppressed {
            AccountStatus::LogoutPending
        } else {
            AccountStatus::Cancelled
        });
    }
    /// Synchronous hidden/frozen-page cleanup; restoration must bootstrap again.
    pub fn suspend(&mut self) {
        self.logout_retry_token = None;
        self.advance();
        self.finish(if self.logout_suppressed {
            AccountStatus::LogoutPending
        } else {
            AccountStatus::Resolving
        });
    }
    /// Missing mandatory browser cleanup hooks fail closed without bootstrap.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Browser listener setup only.
    pub fn lifecycle_unavailable(&mut self) {
        self.logout_retry_token = None;
        self.advance();
        self.finish(self.failure_status(AccountFailure::Unavailable));
    }

    /// Route-owner cleanup preserves only the document's non-secret logout intent.
    pub fn route_exit(&mut self) {
        self.suspend();
    }

    fn failure_status(&self, failure: AccountFailure) -> AccountStatus {
        if self.logout_suppressed {
            return AccountStatus::LogoutPending;
        }
        match failure {
            AccountFailure::Transport => AccountStatus::Disconnected,
            AccountFailure::Unavailable => AccountStatus::Unavailable,
            AccountFailure::Protocol | AccountFailure::Unauthenticated => AccountStatus::Error,
        }
    }
    fn recover(&mut self, failure: AccountFailure) -> Option<AccountRequest> {
        self.snapshot.status = self.failure_status(failure);
        self.context(ContextPurpose::Diagnose(failure))
    }

    /// Applies only the exact active ticket. Transport errors after 200 headers
    /// are errors, never an empty profile, and trigger one fresh context diagnosis.
    pub fn complete(
        &mut self,
        request: &AccountRequest,
        result: Result<HttpResponse, AccountFailure>,
    ) -> Option<AccountRequest> {
        if self.pending.as_ref() != Some(request) {
            return None;
        }
        self.pending = None;
        let response = match result {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            Ok(_) => return self.failed(request.kind, AccountFailure::Protocol),
            Err(failure) => return self.failed(request.kind, failure),
        };
        match request.kind {
            RequestKind::Context => self.context_complete(&response),
            RequestKind::Profile => self.profile_complete(&response),
            RequestKind::Refresh | RequestKind::Logout => {
                if response.status != 204 || !response.body.is_empty() {
                    return self.failed(request.kind, status_failure(response.status));
                }
                if request.kind == RequestKind::Logout {
                    self.clear_private();
                    self.logout_suppressed = false;
                    self.logout_retry_token = None;
                    self.logout_target_fingerprint = None;
                    self.had_authenticated = false;
                    self.finish(AccountStatus::SignedOut);
                    None
                } else {
                    self.context(ContextPurpose::ReadProfile)
                }
            }
        }
    }
    fn failed(&mut self, kind: RequestKind, failure: AccountFailure) -> Option<AccountRequest> {
        self.clear_private();
        if kind == RequestKind::Context {
            self.finish(self.failure_status(failure));
            None
        } else {
            self.recover(failure)
        }
    }
    fn context_complete(&mut self, response: &HttpResponse) -> Option<AccountRequest> {
        if !matches!(response.status, 200 | 503) {
            return self.failed(RequestKind::Context, status_failure(response.status));
        }
        let shape = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
        if !shape.as_ref().is_some_and(|value| {
            value.is_object()
                && value
                    .get("capabilities")
                    .is_some_and(serde_json::Value::is_object)
        }) {
            return self.failed(RequestKind::Context, AccountFailure::Protocol);
        }
        // Decode original bytes, retaining Serde duplicate-known-field rejection.
        let context = serde_json::from_slice::<ContextResponse>(&response.body)
            .ok()
            .filter(|value| value.validate_for_browser().is_ok())
            .filter(|value| {
                (response.status == 503) == (value.disposition == SessionDisposition::Unavailable)
            });
        let Some(context) = context else {
            return self.failed(RequestKind::Context, AccountFailure::Protocol);
        };
        match context.disposition {
            SessionDisposition::Authenticated => {
                self.had_authenticated = true;
                self.context_account = context.account_id;
                self.csrf_token = context.csrf_token;
                if self.logout_suppressed {
                    let same_target = self.csrf_token.as_ref().is_some_and(|token| {
                        self.logout_target_fingerprint
                            == Some(Sha256::digest(token.as_bytes()).into())
                    });
                    self.context_account = None;
                    if same_target {
                        self.logout_retry_token.clone_from(&self.csrf_token);
                    } else {
                        // Another subject, another same-account session or adapter restart
                        // can never become the target of the earlier logout intent.
                        self.logout_retry_token = None;
                    }
                    self.csrf_token = None;
                    if same_target && matches!(self.context_purpose, ContextPurpose::RetryLogout) {
                        return self.request(RequestKind::Logout, self.logout_retry_token.clone());
                    }
                    self.finish(if same_target {
                        AccountStatus::LogoutPending
                    } else {
                        AccountStatus::LogoutContextChanged
                    });
                    None
                } else if let ContextPurpose::Diagnose(failure) = self.context_purpose {
                    self.clear_private();
                    self.finish(self.failure_status(failure));
                    None
                } else {
                    self.request(RequestKind::Profile, None)
                }
            }
            SessionDisposition::SignedOut => {
                self.clear_private();
                self.finish(if self.logout_suppressed {
                    AccountStatus::LogoutPending
                } else if self.had_authenticated {
                    AccountStatus::Expired
                } else {
                    AccountStatus::SignedOut
                });
                None
            }
            SessionDisposition::Unavailable => {
                self.clear_private();
                self.finish(if self.logout_suppressed {
                    AccountStatus::LogoutPending
                } else {
                    AccountStatus::Unavailable
                });
                None
            }
        }
    }
    fn profile_complete(&mut self, response: &HttpResponse) -> Option<AccountRequest> {
        if response.status != 200 {
            return self.failed(RequestKind::Profile, status_failure(response.status));
        }
        if !serde_json::from_slice::<serde_json::Value>(&response.body)
            .is_ok_and(|value| value.is_object())
        {
            return self.failed(RequestKind::Profile, AccountFailure::Protocol);
        }
        let profile = serde_json::from_slice::<SelfProfileResponse>(&response.body)
            .ok()
            .filter(|value| value.validate().is_ok())
            .filter(|value| Some(&value.account_id) == self.context_account.as_ref());
        let Some(profile) = profile else {
            return self.failed(RequestKind::Profile, AccountFailure::Protocol);
        };
        self.finish(AccountStatus::Authenticated {
            account_id: profile.account_id,
        });
        None
    }
}
fn status_failure(status: u16) -> AccountFailure {
    match status {
        401 | 409 => AccountFailure::Unauthenticated,
        503 => AccountFailure::Unavailable,
        _ => AccountFailure::Protocol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "00000000000000000000000000000001";
    const B: &str = "00000000000000000000000000000002";
    const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const OTHER_TOKEN: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBA";

    #[allow(clippy::unnecessary_wraps)] // Mirror the Result-shaped transport seam for fixtures.
    fn response(status: u16, body: &serde_json::Value) -> Result<HttpResponse, AccountFailure> {
        Ok(HttpResponse {
            status,
            body: serde_json::to_vec(body).unwrap(),
        })
    }
    fn context(account: &str, token: &str) -> Result<HttpResponse, AccountFailure> {
        response(
            200,
            &json!({"version":1,"disposition":"authenticated", "account_id":account,
            "csrf_token":token, "capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":true}}),
        )
    }
    fn signed_out() -> Result<HttpResponse, AccountFailure> {
        response(
            200,
            &json!({"version":1,"disposition":"signed_out", "account_id":null,
            "csrf_token":null,"capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":false}}),
        )
    }
    fn profile(account: &str) -> Result<HttpResponse, AccountFailure> {
        response(200, &json!({"version":1,"account_id":account}))
    }
    #[allow(clippy::unnecessary_wraps)] // Mirror the Result-shaped transport seam for fixtures.
    fn no_content() -> Result<HttpResponse, AccountFailure> {
        Ok(HttpResponse {
            status: 204,
            body: Vec::new(),
        })
    }
    fn ready(core: &mut AccountCore, account: &str) {
        let request = core.recheck().unwrap();
        let request = core.complete(&request, context(account, TOKEN)).unwrap();
        assert_eq!(request.kind(), RequestKind::Profile);
        assert_eq!(core.snapshot().status, AccountStatus::Resolving);
        assert!(core.complete(&request, profile(account)).is_none());
        assert_eq!(
            core.snapshot().status,
            AccountStatus::Authenticated {
                account_id: account.to_owned()
            }
        );
    }

    #[test]
    fn transport_and_request_debug_never_include_csrf_or_private_body() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let request = core.logout().unwrap();
        let response = context(A, TOKEN).unwrap();
        for debug in [format!("{request:?}"), format!("{response:?}")] {
            assert!(!debug.contains(TOKEN));
            assert!(!debug.contains(A));
        }
    }
    #[test]
    fn browser_requires_exact_https_scheme() {
        assert!(trusted_browser_scheme("https:"));
        for scheme in ["http:", "file:", "HTTPS:", "https", "blob:"] {
            assert!(!trusted_browser_scheme(scheme));
        }
    }
    #[test]
    fn context_then_subject_equal_profile_is_required() {
        let mut core = AccountCore::default();
        let first = core.recheck().unwrap();
        let private = core.complete(&first, context(A, TOKEN)).unwrap();
        let diagnosis = core.complete(&private, profile(B)).unwrap();
        assert_eq!(core.snapshot().status, AccountStatus::Error);
        assert_eq!(diagnosis.kind(), RequestKind::Context);
        assert!(core.complete(&diagnosis, context(B, TOKEN)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Error);
        ready(&mut core, B);
    }
    #[test]
    fn repeated_clicks_are_single_flight_through_all_request_steps() {
        let mut core = AccountCore::default();
        let first = core.recheck().unwrap();
        assert!(core.recheck().is_none());
        assert!(core.refresh().is_none());
        assert!(core.logout().is_none());
        let private = core.complete(&first, context(A, TOKEN)).unwrap();
        assert!(core.recheck().is_none());
        core.complete(&private, profile(A));
        let rotate = core.refresh().unwrap();
        assert!(core.refresh().is_none());
        assert!(core.logout().is_none());
        assert_eq!(rotate.method(), "POST");
        assert_eq!(rotate.body(), Some("{}"));
        let ctx = core.complete(&rotate, no_content()).unwrap();
        let private = core.complete(&ctx, context(A, TOKEN)).unwrap();
        core.complete(&private, profile(A));
        let logout = core.logout().unwrap();
        assert!(core.logout().is_none());
        assert!(core.recheck().is_none());
        core.complete(&logout, no_content());
        assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
    }
    #[test]
    fn late_a_after_b_and_aborted_requests_never_repopulate_private_output() {
        let mut core = AccountCore::default();
        let a = core.recheck().unwrap();
        let a_private = core.complete(&a, context(A, TOKEN)).unwrap();
        core.cancel();
        let b = core.recheck().unwrap();
        let b_private = core.complete(&b, context(B, OTHER_TOKEN)).unwrap();
        core.complete(&b_private, profile(B));
        let expected = core.snapshot();
        assert!(core.complete(&a_private, profile(A)).is_none());
        assert!(core.complete(&a, context(A, TOKEN)).is_none());
        assert_eq!(core.snapshot(), expected);
    }
    #[test]
    fn duplicate_first_context_cannot_match_recovery_context_in_same_generation() {
        let mut core = AccountCore::default();
        let first = core.recheck().unwrap();
        let private = core.complete(&first, context(A, TOKEN)).unwrap();
        let diagnosis = core
            .complete(&private, Err(AccountFailure::Transport))
            .unwrap();
        assert_ne!(first, diagnosis);
        assert!(core.complete(&first, context(B, OTHER_TOKEN)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.complete(&diagnosis, signed_out()).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Expired);
    }
    #[test]
    fn route_exit_cancel_hidden_and_restore_require_new_context_and_profile() {
        for cleanup in [
            AccountCore::cancel as fn(&mut AccountCore),
            AccountCore::route_exit,
            AccountCore::suspend,
        ] {
            let mut core = AccountCore::default();
            ready(&mut core, A);
            let previous = core.snapshot().presentation_generation;
            cleanup(&mut core);
            assert!(core.csrf_token.is_none());
            assert!(core.context_account.is_none());
            assert!(!matches!(
                core.snapshot().status,
                AccountStatus::Authenticated { .. }
            ));
            assert!(core.snapshot().presentation_generation > previous);
            let restore = core.recheck().unwrap();
            let private = core.complete(&restore, context(A, TOKEN)).unwrap();
            assert_eq!(core.snapshot().status, AccountStatus::Resolving);
            core.complete(&private, profile(A));
        }
    }
    #[test]
    fn malformed_unknown_array_duplicate_and_unsupported_capabilities_fail_closed() {
        let valid = json!({"version":1,"disposition":"authenticated","account_id":A,"csrf_token":TOKEN,
            "capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":true}});
        let mut nested_array = valid.clone();
        nested_array["capabilities"] = json!([false, false, false, true]);
        let mut future = valid.clone();
        future["capabilities"]["future"] = json!(true);
        let mut unavailable = valid.clone();
        unavailable["capabilities"]["login"] = json!(true);
        let mut version = valid.clone();
        version["version"] = json!(2);
        let positional = json!([1, "authenticated", A, TOKEN, [false, false, false, true]]);
        let duplicate = format!("{{\"version\":1,\"version\":1,\"disposition\":\"authenticated\",\"account_id\":\"{A}\",\"csrf_token\":\"{TOKEN}\",\"capabilities\":{{\"login\":false,\"register\":false,\"friends\":false,\"read_self_profile\":true}}}}").into_bytes();
        for body in [
            serde_json::to_vec(&nested_array).unwrap(),
            serde_json::to_vec(&future).unwrap(),
            serde_json::to_vec(&unavailable).unwrap(),
            serde_json::to_vec(&version).unwrap(),
            serde_json::to_vec(&positional).unwrap(),
            duplicate,
            b"{".to_vec(),
            vec![b' '; MAX_RESPONSE_BYTES + 1],
        ] {
            let mut core = AccountCore::default();
            let request = core.recheck().unwrap();
            assert!(core
                .complete(&request, Ok(HttpResponse { status: 200, body }))
                .is_none());
            assert_eq!(core.snapshot().status, AccountStatus::Error);
            assert!(core.csrf_token.is_none());
            assert!(core.context_account.is_none());
        }
    }
    #[test]
    fn positional_and_duplicate_profile_are_not_accepted_as_objects() {
        for body in [
            serde_json::to_vec(&json!([1, A])).unwrap(),
            format!("{{\"version\":1,\"account_id\":\"{A}\",\"account_id\":\"{A}\"}}").into_bytes(),
        ] {
            let mut core = AccountCore::default();
            let request = core.recheck().unwrap();
            let private = core.complete(&request, context(A, TOKEN)).unwrap();
            let diagnosis = core
                .complete(&private, Ok(HttpResponse { status: 200, body }))
                .unwrap();
            assert_eq!(diagnosis.kind(), RequestKind::Context);
            assert_eq!(core.snapshot().status, AccountStatus::Error);
        }
    }
    #[test]
    fn private_body_transport_failure_after_headers_is_error_then_context_diagnosis() {
        let mut core = AccountCore::default();
        let request = core.recheck().unwrap();
        let private = core.complete(&request, context(A, TOKEN)).unwrap();
        let diagnosis = core
            .complete(&private, Err(AccountFailure::Transport))
            .unwrap();
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.csrf_token.is_none());
        assert!(core.context_account.is_none());
        core.complete(&diagnosis, context(A, TOKEN));
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.csrf_token.is_none());
        assert!(core.context_account.is_none());
        ready(&mut core, A);
    }
    #[test]
    fn refresh_rotation_refetches_context_but_does_not_assume_unexpired_authority() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let rotation = core.refresh().unwrap();
        assert_eq!(rotation.csrf_token(), Some(TOKEN));
        let request = core.complete(&rotation, no_content()).unwrap();
        assert_eq!(request.kind(), RequestKind::Context);
        assert_eq!(core.snapshot().status, AccountStatus::Resolving);
        core.complete(&request, signed_out());
        assert_eq!(core.snapshot().status, AccountStatus::Expired);
        assert!(core.refresh().is_none());
    }
    #[test]
    fn lost_refresh_response_diagnoses_context_without_retrying_old_mutation() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let rotation = core.refresh().unwrap();
        let request = core
            .complete(&rotation, Err(AccountFailure::Transport))
            .unwrap();
        assert_eq!(request.kind(), RequestKind::Context);
        assert!(core.complete(&request, context(A, TOKEN)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.refresh().is_none());
    }
    #[test]
    fn interrupted_refresh_cannot_restore_after_cancel_even_if_server_rotated_cookie() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let rotation = core.refresh().unwrap();
        core.cancel();
        assert!(core.complete(&rotation, no_content()).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::Cancelled);
        ready(&mut core, A);
    }
    #[test]
    fn logout_uncertainty_suppresses_profile_and_same_route_explicit_retry_can_revoke_terminal() {
        for terminal in [false, true] {
            let mut core = AccountCore::default();
            ready(&mut core, A);
            let logout = core.logout().unwrap();
            let diagnosis = core
                .complete(&logout, Err(AccountFailure::Transport))
                .unwrap();
            core.complete(
                &diagnosis,
                if terminal {
                    signed_out()
                } else {
                    context(A, TOKEN)
                },
            );
            assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
            let retry = core.logout().unwrap();
            assert_eq!(retry.kind(), RequestKind::Logout);
            assert_eq!(retry.csrf_token(), Some(TOKEN));
            core.complete(&retry, no_content());
            assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
            assert!(core.logout_retry_token.is_none());
            assert!(core.logout_target_fingerprint.is_none());
        }
    }
    #[test]
    fn logout_cleanup_erases_reusable_retry_but_pending_intent_survives_route_revisit() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let logout = core.logout().unwrap();
        core.route_exit();
        assert!(core.logout_retry_token.is_none());
        assert!(core.csrf_token.is_none());
        assert!(core.complete(&logout, no_content()).is_none());
        let request = core.recheck().unwrap();
        core.complete(&request, signed_out());
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        let retry = core.logout().unwrap();
        assert_eq!(retry.kind(), RequestKind::Context);
        assert!(core.complete(&retry, signed_out()).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
    }
    #[test]
    fn logout_after_route_revisit_only_uses_fresh_exact_target_context() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let logout = core.logout().unwrap();
        core.complete(&logout, Err(AccountFailure::Transport));
        core.route_exit();
        let retry = core.logout().unwrap();
        assert_eq!(retry.kind(), RequestKind::Context);
        let revocation = core.complete(&retry, context(A, TOKEN)).unwrap();
        assert_eq!(revocation.kind(), RequestKind::Logout);
        core.complete(&revocation, no_content());
        assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
    }
    #[test]
    fn pending_logout_never_revokes_another_account_new_session_or_restarted_adapter_context() {
        for (account, token) in [(B, OTHER_TOKEN), (A, OTHER_TOKEN)] {
            for cleanup in [false, true] {
                let mut core = AccountCore::default();
                ready(&mut core, A);
                let logout = core.logout().unwrap();
                let diagnosis = core
                    .complete(&logout, Err(AccountFailure::Transport))
                    .unwrap();
                if cleanup {
                    core.route_exit();
                    let request = core.logout().unwrap();
                    assert!(core.complete(&request, context(account, token)).is_none());
                } else {
                    assert!(core.complete(&diagnosis, context(account, token)).is_none());
                }
                assert_eq!(core.snapshot().status, AccountStatus::LogoutContextChanged);
                assert!(core.logout_retry_token.is_none());
                assert!(core.csrf_token.is_none());
                let retry = core.logout().unwrap();
                assert_eq!(retry.kind(), RequestKind::Context);
                assert!(core.complete(&retry, context(account, token)).is_none());
                assert_eq!(core.snapshot().status, AccountStatus::LogoutContextChanged);
            }
        }
    }
    #[test]
    fn malformed_success_body_does_not_confirm_logout_or_rotation() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let logout = core.logout().unwrap();
        let request = core
            .complete(
                &logout,
                Ok(HttpResponse {
                    status: 204,
                    body: b"{}".to_vec(),
                }),
            )
            .unwrap();
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        core.complete(&request, signed_out());
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
    }
    #[test]
    fn unavailable_is_distinct_from_signed_out_and_missing_lifecycle_hooks_fail_closed() {
        let mut core = AccountCore::default();
        let request = core.recheck().unwrap();
        core.complete(&request, Err(AccountFailure::Unavailable));
        assert_eq!(core.snapshot().status, AccountStatus::Unavailable);
        ready(&mut core, A);
        core.lifecycle_unavailable();
        assert_eq!(core.snapshot().status, AccountStatus::Unavailable);
        assert!(core.csrf_token.is_none());
        assert!(core.context_account.is_none());
    }
}
