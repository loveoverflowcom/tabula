#![allow(clippy::doc_markdown)] // `@ai.*` values are machine-readable paths.

//! Canonical Werewolf match state and structural validation barriers. (doc 02 §10.2, doc 08 §5.1)
//!
//! @ai.role domain-types
//! @ai.domain werewolf.rules.state
//! @ai.pure true
//! @ai.invariant state-structural-validity
//! @ai.invariant canonical-round-trip
//! @ai.evidence tests::state::state_canonical_round_trip
//! @ai.evidence tests::state::state_reconstruction_rejects_missing_role

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tabula_core::{LogicalTime, MatchOutcome, OutcomeKind, SeatId, TimerId};
use tabula_game_api::ConfigError;

use super::config::{Config, PhaseDuration, SeatCount, MAX_SEATS, MIN_SEATS};
use super::role::{Alignment, Preset, Role, RoleCounts};

/// Match phase progression in Werewolf. (doc 08 §5.1, W-D16)
///
/// Werewolf progresses through five fixed-duration phases per round:
/// `Night -> Dawn -> Day -> Vote -> Dusk`, returning to `Night` for the next round
/// until victory conditions are reached or round limit terminates in stalemate.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Phase {
    Night,
    Dawn,
    Day,
    Vote,
    Dusk,
    Ended,
}

impl Phase {
    /// Returns `true` if this is an active playing phase (not `Ended`).
    #[must_use]
    pub const fn is_playing(self) -> bool {
        !matches!(self, Self::Ended)
    }
}

/// Connection and participation lifecycle state for a seat. (W-D8)
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub enum PlayerStatus {
    /// Normal connected participant.
    #[default]
    Active,
    /// Temporarily disconnected; phase continues and missing actions default at expiry.
    Disconnected,
    /// Player went idle.
    Idle,
    /// Permanently absent (abandoned/vacated); seat retains role, choices default.
    PermanentlyAbsent,
}

impl PlayerStatus {
    /// Returns `true` if this seat can submit player commands.
    #[must_use]
    pub const fn can_act(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Inventory of one-time potions held by the Witch. (W-D4)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WitchPotions {
    pub heal: bool,
    pub poison: bool,
}

impl Default for WitchPotions {
    fn default() -> Self {
        Self {
            heal: true,
            poison: true,
        }
    }
}

/// A player's vote choice during the Day voting phase. (W-D6, W-D7)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Ballot {
    /// Vote to eliminate a specific living seat.
    Target(SeatId),
    /// Explicit abstention from voting.
    Abstain,
}

/// A living player's private night action submission. (W-D3..W-D5)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum NightChoice {
    /// Werewolf attack consensus candidate (or pass with `None`).
    WolfTarget(Option<SeatId>),
    /// Seer investigation target (alignment revealed at Dawn).
    Investigate(SeatId),
    /// Doctor protection target (or pass with `None`).
    Protect(Option<SeatId>),
    /// Witch heal target against wolf attack (or pass with `None`).
    WitchHeal(Option<SeatId>),
    /// Witch poison kill target (or pass with `None`).
    WitchPoison(Option<SeatId>),
    /// Hunter precommitted retaliation target (or pass with `None`).
    HunterMark(Option<SeatId>),
    /// Explicit pass with no action.
    Pass,
}

/// Authoritative, canonical server state for a Werewolf match. (doc 02 §12.3, doc 08 §5)
///
/// Contains only canonical game facts. Client views (`View`) and redacted event
/// projections (`ViewEvent`) are derived from this state (I-5, I-6).
///
/// All collections use deterministic ordered data structures (`Vec`, `BTreeMap`, `BTreeSet`)
/// to guarantee reproducibility across all targets and builds (I-2, I-4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawState", into = "RawState")]
pub struct State {
    pub(crate) config: Config,
    pub(crate) roster: Vec<SeatId>,
    pub(crate) roles: BTreeMap<SeatId, Role>,
    pub(crate) alive: BTreeSet<SeatId>,
    pub(crate) revealed: BTreeMap<SeatId, Role>,
    pub(crate) phase: Phase,
    pub(crate) round: u32,
    pub(crate) current_timer: TimerId,
    pub(crate) phase_ends_at: LogicalTime,
    pub(crate) player_status: BTreeMap<SeatId, PlayerStatus>,
    pub(crate) last_doctor_target: Option<SeatId>,
    pub(crate) witch_potions: Option<WitchPotions>,
    pub(crate) seer_history: BTreeMap<SeatId, Alignment>,
    pub(crate) night_choices: BTreeMap<SeatId, NightChoice>,
    pub(crate) votes: BTreeMap<SeatId, Ballot>,
    pub(crate) outcome: Option<MatchOutcome>,
}

