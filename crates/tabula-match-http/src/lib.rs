//! Bounded authenticated direct-match HTTP DTOs (ADR-0041, doc03 §4/7).
//! Default types are pure and WASM-safe. Native `isolated` composes current
//! session/admission authority, actor, durable commits and actual body guards.
//! Production stays closed. HTTP version2 wraps unchanged match wire0.1, never
//! canonical state, seed, index, time, hash, events or operation ledger (I-5/I-6).
#![forbid(unsafe_code)]
mod bounds;
mod dto;
pub use dto::*;
#[cfg(all(feature = "isolated", not(target_arch = "wasm32")))]
pub mod isolated;
