//! # `tabula-auth` — Kanidm-backed account authentication production gate
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
//! The default production entrypoint opens no listener or provider flow.
//! ADR-0038 implements invited web OIDC only in the native opt-in library and
//! disposable acceptance; no production activation is inferred.
//! ADR-0047 opens an explicit native loopback `local-dev` CLI using this library.
//! Production deployment and provider security-event synchronization remain gated.

#![forbid(unsafe_code)]

mod bootstrap;

fn main() -> std::process::ExitCode {
    bootstrap::run()
}