/// Unvalidated deserialization DTO for [`State`] reconstruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawState {
    pub config: Config,
    pub roster: Vec<SeatId>,
    pub roles: BTreeMap<SeatId, Role>,
    pub alive: BTreeSet<SeatId>,
    pub revealed: BTreeMap<SeatId, Role>,
    pub phase: Phase,
    pub round: u32,
    pub current_timer: TimerId,
    pub phase_ends_at: LogicalTime,
    pub player_status: BTreeMap<SeatId, PlayerStatus>,
    pub last_doctor_target: Option<SeatId>,
    pub witch_potions: Option<WitchPotions>,
    pub seer_history: BTreeMap<SeatId, Alignment>,
    pub night_choices: BTreeMap<SeatId, NightChoice>,
    pub votes: BTreeMap<SeatId, Ballot>,
    pub outcome: Option<MatchOutcome>,
}

impl From<State> for RawState {
    fn from(s: State) -> Self {
        Self {
            config: s.config,
            roster: s.roster,
            roles: s.roles,
            alive: s.alive,
            revealed: s.revealed,
            phase: s.phase,
            round: s.round,
            current_timer: s.current_timer,
            phase_ends_at: s.phase_ends_at,
            player_status: s.player_status,
            last_doctor_target: s.last_doctor_target,
            witch_potions: s.witch_potions,
            seer_history: s.seer_history,
            night_choices: s.night_choices,
            votes: s.votes,
            outcome: s.outcome,
        }
    }
}

