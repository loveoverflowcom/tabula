//! Trusted browser-provider port, composed only by the opt-in auth service.
//! Raw callback values are structure, never authentication proof (ADR-0034).

use std::future::Future;

use crate::{AccountEpoch, ProviderIdentityKey, SessionError};

/// Bounded raw OIDC callback. The provider must compare exact issuer and
/// single-use state, exchange PKCE code and verify nonce/signature/audience.
#[derive(Clone, PartialEq, Eq)]
pub struct BrowserLoginCallback {
    state: String,
    code: String,
    issuer: Option<String>,
}
impl BrowserLoginCallback {
    pub const MAX_STATE_BYTES: usize = 256;
    pub const MAX_CODE_BYTES: usize = 2048;
    pub const MAX_ISSUER_BYTES: usize = 1024;

    /// Checks bounds/ASCII only; provider verification is still mandatory.
    pub fn new(state: String, code: String, issuer: Option<String>) -> Result<Self, SessionError> {
        for (value, max) in [
            (&state, Self::MAX_STATE_BYTES),
            (&code, Self::MAX_CODE_BYTES),
        ] {
            if value.is_empty()
                || value.len() > max
                || !value.bytes().all(|byte| byte.is_ascii_graphic())
            {
                return Err(SessionError::InvalidInput);
            }
        }
        if issuer.as_ref().is_some_and(|value| {
            value.is_empty()
                || value.len() > Self::MAX_ISSUER_BYTES
                || !value.bytes().all(|byte| byte.is_ascii_graphic())
        }) {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self {
            state,
            code,
            issuer,
        })
    }
    pub fn state(&self) -> &str {
        &self.state
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }
}
impl std::fmt::Debug for BrowserLoginCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserLoginCallback([REDACTED])")
    }
}

/// Provider-built HTTPS authorization URL; contains no session credential.
pub struct BrowserLoginStart {
    pub authorization_url: String,
}
impl std::fmt::Debug for BrowserLoginStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserLoginStart([REDACTED])")
    }
}

/// Verified exact identity and its epoch captured before provider navigation.
/// Never resolve a fresh epoch at callback; issuance compares this old value.
#[derive(Debug)]
pub struct CompletedBrowserLogin {
    pub identity: ProviderIdentityKey,
    pub expected_epoch: AccountEpoch,
}

/// Internal trust boundary for a configured, verified OIDC provider (ADR-0034).
/// Begin captures bounded invited identity/epoch facts and independent random
/// state/nonce/PKCE; complete consumes them once, pins issuer+subject and returns
/// the captured epoch. Cancellation is synchronous, bounded and idempotent;
/// racing async operations must not restore cancelled provider state.
/// A structural identity, callback or test double is not provider login proof.
pub trait BrowserLoginProvider: Send + Sync {
    fn begin(
        &self,
        preauth_binding: String,
    ) -> impl Future<Output = Result<BrowserLoginStart, SessionError>> + Send;
    fn complete(
        &self,
        preauth_binding: String,
        callback: BrowserLoginCallback,
    ) -> impl Future<Output = Result<CompletedBrowserLogin, SessionError>> + Send;
    /// Side-effect-free, synchronous and bounded ownership probe. False must
    /// leave the genuine pending flow intact, even with a forged issuer/code.
    /// This is not authentication proof; complete still verifies everything.
    fn matches_callback(
        &self,
        preauth_binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError>;
    fn cancel(&self, preauth_binding: &str);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_raw_fields_are_bounded_ascii_optional_issuer_and_redacted() {
        let value = BrowserLoginCallback::new("state".into(), "code".into(), None).unwrap();
        assert_eq!(value.issuer(), None);
        assert_eq!(format!("{value:?}"), "BrowserLoginCallback([REDACTED])");
        for (state, code, issuer) in [
            (String::new(), "code".to_owned(), None),
            ("x".repeat(257), "code".to_owned(), None),
            ("state".to_owned(), "x".repeat(2049), None),
            ("state".to_owned(), "co de".to_owned(), None),
            ("state".to_owned(), "code".to_owned(), Some(String::new())),
            (
                "state".to_owned(),
                "code".to_owned(),
                Some("x".repeat(1025)),
            ),
        ] {
            assert!(BrowserLoginCallback::new(state, code, issuer).is_err());
        }
    }
}
