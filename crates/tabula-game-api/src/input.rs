//! The single ordered input stream. (doc 00 §3.1, ADR-003)
//!
//! > **Everything that can change a match is one input, in one totally ordered
//! > stream, appended to one log.**
//!
//! This is the most important structural decision in the whole platform. Player
//! commands, timer expiries, seat lifecycle changes, and admin actions are all
//! `Input` variants. There is no second channel.
//!
//! ## What that buys, all of which we want
//!
//! - **Replay is trivial and total.** Replaying the input stream from a snapshot
//!   reproduces state exactly, *including* timeouts and disconnect handling.
//!   Nothing "happened outside the log".
//! - **Disconnect/AFK ownership becomes clean.** The platform decides when a seat
//!   is disconnected (it owns the sockets). The game decides what that means.
//! - **Timers are deterministic.** The game asks for a timer at logical time `T`;
//!   the shell fires it by wall clock but records it as an input at `T`.
//! - **Bots need no special path.** A bot is a seat whose commands come from a
//!   function of that seat's projection. It enters through `Input::Player` like
//!   anyone else, and can be rejected like anyone else.
//!
//! Extending this enum with new variants is normal evolution. Adding a *second
//! mutation path* is not.

use serde::{Deserialize, Serialize};
use tabula_core::{AbortReason, MatchOutcome, SeatChange, SeatId, TimerId};

/// One thing that can change match state.
///
/// Generic over the game's `Command` so that the typed world stays typed all the
/// way down; the platform only ever handles the erased form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Input<C> {
    /// A player — or a bot occupying a seat — issued a game command.
    Player { seat: SeatId, command: C },

    /// A timer the game itself requested has expired.
    ///
    /// The game asked for it with [`crate::effect::Effect::SetTimer`]. On restart
    /// the runtime re-derives pending timers **from state**, never from memory,
    /// which is why chess survives a deploy mid-game. (doc 03 §12.1)
    Timer { timer: TimerId },

    /// The platform observed a seat lifecycle change and is informing the game.
    ///
    /// Only delivered if `capabilities.reconnect.notify_rules` is true.
    Seat { seat: SeatId, change: SeatChange },

    /// Operator or system action.
    Admin(AdminInput),
}

/// Out-of-band control, authorised by the platform, meaning decided by the game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AdminInput {
    /// Operator/abuse cancellation or infra failure. The game turns this into a
    /// terminal state; the platform handles refunds and rating exclusion.
    Cancel {
        reason: AbortReason,
    },

    /// Only accepted when `capabilities.pausable` is true. The platform decides
    /// *whether* pausing is permitted (ranked: no); the game decides what it does
    /// to timers and legality. (doc 00 §6.3)
    Pause,
    Resume,

    /// Operator forces a specific result. Rare, audited, and always logged.
    ForceEnd {
        outcome: MatchOutcome,
    },
}
