//! Laws over generated inputs: transactional rejection, invariant
//! preservation, hint soundness, and replay equivalence.
//!
//! Every generator here produces **reachable** states — states the rules
//! themselves built from `create` and legal placements — because these are
//! semantic laws. Asserting a rule over a board assembled field-by-field would
//! prove nothing and would fail for reasons that are not bugs
//! (`rust-property-testing` §"Reachable state vs arbitrary state").
//!
//! The *inputs* fed to those states are the opposite: deliberately arbitrary,
//! including coordinates far off the board, seats that do not exist, and
//! rotations that cannot match. A generator that only emits legal actions
//! cannot prove anything about rejection.

mod support;

use proptest::prelude::*;
use tabula_core::{canonical_encode, MatchSeed, SeatId};
use tabula_game_api::{AdminInput, GameRules, Input};
use tabula_game_tiles::{
    rules::{legal_placements, MAX_COORD},
    Command, Coord, Rotation, State, Status, TilesRules,
};

use support::{apply_at, config, create, next_placement, SEATS_MAX, SEATS_MIN};

/// Pinned for the per-PR tier. Raise with `PROPTEST_CASES` for a nightly run.
const CASES: u32 = 96;

fn proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Replay a legal prefix to reach a state the rules really can be in.
///
/// The prefix is weighted toward *longer* games on purpose: short prefixes are
/// cheap to generate and are exactly where the interesting transitions are not
/// (`rust-property-testing` §generators — "watch the bias").
fn reachable(seed_byte: u8, seats: u8, steps: usize) -> State {
    let seed = MatchSeed::from_bytes([seed_byte; 32]);
    let mut state = create(&seed, seats, config());
    for step in 0..steps {
        let Some(input) = next_placement(&state) else {
            break;
        };
        apply_at(&mut state, input, &seed, step as u64 + 1)
            .expect("a first-legal placement is legal");
    }
    state
}

fn reachable_strategy() -> impl Strategy<Value = State> {
    (
        any::<u8>(),
        SEATS_MIN..=SEATS_MAX,
        prop_oneof![
            1 => 0usize..4,
            3 => 4usize..40,
            2 => 40usize..80,
        ],
    )
        .prop_map(|(seed_byte, seats, steps)| reachable(seed_byte, seats, steps))
}

/// Arbitrary — not reachable — player input. Most of it is illegal, which is
/// the point.
fn hostile_input() -> impl Strategy<Value = Input<Command>> {
    prop_oneof![
        8 => (
            any::<u8>(),
            -MAX_COORD..=MAX_COORD,
            -MAX_COORD..=MAX_COORD,
            0u8..4,
        )
            .prop_map(|(seat, x, y, rotation)| Input::Player {
                seat: SeatId(seat),
                command: Command::PlaceTile {
                    at: Coord::new(x, y).expect("generated inside the playable space"),
                    rotation: Rotation::from_quarter_turns(rotation),
                },
            }),
        1 => any::<u16>().prop_map(|id| Input::Timer {
            timer: tabula_core::TimerId(id)
        }),
        1 => Just(Input::Admin(AdminInput::Pause)),
        1 => Just(Input::Admin(AdminInput::Resume)),
    ]
}

