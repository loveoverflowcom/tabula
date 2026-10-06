//! Generation-fenced account decisions for ADR-0031 §6 and ADR-0036 PR3.
//!
//! The core holds only the current document's CSRF context, never credentials.
//! HTTP DTO validation is necessary but cannot replace durable server authority.

use serde::{de::IgnoredAny, Deserialize};
use sha2::{Digest, Sha256};
use tabula_session_http::{
    ContextResponse, LoginStartResponse, SelfProfileResponse, SessionDisposition,
};

/// Initial authenticated deployment is same-origin HTTPS only (ADR-0031 §2).
#[must_use]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Native fixtures do not Fetch.
pub fn trusted_browser_scheme(protocol: &str) -> bool {
    protocol == "https:"
}

/// Maximum decoded response body accepted by this compact isolated client.
pub const MAX_RESPONSE_BYTES: usize = 8192;

// Serde structs also accept positional arrays. Keep the HTTP object boundary
// without retaining a recursive JSON Value tree in the browser. The subsequent
// DTO decode uses the original bytes and still rejects duplicate known fields.
fn object_body(body: &[u8]) -> bool {
    body.iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'{')
}

#[derive(Deserialize)]
struct ContextShape {
    #[serde(rename = "capabilities")]
    _capabilities: ObjectShape,
}

struct ObjectShape;

impl<'de> Deserialize<'de> for ObjectShape {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = ObjectShape;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(ObjectShape)
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

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
    LoginRedirecting,
    LoginUnavailable,
    LogoutStorageUnavailable,
}

/// One same-document operation; repeated clicks cannot create another request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountOperation {
    Login,
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
    pub login_available: bool,
}

/// Fixed same-origin endpoints, never user-supplied URLs or account overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Login,
    LoginCancel,
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
            RequestKind::Login => "/api/v1/auth/login",
            RequestKind::Context => "/api/v1/auth/context",
            RequestKind::Profile => "/api/v1/me",
            RequestKind::Refresh => "/api/v1/auth/refresh",
            RequestKind::Logout | RequestKind::LoginCancel => "/api/v1/auth/logout",
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
    /// Non-authorizing key for a targeted revocation receipt, never the CSRF itself.
    #[must_use]
    pub fn logout_intent(&self) -> Option<String> {
        (self.kind == RequestKind::Logout)
            .then(|| {
                self.csrf_token.as_ref().map(|token| {
                    encode_logout_fingerprint(&Sha256::digest(token.as_bytes()).into())
                })
            })
            .flatten()
    }
    #[must_use]
    pub const fn body(&self) -> Option<&'static str> {
        match self.kind {
            RequestKind::Context | RequestKind::Profile => None,
            _ => Some("{}"),
        }
    }
}

/// One-use current-generation top-level navigation. Provider URLs stay out of Debug/UI.
#[derive(PartialEq, Eq)]
pub struct LoginNavigation {
    generation: u64,
    authorization_url: String,
}
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl LoginNavigation {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }
}
impl std::fmt::Debug for LoginNavigation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginNavigation")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
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
    DiagnoseLogin,
    StartLogin,
}

/// Maximum unresolved non-authorizing targets per browser origin; excess fails closed.
pub const MAX_LOGOUT_INTENTS: usize = 8;

fn encode_logout_fingerprint(fingerprint: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut value = String::from("v1:");
    for byte in fingerprint {
        let _ = write!(value, "{byte:02x}");
    }
    value
}
pub(super) fn decode_logout_fingerprint(value: &str) -> Result<[u8; 32], InvalidLogoutIntent> {
    if value.len() != 67
        || !value.starts_with("v1:")
        || !value.as_bytes()[3..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(InvalidLogoutIntent);
    }
    let mut fingerprint = [0; 32];
    for (index, byte) in fingerprint.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[3 + index * 2..5 + index * 2], 16)
            .map_err(|_| InvalidLogoutIntent)?;
    }
    Ok(fingerprint)
}

/// A malformed saved suppression hint; absence and corruption are distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidLogoutIntent;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SocialAccess {
    Available,
    Unavailable,
}

