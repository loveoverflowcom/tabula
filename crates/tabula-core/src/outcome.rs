//! How a match ended. (doc 02 §2)
//!
//! ## The one rule
//!
//! **Games never compute ratings, currency, or rewards** (ADR-024, doc 00 §6.3).
//! A game emits a trustworthy, structured [`MatchOutcome`]; the platform's rating
//! service consumes it. That is what keeps ladder integrity uniform across games
//! that have nothing else in common.
//!
//! `standings` must cover every seat exactly once with contiguous ranks starting
//! at 0 — the testkit asserts this (`outcome_wellformed`, doc 02 §11.1) because a
//! malformed standings list corrupts ratings silently.

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::ids::SeatId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchOutcome {
    pub kind: OutcomeKind,

    /// Full ordering, needed by the rating system. Rank 0 = winner; ties share a
    /// rank. Placement-rated games (cards, tiles) use the whole ordering, not
    /// just the winner.
    pub standings: SmallVec<[Standing; 8]>,

    /// Free-form, game-defined summary for UI: "checkmate", "3 wolves remain".
    ///
    /// Human-facing text, so it must be an i18n key or public-safe — never a
    /// carrier for hidden information.
    pub summary: CompactString,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    pub seat: SeatId,
    pub rank: u8,
    pub score: i64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeKind {
    Decisive,
    Draw,

    /// Ended early. **Must not count for ratings** — the rating job checks this
    /// variant, not the reason.
    Aborted {
        reason: AbortReason,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortReason {
    NotEnoughPlayers,
    OperatorCancelled,
    PlatformFailure,

    /// A game's `apply` panicked and was caught by the runtime's `catch_unwind`
    /// (doc 01 §5.2). Always a Sev-2 bug: it violates contract R3 (`apply` never
    /// panics on any input). The process survives; that match does not.
    RulesPanic,

    TimedOutIdle,
}
