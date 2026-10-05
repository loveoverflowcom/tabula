//! Internal auth-session lifecycle policy and durable authority ports.
//!
//! ADR-0036 opens only this isolated foundation of Phase 4. Both service
//! bootstraps, provider verification, production HTTP/WS enforcement and
//! production deployment remain gated. Isolated HTTP credential operations and
//! a bounded server-frame publication port are defined here. This library is
//! owned by the auth lifecycle boundary in ADR-0034, never by deterministic game rules.
//!
//! Checked records describe stored facts. Observations and connection bindings
//! are snapshots, **not permits** to commit a later effect or deliver private
//! output. An adapter must recheck authority in the same ordering boundary as
//! every protected effect (ADR-0031 §5).

#![forbid(unsafe_code)]

mod browser_login;
mod credential;
mod http_ports;
mod policy;
mod ports;
mod types;

pub use browser_login::{
    BrowserLoginCallback, BrowserLoginProvider, BrowserLoginStart, CompletedBrowserLogin,
};
pub use credential::{CredentialDigest, SessionCredential};
pub use http_ports::{
    CredentialOperation, HttpSessionAuthority, RotateCredential, SelfProfileSnapshot,
    SessionContextBinding, SessionPublication,
};
pub use policy::{
    AccountRecord, ActivityKind, RawAccountRecord, RawSessionRecord, SessionBinding, SessionRecord,
    SessionSnapshot, ABSOLUTE_LIFETIME_MS, IDLE_LIFETIME_MS,
};
pub use ports::{IssueSession, RotateSession, SessionAuthority};
pub use types::{
    AccountEpoch, AuthSessionId, CredentialGeneration, ProviderIdentityKey, SessionChannel,
    SessionContextId, SessionError, UnixMillis,
};