/// Pure operation lifecycle shared by route owners in one application document.
/// Logout suppression can be exported as bounded non-authorizing fingerprints;
/// identity, credentials and CSRF never cross document cleanup/persistence.
pub struct AccountCore {
    registration_token: Option<String>,
    social_access: SocialAccess,
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
    logout_target_fingerprints: Vec<[u8; 32]>,
    context_purpose: ContextPurpose,
    login_token: Option<String>,
    login_navigation: Option<LoginNavigation>,
    persistence_unavailable: bool,
}
impl Default for AccountCore {
    fn default() -> Self {
        Self {
            registration_token: None,
            social_access: SocialAccess::Unavailable,
            generation: 0,
            sequence: 0,
            pending: None,
            snapshot: AccountSnapshot {
                presentation_generation: 0,
                status: AccountStatus::Resolving,
                busy: None,
                login_available: false,
            },
            context_account: None,
            csrf_token: None,
            had_authenticated: false,
            logout_suppressed: false,
            logout_retry_token: None,
            logout_target_fingerprints: Vec::new(),
            context_purpose: ContextPurpose::ReadProfile,
            login_token: None,
            login_navigation: None,
            persistence_unavailable: false,
        }
    }
}
impl AccountCore {
    /// Current document-only control facts for v2 leaf adapters (ADR-0043).
    /// The caller must also fence route, connectivity and presentation generation.
    #[cfg(feature = "account-social")]
    pub(super) fn current_document_context(&self) -> Option<(String, String)> {
        if self.pending.is_some() || self.logout_suppressed || self.persistence_unavailable {
            return None;
        }
        let AccountStatus::Authenticated { account_id } = &self.snapshot.status else {
            return None;
        };
        if self.snapshot.busy.is_some() || self.context_account.as_ref() != Some(account_id) {
            return None;
        }
        Some((account_id.clone(), self.csrf_token.clone()?))
    }

    /// Signed-out preauthentication control for explicit enrollment navigation.
    #[cfg(feature = "account-social")]
    pub(super) fn current_pre_auth_control(&self) -> Option<String> {
        if self.pending.is_some()
            || self.logout_suppressed
            || self.persistence_unavailable
            || self.snapshot.busy.is_some()
            || !matches!(
                self.snapshot.status,
                AccountStatus::SignedOut | AccountStatus::Expired
            )
        {
            return None;
        }
        self.registration_token.clone()
    }

    #[cfg(feature = "account-social")]
    pub(super) fn social_available(&self) -> bool {
        self.social_access == SocialAccess::Available && self.current_document_context().is_some()
    }
    #[must_use]
    pub fn snapshot(&self) -> AccountSnapshot {
        self.snapshot.clone()
    }

    fn clear_private(&mut self) {
        self.registration_token = None;
        self.social_access = SocialAccess::Unavailable;
        self.context_account = None;
        self.csrf_token = None;
        self.login_token = None;
    }
    fn advance(&mut self) -> bool {
        self.pending = None;
        self.login_navigation = None;
        self.clear_private();
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
            true
        } else {
            self.snapshot = AccountSnapshot {
                presentation_generation: self.generation,
                status: AccountStatus::Error,
                busy: None,
                login_available: false,
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
            login_available: !self.logout_suppressed
                && !self.persistence_unavailable
                && self.login_token.is_some()
                && matches!(status, AccountStatus::SignedOut | AccountStatus::Expired),
            status: if self.persistence_unavailable {
                AccountStatus::LogoutStorageUnavailable
            } else {
                status
            },
            busy: None,
        };
    }

    /// Explicit bootstrap/recovery. Pending operations are single-flight.
    pub fn recheck(&mut self) -> Option<AccountRequest> {
        if self.pending.is_some()
            || self.snapshot.busy == Some(AccountOperation::Login)
            || self.persistence_unavailable
            || !self.advance()
        {
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
            login_available: false,
        };
        self.context(ContextPurpose::ReadProfile)
    }

    /// Explicit invited-account login starts only from validated signed-out context.
    /// A live account or unresolved revocation must be logged out first.
    pub fn login(&mut self) -> Option<AccountRequest> {
        if self.pending.is_some()
            || !self.snapshot.login_available
            || self.logout_suppressed
            || self.persistence_unavailable
            || !matches!(
                self.snapshot.status,
                AccountStatus::SignedOut | AccountStatus::Expired
            )
        {
            return None;
        }
        let token = self.login_token.clone()?;
        if !self.advance() {
            return None;
        }
        self.snapshot = AccountSnapshot {
            status: AccountStatus::Resolving,
            busy: Some(AccountOperation::Login),
            presentation_generation: self.generation,
            login_available: false,
        };
        // Explicit user intent first retires any earlier pending/consumed preauth
        // attempt. This token cannot revoke an authenticated replacement session.
        self.request(RequestKind::LoginCancel, Some(token))
    }

    /// Consume the current validated navigation once; URLs are never presentation data.
    pub fn take_login_navigation(&mut self) -> Option<LoginNavigation> {
        self.login_navigation.take().filter(|navigation| {
            navigation.generation == self.generation
                && self.snapshot.status == AccountStatus::LoginRedirecting
                && !self.logout_suppressed
                && !self.persistence_unavailable
        })
    }

