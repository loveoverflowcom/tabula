//! Verified Kanidm authorization-code/enrollment adapters (ADR-0038/0044).
//! ADR-0047 composes them only for explicit local/dev; production remains gated.
#![forbid(unsafe_code)]

#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod config;
#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod http;
#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod oidc;

#[cfg(all(feature = "local-dev", not(target_arch = "wasm32")))]
pub mod local_dev;
