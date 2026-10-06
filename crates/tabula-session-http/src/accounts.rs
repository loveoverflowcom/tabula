//! Version 2 account enrollment and permitted profile DTOs (ADR-0043).
//!
//! These are transport values, never authority. Native adapters revalidate input
//! and current session/resource policy; browsers fence operation and route scope.

use serde::{Deserialize, Serialize};

/// Pure social transport contracts; runtime authority is a separate native opt-in.
pub use tabula_lobby::social;

use crate::{canonical_account_id, canonical_token, InvalidHttpResponse, LoginStartResponse};

/// New isolated account surface; the version 1 session surface stays compatible.
pub const ACCOUNT_CONTRACT_VERSION: u8 = 2;

/// Visibility of profile details to other signed-in accounts (ADR-0043).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileVisibility {
    Public,
    Friends,
    Private,
}

/// Provider navigation only; no provider credential or session bearer.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentStartResponse {
    pub version: u8,
    pub authorization_url: String,
}

impl std::fmt::Debug for EnrollmentStartResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnrollmentStartResponse([REDACTED])")
    }
}

impl EnrollmentStartResponse {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        if self.version != ACCOUNT_CONTRACT_VERSION {
            return Err(InvalidHttpResponse);
        }
        LoginStartResponse {
            version: crate::HTTP_CONTRACT_VERSION,
            authorization_url: self.authorization_url.clone(),
        }
        .validate()
    }
}

/// Current enrollment grant state; acceptance never establishes a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentDisposition {
    Ready,
    AcceptedWithoutSession,
    Rejected,
    Unavailable,
}

/// Actual approved registration field bounds, not provider credential policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentFieldPolicy {
    pub handle_min_length: u8,
    pub handle_max_length: u8,
    pub display_name_max_chars: u16,
    pub display_name_max_bytes: u16,
}

impl EnrollmentFieldPolicy {
    pub const APPROVED: Self = Self {
        handle_min_length: 3,
        handle_max_length: 32,
        display_name_max_chars: 64,
        display_name_max_bytes: 256,
    };
}

/// Cookie-bound enrollment control facts. Secret synchronizer stays in memory.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentContextResponse {
    pub version: u8,
    pub disposition: EnrollmentDisposition,
    pub operation_id: Option<String>,
    pub csrf_token: Option<String>,
    pub field_policy: Option<EnrollmentFieldPolicy>,
    /// No agreement exists in this isolated product; only `None` is admitted.
    pub agreement: Option<String>,
}

impl std::fmt::Debug for EnrollmentContextResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollmentContextResponse")
            .field("version", &self.version)
            .field("disposition", &self.disposition)
            .finish_non_exhaustive()
    }
}

impl EnrollmentContextResponse {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        if self.version != ACCOUNT_CONTRACT_VERSION || self.agreement.is_some() {
            return Err(InvalidHttpResponse);
        }
        let ready = self.disposition == EnrollmentDisposition::Ready;
        let controls = self.operation_id.as_deref().is_some_and(valid_operation_id)
            && self.csrf_token.as_deref().is_some_and(canonical_token)
            && self.field_policy == Some(EnrollmentFieldPolicy::APPROVED);
        let empty =
            self.operation_id.is_none() && self.csrf_token.is_none() && self.field_policy.is_none();
        if (ready && controls) || (!ready && empty) {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}

/// Explicit registration mutation; credentials and agreements are absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationRequest {
    pub operation_id: String,
    pub handle: String,
    pub display_name: String,
}

impl RegistrationRequest {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        validate_fields(&self.operation_id, &self.handle, &self.display_name)
    }
}

/// Generic completion deliberately discloses no existing-account/handle reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationDisposition {
    AcceptedWithoutSession,
    Rejected,
}

/// Known durable registration result, never authenticated UI authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationResponse {
    pub version: u8,
    pub disposition: RegistrationDisposition,
}

impl RegistrationResponse {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        version(self.version)
    }
}

/// Current self-only profile and revision for explicit compare-and-set edits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelfAccountProfileResponse {
    pub version: u8,
    pub account_id: String,
    pub handle: String,
    pub display_name: String,
    pub visibility: ProfileVisibility,
    pub revision: u64,
    pub as_of_ms: u64,
}

impl SelfAccountProfileResponse {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        version(self.version)?;
        if canonical_account_id(&self.account_id)
            && valid_handle(&self.handle)
            && valid_display_name(&self.display_name)
            && self.revision > 0
            && i64::try_from(self.revision).is_ok()
            && i64::try_from(self.as_of_ms).is_ok()
        {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}

/// Currently permitted other profile; no policy, provider or private self fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtherAccountProfileResponse {
    pub version: u8,
    pub account_id: String,
    pub handle: String,
    pub display_name: String,
    pub as_of_ms: u64,
}

