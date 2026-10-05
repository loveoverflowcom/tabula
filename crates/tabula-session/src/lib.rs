//! Internal auth-session lifecycle policy and durable authority ports.
//!
//! ADR-0035 opens only this isolated foundation of Phase 4. Both service
//! bootstraps, provider verification, HTTP/WS enforcement, CSRF issuance and
//! production deployment remain gated. This library is owned by the auth
//! lifecycle boundary in ADR-0034, never by deterministic game rules.
//!
//! Checked records describe stored facts. Observations and connection bindings
//! are snapshots, **not permits** to commit a later effect or deliver private
//! output. An adapter must recheck authority in the same ordering boundary as
//! every protected effect (ADR-0031 §5).

#![forbid(unsafe_code)]

mod credential;
mod policy;
mod ports;
mod types;

pub use credential::{CredentialDigest, SessionCredential};
pub use policy::{
    AccountRecord, ActivityKind, RawAccountRecord, RawSessionRecord, SessionBinding, SessionRecord,
    SessionSnapshot, ABSOLUTE_LIFETIME_MS, IDLE_LIFETIME_MS,
};
pub use ports::{IssueSession, RotateSession, SessionAuthority};
pub use types::{
    AccountEpoch, AuthSessionId, CredentialGeneration, ProviderIdentityKey, SessionChannel,
    SessionContextId, SessionError, UnixMillis,
};