proptest! {
    #![proptest_config(proptest_config())]

    /// **Contract R2, over reachable states and hostile input.** Strictly
    /// stronger than the fixture-driven conformance check, which exercises one
    /// hand-picked rejection.
    #[test]
    fn a_rejected_input_leaves_the_canonical_encoding_byte_identical(
        mut state in reachable_strategy(),
        input in hostile_input(),
    ) {
        let before = canonical_encode(&state).expect("a State encodes");
        let seed = MatchSeed::from_bytes([7u8; 32]);
        if apply_at(&mut state, input, &seed, 9_999).is_err() {
            prop_assert_eq!(
                canonical_encode(&state).expect("a State encodes"),
                before
            );
        }
    }

    /// **Invariant preservation.** Whatever an accepted input does, the result
    /// is still a state the validator would accept — checked against the very
    /// same function a decode uses, so there is one definition of "well
    /// formed" rather than a test-only restatement of it.
    #[test]
    fn an_accepted_input_leaves_a_state_the_validator_still_accepts(
        mut state in reachable_strategy(),
        input in hostile_input(),
    ) {
        let seed = MatchSeed::from_bytes([7u8; 32]);
        if apply_at(&mut state, input, &seed, 9_999).is_ok() {
            prop_assert_eq!(state.check_invariants(), Ok(()));
            // And it survives a canonical round trip, which is the same
            // validator reached by a different door.
            let bytes = canonical_encode(&state).expect("a State encodes");
            prop_assert!(tabula_core::canonical_decode::<State>(&bytes).is_ok());
        }
    }

    /// **Hint soundness over reachable states**, not just at two hand-picked
    /// positions the way the conformance suite checks enumerations.
    #[test]
    fn every_advertised_placement_is_accepted_by_apply(state in reachable_strategy()) {
        prop_assume!(state.status() == Status::Playing);
        let kind = state.drawn().expect("a playing match holds a drawn tile");
        let seed = MatchSeed::from_bytes([7u8; 32]);

        for (at, rotations) in legal_placements(state.board(), kind) {
            for rotation in rotations {
                let mut probe = state.clone();
                let outcome = apply_at(
                    &mut probe,
                    Input::Player {
                        seat: state.turn(),
                        command: Command::PlaceTile { at, rotation },
                    },
                    &seed,
                    1,
                );
                prop_assert!(
                    outcome.is_ok(),
                    "legal_placements advertised {at:?} {rotation:?} but apply refused it"
                );
            }
        }
    }

    /// **Determinism.** The same setup and the same ordered inputs produce the
    /// same canonical bytes, run from scratch twice. Two independently built
    /// runs in one process is what actually catches nondeterministic iteration
    /// order (I-2) — a single run cannot.
    #[test]
    fn the_same_script_run_twice_produces_identical_canonical_state(
        seed_byte in any::<u8>(),
        seats in SEATS_MIN..=SEATS_MAX,
        steps in 1usize..40,
    ) {
        let first = reachable(seed_byte, seats, steps);
        let second = reachable(seed_byte, seats, steps);
        prop_assert_eq!(
            canonical_encode(&first).expect("a State encodes"),
            canonical_encode(&second).expect("a State encodes")
        );
        prop_assert_eq!(
            TilesRules::state_hash(&first).0,
            TilesRules::state_hash(&second).0
        );
    }

    /// **Replay equivalence.** Recording a live run's accepted inputs and
    /// replaying them from `create` reproduces the state at *every* checkpoint,
    /// not merely at the end — a final-hash-only comparison would say a
    /// divergence happened without saying where.
    #[test]
    fn replaying_the_recorded_inputs_reproduces_every_checkpoint(
        seed_byte in any::<u8>(),
        seats in SEATS_MIN..=SEATS_MAX,
        steps in 1usize..40,
    ) {
        let seed = MatchSeed::from_bytes([seed_byte; 32]);

        // Live: play, recording each accepted input and the hash after it.
        let mut live = create(&seed, seats, config());
        let mut recorded = Vec::new();
        for step in 0..steps {
            let Some(input) = next_placement(&live) else { break };
            let index = step as u64 + 1;
            apply_at(&mut live, input.clone(), &seed, index).expect("legal");
            recorded.push((index, input, TilesRules::state_hash(&live).0));
        }
        prop_assume!(!recorded.is_empty());

        // Replay: same seed, same inputs, nothing else carried over.
        let mut replayed = create(&seed, seats, config());
        for (index, input, expected) in &recorded {
            apply_at(&mut replayed, input.clone(), &seed, *index)
                .expect("a recorded input replays");
            prop_assert_eq!(
                TilesRules::state_hash(&replayed).0,
                *expected,
                "replay diverged at input index {}",
                index
            );
        }
        prop_assert_eq!(
            canonical_encode(&replayed).expect("a State encodes"),
            canonical_encode(&live).expect("a State encodes")
        );
    }
}

/// A deterministic regression alongside the properties: the shortest sequence
/// that once broke `Coord`, kept as an ordinary test so it runs even when the
/// property suite is skipped or reconfigured.
///
/// `i16::MIN` reached `Coord::new`, whose `abs()` spelling panicked instead of
/// rejecting — a contract R3 violation on hostile input.
#[test]
fn the_extreme_coordinate_that_once_panicked_is_rejected() {
    assert!(Coord::new(i16::MIN, 0).is_err());
    assert!(Coord::new(0, i16::MIN).is_err());
    assert!(Coord::new(MAX_COORD + 1, 0).is_err());
}
