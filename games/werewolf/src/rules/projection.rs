//! Knowledge-shaped client data and event non-existence (I-5/I-6, W-D12–W-D15).
use super::{
    reducer, Alignment, Ballot, Command, Event, NightChoice, Phase, PlayerStatus, Role, State,
    WitchPotions,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tabula_core::{LogicalTime, MatchOutcome, SeatId, Viewer};

/// An authorized role fact, or explicit lack of knowledge.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleKnowledge {
    Hidden,
    Known(Role),
}
/// One public roster row plus this viewer's authorized role knowledge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatView {
    pub seat: SeatId,
    pub alive: bool,
    pub status: PlayerStatus,
    pub role: RoleKnowledge,
}
/// Identity of the selected view; unknown seats safely become Outside.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Perspective {
    Outside,
    Seat {
        seat: SeatId,
        role: Role,
        alive: bool,
        can_act: bool,
    },
    Audit,
}
/// Authorized private knowledge. Option values mean a real absent choice/resource,
/// never a secret field redacted by replacing its value with None (doc 02 §7.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivateKnowledge {
    None,
    Living {
        choice: Option<NightChoice>,
        wolf_team: Vec<SeatId>,
        seer_reports: BTreeMap<SeatId, Alignment>,
        doctor_previous: Option<SeatId>,
        hunter_mark: Option<SeatId>,
        witch_potions: Option<WitchPotions>,
    },
    Full {
        night_choices: BTreeMap<SeatId, NightChoice>,
        seer_reports: BTreeMap<SeatId, Alignment>,
        witch_potions: Option<WitchPotions>,
        history: Vec<ViewEvent>,
        hunter_mark: Option<SeatId>,
        hunter_fired: bool,
        doctor_previous: Option<SeatId>,
    },
}
/// Sole client state type. No seed, canonical version, private action counts, or State.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub phase: Phase,
    pub round: u32,
    pub phase_ends_at: LogicalTime,
    pub roster: Vec<SeatView>,
    pub perspective: Perspective,
    pub knowledge: PrivateKnowledge,
    pub legal_commands: Vec<Command>,
    pub votes: BTreeMap<SeatId, Ballot>,
    pub public_history: Vec<ViewEvent>,
    pub outcome: Option<MatchOutcome>,
}
/// Client event vocabulary; public death events never carry cause or killer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewEvent {
    PhaseChanged {
        phase: Phase,
        round: u32,
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
    RolesAssigned {
        roles: BTreeMap<SeatId, Role>,
    },
}
fn full_vision(state: &State, viewer: Viewer) -> bool {
    match viewer {
        Viewer::Audit => true,
        Viewer::Seat(seat) => state.roles.contains_key(&seat) && !state.alive.contains(&seat),
        Viewer::Spectator(_) => false,
    }
}
/// Derive all authorized current facts; projection itself never mutates state.
#[must_use]
pub fn project(state: &State, viewer: Viewer) -> View {
    let full = full_vision(state, viewer);
    let own = viewer.seat().filter(|seat| state.roles.contains_key(seat));
    let perspective = match viewer {
        Viewer::Audit => Perspective::Audit,
        Viewer::Seat(seat) if own.is_some() => Perspective::Seat {
            seat,
            role: state.roles[&seat],
            alive: state.alive.contains(&seat),
            can_act: state.phase.is_playing()
                && state.alive.contains(&seat)
                && state.player_status[&seat].can_act(),
        },
        Viewer::Seat(_) | Viewer::Spectator(_) => Perspective::Outside,
    };
    let roster = state
        .roster
        .iter()
        .map(|&seat| SeatView {
            seat,
            alive: state.alive.contains(&seat),
            status: state.player_status[&seat],
            role: if full
                || state.phase == Phase::Ended
                || state.revealed.contains_key(&seat)
                || own == Some(seat)
                || (own.is_some_and(|own| state.roles[&own].is_wolf())
                    && state.roles[&seat].is_wolf())
            {
                RoleKnowledge::Known(state.roles[&seat])
            } else {
                RoleKnowledge::Hidden
            },
        })
        .collect();
    let knowledge = private_knowledge(state, viewer, full, own);
    let outsider = Viewer::Spectator(tabula_core::SpectatorTier::Live);
    View {
        phase: state.phase,
        round: state.round,
        phase_ends_at: state.phase_ends_at,
        roster,
        perspective,
        knowledge,
        legal_commands: own.map_or_else(Vec::new, |seat| reducer::commands(state, seat)),
        votes: state.votes.clone(),
        public_history: state
            .history
            .iter()
            .filter_map(|event| view_event(state, event, outsider))
            .collect(),
        outcome: state.outcome.clone(),
    }
}
/// Private event existence is invisible to unauthorized clients, including outsiders at Ended.
#[must_use]
pub fn view_event(state: &State, event: &Event, viewer: Viewer) -> Option<ViewEvent> {
    let full = full_vision(state, viewer);
    match event {
        Event::RolesAssigned { roles } => {
            matches!(viewer, Viewer::Audit).then(|| ViewEvent::RolesAssigned {
                roles: roles.clone(),
            })
        }
        Event::PhaseChanged {
            phase,
            round,
            ends_at,
            ..
        } => Some(ViewEvent::PhaseChanged {
            phase: *phase,
            round: *round,
            ends_at: *ends_at,
        }),
        Event::NightActionSubmitted {
            seat,
            choice,
            round,
        } => (full || viewer == Viewer::Seat(*seat)).then_some(ViewEvent::NightActionSubmitted {
            seat: *seat,
            choice: *choice,
            round: *round,
        }),
        Event::SeerReport {
            seer,
            target,
            alignment,
            round,
        } => (full || viewer == Viewer::Seat(*seer)).then_some(ViewEvent::SeerReport {
            seer: *seer,
            target: *target,
            alignment: *alignment,
            round: *round,
        }),
        Event::NightResolved {
            round,
            attacked,
            protected,
            healed,
            poisoned,
        } => full.then_some(ViewEvent::NightResolved {
            round: *round,
            attacked: *attacked,
            protected: *protected,
            healed: *healed,
            poisoned: *poisoned,
        }),
        Event::HunterTriggered { hunter, target } => full.then_some(ViewEvent::HunterTriggered {
            hunter: *hunter,
            target: *target,
        }),
        Event::BallotChanged { seat, ballot } => Some(ViewEvent::BallotChanged {
            seat: *seat,
            ballot: *ballot,
        }),
        Event::VoteResolved {
            round,
            tally,
            eliminated,
        } => Some(ViewEvent::VoteResolved {
            round: *round,
            tally: tally.clone(),
            eliminated: *eliminated,
        }),
        Event::DeathRevealed { seat, role } => Some(ViewEvent::DeathRevealed {
            seat: *seat,
            role: *role,
        }),
        Event::SeatStatusChanged { seat, status } => Some(ViewEvent::SeatStatusChanged {
            seat: *seat,
            status: *status,
        }),
        Event::MatchEnded { outcome } => Some(ViewEvent::MatchEnded {
            outcome: outcome.clone(),
        }),
    }
}

