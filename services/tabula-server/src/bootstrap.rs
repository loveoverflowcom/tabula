//! PHASE 4 — process composition; doc 03 §1 and doc 06 §11.3.

/// Keep startup closed until the Phase 3 exit and Phase 4 runtime are proven.
#[cfg(all(feature = "local-dev", not(target_arch = "wasm32")))]
pub(crate) fn run() -> std::process::ExitCode {
    tabula_server::local_dev::run()
}

#[cfg(not(all(feature = "local-dev", not(target_arch = "wasm32"))))]
pub(crate) fn run() -> std::process::ExitCode {
    // TODO(phase 4, #54): validate config, initialize telemetry, connect storage,
    // check migrations, load the registry, compose HTTP/WS, then serve with drain.
    // Keep the gate until these are real; never report readiness from a skeleton.
    eprintln!(
        "tabula-server is a Phase 4 deliverable (docs/architecture/07-phases-and-implementation-roadmap.md).\n\
         Gate: four games pass conformance AND the game contract stopped changing (Phase 3 exit).\n\
         Doc 09 §7: building the server on a moving contract is how protocols get corrupted."
    );
    std::process::ExitCode::FAILURE
}
