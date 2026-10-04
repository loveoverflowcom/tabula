//! PHASE 4 — auth process composition; ADR-0034.

/// Refuse startup until the provider and Tabula session boundaries are proven.
pub(crate) fn run() -> std::process::ExitCode {
    // TODO(phase 4, #54): validate config, initialize redacted telemetry, connect
    // durable session storage, verify Kanidm discovery/keys, compose routes and
    // serve with bounded shutdown. Add dependencies only with real implementation.
    eprintln!(
        "tabula-auth is a Phase 4 skeleton for issue #54 (ADR-0034).\n\
         Gate: Kanidm integration and ADR-0031 session/revocation evidence.\n\
         No auth endpoint or credential handling is active."
    );
    std::process::ExitCode::FAILURE
}
