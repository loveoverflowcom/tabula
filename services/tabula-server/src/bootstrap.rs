//! PHASE 4 — process composition; doc 03 §1 and doc 06 §11.3.

/// Run only ADR-0047's explicit local/dev composition.
#[cfg(all(feature = "local-dev", not(target_arch = "wasm32")))]
pub(crate) fn run() -> std::process::ExitCode {
    tabula_server::local_dev::run()
}

#[cfg(not(all(feature = "local-dev", not(target_arch = "wasm32"))))]
/// Keep default production startup closed until its separate gates are proven.
pub(crate) fn run() -> std::process::ExitCode {
    // ADR-0047's implemented local/dev lifecycle does not supply production
    // deployment/provider acceptance or complete the phase exits.
    eprintln!(
        "tabula-server is a Phase 4 deliverable (docs/architecture/07-phases-and-implementation-roadmap.md).\n\
         Gate: four games pass conformance AND the game contract stopped changing (Phase 3 exit).\n\
         Doc 09 §7: building the server on a moving contract is how protocols get corrupted."
    );
    std::process::ExitCode::FAILURE
}
