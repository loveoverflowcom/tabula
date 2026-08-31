//! # `tabula-game-api` — the game contract
//!
//! **Phase 0.** Real from day one (doc 07 Phase 0, doc 09 §7 step 6).
//!
//! > This is the most important contract in the platform. Changing it changes
//! > every game. — doc 02 header
//!
//! ## What a game module supplies
//!
//! ```text
//! GameMetadata      identity, name, version, players, art direction hooks
//! GameCapabilities  declarative facts the platform needs to run it safely
//! GameRules         the pure deterministic core
//! GameBot           optional AI policies, consuming projections only
//! GamePresentation  optional, client-only — lives in `tabula-presentation`
//! AssetPack         optional, versioned art/audio
//! Tests             the tabula-testkit conformance suite (mandatory)
//! ```
//!
//! It supplies **nothing else**. It has no access to the network, the database,
//! the clock, the OS, or the renderer's internals.
//!
//! ## The shape in one line
//!
//! ```text
//! apply(&mut State, Input<Command>, &mut Ctx) -> Result<Outcome, RuleError>
//! ```
//!
//! Pure. Synchronous. Total. Transactional on error.
//!
//! ## The determinism boundary
//!
//! This is the whole list. If your rules code needs something that is not on the
//! left, it belongs to the platform and reaches you as an `Input`. (ADR-026)
//!
//! | Rules code MAY depend on | Rules code MUST NOT depend on |
//! |---|---|
//! | `State`, `Command`, `Config` | wall clock, `SystemTime`, `Instant` (I-3) |
//! | `ctx.now` — logical time from the log | OS randomness, `getrandom`, `thread_rng` (I-4) |
//! | `ctx.index` — this input's log position | the network, the filesystem, a database (I-1) |
//! | `ctx.rng` — the deterministic RNG (I-4) | thread scheduling, parallelism inside rules |
//! | `SeatRoster` as passed to `create` | `HashMap`/`HashSet` iteration (I-2) |
//! | ordered collections: `BTreeMap`, `Vec` | `f32`/`f64` in canonical state (doc 00 §5.1) |
//! | integers and fixed-point arithmetic | pointer or address-derived values |
//! | | `Debug` output or `serde_json` for hashing (ADR-021) |
//!
//! Enforcement is mechanical, not cultural: `deps.toml` + `xtask check-deps` walk
//! the resolved dependency graph, `clippy.toml` bans the hazardous types in every
//! rules-tier crate, and `tabula_testkit::determinism` catches at runtime what
//! neither can see. All three are proven to fire by
//! `tabula-testkit/tests/harness_catches_violations.rs`.
//!
//! ### The four semantics a game author has to know
//!
//! ```text
//! Transition   State × Input<Command> × Ctx  →  Ok(Outcome { events, effects })
//!                                            →  Err(RuleError)
//!              `events` is ORDERED and stored verbatim; the order is contract (R7).
//!
//! Rejection    Err ⇒ total no-op: state byte-identical (R2), state_version
//!              unchanged (I-7), RNG stream unaffected (R8 — nothing to rewind,
//!              because each input's stream derives from (seed, index) alone).
//!
//! Hash         blake3(b"tabula.state.v1" ‖ RULES_VERSION ‖ ENCODING_VERSION ‖ postcard(state)).
//!              Authoritative semantic state only. Meaningful within ONE
//!              RulesVersion; comparing across versions is a category error.
//!
//! Versioning   ENCODING_VERSION  the encoding framework, platform-wide
//!              RulesVersion      State/Command/Event encoding AND apply/project
//!                                behaviour — a match runs one for its whole life
//!              GameVersion       package semver; never affects a live match
//! ```
//!
//! ## Contract lock status
//!
//! Doc 07's contract timeline: `GameRules` / `Input` / `Effect` / `Ctx` are
//! introduced in Phase 0 and **locked after Phase 3**. Phase 1 (chess) and
//! Phase 3 (cards, tiles, werewolf) exist specifically to shake this API out
//! before networking depends on it. Phase 3's exit criterion is literally "no
//! change required to `tabula-core`/`tabula-game-api` in the final two weeks".
//!
//! So: churn here is *expected and welcome* through Phase 3, and *expensive*
//! after it. Use that window.
//!
//! ## Reading order
//!
//! doc 00 §3 (functional core / imperative shell) → doc 02 §3–§5 → doc 02 §10
//! (the complete worked example).

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod a11y;
pub mod bot;
pub mod capabilities;
pub mod ctx;
pub mod effect;
pub mod error;
pub mod input;
pub mod metadata;
pub mod module;
pub mod rules;

pub use a11y::{A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId};
pub use bot::GameBot;
pub use capabilities::{
    AsyncTurnPolicy, BotLevels, BotLevelsError, Budget, ChatChannelSpec, ChatPolicy, Durability,
    GameCapabilities, GameCapabilitiesError, RankedSupport, RatingKind, ReconnectPolicy,
    SeatCounts, SeatCountsError, SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy,
    TeamSpec, TeamSpecError, TurnModel, VoiceRequirement,
};
pub use ctx::Ctx;
pub use effect::{ChatScopes, CheckpointLabel, Effect, Notice, VoiceScopes};
pub use error::{ConfigError, InitError, MigrateError};
pub use input::{AdminInput, Input};
pub use metadata::{
    Category, Complexity, ContentRating, DurationRange, DurationRangeError, GameMetadata,
};
pub use module::GameModule;
pub use rules::{GameRules, Init, LegalCommands, Outcome};

// Re-exported so a game crate can `use tabula_game_api::*;` and have the whole
// vocabulary, exactly as the worked example in doc 02 §10.2 does.
pub use tabula_core::*;
