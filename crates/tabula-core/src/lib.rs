//! # `tabula-core` — the deterministic kernel
//!
//! **Phase 0.** This crate is real from day one (doc 07 Phase 0, doc 09 §7 step 5).
//!
//! Everything here is pure, synchronous, and replay-stable. It is the vocabulary
//! that game rules, the match runtime, the protocol, and the client all share.
//!
//! ## What belongs here
//!
//! Identity (`MatchId`, `SeatId`, `UserId`), logical time, the deterministic RNG,
//! the viewer/audience model, seat lifecycle, match outcomes, and canonical
//! hashing. Nothing else. If a type needs a clock, a socket, a database, or a
//! user account to be meaningful, it belongs one layer up.
//!
//! ## What must never happen here
//!
//! | Banned | Invariant | Why |
//! |---|---|---|
//! | `std::time::{Instant, SystemTime}` | I-3 | Wall clock makes replay impossible |
//! | `HashMap` / `HashSet` in any public API | I-2 | Iteration order is not deterministic |
//! | `f32` / `f64` in canonical types | doc 00 §5.1 | Float results vary across arch and WASM |
//! | `rand::thread_rng`, OS entropy | I-4 | Randomness must come from `MatchSeed` |
//! | `unsafe` | ADR-021 | Forbidden workspace-wide |
//!
//! Enforcement is mechanical, not cultural: `clippy.toml` in this crate lists the
//! banned types and methods, and `cargo xtask check-deps` walks the resolved
//! dependency graph against `deps.toml`.
//!
//! ## Reading order
//!
//! doc 00 §5 (the deterministic game model) then doc 02 §2 (these exact types).

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
// Determinism guard: the workspace sets this to `warn`; rules-tier crates
// escalate it. A float in canonical state is a cross-architecture divergence
// waiting for a nightly replay job to find it.
#![deny(clippy::float_arithmetic)]

extern crate alloc;

pub mod audience;
pub mod error;
pub mod hash;
pub mod ids;
pub mod outcome;
pub mod rng;
pub mod seat;
pub mod time;
pub mod viewer;

pub use audience::Audience;
pub use error::{RuleError, RuleErrorCode};
pub use hash::{canonical_encode, canonical_hash, StateHash, ENCODING_VERSION};
pub use ids::{
    GameId, GameVersion, InputIndex, MatchId, RulesVersion, SeatId, SessionId, StateVersion,
    TimerId, UserId,
};
pub use outcome::{AbortReason, MatchOutcome, OutcomeKind, Standing};
pub use rng::{DetRng, MatchSeed};
pub use seat::{BotLevel, Occupant, SeatChange, SeatEntry, SeatRoster};
pub use time::{LogicalTime, Millis};
pub use viewer::{SpectatorTier, Viewer};
