//! Invited-account Kanidm authorization-code adapter. ADR-0038 records the
//! narrow opt-in implementation; neither production service is activated.
#![forbid(unsafe_code)]

#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod config;
#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod http;
#[cfg(all(feature = "web-oidc", not(target_arch = "wasm32")))]
pub mod oidc;