/// Why a candidate or reconstructed [`State`] is structurally inadmissible.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("seat count {got} is outside allowed range [{MIN_SEATS}..={MAX_SEATS}]")]
    InvalidSeatCount { got: usize },
    #[error("roster seats must be strictly sorted ascending and unique")]
    RosterNotSortedOrUnique,
    #[error("seat {seat:?} is missing a role assignment")]
    MissingRole { seat: SeatId },
    #[error("role map contains unknown seat {seat:?}")]
    UnknownSeatInRoles { seat: SeatId },
    #[error("role distribution does not match preset {preset:?} for {seats} seats: got {actual:?}, expected {expected:?}")]
    RoleCountMismatch {
        preset: Preset,
        seats: u8,
        actual: RoleCounts,
        expected: RoleCounts,
    },
    #[error("seat {seat:?} is missing a player status")]
    MissingPlayerStatus { seat: SeatId },
    #[error("player status map contains unknown seat {seat:?}")]
    UnknownSeatInPlayerStatus { seat: SeatId },
    #[error("alive set contains unknown seat {seat:?}")]
    UnknownSeatInAlive { seat: SeatId },
    #[error("revealed map contains unknown seat {seat:?}")]
    UnknownSeatInRevealed { seat: SeatId },
    #[error(
        "revealed role {revealed:?} for seat {seat:?} does not match assigned role {assigned:?}"
    )]
    RevealedRoleMismatch {
        seat: SeatId,
        revealed: Role,
        assigned: Role,
    },
    #[error("seat {seat:?} cannot be both alive and revealed during playing phases")]
    AliveAndRevealedConflict { seat: SeatId },
    #[error("playing match must have at least one living seat")]
    EmptyAliveSetInPlayingMatch,
    #[error("playing match has seat {seat:?} that is neither alive nor revealed")]
    SeatNeitherAliveNorRevealed { seat: SeatId },
    #[error("round {round} is invalid (max_rounds is {max})")]
    InvalidRound { round: u32, max: u32 },
    #[error("timer id 0 is invalid")]
    InvalidTimerId,
    #[error("votes container must be empty outside Phase::Vote")]
    VotesOutsideVotePhase,
    #[error("voting seat {voter:?} is not alive")]
    DeadSeatVoted { voter: SeatId },
    #[error("vote target {target:?} is not alive or unknown")]
    InvalidVoteTarget { target: SeatId },
    #[error("voter {voter:?} attempted to vote for self")]
    SelfVoteNotAllowed { voter: SeatId },
    #[error("night choices container must be empty outside Phase::Night")]
    NightChoicesOutsideNightPhase,
    #[error("night action actor {actor:?} is not alive")]
    DeadSeatActedInNight { actor: SeatId },
    #[error("night choice target {target:?} is unknown")]
    UnknownNightTarget { target: SeatId },
    #[error("night choice target {target:?} is not living")]
    DeadNightTarget { target: SeatId },
    #[error("night action actor {actor:?} cannot target self")]
    SelfNightTarget { actor: SeatId },
    #[error("werewolf target {target:?} is a werewolf")]
    WerewolfTargetNotAllowed { target: SeatId },
    #[error("night choice {choice:?} is unauthorized for role {role:?}")]
    UnauthorizedNightChoice { role: Role, choice: NightChoice },
    #[error("Doctor target {target:?} is unknown")]
    UnknownDoctorTarget { target: SeatId },
    #[error("last_doctor_target is set but match has no Doctor")]
    DoctorTargetWithoutDoctor,
    #[error("witch_potions must be Some when Witch is in match, None otherwise")]
    InvalidWitchPotionOwnership,
    #[error("Seer history target {target:?} is unknown")]
    UnknownSeerHistoryTarget { target: SeatId },
    #[error("Seer history alignment {alignment:?} does not match target {target:?} true alignment {expected:?}")]
    SeerHistoryAlignmentMismatch {
        target: SeatId,
        alignment: Alignment,
        expected: Alignment,
    },
    #[error("seer_history is non-empty but match has no Seer")]
    SeerHistoryWithoutSeer,
    #[error("match is in Phase::Ended but has no outcome")]
    EndedWithoutOutcome,
    #[error("match is playing but has a terminal outcome")]
    PlayingWithOutcome,
    #[error("outcome standings length does not match roster length")]
    OutcomeStandingsMismatch,
    #[error("outcome standings are missing seat {seat:?}")]
    MissingSeatInOutcome { seat: SeatId },
    #[error("deadline overflow when adding duration {duration:?} to now {now:?}")]
    DeadlineOverflow {
        now: LogicalTime,
        duration: PhaseDuration,
    },
    #[error("roster validation error: {0}")]
    RosterConfig(String),
}

impl From<ConfigError> for StateError {
    fn from(err: ConfigError) -> Self {
        Self::RosterConfig(err.to_string())
    }
}

impl TryFrom<RawState> for State {
    type Error = StateError;

    fn try_from(raw: RawState) -> Result<Self, Self::Error> {
        let seat_count = validate_roster(&raw.roster)?;
        let role_counts = validate_roles(&raw.roster, &raw.roles, raw.config.preset, seat_count)?;
        validate_player_status(&raw.roster, &raw.player_status)?;
        validate_liveness(
            &raw.roster,
            &raw.alive,
            &raw.revealed,
            raw.phase,
            &raw.roles,
        )?;
        validate_round_and_timer(&raw.config, raw.round, raw.current_timer)?;
        validate_votes(&raw.alive, raw.phase, &raw.votes)?;
        validate_night_choices(
            &raw.roster,
            &raw.alive,
            &raw.roles,
            raw.phase,
            &raw.night_choices,
        )?;
        validate_resources(
            &raw.roster,
            &raw.roles,
            role_counts,
            raw.last_doctor_target,
            raw.witch_potions,
            &raw.seer_history,
        )?;
        validate_outcome(&raw.roster, raw.phase, raw.outcome.as_ref())?;

        Ok(Self {
            config: raw.config,
            roster: raw.roster,
            roles: raw.roles,
            alive: raw.alive,
            revealed: raw.revealed,
            phase: raw.phase,
            round: raw.round,
            current_timer: raw.current_timer,
            phase_ends_at: raw.phase_ends_at,
            player_status: raw.player_status,
            last_doctor_target: raw.last_doctor_target,
            witch_potions: raw.witch_potions,
            seer_history: raw.seer_history,
            night_choices: raw.night_choices,
            votes: raw.votes,
            outcome: raw.outcome,
        })
    }
}

