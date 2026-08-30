//! # `tabula-desktop` — OPTIONAL Tauri shell
//!
//! > ## PHASE 5, OPTIONAL, AND AN EXPERIMENT
//! >
//! > ADR-019: **Tauri is optional and never required for gameplay on any
//! > platform.** Gameplay must not sit inside a WebView — WebView input latency
//! > and rendering would become the product's ceiling.
//!
//! This crate exists only if the launcher/updater/notification value turns out to
//! be real. Decide in Phase 5. The fallback, explicitly listed in doc 09 §3.2, is
//! **ship without it**.
//!
//! If it does land, it wraps the native Macroquad binary; it does not host it in
//! a `WebView`. Tauri **mobile** is not on the table before Phase 6 exits, and then
//! only for shell screens.
//!
//! ## What it would be for
//!
//! ```text
//! launcher        pick a game, resume a match, see friends online
//! updater         staged rollouts, delta updates
//! notifications   native OS notifications for async turns
//! deep links      tabula://match/<id>
//! ```
//!
//! ## The alternative already works
//!
//! `cargo-dist` + GitHub Releases ships signed desktop artifacts with no Tauri at
//! all (doc 01 §1.3). That is the baseline this crate must beat, not a fallback
//! it can assume away.
//!
//! ## The honest test
//!
//! If gameplay ever depends on this crate existing, ADR-019 has been violated.
//! Native Macroquad must remain a complete, shippable product on its own.

fn main() {
    eprintln!(
        "tabula-desktop is an OPTIONAL Phase 5 experiment (ADR-019).\n\
         Gameplay never requires it: build `tabula-game-client` instead."
    );
    std::process::exit(1);
}
