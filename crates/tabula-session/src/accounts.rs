//! SQL-free account enrollment and permitted profile contracts (ADR-0044).
use crate::{
    AccountEpoch, BrowserLoginCallback, BrowserLoginStart, CredentialDigest, CredentialOperation,
    HttpSessionAuthority, ProviderIdentityKey, SessionError, SessionPublication, UnixMillis,
};
use std::future::Future;
use tabula_core::UserId;

/// Canonical scoped idempotency key, never authentication or resource authority.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct AccountOperationId(u128);
impl AccountOperationId {
    pub fn parse(value: &str) -> Result<Self, SessionError> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(SessionError::InvalidInput);
        }
        let id = u128::from_str_radix(value, 16).map_err(|_| SessionError::InvalidInput)?;
        if id == 0 {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self(id))
    }
    pub fn generate() -> Result<Self, SessionError> {
        let raw = crate::SessionCredential::generate()?;
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&raw.digest().as_bytes()[..16]);
        let id = u128::from_be_bytes(bytes);
        if id == 0 {
            Err(SessionError::Unavailable)
        } else {
            Ok(Self(id))
        }
    }
    pub const fn get(self) -> u128 {
        self.0
    }
    pub fn encoded(self) -> String {
        format!("{:032x}", self.0)
    }
}
/// Stable public account handle; exact canonical input is required.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AccountHandle(String);
impl AccountHandle {
    pub fn new(value: String) -> Result<Self, SessionError> {
        if !(3..=32).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            Err(SessionError::InvalidInput)
        } else {
            Ok(Self(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Approved display name preserving Unicode text with bounded plain-text output.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AccountDisplayName(String);
impl AccountDisplayName {
    pub fn new(value: String) -> Result<Self, SessionError> {
        if value.trim() != value {
            return Err(SessionError::InvalidInput);
        }
        if value.is_empty()
            || value.len() > 256
            || value.chars().count() > 64
            || value
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            Err(SessionError::InvalidInput)
        } else {
            Ok(Self(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Current disclosure policy to other authenticated accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountProfileVisibility {
    Public,
    Friends,
    Private,
}
impl AccountProfileVisibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Friends => "friends",
            Self::Private => "private",
        }
    }
    pub fn parse(value: &str) -> Result<Self, SessionError> {
        match value {
            "public" => Ok(Self::Public),
            "friends" => Ok(Self::Friends),
            "private" => Ok(Self::Private),
            _ => Err(SessionError::Unavailable),
        }
    }
}
/// Persisted profile facts; never a permission to publish later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountProfile {
    pub user_id: UserId,
    pub handle: AccountHandle,
    pub display_name: AccountDisplayName,
    pub visibility: AccountProfileVisibility,
    pub revision: u64,
    pub as_of: UnixMillis,
}
/// Durable enrollment policy captured before provider navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnrollmentPolicy {
    pub epoch: AccountEpoch,
    pub enabled: bool,
}
/// Grant state without exposing a provider identity or any bearer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnrollmentStatus {
    Ready,
    AcceptedWithoutSession,
    Rejected,
}
/// Cookie-bound durable grant control facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnrollmentContext {
    pub operation_id: AccountOperationId,
    pub status: EnrollmentStatus,
    pub policy_epoch: AccountEpoch,
}
/// Verified-provider handoff; only an OIDC verifier may construct these trusted facts.
#[derive(Clone, Debug)]
pub struct VerifiedEnrollment {
    pub identity: ProviderIdentityKey,
    pub expected_policy_epoch: AccountEpoch,
    /// Previously mapped admission captured before provider navigation. `None`
    /// permits a new subject, never borrowing a later epoch to revive a legacy account.
    pub expected_account_epoch: Option<AccountEpoch>,
}
/// No session is issued by enrollment or registration.
pub trait BrowserEnrollmentProvider: Send + Sync {
    fn begin_enrollment(
        &self,
        binding: String,
    ) -> impl Future<Output = Result<BrowserLoginStart, SessionError>> + Send;
    fn matches_enrollment_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError>;
    fn complete_enrollment(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
    ) -> impl Future<Output = Result<VerifiedEnrollment, SessionError>> + Send;
    fn cancel_enrollment(&self, binding: &str);
}
/// Registration grants are one-use, durably cookie-bound and expire in five minutes.
pub trait EnrollmentAuthority: Send + Sync {
    fn enrollment_policy(
        &self,
    ) -> impl Future<Output = Result<EnrollmentPolicy, SessionError>> + Send;
    fn admitted_accounts(
        &self,
        issuer: String,
    ) -> impl Future<Output = Result<Vec<(ProviderIdentityKey, AccountEpoch)>, SessionError>> + Send;
    /// Non-authorizing usability hint. The HTTP caller holds its current account
    /// publication while reading this existence bit; no profile values are exposed.
    fn account_profile_ready(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<bool, SessionError>> + Send;
    fn create_enrollment(
        &self,
        verified: VerifiedEnrollment,
        digest: CredentialDigest,
        operation_id: AccountOperationId,
    ) -> impl Future<Output = Result<EnrollmentContext, SessionError>> + Send;
    fn read_enrollment(
        &self,
        digest: CredentialDigest,
    ) -> impl Future<Output = Result<EnrollmentContext, SessionError>> + Send;
    fn register_account(
        &self,
        digest: CredentialDigest,
        operation_id: AccountOperationId,
        handle: AccountHandle,
        display_name: AccountDisplayName,
    ) -> impl Future<Output = Result<EnrollmentStatus, SessionError>> + Send;
}
/// Protected profile reads and CAS writes share current account/session ordering.
pub trait AccountProfileAuthority: HttpSessionAuthority {
    type ProfilePublication: SessionPublication;
    fn self_account_profile(
        &self,
        operation: CredentialOperation,
    ) -> impl Future<Output = Result<(Self::ProfilePublication, Option<AccountProfile>), SessionError>>
           + Send;
    fn other_account_profile(
        &self,
        operation: CredentialOperation,
        handle: AccountHandle,
    ) -> impl Future<Output = Result<(Self::ProfilePublication, AccountProfile), SessionError>> + Send;
    fn update_account_profile(
        &self,
        operation: CredentialOperation,
        id: AccountOperationId,
        expected_revision: u64,
        name: AccountDisplayName,
        visibility: AccountProfileVisibility,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approved_fields_reject_ambiguous_or_hostile_input() {
        for handle in ["ab", "Alice", "a-b", " a_b", "ábcd", "a\ncd"] {
            assert!(AccountHandle::new(handle.into()).is_err());
        }
        for handle in ["abc", "alice_2", &"a".repeat(32)] {
            assert!(AccountHandle::new(handle.into()).is_ok());
        }
        assert!(AccountDisplayName::new("Ngọc Hà".into()).is_ok());
        assert!(AccountDisplayName::new("  Ngọc Hà  ".into()).is_err());
        for name in [String::new(), "x\ny".into(), "a".repeat(65), "𐀀".repeat(65)] {
            assert!(AccountDisplayName::new(name).is_err());
        }
    }
    #[test]
    fn operation_id_rejects_zero_noncanonical_and_wrong_width() {
        for id in [
            "00000000000000000000000000000000",
            "0000000000000000000000000000000A",
            "1",
            "0000000000000000000000000000000g",
        ] {
            assert!(AccountOperationId::parse(id).is_err());
        }
        assert_eq!(
            AccountOperationId::parse("00000000000000000000000000000001")
                .unwrap()
                .get(),
            1
        );
    }
}

/// Maximum prevalidated isolated socket text frame; never a queued unbounded body.
pub const MAX_SOCKET_FRAME_BYTES: usize = 512 * 1024;
/// Owned bounded private candidate, unreleasable until a live handoff (ADR-0044).
pub struct BoundedSocketFrame(String);
impl BoundedSocketFrame {
    pub fn new(text: String) -> Result<Self, SessionError> {
        if text.is_empty() || text.len() > MAX_SOCKET_FRAME_BYTES {
            Err(SessionError::InvalidInput)
        } else {
            Ok(Self(text))
        }
    }
    pub fn into_text(self) -> String {
        self.0
    }
}
impl std::fmt::Debug for BoundedSocketFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoundedSocketFrame([REDACTED])")
    }
}
/// One-shot transfer of an owned, prevalidated frame to an already-ready transport.
/// The live deadline check and owned transfer are its explicit linearization point;
/// physical account exclusions remain held through the bounded synchronous callback.
/// The callback must not await, block or perform arbitrary work. Already transferred
/// frames are outside recall; no post-effect suppression is claimed. This operation
/// shares one-shot state with pure candidate `SessionPublication::publish`.
pub trait SocketFramePublication: SessionPublication {
    fn handoff<R>(
        &mut self,
        frame: BoundedSocketFrame,
        transport: impl FnOnce(BoundedSocketFrame) -> R,
    ) -> Result<R, SessionError>;
}

#[cfg(test)]
mod socket_frame_tests {
    use super::*;
    #[test]
    fn owned_socket_frame_is_byte_bounded_and_redacted() {
        assert!(BoundedSocketFrame::new(String::new()).is_err());
        let exact = "x".repeat(MAX_SOCKET_FRAME_BYTES);
        assert_eq!(
            BoundedSocketFrame::new(exact.clone()).unwrap().into_text(),
            exact
        );
        assert!(BoundedSocketFrame::new("x".repeat(MAX_SOCKET_FRAME_BYTES + 1)).is_err());
        assert_eq!(
            format!(
                "{:?}",
                BoundedSocketFrame::new("private-account-detail".into()).unwrap()
            ),
            "BoundedSocketFrame([REDACTED])"
        );
    }
}
