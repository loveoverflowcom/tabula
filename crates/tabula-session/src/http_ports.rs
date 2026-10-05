//! Credential-only ports for the isolated HTTP boundary (ADR-0036).

use std::future::Future;

use tabula_core::UserId;

use crate::{
    AccountEpoch, CredentialDigest, CredentialGeneration, SessionAuthority, SessionChannel,
    SessionContextId, SessionError, SessionSnapshot,
};

/// Expected binding of an independently verified synchronizer token.
/// This is a comparison value, never proof of credential or CSRF possession.
/// The HTTP adapter verifies its secret-keyed token before supplying these facts;
/// storage compares them again under the account/session ordering locks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionContextBinding {
    pub context_id: SessionContextId,
    pub authorization_epoch: AccountEpoch,
}

/// Current credential and exact transport channel for one protected HTTP operation.
/// Raw HTTP must never substitute a public record ID or historical connection binding.
#[derive(Clone, Copy, Debug)]
pub struct CredentialOperation {
    pub digest: CredentialDigest,
    pub channel: SessionChannel,
    /// Browser mutations require this binding; native bearer operations omit it.
    /// Read operations may omit it to bootstrap a fresh synchronizer token.
    pub context: Option<SessionContextBinding>,
}

/// Current-credential rotation with an in-lock context/generation comparison.
#[derive(Clone, Copy, Debug)]
pub struct RotateCredential {
    pub credential: CredentialOperation,
    pub expected_generation: CredentialGeneration,
    pub replacement_digest: CredentialDigest,
}

/// Approved minimal existing self identity (ADR-0036). No provider fields are exposed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SelfProfileSnapshot {
    user_id: UserId,
}

impl SelfProfileSnapshot {
    /// Builds the minimal profile only from a checked current session observation.
    /// Like the observation, this is a snapshot, never a later publication permit.
    pub const fn from_session(session: &SessionSnapshot) -> Self {
        Self {
            user_id: session.user_id(),
        }
    }

    pub const fn user_id(self) -> UserId {
        self.user_id
    }
}

impl std::fmt::Debug for SelfProfileSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SelfProfileSnapshot([REDACTED])")
    }
}

/// A bounded, one-shot server-frame publication guard (ADR-0036).
///
/// The adapter commits observed clock/expiry facts before returning this guard.
/// A distinct cross-process authority lock remains held until guard release or
/// its short lease expires. A durable exclusion survives publication-backend loss
/// under the trusted deployment-clock assumption. `publish` checks lease/deadline
/// before and after synchronous first-frame construction. Cleanup is independent
/// of construction; an expired callback result is suppressed. Keep the
/// guard in the response body through its first frame. The callback must build one
/// bounded private frame/candidate synchronously without blocking or awaiting.
/// ADR-0041 permits private candidate staging only when it is unreleasable until
/// guard success and is discarded on failure. No external I/O, transmission or
/// irreversible effect is allowed. The guard rechecks expiry after construction
/// and drops an expired result.
///
/// This fences the server body handoff, not arrival of already released/buffered
/// bytes, TCP client receipt, subsequent frames, `WebSockets` or all of S09.
pub trait SessionPublication: Send {
    /// Current facts for constructing the private response within this guard.
    fn snapshot(&self) -> &SessionSnapshot;

    /// Executes at most one synchronous first-frame publication while live.
    /// Expired, released or repeated handoffs fail without invoking the callback.
    fn publish<R>(
        &mut self,
        publication: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError>;
}

/// Concrete protected HTTP operations, sharing the durable session lock order.
/// The existing connection-binding port is intentionally absent from this path.
pub trait HttpSessionAuthority: SessionAuthority {
    type Publication: SessionPublication;

    /// Current authenticated read, with no idle renewal.
    fn read_session(
        &self,
        request: CredentialOperation,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Terminal-current-verifier facts for CSRF validation and idempotent logout only.
    /// This may observe revoked/expired/old-epoch records, but must never authorize
    /// a profile, another protected operation or private publication. Replaced
    /// verifiers, wrong channels, corruption and clock regression still reject.
    fn read_logout_context(
        &self,
        request: CredentialOperation,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Minimal current self-profile, with no invented identity or profile fields.
    fn read_self_profile(
        &self,
        request: CredentialOperation,
    ) -> impl Future<Output = Result<SelfProfileSnapshot, SessionError>> + Send;

    /// Current verifier/channel/context and generation CAS in one durable boundary.
    fn rotate_credential(
        &self,
        request: RotateCredential,
    ) -> impl Future<Output = Result<SessionSnapshot, SessionError>> + Send;

    /// Idempotently revokes only the record still holding this current verifier.
    /// Browser mutations reject a missing/mismatched context before revocation.
    fn revoke_credential(
        &self,
        request: CredentialOperation,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;

    /// Rechecks current authority and retains a bounded frame publication fence.
    fn begin_publication(
        &self,
        request: CredentialOperation,
    ) -> impl Future<Output = Result<Self::Publication, SessionError>> + Send;
}
