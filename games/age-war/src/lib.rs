//! # Age War: PHASE D01 design-only scaffold
//!
//! **C01 is blocked on the explicit D06 owner gate.** See
//! `docs/games/age-war/RULES.md` and `CONTENT.md`; doc 00 §1.2 and doc 08 §6
//! exclude a real-time action runtime from the existing platform contract.
//!
//! This crate contains original, **UNBALANCED** starting descriptors, schema
//! validation and integer design-math examples only. It does not implement
//! `GameRules`, `GameModule`, `GameBot`, a presenter, a renderer, a simulator,
//! wire types, snapshot decoding, or registry/catalog registration.
//! Features preserve the existing game-crate shape (doc 01 §5.1). Enabling
//! any combination does not open C01 or imply runtime availability.
//!
//! Allowed dependencies: the `games/*` row of `deps.toml`, with presentation
//! and testkit dependencies optional. D01 uses no runtime data parser, float,
//! wall clock, randomness, unordered collections or external content loader.
//! Descriptor version 1 is a design revision, **not** a `RulesVersion` or a
//! supported replay/serialization format (doc 02 §9.2, I-13/I-16).

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod catalog;
pub mod math;
pub mod phase_gate;
pub mod schema;