fn validate_roster(roster: &[SeatId]) -> Result<SeatCount, StateError> {
    let got = roster.len();
    if !(usize::from(MIN_SEATS)..=usize::from(MAX_SEATS)).contains(&got) {
        return Err(StateError::InvalidSeatCount { got });
    }
    if !roster.windows(2).all(|window| window[0] < window[1]) {
        return Err(StateError::RosterNotSortedOrUnique);
    }
    SeatCount::new(u8::try_from(got).map_err(|_| StateError::InvalidSeatCount { got })?)
        .map_err(|_| StateError::InvalidSeatCount { got })
}

fn validate_roles(
    roster: &[SeatId],
    roles: &BTreeMap<SeatId, Role>,
    preset: Preset,
    seat_count: SeatCount,
) -> Result<RoleCounts, StateError> {
    for &seat in roles.keys() {
        if !roster.contains(&seat) {
            return Err(StateError::UnknownSeatInRoles { seat });
        }
    }
    for &seat in roster {
        if !roles.contains_key(&seat) {
            return Err(StateError::MissingRole { seat });
        }
    }

    let mut actual = RoleCounts {
        werewolves: 0,
        seers: 0,
        doctors: 0,
        hunters: 0,
        witches: 0,
        villagers: 0,
    };
    for role in roles.values() {
        match role {
            Role::Werewolf => actual.werewolves += 1,
            Role::Seer => actual.seers += 1,
            Role::Doctor => actual.doctors += 1,
            Role::Hunter => actual.hunters += 1,
            Role::Witch => actual.witches += 1,
            Role::Villager => actual.villagers += 1,
        }
    }
    let expected = preset.role_counts(seat_count);
    if actual != expected {
        return Err(StateError::RoleCountMismatch {
            preset,
            seats: seat_count.get(),
            actual,
            expected,
        });
    }
    Ok(actual)
}

fn validate_player_status(
    roster: &[SeatId],
    statuses: &BTreeMap<SeatId, PlayerStatus>,
) -> Result<(), StateError> {
    for &seat in statuses.keys() {
        if !roster.contains(&seat) {
            return Err(StateError::UnknownSeatInPlayerStatus { seat });
        }
    }
    for &seat in roster {
        if !statuses.contains_key(&seat) {
            return Err(StateError::MissingPlayerStatus { seat });
        }
    }
    Ok(())
}

fn validate_liveness(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    revealed: &BTreeMap<SeatId, Role>,
    phase: Phase,
    roles: &BTreeMap<SeatId, Role>,
) -> Result<(), StateError> {
    for &seat in alive {
        if !roster.contains(&seat) {
            return Err(StateError::UnknownSeatInAlive { seat });
        }
    }
    for (&seat, &revealed_role) in revealed {
        if !roster.contains(&seat) {
            return Err(StateError::UnknownSeatInRevealed { seat });
        }
        let assigned_role = roles[&seat];
        if revealed_role != assigned_role {
            return Err(StateError::RevealedRoleMismatch {
                seat,
                revealed: revealed_role,
                assigned: assigned_role,
            });
        }
    }

    if phase.is_playing() {
        if alive.is_empty() {
            return Err(StateError::EmptyAliveSetInPlayingMatch);
        }
        for &seat in alive {
            if revealed.contains_key(&seat) {
                return Err(StateError::AliveAndRevealedConflict { seat });
            }
        }
        for &seat in roster {
            if !alive.contains(&seat) && !revealed.contains_key(&seat) {
                return Err(StateError::SeatNeitherAliveNorRevealed { seat });
            }
        }
    }
    Ok(())
}

fn validate_round_and_timer(
    config: &Config,
    round: u32,
    current_timer: TimerId,
) -> Result<(), StateError> {
    let max_rounds = config.max_rounds.get();
    if round < 1 || round > max_rounds {
        return Err(StateError::InvalidRound {
            round,
            max: max_rounds,
        });
    }
    if current_timer.0 == 0 {
        return Err(StateError::InvalidTimerId);
    }
    Ok(())
}

