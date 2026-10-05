#![allow(clippy::doc_markdown)] // `@ai.*` values are machine-readable paths.

//! Complete pure `ClassicV1` rules and knowledge projections. (doc 02 §12.3, doc 08 §5)
//!
//! Owns validated state, fixed-window commands, simultaneous resolution,
//! canonical events and per-viewer disclosure. Runtime I/O and timers remain
//! the host's mechanism; the decisions returned here are deterministic data.
//!
//! @ai.role functional-core
//! @ai.domain werewolf.rules
//! @ai.pure true
//! @ai.invariant deterministic-role-assignment
//! @ai.invariant roster-order-invariance
//! @ai.evidence tests::assignment::seed_determinism_produces_byte_identical_state
//! @ai.evidence tests::assignment::roster_order_invariance_produces_identical_assignment

pub mod config;
pub mod projection;
pub mod reducer;
pub mod scopes;
pub use projection::{Perspective, PrivateKnowledge, RoleKnowledge, SeatView, View, ViewEvent};
pub use reducer::{Command, WerewolfRules};
pub mod event;
pub mod role;
#[cfg(feature = "testkit")]
mod secrets;
pub mod state;

use std::collections::{BTreeMap, BTreeSet};

use smallvec::SmallVec;
use tabula_core::{
    rng::domain, DetRng, InputIndex, LogicalTime, MatchSeed, RulesVersion, SeatId, SeatRoster,
    TimerId,
};

pub use config::{
    Config, ConfigValidationError, DurationError, MaxRounds, MaxRoundsError, PhaseDuration,
    PhaseDurations, RawConfig, RawPhaseDurations, SeatCount, SeatCountError, VoteMode,
    DEFAULT_DAWN_MS, DEFAULT_DAY_MS, DEFAULT_DUSK_MS, DEFAULT_MAX_ROUNDS, DEFAULT_NIGHT_MS,
    DEFAULT_VOTE_MS, MAX_MAX_ROUNDS, MAX_PHASE_DURATION_MS, MAX_SEATS, MIN_MAX_ROUNDS,
    MIN_PHASE_DURATION_MS, MIN_SEATS,
};
pub use event::Event;
pub use role::{Alignment, Preset, Role, RoleCounts};
pub use state::{
    checked_deadline, Ballot, NightChoice, Phase, PlayerStatus, RawState, State, StateError,
    WitchPotions,
};

/// Rules source version. Matches `game.toml` `rules_version`.
pub const RULES_VERSION: RulesVersion = RulesVersion(2);

/// BLAKE3 hash of canonical rules source, produced by `build.rs`.
pub const RULES_HASH: [u8; 32] = *include_bytes!(concat!(env!("OUT_DIR"), "/rules_hash.bin"));

/// Named RNG domain for initial role distribution shuffle. (W-D2, doc 08 §5.1)
pub const DOMAIN_ROLES: u32 = domain::GAME_BASE + 1;

/// Authoritative creation kernel for Werewolf initial Night-1 match state. (W-D1, W-D2)
///
/// Algorithm:
/// 1. Validate config and seat roster (6..=20 occupied seats, no pre-assigned teams).
/// 2. Collect and canonically sort `SeatId`s.
/// 3. Obtain exact `ClassicV1` role multiset for this seat count.
/// 4. Shuffle multiset exactly once using `ctx.rng.stream(DOMAIN_ROLES)`.
/// 5. Zip shuffled roles with sorted seats to form authoritative role assignment.
/// 6. Construct and structurally validate Night-1 canonical [`State`].
/// 7. Return validated [`State`] and initial canonical [`Event`]s (`RolesAssigned`, `PhaseChanged`).
///
/// # Errors
/// Returns [`StateError`] if roster validation fails, or if deadline addition overflows.
pub fn create_initial_state(
    config: &Config,
    roster: &SeatRoster,
    now: LogicalTime,
    rng: &mut DetRng,
) -> Result<(State, SmallVec<[Event; 2]>), StateError> {
    let seat_count = config.validate_roster(roster)?;
    let mut seats: Vec<SeatId> = roster.iter().map(|e| e.seat).collect();
    seats.sort_unstable();

    let phase_ends_at = checked_deadline(now, config.phase_durations.night)?;
    let mut roles = config.preset.role_counts(seat_count).multiset();
    rng.stream(DOMAIN_ROLES).shuffle(&mut roles);

    let role_map: BTreeMap<SeatId, Role> = seats.iter().copied().zip(roles).collect();
    let alive: BTreeSet<SeatId> = seats.iter().copied().collect();
    let player_status: BTreeMap<SeatId, PlayerStatus> =
        seats.iter().map(|&s| (s, PlayerStatus::Active)).collect();

    let timer_id = TimerId(1);
    let round = 1;
    let phase = Phase::Night;

    let witch_potions = if config.preset.role_counts(seat_count).witches > 0 {
        Some(WitchPotions::default())
    } else {
        None
    };

    let raw_state = RawState {
        config: *config,
        roster: seats,
        roles: role_map.clone(),
        alive,
        revealed: BTreeMap::new(),
        phase,
        round,
        current_timer: timer_id,
        phase_ends_at,
        player_status,
        last_doctor_target: None,
        witch_potions,
        seer_history: BTreeMap::new(),
        night_choices: BTreeMap::new(),
        votes: BTreeMap::new(),
        outcome: None,
        hunter_mark: None,
        hunter_fired: false,
        history: Vec::new(),
    };

    let mut state = State::try_from(raw_state)?;

    let events = smallvec::smallvec![
        Event::RolesAssigned { roles: role_map },
        Event::PhaseChanged {
            phase,
            round,
            timer_id,
            ends_at: phase_ends_at,
        },
    ];

    state.history = events.to_vec();
    Ok((state, events))
}

/// Convenience creation helper seeding the deterministic RNG from [`MatchSeed`].
///
/// # Errors
/// Returns [`StateError`] if initial state creation fails.
pub fn create_initial_state_from_seed(
    config: &Config,
    roster: &SeatRoster,
    now: LogicalTime,
    seed: &MatchSeed,
) -> Result<(State, SmallVec<[Event; 2]>), StateError> {
    let mut rng = DetRng::for_input(seed, InputIndex(0));
    create_initial_state(config, roster, now, &mut rng)
}