impl OtherAccountProfileResponse {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        version(self.version)?;
        if canonical_account_id(&self.account_id)
            && valid_handle(&self.handle)
            && valid_display_name(&self.display_name)
            && i64::try_from(self.as_of_ms).is_ok()
        {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}

/// User-approved fields only; handles remain immutable (ADR-0043).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdateRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub display_name: String,
    pub visibility: ProfileVisibility,
}

impl ProfileUpdateRequest {
    pub fn validate(&self) -> Result<(), InvalidHttpResponse> {
        if valid_operation_id(&self.operation_id)
            && self.expected_revision > 0
            && self.expected_revision < i64::MAX as u64
            && valid_display_name(&self.display_name)
        {
            Ok(())
        } else {
            Err(InvalidHttpResponse)
        }
    }
}

fn version(value: u8) -> Result<(), InvalidHttpResponse> {
    if value == ACCOUNT_CONTRACT_VERSION {
        Ok(())
    } else {
        Err(InvalidHttpResponse)
    }
}

/// Canonical operation identifier shape only; possession conveys no authority.
pub fn valid_operation_id(value: &str) -> bool {
    canonical_account_id(value)
}

/// Immutable handle is exact lowercase ASCII, never silently normalized.
pub fn valid_handle(value: &str) -> bool {
    (3..=32).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Preserve Unicode accents/IME output while rejecting controls and edge spaces.
pub fn valid_display_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.chars().count() <= 64
        && value.trim() == value
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
}

fn validate_fields(operation: &str, handle: &str, name: &str) -> Result<(), InvalidHttpResponse> {
    if valid_operation_id(operation) && valid_handle(handle) && valid_display_name(name) {
        Ok(())
    } else {
        Err(InvalidHttpResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_boundaries_preserve_unicode_and_reject_noncanonical_input() {
        for handle in ["abc", "_12", &"a".repeat(32)] {
            assert!(valid_handle(handle));
        }
        for handle in ["", "ab", "Alice", " an ", "nguyễn", &"a".repeat(33)] {
            assert!(!valid_handle(handle));
        }
        for name in ["Nguyễn An", "e\u{301}", &"🦊".repeat(64)] {
            assert!(valid_display_name(name));
        }
        for name in [
            "",
            " a",
            "a ",
            "a\n",
            "a\0b",
            "a\u{2028}b",
            "a\u{2029}b",
            &"a".repeat(65),
        ] {
            assert!(!valid_display_name(name));
        }
    }

    #[test]
    fn enrollment_controls_exist_only_for_ready_grant() {
        let mut value = EnrollmentContextResponse {
            version: 2,
            disposition: EnrollmentDisposition::Ready,
            operation_id: Some("00000000000000000000000000000001".into()),
            csrf_token: Some("A".repeat(43)),
            field_policy: Some(EnrollmentFieldPolicy::APPROVED),
            agreement: None,
        };
        assert!(value.validate().is_ok());
        value.disposition = EnrollmentDisposition::AcceptedWithoutSession;
        assert!(value.validate().is_err());
        value.operation_id = None;
        value.csrf_token = None;
        value.field_policy = None;
        assert!(value.validate().is_ok());
        value.agreement = Some("invented terms".into());
        assert!(value.validate().is_err());
    }

    #[test]
    fn hostile_decode_and_incompatible_profile_are_rejected() {
        let value: RegistrationRequest = serde_json::from_str(
            r#"{"operation_id":"00000000000000000000000000000000","handle":"abc","display_name":"Name"}"#,
        ).unwrap();
        assert!(value.validate().is_err());
        assert!(serde_json::from_str::<RegistrationRequest>(
            r#"{"operation_id":"00000000000000000000000000000001","handle":"abc","display_name":"Name","password":"secret"}"#,
        ).is_err());
        let mut profile = SelfAccountProfileResponse {
            version: 2,
            account_id: "00000000000000000000000000000001".into(),
            handle: "abc".into(),
            display_name: "An".into(),
            visibility: ProfileVisibility::Private,
            revision: 1,
            as_of_ms: 1,
        };
        assert!(profile.validate().is_ok());
        profile.version = 1;
        assert!(profile.validate().is_err());
        profile.version = 2;
        profile.revision = 0;
        assert!(profile.validate().is_err());
        profile.revision = u64::MAX;
        assert!(profile.validate().is_err());
    }
}
