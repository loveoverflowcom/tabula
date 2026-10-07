//! PHASE 4 — auth process composition; ADR-0034.

/// Run only ADR-0047's explicit local/dev provider composition.
#[cfg(all(feature = "local-dev", not(target_arch = "wasm32")))]
pub(crate) fn run() -> std::process::ExitCode {
    tabula_auth::local_dev::run()
}

#[cfg(not(all(feature = "local-dev", not(target_arch = "wasm32"))))]
/// Refuse default production startup; local/dev proof is not deployment evidence.
pub(crate) fn run() -> std::process::ExitCode {
    // The explicit local/dev composition exists under ADR-0047; production
    // provider events, credential policy and target/deployment evidence stay gated.
    eprintln!(
        "tabula-auth is a Phase 4 skeleton for issue #54 (ADR-0034).\n\
         Gate: production TLS, provider security-event synchronization and ADR-0031 delivery/target evidence.\n\
         No auth endpoint or credential handling is active."
    );
    std::process::ExitCode::FAILURE
}
