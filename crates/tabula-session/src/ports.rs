use std::future::Future;

use tabula_core::UserId;

use crate::{
    AccountEpoch, AccountRecord, AuthSessionId, CredentialDigest, CredentialGeneration,
    ProviderIdentityKey, SessionBinding, SessionChannel, SessionContextId, SessionError,
    SessionSnapshot,
};

/// Internal issuance request. The exact identity key and expected epoch must be
/// checked together under the account lock. This DTO is not provider login proof.
#[derive(Clone, Debug)]
pub struct IssueSession {
    pub identity: ProviderIdentityKey,
    pub expected_epoch: AccountEpoch,
    pub id: AuthSessionId,
    pub channel: SessionChannel,
    pub credential_digest: CredentialDigest,
    pub context_id: SessionContextId,
}

/// Same-record verifier CAS. Only the winning durable commit may result in an
/// adapter emitting the replacement credential. No secret is carried here.
#[derive(Clone, Debug)]
pub struct RotateSession {
    pub current_digest: CredentialDigest,
    pub channel: SessionChannel,
    pub expected_generation: CredentialGeneration,
    pub replacement_digest: CredentialDigest,
}

/// Durable session lifecycle authority, implemented by storage (ADR-0034/0036).
///
/// Operations sample trusted server time **after** acquiring their ordering
/// locks. All account/session changes are atomic. Observations persist the clock
/// high-water and terminal expiry even when returning unauthenticated. Every
/// storage/corruption/clock failure is unavailable, without private diagnostics.
///
/// Successful observations are snapshots only. They cannot authorize effects
/// after an unrelated await. Cross-process private-output and gameplay fencing,
/// provider verification, HTTP/WS and production bootstraps remain gated.
pub trait SessionAuthority: Send + Sync {
    /// Read-only exact provider-key resolution; never silently provisions accounts.
    fn account_snapshot(
        &self,
        identity: ProviderIdentityKey,
    ) -> impl Future<Output = Result<AccountRecord, SessionError>> + Send;

    /// Atomically checks active account+expected epoch and inserts one new record.
    fn issue_session(
        &self,
        request: IssueSession,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Rechecks the currently presented verifier and its transport channel.
    fn observe_credential(
        &self,
        digest: CredentialDigest,
        channel: SessionChannel,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Rechecks a previously established connection binding, independent of rotation.
    /// The caller must retain trusted established-connection provenance. Never
    /// construct this request from raw HTTP/wire input or public record IDs.
    fn observe_binding(
        &self,
        binding: SessionBinding,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Atomically rotates the currently presented verifier/generation; no idle renewal.
    fn rotate_session(
        &self,
        request: RotateSession,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Durably and idempotently revokes one current-session record.
    /// The caller must retain trusted established-connection provenance; a public
    /// record ID or structurally rebuilt binding is not logout authority.
    fn revoke_session(
        &self,
        binding: SessionBinding,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;

    /// CAS epoch advance that fences all older sessions and in-flight issuance.
    fn invalidate_account_epoch(
        &self,
        user_id: UserId,
        expected_epoch: AccountEpoch,
    ) -> impl Future<Output = Result<AccountRecord, SessionError>> + Send;
}