fn validate_votes(
    alive: &BTreeSet<SeatId>,
    phase: Phase,
    votes: &BTreeMap<SeatId, Ballot>,
) -> Result<(), StateError> {
    if phase != Phase::Vote && !votes.is_empty() {
        return Err(StateError::VotesOutsideVotePhase);
    }
    if phase == Phase::Vote {
        for (&voter, ballot) in votes {
            if !alive.contains(&voter) {
                return Err(StateError::DeadSeatVoted { voter });
            }
            if let Ballot::Target(target) = ballot {
                if !alive.contains(target) {
                    return Err(StateError::InvalidVoteTarget { target: *target });
                }
                if *target == voter {
                    return Err(StateError::SelfVoteNotAllowed { voter });
                }
            }
        }
    }
    Ok(())
}

fn validate_night_choices(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    roles: &BTreeMap<SeatId, Role>,
    phase: Phase,
    choices: &BTreeMap<SeatId, NightChoice>,
) -> Result<(), StateError> {
    if phase != Phase::Night && !choices.is_empty() {
        return Err(StateError::NightChoicesOutsideNightPhase);
    }
    if phase != Phase::Night {
        return Ok(());
    }
    for (&actor, choice) in choices {
        if !alive.contains(&actor) {
            return Err(StateError::DeadSeatActedInNight { actor });
        }
        let role = roles[&actor];
        match choice {
            NightChoice::WolfTarget(target) => {
                validate_choice_role(role, Role::Werewolf, *choice)?;
                validate_optional_non_wolf_target(roster, alive, roles, *target)?;
            }
            NightChoice::Investigate(target) => {
                validate_choice_role(role, Role::Seer, *choice)?;
                validate_other_living_target(roster, alive, actor, *target)?;
            }
            NightChoice::Protect(target) => {
                validate_choice_role(role, Role::Doctor, *choice)?;
                validate_optional_living_target(roster, alive, *target)?;
            }
            NightChoice::WitchHeal(target) | NightChoice::WitchPoison(target) => {
                validate_choice_role(role, Role::Witch, *choice)?;
                validate_optional_living_target(roster, alive, *target)?;
            }
            NightChoice::HunterMark(target) => {
                validate_choice_role(role, Role::Hunter, *choice)?;
                validate_optional_other_living_target(roster, alive, actor, *target)?;
            }
            NightChoice::Pass => {
                if role == Role::Villager {
                    return Err(StateError::UnauthorizedNightChoice {
                        role,
                        choice: *choice,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_choice_role(
    actual: Role,
    expected: Role,
    choice: NightChoice,
) -> Result<(), StateError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StateError::UnauthorizedNightChoice {
            role: actual,
            choice,
        })
    }
}

fn validate_optional_living_target(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    target: Option<SeatId>,
) -> Result<(), StateError> {
    if let Some(target) = target {
        validate_living_target(roster, alive, target)?;
    }
    Ok(())
}

fn validate_optional_non_wolf_target(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    roles: &BTreeMap<SeatId, Role>,
    target: Option<SeatId>,
) -> Result<(), StateError> {
    if let Some(target) = target {
        validate_living_target(roster, alive, target)?;
        if roles[&target].is_wolf() {
            return Err(StateError::WerewolfTargetNotAllowed { target });
        }
    }
    Ok(())
}

fn validate_optional_other_living_target(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    actor: SeatId,
    target: Option<SeatId>,
) -> Result<(), StateError> {
    if let Some(target) = target {
        validate_other_living_target(roster, alive, actor, target)?;
    }
    Ok(())
}

fn validate_living_target(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    target: SeatId,
) -> Result<(), StateError> {
    if !roster.contains(&target) {
        return Err(StateError::UnknownNightTarget { target });
    }
    if !alive.contains(&target) {
        return Err(StateError::DeadNightTarget { target });
    }
    Ok(())
}

fn validate_other_living_target(
    roster: &[SeatId],
    alive: &BTreeSet<SeatId>,
    actor: SeatId,
    target: SeatId,
) -> Result<(), StateError> {
    validate_living_target(roster, alive, target)?;
    if target == actor {
        return Err(StateError::SelfNightTarget { actor });
    }
    Ok(())
}

fn validate_resources(
    roster: &[SeatId],
    roles: &BTreeMap<SeatId, Role>,
    role_counts: RoleCounts,
    last_doctor_target: Option<SeatId>,
    witch_potions: Option<WitchPotions>,
    seer_history: &BTreeMap<SeatId, Alignment>,
) -> Result<(), StateError> {
    if let Some(target) = last_doctor_target {
        if role_counts.doctors == 0 {
            return Err(StateError::DoctorTargetWithoutDoctor);
        }
        if !roster.contains(&target) {
            return Err(StateError::UnknownDoctorTarget { target });
        }
    }
    if (role_counts.witches > 0) != witch_potions.is_some() {
        return Err(StateError::InvalidWitchPotionOwnership);
    }
    if role_counts.seers == 0 && !seer_history.is_empty() {
        return Err(StateError::SeerHistoryWithoutSeer);
    }
    for (&target, &alignment) in seer_history {
        if !roster.contains(&target) {
            return Err(StateError::UnknownSeerHistoryTarget { target });
        }
        let expected = roles[&target].alignment();
        if alignment != expected {
            return Err(StateError::SeerHistoryAlignmentMismatch {
                target,
                alignment,
                expected,
            });
        }
    }
    Ok(())
}

fn validate_outcome(
    roster: &[SeatId],
    phase: Phase,
    outcome: Option<&MatchOutcome>,
) -> Result<(), StateError> {
    match (phase, outcome) {
        (Phase::Ended, None) => Err(StateError::EndedWithoutOutcome),
        (Phase::Ended, Some(outcome)) => {
            if !matches!(outcome.kind(), OutcomeKind::Aborted { .. }) {
                if outcome.standings().len() != roster.len() {
                    return Err(StateError::OutcomeStandingsMismatch);
                }
                for &seat in roster {
                    if !outcome
                        .standings()
                        .iter()
                        .any(|standing| standing.seat == seat)
                    {
                        return Err(StateError::MissingSeatInOutcome { seat });
                    }
                }
            }
            Ok(())
        }
        (phase, Some(_)) if phase.is_playing() => Err(StateError::PlayingWithOutcome),
        _ => Ok(()),
    }
}

impl State {
    #[must_use]
    pub const fn config(&self) -> Config {
        self.config
    }

    #[must_use]
    pub fn roster(&self) -> &[SeatId] {
        &self.roster
    }

    #[must_use]
    pub const fn roles(&self) -> &BTreeMap<SeatId, Role> {
        &self.roles
    }

    #[must_use]
    pub const fn alive(&self) -> &BTreeSet<SeatId> {
        &self.alive
    }

    #[must_use]
    pub const fn revealed(&self) -> &BTreeMap<SeatId, Role> {
        &self.revealed
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub const fn round(&self) -> u32 {
        self.round
    }

    #[must_use]
    pub const fn current_timer(&self) -> TimerId {
        self.current_timer
    }

    #[must_use]
    pub const fn phase_ends_at(&self) -> LogicalTime {
        self.phase_ends_at
    }

    #[must_use]
    pub const fn player_status(&self) -> &BTreeMap<SeatId, PlayerStatus> {
        &self.player_status
    }

    #[must_use]
    pub const fn last_doctor_target(&self) -> Option<SeatId> {
        self.last_doctor_target
    }

    #[must_use]
    pub const fn witch_potions(&self) -> Option<WitchPotions> {
        self.witch_potions
    }

    #[must_use]
    pub const fn seer_history(&self) -> &BTreeMap<SeatId, Alignment> {
        &self.seer_history
    }

    #[must_use]
    pub const fn night_choices(&self) -> &BTreeMap<SeatId, NightChoice> {
        &self.night_choices
    }

    #[must_use]
    pub const fn votes(&self) -> &BTreeMap<SeatId, Ballot> {
        &self.votes
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<&MatchOutcome> {
        self.outcome.as_ref()
    }
}

/// Computes `now + duration` without wrapping, returning an error if `u64` arithmetic overflows.
///
/// # Errors
/// Returns [`StateError::DeadlineOverflow`] if `now + duration` exceeds `u64::MAX`.
pub fn checked_deadline(
    now: LogicalTime,
    duration: PhaseDuration,
) -> Result<LogicalTime, StateError> {
    now.0
        .checked_add(duration.millis())
        .map(LogicalTime)
        .ok_or(StateError::DeadlineOverflow { now, duration })
}
