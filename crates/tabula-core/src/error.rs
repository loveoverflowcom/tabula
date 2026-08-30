//! Rule rejections. (doc 02 §3.1, §3.2)
//!
//! ## Rejection is data, not a panic and not a silent no-op
//!
//! `apply` is the single authority on legality and returns `Result`. There is no
//! separate `validate_command` — two functions that must agree about legality are
//! a permanent source of divergence bugs, and that class of bug becomes an
//! exploit. (doc 02 §3.2)
//!
//! A rejection must travel to the client so the UI can show *why* and so
//! anti-cheat can count violations. Returning `Ok` with no events for an illegal
//! command leaves the client hanging and the abuse counter blind. (doc 02 §13)
//!
//! ## The detail field is client-visible
//!
//! `detail` reaches the player. **Never put hidden information in it.** "You do
//! not hold the Ace of Spades" is a leak; `IllegalMove` is not. Codes are
//! localised by the client from a stable key; `detail` is developer-facing
//! colour that must remain public-safe. (doc 02 §13)

use compact_str::CompactString;
use serde::{Deserialize, Serialize};

/// A rejected input.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize)]
#[error("{code:?}: {detail}")]
pub struct RuleError {
    pub code: RuleErrorCode,
    /// Developer-facing. Must never leak hidden information.
    pub detail: CompactString,
}

impl RuleError {
    /// The common case: a code with no extra detail.
    #[must_use]
    pub fn code(code: RuleErrorCode) -> Self {
        Self {
            code,
            detail: CompactString::default(),
        }
    }

    #[must_use]
    pub fn with_detail(code: RuleErrorCode, detail: &str) -> Self {
        Self {
            code,
            detail: CompactString::from(detail),
        }
    }
}

/// Stable rejection codes the client can localise.
///
/// Games use these; they do not define their own error enum, because the client
/// shell needs to render rejections for a game it knows nothing about (I-9).
///
/// TODO(phase 1): this list is from doc 02 §3.1 and is expected to grow as chess
/// and cards are written. Additions are normal; **renames are not** — the client
/// localisation keys are derived from these names.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RuleErrorCode {
    /// A seat acted out of turn.
    NotYourTurn,
    /// Syntactically valid, semantically illegal for the current state.
    IllegalMove,
    /// The command exists but not in this phase.
    WrongPhase,
    /// The command decoded but this game has no such command.
    UnknownCommand,
    /// The match has already ended.
    MatchOver,
    /// A valid platform input the game chooses not to support (e.g. `Admin(Pause)`
    /// when `capabilities.pausable = false`).
    Unsupported,
    /// The acting seat does not exist in this match.
    NoSuchSeat,
}
