//! PHASE 4 — auth process composition; ADR-0034.

/// Refuse production startup; ADR-0038 isolated proof is not deployment evidence.
pub(crate) fn run() -> std::process::ExitCode {
    // TODO(phase 4, #54): validate config, initialize redacted telemetry, connect
    // durable session storage, integrate proven provider adapter, compose routes and
    // serve with bounded shutdown. Add dependencies only with real implementation.
    eprintln!(
        "tabula-auth is a Phase 4 skeleton for issue #54 (ADR-0034).\n\
         Gate: production TLS, provider security-event synchronization and ADR-0031 delivery/target evidence.\n\
         No auth endpoint or credential handling is active."
    );
    std::process::ExitCode::FAILURE
}