fn private_knowledge(
    state: &State,
    viewer: Viewer,
    full: bool,
    own: Option<SeatId>,
) -> PrivateKnowledge {
    if full {
        PrivateKnowledge::Full {
            night_choices: state.night_choices.clone(),
            hunter_mark: state.hunter_mark,
            hunter_fired: state.hunter_fired,
            doctor_previous: state.last_doctor_target,
            seer_reports: state.seer_history.clone(),
            witch_potions: state.witch_potions,
            history: state
                .history
                .iter()
                .filter_map(|event| view_event(state, event, viewer))
                .collect(),
        }
    } else if let Some(seat) = own {
        let role = state.roles[&seat];
        PrivateKnowledge::Living {
            choice: state.night_choices.get(&seat).copied(),
            wolf_team: if role.is_wolf() {
                state
                    .roster
                    .iter()
                    .copied()
                    .filter(|seat| state.roles[seat].is_wolf())
                    .collect()
            } else {
                vec![]
            },
            seer_reports: if role == Role::Seer {
                state.seer_history.clone()
            } else {
                BTreeMap::new()
            },
            doctor_previous: if role == Role::Doctor {
                state.last_doctor_target
            } else {
                None
            },
            hunter_mark: if role == Role::Hunter {
                state.hunter_mark
            } else {
                None
            },
            witch_potions: if role == Role::Witch {
                state.witch_potions
            } else {
                None
            },
        }
    } else {
        PrivateKnowledge::None
    }
}
