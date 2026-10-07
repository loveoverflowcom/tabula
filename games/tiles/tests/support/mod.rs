//! Shared fixture data and a tiny match driver for the Tiles test suite.
//!
//! The driver exists because a legal Tiles script depends on the shuffled bag,
//! which depends on the seed — so a hand-written literal script would be a
//! transcription of one seed's shuffle and would rot the moment the tile set
//! changed. Deriving the script by always taking the **first legal placement in
//! canonical order** keeps it legal by construction and keeps it reproducible.
//!
//! Note what this does *not* do: it never decides whether a placement is
//! correct. The oracles for that live in `tests/rules.rs` (exhaustive) and in
//! the crate's own `placement` tests, and they are written from the definition
//! of adjacency rather than from `apply`.

#![allow(dead_code)]

use tabula_core::{
    DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input, Outcome};
use tabula_game_tiles::{
    rules::first_legal_placement, Command, Config, State, Status, TilesRules, TurnPhase,
};

pub const SEED: [u8; 32] = [42u8; 32];
/// Mirror of the crate's own seat bounds, so generators stay inside them.
pub const SEATS_MIN: u8 = tabula_game_tiles::rules::MIN_SEATS;
pub const SEATS_MAX: u8 = tabula_game_tiles::rules::MAX_SEATS;
pub const ALT_SEED: [u8; 32] = [43u8; 32];

pub fn seed() -> MatchSeed {
    MatchSeed::from_bytes(SEED)
}

pub fn roster(count: u8) -> SeatRoster {
    SeatRoster::new(
        (0..count)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("fixture seats are unique")
}

pub fn config() -> Config {
    Config {
        turn_deadline_ms: 0,
    }
}

pub fn timed_config(turn_deadline_ms: u64) -> Config {
    Config { turn_deadline_ms }
}

/// Build the opening position exactly as the platform would.
pub fn create(seed: &MatchSeed, seats: u8, cfg: Config) -> State {
    let mut rng = DetRng::for_input(seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    TilesRules::create(&cfg, &roster(seats), &mut ctx)
        .expect("the fixture roster and config are valid")
        .state
}

/// Apply one input at a given index, the way the runtime numbers them.
pub fn apply_at(
    state: &mut State,
    input: Input<Command>,
    seed: &MatchSeed,
    index: u64,
) -> Result<Outcome<TilesRules>, tabula_core::RuleError> {
    let index = InputIndex(index);
    let mut rng = DetRng::for_input(seed, index);
    let mut ctx = Ctx {
        now: LogicalTime(index.0),
        index,
        rng: &mut rng,
        budget: Budget::default(),
    };
    TilesRules::apply(state, input, &mut ctx)
}

/// The input a "first legal in canonical order, claim greedily" player would
/// issue now — a placement in the placement step, a claim or a pass in the
/// claim step.
///
/// Claiming greedily is deliberate: a driver that always passed would never
/// put a follower on the board, and every scoring, ownership, and
/// follower-return assertion downstream would be vacuous.
pub fn next_input(state: &State) -> Option<Input<Command>> {
    if state.status() != Status::Playing || state.paused() {
        return None;
    }
    let seat = state.turn();
    match state.phase() {
        TurnPhase::PlaceTile => {
            let kind = state.drawn()?;
            let (at, rotation) = first_legal_placement(state.board(), kind)?;
            Some(Input::Player {
                seat,
                command: Command::PlaceTile { at, rotation },
            })
        }
        TurnPhase::PlaceMeeple => Some(Input::Player {
            seat,
            command: match state.claimable_segments().first() {
                Some(segment) => Command::PlaceMeeple { segment: *segment },
                None => Command::SkipMeeple,
            },
        }),
    }
}

/// The placement the same player would make now, or `None` if it is not the
/// placement step.
pub fn next_placement(state: &State) -> Option<Input<Command>> {
    match next_input(state) {
        Some(
            input @ Input::Player {
                command: Command::PlaceTile { .. },
                ..
            },
        ) => Some(input),
        _ => None,
    }
}

/// Issue up to `max_inputs` canonical inputs from the opening position,
/// returning the reached state and the script that reached it.
pub fn drive(
    seed: &MatchSeed,
    seats: u8,
    cfg: Config,
    max_inputs: usize,
) -> (State, Vec<Input<Command>>) {
    let mut state = create(seed, seats, cfg);
    let mut script = Vec::new();
    for step in 0..max_inputs {
        let Some(input) = next_input(&state) else {
            break;
        };
        apply_at(&mut state, input.clone(), seed, step as u64 + 1)
            .expect("a driven input is legal");
        script.push(input);
    }
    (state, script)
}

/// Drive at least `min_inputs` inputs and then keep going until the seat on
/// turn is in the claim step.
///
/// The conformance suite's `legal_commands` check only inspects `Enumerated`
/// results, and Tiles only enumerates in the claim step — so a fixture whose
/// script happened to stop at a turn boundary would make that check silently
/// vacuous.
pub fn drive_to_claim_phase(
    seed: &MatchSeed,
    seats: u8,
    cfg: Config,
    min_inputs: usize,
) -> (State, Vec<Input<Command>>) {
    let mut state = create(seed, seats, cfg);
    let mut script = Vec::new();
    loop {
        if script.len() >= min_inputs && state.phase() == TurnPhase::PlaceMeeple {
            break;
        }
        let Some(input) = next_input(&state) else {
            break;
        };
        let index = script.len() as u64 + 1;
        apply_at(&mut state, input.clone(), seed, index).expect("a driven input is legal");
        script.push(input);
    }
    (state, script)
}

/// A script that plays the match to its terminal state. The bag holds 71
/// tiles and each turn is at most two inputs, so 256 is a generous bound.
pub fn full_script(seed: &MatchSeed, seats: u8, cfg: Config) -> Vec<Input<Command>> {
    drive(seed, seats, cfg, 256).1
}