    /// A failed top-level navigation never restores the retired context.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn login_navigation_failed(&mut self) {
        self.advance();
        self.finish(AccountStatus::LoginUnavailable);
    }

    /// Exact current authenticated context hint, for a final synchronous marker read.
    #[must_use]
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn current_context_intent(&self) -> Option<String> {
        self.context_account.as_ref()?;
        self.csrf_token
            .as_ref()
            .map(|token| encode_logout_fingerprint(&Sha256::digest(token.as_bytes()).into()))
    }

    /// Whether a queued next step survived storage/lifecycle fencing.
    #[must_use]
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn request_pending(&self, request: &AccountRequest) -> bool {
        self.pending.as_ref() == Some(request)
    }

    /// Canonical non-authorizing intents; no account ID, CSRF or credential is saved.
    /// Separate immutable target keys prevent cross-document lost-update races.
    #[must_use]
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn logout_intents(&self) -> Vec<String> {
        self.logout_target_fingerprints
            .iter()
            .map(encode_logout_fingerprint)
            .collect()
    }

    /// Single-target compatibility for isolated lifecycle fixtures.
    #[must_use]
    #[cfg(test)]
    pub fn logout_intent(&self) -> Option<String> {
        self.logout_intents().into_iter().next()
    }

    /// Import a same-origin suppression hint before current authority is resolved.
    /// Hints are merged, never replaced or cleared by a storage read/removal event.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn restore_logout_intent(
        &mut self,
        value: Option<&str>,
    ) -> Result<(), InvalidLogoutIntent> {
        let Some(value) = value else {
            return Ok(());
        };
        let fingerprint = decode_logout_fingerprint(value)?;
        if !self.logout_target_fingerprints.contains(&fingerprint) {
            if self.logout_target_fingerprints.len() >= MAX_LOGOUT_INTENTS {
                return Err(InvalidLogoutIntent);
            }
            self.logout_retry_token = None;
            self.advance();
            self.logout_suppressed = true;
            self.logout_target_fingerprints.push(fingerprint);
            self.finish(AccountStatus::LogoutPending);
        }
        Ok(())
    }

    /// Storage failure cannot be mistaken for an absent intent or allow login.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Browser persistence boundary.
    pub fn logout_storage_unavailable(&mut self) {
        self.persistence_unavailable = true;
        if self
            .pending
            .as_ref()
            .is_some_and(|request| request.kind == RequestKind::Logout)
        {
            // Still attempt the already targeted revocation; persistence failure
            // cannot turn a server receipt into a claimed cross-reload guarantee.
            self.clear_private();
            self.snapshot.status = AccountStatus::LogoutStorageUnavailable;
            self.snapshot.login_available = false;
        } else {
            self.advance();
            self.finish(AccountStatus::LogoutStorageUnavailable);
        }
    }

    /// A successful storage read enables a fresh bootstrap, never clears logout intent.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Browser persistence boundary.
    pub fn logout_storage_ready(&mut self) {
        self.persistence_unavailable = false;
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
            login_available: false,
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
            self.logout_target_fingerprints
                .push(Sha256::digest(token.as_bytes()).into());
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
            login_available: false,
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
    /// A browser disconnection retires requests and private data without deciding
    /// session validity. A targeted, unconfirmed logout remains unconfirmed.
    pub fn disconnect(&mut self) {
        self.advance();
        self.finish(self.failure_status(AccountFailure::Transport));
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
        if self.persistence_unavailable {
            return AccountStatus::LogoutStorageUnavailable;
        }
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
        if self.persistence_unavailable && request.kind != RequestKind::Logout {
            self.clear_private();
            self.finish(AccountStatus::LogoutStorageUnavailable);
            return None;
        }
        let response = match result {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            Ok(_) => return self.failed(request.kind, AccountFailure::Protocol),
            Err(failure) => return self.failed(request.kind, failure),
        };
        match request.kind {
            RequestKind::Login => self.login_complete(&response),
            RequestKind::LoginCancel => {
                if response.status != 204 || !response.body.is_empty() {
                    return self.failed(RequestKind::LoginCancel, status_failure(response.status));
                }
                self.context(ContextPurpose::StartLogin)
            }
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
                    let revoked = request
                        .csrf_token
                        .as_ref()
                        .map(|token| <[u8; 32]>::from(Sha256::digest(token.as_bytes())));
                    self.logout_target_fingerprints
                        .retain(|fingerprint| Some(*fingerprint) != revoked);
                    self.logout_suppressed = !self.logout_target_fingerprints.is_empty();
                    self.had_authenticated = false;
                    self.finish(if self.logout_suppressed {
                        AccountStatus::LogoutPending
                    } else {
                        AccountStatus::SignedOut
                    });
                    None
                } else {
                    self.context(ContextPurpose::ReadProfile)
                }
            }
        }
    }
    fn failed(&mut self, kind: RequestKind, failure: AccountFailure) -> Option<AccountRequest> {
        self.clear_private();
        if matches!(kind, RequestKind::Login | RequestKind::LoginCancel) {
            self.snapshot.status = AccountStatus::LoginUnavailable;
            self.context(ContextPurpose::DiagnoseLogin)
        } else if kind == RequestKind::Context {
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
        if !object_body(&response.body)
            || serde_json::from_slice::<ContextShape>(&response.body).is_err()
        {
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
        self.apply_context(context)
    }
    fn apply_context(&mut self, context: ContextResponse) -> Option<AccountRequest> {
        match context.disposition {
            SessionDisposition::Authenticated => {
                self.social_access = if context.capabilities.friends {
                    SocialAccess::Available
                } else {
                    SocialAccess::Unavailable
                };
                self.had_authenticated = true;
                self.context_account = context.account_id;
                self.csrf_token = context.csrf_token;
                if self.logout_suppressed {
                    let same_target = self.csrf_token.as_ref().is_some_and(|token| {
                        self.logout_target_fingerprints
                            .contains(&Sha256::digest(token.as_bytes()).into())
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
                } else if matches!(
                    self.context_purpose,
                    ContextPurpose::DiagnoseLogin | ContextPurpose::StartLogin
                ) {
                    self.clear_private();
                    self.finish(AccountStatus::LoginUnavailable);
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
                self.registration_token = context
                    .capabilities
                    .register
                    .then(|| context.csrf_token.clone())
                    .flatten();
                if matches!(self.context_purpose, ContextPurpose::StartLogin) {
                    if self.logout_suppressed || !context.capabilities.login {
                        self.finish(if self.logout_suppressed {
                            AccountStatus::LogoutPending
                        } else {
                            AccountStatus::LoginUnavailable
                        });
                        return None;
                    }
                    return self.request(RequestKind::Login, context.csrf_token);
                }
                if !self.logout_suppressed
                    && !matches!(self.context_purpose, ContextPurpose::DiagnoseLogin)
                {
                    self.login_token = context
                        .capabilities
                        .login
                        .then_some(context.csrf_token)
                        .flatten();
                }
                self.finish(if self.logout_suppressed {
                    AccountStatus::LogoutPending
                } else if matches!(self.context_purpose, ContextPurpose::DiagnoseLogin) {
                    AccountStatus::LoginUnavailable
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
    fn login_complete(&mut self, response: &HttpResponse) -> Option<AccountRequest> {
        if response.status != 200 || !object_body(&response.body) {
            return self.failed(RequestKind::Login, status_failure(response.status));
        }
        let login = serde_json::from_slice::<LoginStartResponse>(&response.body)
            .ok()
            .filter(|value| value.validate().is_ok());
        let Some(login) = login else {
            return self.failed(RequestKind::Login, AccountFailure::Protocol);
        };
        self.clear_private();
        self.login_navigation = Some(LoginNavigation {
            generation: self.generation,
            authorization_url: login.authorization_url,
        });
        self.finish(AccountStatus::LoginRedirecting);
        self.snapshot.busy = Some(AccountOperation::Login);
        None
    }

    fn profile_complete(&mut self, response: &HttpResponse) -> Option<AccountRequest> {
        if response.status != 200 {
            return self.failed(RequestKind::Profile, status_failure(response.status));
        }
        if !object_body(&response.body) {
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
    fn disconnection_clears_private_facts_and_requires_fresh_context_then_profile() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let generation = core.snapshot().presentation_generation;
        core.disconnect();
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.snapshot().busy.is_none());
        assert!(core.snapshot().presentation_generation > generation);
        assert!(core.context_account.is_none());
        assert!(core.csrf_token.is_none());
        assert!(core.login_token.is_none());
        assert!(core.refresh().is_none());
        let context_request = core.recheck().unwrap();
        assert_eq!(context_request.kind(), RequestKind::Context);
        let profile_request = core
            .complete(&context_request, context(B, OTHER_TOKEN))
            .unwrap();
        assert_eq!(core.snapshot().status, AccountStatus::Resolving);
        core.complete(&profile_request, profile(B));
        assert_eq!(
            core.snapshot().status,
            AccountStatus::Authenticated {
                account_id: B.to_owned()
            }
        );
    }
    #[test]
    fn disconnection_retires_in_flight_profile_and_login_navigation() {
        let mut core = AccountCore::default();
        let context_request = core.recheck().unwrap();
        let old_profile = core.complete(&context_request, context(A, TOKEN)).unwrap();
        core.disconnect();
        let disconnected = core.snapshot();
        assert!(core.complete(&old_profile, profile(A)).is_none());
        assert_eq!(core.snapshot(), disconnected);

        let context_request = core.recheck().unwrap();
        core.complete(
            &context_request,
            response(
                200,
                &json!({"version":1,"disposition":"signed_out","account_id":null,
                "csrf_token":TOKEN,"capabilities":{"login":true,"register":false,
                "friends":false,"read_self_profile":false}}),
            ),
        );
        let cancel = core.login().unwrap();
        let context_request = core.complete(&cancel, no_content()).unwrap();
        let login = core
            .complete(
                &context_request,
                response(
                    200,
                    &json!({"version":1,"disposition":"signed_out","account_id":null,
                "csrf_token":TOKEN,"capabilities":{"login":true,"register":false,
                "friends":false,"read_self_profile":false}}),
                ),
            )
            .unwrap();
        core.complete(
            &login,
            response(
                200,
                &json!({"version":1,"authorization_url":"https://id.example/oauth2/authorize"}),
            ),
        );
        assert_eq!(core.snapshot().status, AccountStatus::LoginRedirecting);
        core.disconnect();
        assert_eq!(core.snapshot().status, AccountStatus::Disconnected);
        assert!(core.take_login_navigation().is_none());
        assert!(!core.snapshot().login_available);
    }
    #[test]
    fn disconnection_keeps_unconfirmed_logout_and_storage_failure_masked() {
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let logout = core.logout().unwrap();
        let intents = core.logout_intents();
        core.disconnect();
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        assert_eq!(core.logout_intents(), intents);
        assert!(core.complete(&logout, no_content()).is_none());
        assert_eq!(core.logout_intents(), intents);
        let context_request = core.recheck().unwrap();
        assert!(core.complete(&context_request, context(A, TOKEN)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        let explicit_retry = core.logout().unwrap();
        assert_eq!(explicit_retry.kind(), RequestKind::Logout);
        assert_eq!(explicit_retry.csrf_token(), Some(TOKEN));
        core.complete(&explicit_retry, no_content());
        assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
        assert!(core.logout_intents().is_empty());

        core.logout_storage_unavailable();
        core.disconnect();
        assert_eq!(
            core.snapshot().status,
            AccountStatus::LogoutStorageUnavailable
        );
        assert!(core.recheck().is_none());
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
            assert!(core.logout_target_fingerprints.is_empty());
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
    fn login_context() -> Result<HttpResponse, AccountFailure> {
        response(
            200,
            &json!({"version":1,"disposition":"signed_out","account_id":null,
            "csrf_token":TOKEN,"capabilities":{"login":true,"register":false,"friends":false,"read_self_profile":false}}),
        )
    }
    fn login_ready(core: &mut AccountCore) {
        let request = core.recheck().unwrap();
        assert!(core.complete(&request, login_context()).is_none());
        assert!(core.snapshot().login_available);
    }
    fn start_login(core: &mut AccountCore) -> AccountRequest {
        let cancellation = core.login().unwrap();
        assert_eq!(cancellation.kind(), RequestKind::LoginCancel);
        let context = core.complete(&cancellation, no_content()).unwrap();
        assert_eq!(context.kind(), RequestKind::Context);
        let login = core.complete(&context, login_context()).unwrap();
        assert_eq!(login.kind(), RequestKind::Login);
        login
    }
    fn authorization_url() -> Result<HttpResponse, AccountFailure> {
        response(
            200,
            &json!({"version":1,"authorization_url":"https://idm.example/ui/oauth2?state=test-only&code_challenge=test-only"}),
        )
    }

    #[test]
    fn explicit_login_requires_current_signed_out_capability_and_is_single_flight() {
        let mut core = AccountCore::default();
        assert!(core.login().is_none());
        let context_request = core.recheck().unwrap();
        core.complete(&context_request, signed_out());
        assert!(!core.snapshot().login_available);
        assert!(core.login().is_none());
        ready(&mut core, A);
        assert!(core.login().is_none());
        let logout = core.logout().unwrap();
        core.complete(&logout, no_content());
        assert!(
            core.login().is_none(),
            "logout receipt is not preauth context"
        );
        login_ready(&mut core);
        let login = start_login(&mut core);
        assert_eq!(login.kind(), RequestKind::Login);
        assert_eq!(login.path(), "/api/v1/auth/login");
        assert_eq!(login.method(), "POST");
        assert_eq!(login.body(), Some("{}"));
        assert_eq!(login.csrf_token(), Some(TOKEN));
        assert_eq!(core.snapshot().busy, Some(AccountOperation::Login));
        assert!(!core.snapshot().login_available);
        assert!(core.csrf_token.is_none());
        assert!(core.context_account.is_none());
        assert!(core.login().is_none());
        assert!(core.logout().is_none());
        assert!(core.recheck().is_none());
        assert!(core.complete(&login, authorization_url()).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::LoginRedirecting);
        let navigation = core.take_login_navigation().unwrap();
        assert_eq!(
            navigation.generation(),
            core.snapshot().presentation_generation
        );
        assert!(navigation
            .authorization_url()
            .starts_with("https://idm.example/"));
        assert!(!format!("{navigation:?}").contains("state="));
        assert!(!format!("{login:?}").contains(TOKEN));
        assert!(core.take_login_navigation().is_none());
        assert!(
            core.recheck().is_none(),
            "navigation remains single-flight until departure/cancel"
        );
    }

    #[test]
    fn cancelled_hidden_departed_and_replacement_login_cannot_navigate() {
        for cleanup in [
            AccountCore::cancel as fn(&mut AccountCore),
            AccountCore::route_exit,
            AccountCore::suspend,
        ] {
            let mut core = AccountCore::default();
            login_ready(&mut core);
            let old = start_login(&mut core);
            cleanup(&mut core);
            assert!(core.complete(&old, authorization_url()).is_none());
            assert!(core.take_login_navigation().is_none());
            assert!(core.csrf_token.is_none());
            login_ready(&mut core);
            let new = start_login(&mut core);
            assert!(core.complete(&old, authorization_url()).is_none());
            assert!(core.take_login_navigation().is_none());
            core.complete(&new, authorization_url());
            cleanup(&mut core);
            assert!(core.take_login_navigation().is_none());
        }
    }

    #[test]
    fn rejected_provider_responses_never_navigate_or_restore_active_account() {
        let mut bodies = vec![
            b"[1,\"https://idm.example/\"]".to_vec(),
            br#"{"version":1,"version":1,"authorization_url":"https://idm.example/"}"#.to_vec(),
            br#"{"version":1,"authorization_url":"https://idm.example/","return_url":"https://evil.example/"}"#.to_vec(),
            br#"{"version":2,"authorization_url":"https://idm.example/"}"#.to_vec(),
            b"{".to_vec(),
            vec![b' '; MAX_RESPONSE_BYTES + 1],
        ];
        for url in [
            "http://idm.example/",
            "javascript:alert(1)",
            "//idm.example/",
            "https://user@idm.example/",
            "https://idm.example/#token",
            "https://idm.example/\\evil",
            "https://idm.example/ space",
        ] {
            bodies.push(serde_json::to_vec(&json!({"version":1,"authorization_url":url})).unwrap());
        }
        for body in bodies {
            let mut core = AccountCore::default();
            login_ready(&mut core);
            let login = start_login(&mut core);
            let diagnosis = core
                .complete(&login, Ok(HttpResponse { status: 200, body }))
                .unwrap();
            assert_eq!(diagnosis.kind(), RequestKind::Context);
            assert!(core.take_login_navigation().is_none());
            assert!(!core.snapshot().login_available);
            assert!(core.complete(&diagnosis, context(B, OTHER_TOKEN)).is_none());
            assert_eq!(core.snapshot().status, AccountStatus::LoginUnavailable);
            assert!(core.context_account.is_none());
            assert!(core.csrf_token.is_none());
        }
    }

    #[test]
    fn provider_outage_and_failed_navigation_require_explicit_new_context() {
        for failure in [
            AccountFailure::Transport,
            AccountFailure::Unavailable,
            AccountFailure::Unauthenticated,
        ] {
            let mut core = AccountCore::default();
            login_ready(&mut core);
            let login = start_login(&mut core);
            let diagnosis = core.complete(&login, Err(failure)).unwrap();
            core.complete(&diagnosis, login_context());
            assert_eq!(core.snapshot().status, AccountStatus::LoginUnavailable);
            assert!(core.login().is_none());
            assert!(core.take_login_navigation().is_none());
            login_ready(&mut core);
            let login = start_login(&mut core);
            core.complete(&login, authorization_url());
            core.take_login_navigation().unwrap();
            core.login_navigation_failed();
            assert_eq!(core.snapshot().status, AccountStatus::LoginUnavailable);
            assert!(core.login().is_none());
        }
    }

    #[test]
    fn persisted_logout_is_bounded_non_authorizing_and_survives_offline_reload() {
        let mut original = AccountCore::default();
        ready(&mut original, A);
        let logout = original.logout().unwrap();
        let intent = original.logout_intent().unwrap();
        assert_eq!(intent.len(), 67);
        assert!(!intent.contains(TOKEN));
        assert!(!intent.contains(A));
        let mut reloaded = AccountCore::default();
        reloaded.restore_logout_intent(Some(&intent)).unwrap();
        assert_eq!(reloaded.snapshot().status, AccountStatus::LogoutPending);
        assert!(reloaded.login().is_none());
        let offline = reloaded.recheck().unwrap();
        reloaded.complete(&offline, Err(AccountFailure::Transport));
        assert_eq!(reloaded.snapshot().status, AccountStatus::LogoutPending);
        let bootstrap = reloaded.recheck().unwrap();
        assert!(reloaded.complete(&bootstrap, context(A, TOKEN)).is_none());
        assert_eq!(reloaded.snapshot().status, AccountStatus::LogoutPending);
        assert!(reloaded.context_account.is_none());
        assert!(reloaded.csrf_token.is_none());
        let retry = reloaded.logout().unwrap();
        assert_eq!(retry.kind(), RequestKind::Logout);
        assert_eq!(retry.csrf_token(), Some(TOKEN));
        reloaded.complete(&retry, no_content());
        assert!(reloaded.logout_intent().is_none());
        login_ready(&mut reloaded);
        let login = start_login(&mut reloaded);
        reloaded.complete(&login, authorization_url());
        assert!(reloaded.take_login_navigation().is_some());
        assert!(original
            .complete(&logout, Err(AccountFailure::Transport))
            .is_some());
    }

    #[test]
    fn persisted_logout_never_targets_replacement_or_unlocks_from_signed_out_absence() {
        let mut original = AccountCore::default();
        ready(&mut original, A);
        original.logout().unwrap();
        let intent = original.logout_intent().unwrap();
        for (account, token) in [(A, OTHER_TOKEN), (B, OTHER_TOKEN)] {
            let mut reloaded = AccountCore::default();
            reloaded.restore_logout_intent(Some(&intent)).unwrap();
            let retry = reloaded.logout().unwrap();
            assert_eq!(retry.kind(), RequestKind::Context);
            assert!(reloaded.complete(&retry, context(account, token)).is_none());
            assert_eq!(
                reloaded.snapshot().status,
                AccountStatus::LogoutContextChanged
            );
            assert!(reloaded.logout_retry_token.is_none());
            assert!(reloaded.login().is_none());
            let recheck = reloaded.recheck().unwrap();
            reloaded.complete(&recheck, login_context());
            assert_eq!(reloaded.snapshot().status, AccountStatus::LogoutPending);
            assert!(!reloaded.snapshot().login_available);
            reloaded.restore_logout_intent(None).unwrap();
            assert_eq!(reloaded.logout_intent().as_deref(), Some(intent.as_str()));
        }
    }

    #[test]
    fn corrupt_or_unavailable_storage_cannot_authorize_recovery_and_write_failure_still_revokes() {
        for corrupt in [
            "",
            "v2:unknown",
            "v1:",
            "v1:ZZ",
            &format!("v1:{}", "a".repeat(65)),
        ] {
            let mut core = AccountCore::default();
            assert_eq!(
                core.restore_logout_intent(Some(corrupt)),
                Err(InvalidLogoutIntent)
            );
            core.logout_storage_unavailable();
            assert_eq!(
                core.snapshot().status,
                AccountStatus::LogoutStorageUnavailable
            );
            assert!(core.recheck().is_none());
            assert!(core.login().is_none());
            core.logout_storage_ready();
            let request = core.recheck().unwrap();
            core.complete(&request, signed_out());
            assert_eq!(core.snapshot().status, AccountStatus::SignedOut);
        }
        let mut core = AccountCore::default();
        ready(&mut core, A);
        let logout = core.logout().unwrap();
        core.logout_storage_unavailable();
        core.complete(&logout, no_content());
        assert!(
            core.logout_intent().is_none(),
            "a targeted revocation receipt remains valid"
        );
        assert_eq!(
            core.snapshot().status,
            AccountStatus::LogoutStorageUnavailable
        );
        assert!(core.recheck().is_none());
        core.logout_storage_ready();
        login_ready(&mut core);
    }
    #[test]
    fn deliberate_login_retires_pending_preauth_then_refetches_before_retrying_lost_start() {
        let mut core = AccountCore::default();
        login_ready(&mut core);
        let old_start = start_login(&mut core);
        let diagnose = core
            .complete(&old_start, Err(AccountFailure::Transport))
            .unwrap();
        core.complete(&diagnose, login_context());
        assert_eq!(core.snapshot().status, AccountStatus::LoginUnavailable);
        assert!(core.login().is_none());
        login_ready(&mut core);
        let cancellation = core.login().unwrap();
        assert_eq!(cancellation.kind(), RequestKind::LoginCancel);
        assert_eq!(cancellation.path(), "/api/v1/auth/logout");
        assert_eq!(cancellation.csrf_token(), Some(TOKEN));
        assert!(core.login().is_none());
        let fresh = core.complete(&cancellation, no_content()).unwrap();
        assert_eq!(fresh.kind(), RequestKind::Context);
        let newer_context = response(
            200,
            &json!({"version":1,"disposition":"signed_out","account_id":null,
            "csrf_token":OTHER_TOKEN,"capabilities":{"login":true,"register":false,"friends":false,"read_self_profile":false}}),
        );
        let new_start = core.complete(&fresh, newer_context).unwrap();
        assert_eq!(new_start.kind(), RequestKind::Login);
        assert_eq!(new_start.csrf_token(), Some(OTHER_TOKEN));
        assert!(core.complete(&old_start, authorization_url()).is_none());
        assert!(core.take_login_navigation().is_none());
        core.complete(&new_start, authorization_url());
        assert!(core.take_login_navigation().is_some());
    }

    #[test]
    fn login_preauth_cancellation_never_switches_active_account_or_clears_logout_marker() {
        let mut core = AccountCore::default();
        login_ready(&mut core);
        let cancellation = core.login().unwrap();
        let fresh = core.complete(&cancellation, no_content()).unwrap();
        assert!(core.complete(&fresh, context(B, OTHER_TOKEN)).is_none());
        assert_eq!(core.snapshot().status, AccountStatus::LoginUnavailable);
        assert!(core.context_account.is_none());
        assert!(core.csrf_token.is_none());

        login_ready(&mut core);
        let cancellation = core.login().unwrap();
        let other_marker = format!("v1:{}", "b".repeat(64));
        core.restore_logout_intent(Some(&other_marker)).unwrap();
        assert!(core.complete(&cancellation, no_content()).is_none());
        assert_eq!(core.logout_intent().as_deref(), Some(other_marker.as_str()));
        assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
        assert!(core.login().is_none());
    }

    #[test]
    fn merged_logout_markers_choose_only_current_exact_target_and_receipt_removes_only_it() {
        let target_a = encode_logout_fingerprint(&Sha256::digest(TOKEN.as_bytes()).into());
        let target_b = encode_logout_fingerprint(&Sha256::digest(OTHER_TOKEN.as_bytes()).into());
        for order in [[&target_a, &target_b], [&target_b, &target_a]] {
            let mut core = AccountCore::default();
            for marker in order {
                core.restore_logout_intent(Some(marker)).unwrap();
            }
            assert_eq!(core.logout_intents().len(), 2);
            let bootstrap = core.recheck().unwrap();
            assert!(core.complete(&bootstrap, context(A, TOKEN)).is_none());
            let receipt_request = core.logout().unwrap();
            assert_eq!(receipt_request.kind(), RequestKind::Logout);
            assert_eq!(receipt_request.csrf_token(), Some(TOKEN));
            assert_eq!(
                receipt_request.logout_intent().as_deref(),
                Some(target_a.as_str())
            );
            core.complete(&receipt_request, no_content());
            assert_eq!(core.logout_intents(), vec![target_b.clone()]);
            assert_eq!(core.snapshot().status, AccountStatus::LogoutPending);
            assert!(core.login().is_none());
            let retry_context = core.logout().unwrap();
            let targeted = core
                .complete(&retry_context, context(B, OTHER_TOKEN))
                .unwrap();
            assert_eq!(targeted.csrf_token(), Some(OTHER_TOKEN));
            core.complete(&targeted, no_content());
            assert!(core.logout_intents().is_empty());
        }
    }

    #[test]
    fn bounded_marker_merge_preserves_all_existing_targets_on_overflow_and_duplicates() {
        let mut core = AccountCore::default();
        for index in 0..MAX_LOGOUT_INTENTS {
            let marker = format!("v1:{index:064x}");
            core.restore_logout_intent(Some(&marker)).unwrap();
            core.restore_logout_intent(Some(&marker)).unwrap();
            assert_eq!(core.logout_intents().len(), index + 1);
        }
        let expected = core.logout_intents();
        assert_eq!(
            core.restore_logout_intent(Some(&format!("v1:{MAX_LOGOUT_INTENTS:064x}"))),
            Err(InvalidLogoutIntent)
        );
        assert_eq!(core.logout_intents(), expected);
        core.logout_storage_unavailable();
        assert_eq!(
            core.snapshot().status,
            AccountStatus::LogoutStorageUnavailable
        );
        assert!(core.recheck().is_none());
    }
}
