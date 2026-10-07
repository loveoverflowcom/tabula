//! Explicit isolated gameplay library composition (ADR-0041).
//! The production main entrypoint remains closed; importing opens no listener.
#![forbid(unsafe_code)]
#[cfg(all(feature = "online-match", not(target_arch = "wasm32")))]
pub use tabula_match_http::isolated::IsolatedMatchHttp;
