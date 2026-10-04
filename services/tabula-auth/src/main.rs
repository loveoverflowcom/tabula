//! # `tabula-auth` — Kanidm-backed account authentication skeleton
//!
//! > ## PHASE 4
//!
//! ADR-0034 reserves this backend boundary for issue #54. Kanidm owns credentials
//! and OIDC issuance; this service adapts verified identity into Tabula sessions.
//! UI belongs to the app shells, not this service. Tabula-server owns profiles,
//! social/game permissions and match grants, and enforces current session authority.
//!
//! Inspired by VOT Workspace's services/vot-auth and services/kanidm at
//! 59190cc5185a54ef40ec4f9625283511a4ed3755; provider code is not copied.
//! ADR-0031 still owns cookie/bearer channels, expiry and revocation.
//! No listener, login, registration, session, discovery or provider call is active.
//! Remove each TODO when its implementation and required evidence land.

#![forbid(unsafe_code)]

mod bootstrap;
mod config;
mod http;
mod kanidm;
mod oidc;
mod session;

fn main() -> std::process::ExitCode {
    bootstrap::run()
}
