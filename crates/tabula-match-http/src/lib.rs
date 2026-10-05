//! Bounded direct-match HTTP DTOs (ADR-0041, doc 03 §§4,7).
//! Recovered HTTP v1 types wrap unchanged match wire 0.1. They are pure and
//! WASM-safe, carrying projections only (I-5/I-6). Native gateway, durable
//! admission and actual body guards are `NOT_IMPLEMENTED`; production stays closed.
//! Dependencies are limited to core/protocol and serde (doc 00 §8).
#![forbid(unsafe_code)]
mod bounds;
mod dto;
pub use dto::*;
