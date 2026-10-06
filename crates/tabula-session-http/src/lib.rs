//! ADR-0036 isolated session/context and minimal read-only self-profile HTTP.
//!
//! Default/WASM builds expose only these explicitly versioned DTOs. The
//! non-default native adapters never start a production service. ADR-0043 adds
//! explicitly composed account/social authority; provider verification stays in
//! the auth service library and gameplay authority remains separate.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub mod accounts;

/// Version of this new, isolated JSON boundary (I-13); no gameplay wire changes.
pub const HTTP_CONTRACT_VERSION: u8 = 1;

/// Current disposition from durable authority; unavailable is never signed out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDisposition {
    Authenticated,
    SignedOut,
    Unavailable,
}

/// Actual bounded capabilities; ADR-0043 opens registration/social only in its
/// explicitly composed isolated authority. Bits never establish permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// Independent returned capabilities, not four coupled lifecycle-state bits.
#[allow(clippy::struct_excessive_bools)]
pub struct AccountCapabilities {
    pub login: bool,
    pub register: bool,
    pub friends: bool,
    pub read_self_profile: bool,
}

/// Read-only context. No session credential, record ID, binding or provider data.
/// Synchronizer tokens are document-memory controls, never login credentials.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextResponse {
    pub version: u8,
    pub disposition: SessionDisposition,
    pub account_id: Option<String>,
    pub csrf_token: Option<String>,
    pub capabilities: AccountCapabilities,
}

impl std::fmt::Debug for ContextResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextResponse")
            .field("version", &self.version)
            .field("disposition", &self.disposition)
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "csrf_token",
                &self.csrf_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

/// Minimal permitted immutable self identity. No invented name/handle/statistics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelfProfileResponse {
    pub version: u8,
    pub account_id: String,
}

/// Safe public RFC-9457-style error body; private infrastructure facts stay out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicProblem {
    pub version: u8,
    pub status: u16,
    pub title: String,
    pub code: String,
}

/// Native rotation response only. Browser refresh never serializes a bearer.
/// Intentionally no Debug implementation that could disclose the credential.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRefreshResponse {
    pub version: u8,
    pub credential: String,
}
impl std::fmt::Debug for NativeRefreshResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeRefreshResponse([REDACTED])")
    }
}

#[cfg(all(feature = "isolated", not(target_arch = "wasm32")))]
pub mod isolated;
#[cfg(all(feature = "social", not(target_arch = "wasm32")))]
pub mod social;

/// Invalid or incompatible data from an isolated HTTP response. Validation
/// constrains shape/version only; it does not establish current server authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidHttpResponse;

fn canonical_account_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && u128::from_str_radix(value, 16).is_ok_and(|id| id != 0)
}
fn canonical_token(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        && value
            .as_bytes()
            .last()
            .is_some_and(|last| b"AEIMQUYcgkosw048".contains(last))
}
impl ContextResponse {
    /// Required browser-adapter shape/version check before applying a response.
    /// Client operation-generation/lifecycle checks remain separate obligations.
    pub fn validate_for_browser(&self) -> Result<(), InvalidHttpResponse> {
        if self.version != HTTP_CONTRACT_VERSION
            || self
                .csrf_token
                .as_deref()
                .is_some_and(|token| !canonical_token(token))
        {
            return Err(InvalidHttpResponse);
        }
        let valid = match self.disposition {
            SessionDisposition::Authenticated => {
                self.account_id.as_deref().is_some_and(canonical_account_id)
                    && self.csrf_token.is_some()
                    && self.capabilities.read_self_profile
                    && !self.capabilities.login
                    && !self.capabilities.register
            }
            SessionDisposition::SignedOut => {
                self.account_id.is_none()
                    && !self.capabilities.read_self_profile
                    && !self.capabilities.friends
                    && (!(self.capabilities.login || self.capabilities.register)
                        || self.csrf_token.is_some())
            }
            SessionDisposition::Unavailable => {
                self.account_id.is_none()
                    && self.csrf_token.is_none()
                    && !self.capabilities.login
                    && !self.capabilities.read_self_profile
                    && !self.capabilities.register
                    && !self.capabilities.friends
            }
        };
        if valid {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}
impl SelfProfileResponse {
    /// Checks the new isolated response version and exact immutable ID encoding.
    /// Consumers must also match the current context subject and operation generation.
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        if self.version == HTTP_CONTRACT_VERSION && canonical_account_id(&self.account_id) {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}

/// Versioned opt-in login navigation result, never a session credential.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginStartResponse {
    pub version: u8,
    pub authorization_url: String,
}
impl std::fmt::Debug for LoginStartResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginStartResponse([REDACTED])")
    }
}
impl LoginStartResponse {
    pub const MAX_URL_BYTES: usize = 4096;
    /// Conservative URL shape check; concrete HTTP also parses the provider URL.
    /// Client lifecycle generation and trusted-provider configuration remain separate.
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        let Some(rest) = self.authorization_url.strip_prefix("https://") else {
            return Err(InvalidHttpResponse);
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if self.version != HTTP_CONTRACT_VERSION
            || self.authorization_url.len() > Self::MAX_URL_BYTES
            || !safe_https_authority(authority)
            || self.authorization_url.contains('\\')
            || self.authorization_url.contains('#')
            || !self
                .authorization_url
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(InvalidHttpResponse);
        }
        Ok(())
    }
}

fn safe_https_authority(authority: &str) -> bool {
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((literal, suffix)) = bracketed.split_once(']') else {
            return false;
        };
        if literal.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        if suffix.is_empty() {
            return true;
        }
        let Some(port) = suffix.strip_prefix(':') else {
            return false;
        };
        ("", Some(port))
    } else {
        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        if host.is_empty()
            || host.len() > 253
            || !host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_alphanumeric)
                    && label
                        .as_bytes()
                        .last()
                        .is_some_and(u8::is_ascii_alphanumeric)
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return false;
        }
        (host, port)
    };
    let _ = host;
    port.is_none_or(|port| {
        !port.is_empty()
            && port.bytes().all(|byte| byte.is_ascii_digit())
            && port.parse::<u16>().is_ok()
    })
}

#[cfg(all(test, feature = "isolated", not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "../tests/support/authority.rs"]
mod test_authority;
#[cfg(all(test, feature = "isolated", not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod test_wire;
