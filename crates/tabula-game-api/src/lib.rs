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
    AsyncTurnPolicy, Budget, ChatChannelSpec, ChatPolicy, Durability, GameCapabilities,
    RankedSupport, RatingKind, ReconnectPolicy, SeatCounts, SeatSpec, SpectatorPolicy,
    StateSizeClass, SubstitutionPolicy, TeamSpec, TurnModel, VoiceRequirement,
};
pub use ctx::Ctx;
pub use effect::{ChatScopes, CheckpointLabel, Effect, Notice, VoiceScopes};
pub use error::{ConfigError, InitError, MigrateError};
pub use input::{AdminInput, Input};
pub use metadata::{Category, Complexity, ContentRating, GameMetadata};
pub use module::GameModule;
pub use rules::{GameRules, Init, LegalCommands, Outcome};

// Re-exported so a game crate can `use tabula_game_api::*;` and have the whole
// vocabulary, exactly as the worked example in doc 02 §10.2 does.
pub use tabula_core::*;
