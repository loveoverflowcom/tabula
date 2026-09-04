//! Werewolf ruleset primitives and configuration. (doc 02 §12.3, doc 08 §5)
//!
//! This module owns the pure, validated domain types for Werewolf match
//! creation and role configuration. Gameplay transitions, reducer logic,
//! projections, and the public [`tabula_game_api::rules::GameRules`] adapter
//! arrive in W2+ after these foundations are validated.

pub mod config;
pub mod role;

pub use config::{
    Config, ConfigValidationError, DurationError, MaxRounds, MaxRoundsError, PhaseDuration,
    PhaseDurations, RawConfig, RawPhaseDurations, SeatCount, SeatCountError, VoteMode,
    DEFAULT_DAWN_MS, DEFAULT_DAY_MS, DEFAULT_DUSK_MS, DEFAULT_MAX_ROUNDS, DEFAULT_NIGHT_MS,
    DEFAULT_VOTE_MS, MAX_MAX_ROUNDS, MAX_PHASE_DURATION_MS, MAX_SEATS, MIN_MAX_ROUNDS,
    MIN_PHASE_DURATION_MS, MIN_SEATS,
};
pub use role::{Alignment, Preset, Role, RoleCounts};
