//! Who is looking. (doc 00 §9.4, doc 02 §2)
//!
//! [`Viewer`] is the input to `project(state, viewer) -> View` and
//! `view_event(state_after, event, viewer) -> Option<ViewEvent>`, the two
//! functions that constitute the platform's entire security boundary (ADR-005).
//!
//! ## Why this is an enum and not `Option<SeatId>`
//!
//! Werewolf. A dead player still *holds a seat* and sees everything; an outside
//! spectator sees only public information. Those are different viewers with
//! different authorisations, and collapsing them into "has a seat or doesn't"
//! makes the distinction inexpressible. (doc 02 §12.3)
//!
//! The most common projection bug in board-game platforms is spectators seeing
//! hidden hands. `tabula-testkit` checks spectator views explicitly, for exactly
//! this reason. (doc 02 §7.1)

use serde::{Deserialize, Serialize};

use crate::ids::SeatId;
use crate::time::Millis;

/// The authorisation identity passed to a projection.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Viewer {
    /// A seated participant — human or bot, alive or dead. The game decides what
    /// that seat is allowed to know.
    Seat(SeatId),

    /// Someone watching who holds no seat.
    Spectator(SpectatorTier),

    /// Support, replay, and audit tooling. Sees canonical information.
    ///
    /// **Never reachable from a game client session.** (doc 00 §9.4 rule 1)
    ///
    /// TODO(phase 4): make this constructible only via an `AuditGrant` capability
    /// token — doc 05 §9.3 specifies the test `audit_viewer_unreachable`, which
    /// requires the variant not be freely constructible from the gateway. Today
    /// it is a plain variant; that is a known gap, not a decision.
    Audit,
}

/// Spectator delay class.
///
/// The **game** decides what a delayed spectator sees; the **platform** enforces
/// the delay by buffering. Delay exists so a spectator cannot relay information
/// to a player in real time — a ranked hidden-information game may use
/// `Delayed { by: 30s }`.
/// (doc 02 §12.2, doc 03 §11.1)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SpectatorTier {
    Live,
    Delayed { by: Millis },
}

impl Viewer {
    /// The seat this viewer occupies, if any.
    ///
    /// Convenience for projections. Do **not** use it to collapse the spectator
    /// and seat cases into one code path — that is how spectators end up seeing
    /// hands.
    #[must_use]
    pub fn seat(self) -> Option<SeatId> {
        match self {
            Self::Seat(s) => Some(s),
            Self::Spectator(_) | Self::Audit => None,
        }
    }
}
