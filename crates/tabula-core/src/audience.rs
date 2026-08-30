//! Who an event or notice is addressed to. (doc 02 §2)
//!
//! Distinct from [`crate::viewer::Viewer`]: `Viewer` is "who is asking",
//! `Audience` is "who this is for". A game uses `Audience` when emitting a notice
//! or when marking a canonical event as server-only.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::ids::SeatId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Audience {
    Everyone,
    Seat(SeatId),
    Seats(SmallVec<[SeatId; 8]>),
    Spectators,

    /// Recorded in the log, shown to nobody until a later event reveals it.
    ///
    /// Werewolf's `RolesAssigned` is the canonical case: it must exist in the
    /// event log for replay and audit, and must reach no client until deaths
    /// reveal roles. (doc 02 §12.3)
    ServerOnly,
}
