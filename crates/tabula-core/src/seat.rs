//! Seats, occupants, and the seat lifecycle. (doc 02 §2, doc 00 §6.3)
//!
//! ## The ownership split this file encodes
//!
//! The **platform** owns sockets, so it decides *when* a seat is disconnected,
//! idle, or abandoned. The **game** owns rules, so it decides *what that means*:
//! chess keeps burning the clock, werewolf auto-abstains, an async tile game does
//! nothing at all.
//!
//! The carrier between them is `Input::Seat { seat, change }` — a
//! [`SeatChange`] delivered through the same single ordered input stream as
//! every player command (ADR-003). There is no second channel, so replay
//! reproduces disconnect handling exactly.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::ids::{SeatId, UserId};

/// The seats of one match and who is in them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "RawSeatRoster")]
pub struct SeatRoster {
    seats: SmallVec<[SeatEntry; 8]>,
}

#[derive(Serialize, Deserialize)]
struct RawSeatRoster {
    seats: SmallVec<[SeatEntry; 8]>,
}

/// Why a [`SeatRoster`] cannot establish unique addressable seats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SeatRosterError {
    #[error("seat {seat:?} occurs more than once in a roster")]
    DuplicateSeat { seat: SeatId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatEntry {
    pub seat: SeatId,
    pub occupant: Occupant,
    /// Team membership for team games; `None` for free-for-all.
    /// Consumed by the lobby (team formation) and ratings (`TeamElo`). (doc 02 §5)
    pub team: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Occupant {
    Human(UserId),
    Bot { level: BotLevel },
    Empty,
}

/// Bot strength. Games choose which levels they support via
/// `SubstitutionPolicy::BotOnly { levels }`. (doc 02 §4.2)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum BotLevel {
    /// Free for any game that implements `legal_commands`: pick a random legal
    /// move. That alone unlocks auto-fill and self-play fuzzing. (doc 02 §6)
    Trivial,
    Easy,
    Medium,
    Hard,
}

/// A platform-observed seat lifecycle transition.
///
/// A game receives these only if it asked for them
/// (`capabilities.reconnect.notify_rules = true`). Handling a variant by
/// returning `Outcome::empty()` is a legitimate, meaningful rules decision — it
/// means "the clock keeps running" (doc 02 §12.1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SeatChange {
    Occupied {
        by: Occupant,
    },
    Vacated,

    /// Socket gone. The platform holds the seat for `capabilities.reconnect.grace`
    /// before escalating to `Abandoned`. (doc 02 §4.2)
    Disconnected,
    Reconnected,

    /// No input for the platform's idle threshold. The game decides whether that
    /// forfeits, auto-passes, auto-abstains, or is ignored. (doc 00 §6.3)
    WentIdle,
    BecameActive,

    /// Operator- or user-initiated abandonment.
    Abandoned,

    /// Substitution: a seat handed to a bot, or from a bot back to a human.
    /// Only legal when `capabilities.substitution` permits it; werewolf forbids
    /// it outright because the seat carries secret knowledge. (doc 02 §12.3)
    OccupantChanged {
        from: Occupant,
        to: Occupant,
    },
}

impl SeatRoster {
    /// Creates a roster whose addressable seat identities are unique.
    ///
    /// Seat identifiers are deliberately *not* required to be contiguous or to
    /// start at zero: no architecture contract grants games that assumption.
    pub fn new(seats: SmallVec<[SeatEntry; 8]>) -> Result<Self, SeatRosterError> {
        for (index, entry) in seats.iter().enumerate() {
            if seats[..index].iter().any(|prior| prior.seat == entry.seat) {
                return Err(SeatRosterError::DuplicateSeat { seat: entry.seat });
            }
        }
        Ok(Self { seats })
    }

    pub fn iter(&self) -> core::slice::Iter<'_, SeatEntry> {
        self.seats.iter()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[SeatEntry] {
        &self.seats
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }

    #[must_use]
    pub fn get(&self, seat: SeatId) -> Option<&SeatEntry> {
        self.seats.iter().find(|e| e.seat == seat)
    }
}

impl<'a> IntoIterator for &'a SeatRoster {
    type Item = &'a SeatEntry;
    type IntoIter = core::slice::Iter<'a, SeatEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl TryFrom<RawSeatRoster> for SeatRoster {
    type Error = SeatRosterError;

    fn try_from(raw: RawSeatRoster) -> Result<Self, Self::Error> {
        Self::new(raw.seats)
    }
}

#[cfg(test)]
mod tests {
    use smallvec::smallvec;

    use super::*;
    use crate::{canonical_decode, canonical_encode};

    fn entry(seat: u8) -> SeatEntry {
        SeatEntry {
            seat: SeatId(seat),
            occupant: Occupant::Empty,
            team: None,
        }
    }

    #[test]
    fn roster_rejects_duplicate_addressable_seats() {
        assert!(SeatRoster::new(smallvec![entry(7), entry(42)]).is_ok());
        assert!(matches!(
            SeatRoster::new(smallvec![entry(7), entry(7)]),
            Err(SeatRosterError::DuplicateSeat { seat: SeatId(7) })
        ));
    }

    #[test]
    fn roster_deserialization_cannot_bypass_unique_seats() {
        let bytes = canonical_encode(&RawSeatRoster {
            seats: smallvec![entry(7), entry(7)],
        })
        .unwrap();
        assert!(canonical_decode::<SeatRoster>(&bytes).is_err());
    }
}
