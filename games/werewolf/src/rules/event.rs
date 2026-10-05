//! Canonical facts; disclosure belongs exclusively to `view_event` (I-5/I-6).
use super::{Alignment, Ballot, NightChoice, Phase, PlayerStatus, Role};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tabula_core::{LogicalTime, MatchOutcome, SeatId, TimerId};

/// Authoritative event log facts, including secret actions and resolution causes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    RolesAssigned {
        roles: BTreeMap<SeatId, Role>,
    },
    PhaseChanged {
        phase: Phase,
        round: u32,
        timer_id: TimerId,
        ends_at: LogicalTime,
    },
    NightActionSubmitted {
        seat: SeatId,
        choice: NightChoice,
        round: u32,
    },
    SeerReport {
        seer: SeatId,
        target: SeatId,
        alignment: Alignment,
        round: u32,
    },
    NightResolved {
        round: u32,
        attacked: Option<SeatId>,
        protected: Option<SeatId>,
        healed: Option<SeatId>,
        poisoned: Option<SeatId>,
    },
    BallotChanged {
        seat: SeatId,
        ballot: Option<Ballot>,
    },
    VoteResolved {
        round: u32,
        tally: BTreeMap<SeatId, u8>,
        eliminated: Option<SeatId>,
    },
    DeathRevealed {
        seat: SeatId,
        role: Role,
    },
    HunterTriggered {
        hunter: SeatId,
        target: Option<SeatId>,
    },
    SeatStatusChanged {
        seat: SeatId,
        status: PlayerStatus,
    },
    MatchEnded {
        outcome: MatchOutcome,
    },
}
