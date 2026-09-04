//! Tiles rule behaviour: turn progression, rejection classes, the deadline,
//! pausing, termination, and the `Hints` contract.
//!
//! Placement legality itself is exhaustively covered inside the crate
//! (`rules::placement::tests`) against an oracle written from the definition of
//! adjacency. This file checks the behaviour *around* it that only `apply` can
//! answer.

mod support;

use tabula_core::{
    canonical_decode, canonical_encode, AbortReason, MatchSeed, RuleErrorCode, SeatId,
};
use tabula_game_api::{AdminInput, GameRules, Input, LegalCommands};
use tabula_game_tiles::{
    rules::{PlaceTileHint, HINT_PLACE_TILE},
    Command, Coord, Event, Rotation, State, Status, TilesRules,
};

use support::{apply_at, config, create, drive, next_placement, timed_config, ALT_SEED, SEED};

const SEATS: u8 = 3;

fn seed() -> MatchSeed {
    MatchSeed::from_bytes(SEED)
}

fn opening() -> State {
    create(&seed(), SEATS, config())
}

// ---------------------------------------------------------------------------
// Creation and turn progression
// ---------------------------------------------------------------------------

#[test]
fn create_opens_with_the_start_tile_placed_and_one_tile_in_hand() {
    let state = opening();
    assert_eq!(state.board().len(), 1);
    assert!(state.board().contains(Coord::ORIGIN));
    assert!(state.drawn().is_some());
    assert_eq!(
        state.bag_remaining(),
        tabula_game_tiles::rules::BAG_SIZE - 1
    );
    assert!(
        state.discarded().is_empty(),
        "every tile in the set is placeable next to the start tile, so the \
         opening draw discards nothing"
    );
    assert_eq!(state.turn(), SeatId(0));
    assert_eq!(state.status(), Status::Playing);
}

#[test]
fn a_placement_hands_the_turn_on_and_draws_for_the_next_seat() {
    let mut state = opening();
    let before_bag = state.bag_remaining();
    let input = next_placement(&state).unwrap();
    let outcome = apply_at(&mut state, input, &seed(), 1).expect("legal");

    assert_eq!(state.turn(), SeatId(1));
    assert_eq!(state.board().len(), 2);
    assert_eq!(state.bag_remaining(), before_bag - 1);
    assert!(state.drawn().is_some());
    assert!(matches!(
        outcome.events.first(),
        Some(Event::TilePlaced { seat, .. }) if *seat == SeatId(0)
    ));
    assert!(outcome
        .events
        .iter()
        .any(|event| matches!(event, Event::TileDrawn { .. })));
}

#[test]
fn turn_order_cycles_through_every_seat() {
    let (state, script) = drive(&seed(), SEATS, config(), 7);
    let seats: Vec<SeatId> = script
        .iter()
        .map(|input| match input {
            Input::Player { seat, .. } => *seat,
            _ => unreachable!("the driver only produces player inputs"),
        })
        .collect();
    assert_eq!(
        seats,
        vec![
            SeatId(0),
            SeatId(1),
            SeatId(2),
            SeatId(0),
            SeatId(1),
            SeatId(2),
            SeatId(0)
        ]
    );
    assert_eq!(state.turn(), SeatId(1));
}

// ---------------------------------------------------------------------------
// Rejection classes — one test per reason, each asserting the exact code
// ---------------------------------------------------------------------------

fn reject_code(state: &mut State, input: Input<Command>, index: u64) -> RuleErrorCode {
    apply_at(state, input, &seed(), index)
        .expect_err("this input must be rejected")
        .code
}

#[test]
fn a_seat_that_is_not_on_turn_is_rejected() {
    let mut state = opening();
    let Input::Player { command, .. } = next_placement(&state).unwrap() else {
        unreachable!()
    };
    assert_eq!(
        reject_code(
            &mut state,
            Input::Player {
                seat: SeatId(2),
                command
            },
            1
        ),
        RuleErrorCode::NotYourTurn
    );
}

#[test]
fn a_seat_that_is_not_in_the_match_is_rejected_before_anything_else() {
    let mut state = opening();
    let Input::Player { command, .. } = next_placement(&state).unwrap() else {
        unreachable!()
    };
    assert_eq!(
        reject_code(
            &mut state,
            Input::Player {
                seat: SeatId(9),
                command
            },
            1
        ),
        RuleErrorCode::NoSuchSeat
    );
}

#[test]
fn placing_on_an_occupied_or_unreachable_square_is_an_illegal_move() {
    let mut state = opening();
    let seat = state.turn();
    for at in [
        Coord::ORIGIN,               // occupied
        Coord::new(40, 40).unwrap(), // touches nothing
    ] {
        assert_eq!(
            reject_code(
                &mut state,
                Input::Player {
                    seat,
                    command: Command::PlaceTile {
                        at,
                        rotation: Rotation::R0
                    },
                },
                1
            ),
            RuleErrorCode::IllegalMove
        );
    }
}

#[test]
fn a_rotation_whose_edges_do_not_match_is_an_illegal_move() {
    let mut state = opening();
    let kind = state.drawn().unwrap();
    let legal = tabula_game_tiles::rules::legal_placements(state.board(), kind);
    let (at, legal_rotations) = legal.first().cloned().expect("something is playable");
    let illegal = Rotation::ALL
        .into_iter()
        .find(|rotation| !legal_rotations.contains(rotation));

    let seat = state.turn();
    if let Some(rotation) = illegal {
        assert_eq!(
            reject_code(
                &mut state,
                Input::Player {
                    seat,
                    command: Command::PlaceTile { at, rotation },
                },
                1
            ),
            RuleErrorCode::IllegalMove
        );
    }
    // And the legal one is accepted from the same square.
    assert!(apply_at(
        &mut state,
        Input::Player {
            seat,
            command: Command::PlaceTile {
                at,
                rotation: legal_rotations[0]
            },
        },
        &seed(),
        2
    )
    .is_ok());
}

#[test]
fn every_rejected_command_leaves_the_state_byte_identical() {
    let (mut state, _) = drive(&seed(), SEATS, config(), 5);
    let before = canonical_encode(&state).unwrap();

    let hostile: Vec<Input<Command>> = vec![
        Input::Player {
            seat: SeatId(200),
            command: Command::PlaceTile {
                at: Coord::ORIGIN,
                rotation: Rotation::R0,
            },
        },
        Input::Player {
            seat: state.turn(),
            command: Command::PlaceTile {
                at: Coord::ORIGIN,
                rotation: Rotation::R270,
            },
        },
        Input::Player {
            seat: (0..SEATS)
                .map(SeatId)
                .find(|seat| *seat != state.turn())
                .unwrap(),
            command: Command::PlaceTile {
                at: Coord::new(1, 1).unwrap(),
                rotation: Rotation::R0,
            },
        },
        Input::Admin(AdminInput::ForceEnd {
            outcome: tabula_core::MatchOutcome::new_for_seats(
                tabula_core::OutcomeKind::Draw,
                smallvec_standings(),
                "forced".into(),
                state.seats(),
            )
            .unwrap(),
        }),
    ];

    for (offset, input) in hostile.into_iter().enumerate() {
        assert!(apply_at(&mut state, input, &seed(), 100 + offset as u64).is_err());
        assert_eq!(
            canonical_encode(&state).unwrap(),
            before,
            "a rejected input must be a total no-op (contract R2)"
        );
    }
}

fn smallvec_standings() -> smallvec::SmallVec<[tabula_core::Standing; 8]> {
    (0..SEATS)
        .map(|index| tabula_core::Standing {
            seat: SeatId(index),
            rank: 0,
            score: 0,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The `Hints` contract — the cross-crate evidence that `CommandHint` is usable
// ---------------------------------------------------------------------------

/// This test is the reason `CommandHint` gained a constructor: before that,
/// `LegalCommands::Hints` could not be built by any crate but
/// `tabula-game-api`, so no game could return it.
///
/// The oracle is `apply` itself: every `(square, rotation)` a hint advertises
/// must be accepted, and every legal one must be advertised — checked against
/// an independent sweep of the frontier, not against `legal_placements`.
#[test]
fn legal_commands_hints_decode_to_exactly_the_accepted_placements() {
    let (state, _) = drive(&seed(), SEATS, config(), 6);
    let kind = state.drawn().expect("a playing match holds a drawn tile");

    let LegalCommands::Hints(hints) = TilesRules::legal_commands(&state, state.turn()) else {
        panic!("Tiles must return Hints during placement, not an enumeration");
    };
    assert!(!hints.is_empty());

    let mut advertised: Vec<(Coord, Rotation)> = Vec::new();
    for hint in &hints {
        assert_eq!(hint.kind(), HINT_PLACE_TILE);
        let payload: PlaceTileHint =
            canonical_decode(hint.data()).expect("a hint payload is canonically encoded");
        assert!(!payload.rotations.is_empty());
        for rotation in payload.rotations {
            advertised.push((payload.at, rotation));
        }
    }
    advertised.sort_unstable();

    // Independent sweep: every free square anywhere near the board, at every
    // rotation, accepted by `apply` on a clone.
    let mut accepted: Vec<(Coord, Rotation)> = Vec::new();
    let mut squares: Vec<Coord> = state
        .board()
        .iter()
        .flat_map(|(coord, _)| coord.orthogonal().map(|(_, n)| n))
        .filter(|coord| !state.board().contains(*coord))
        .collect();
    squares.sort_unstable();
    squares.dedup();
    for at in squares {
        for rotation in Rotation::ALL {
            let mut probe = state.clone();
            let outcome = apply_at(
                &mut probe,
                Input::Player {
                    seat: state.turn(),
                    command: Command::PlaceTile { at, rotation },
                },
                &seed(),
                7,
            );
            if outcome.is_ok() {
                accepted.push((at, rotation));
            }
        }
    }
    accepted.sort_unstable();

    assert_eq!(
        advertised,
        accepted,
        "hints must advertise exactly the placements apply() accepts for {}",
        kind.def().name
    );
}

#[test]
fn a_seat_that_cannot_act_gets_no_hints() {
    let state = opening();
    let waiting = (0..SEATS)
        .map(SeatId)
        .find(|seat| *seat != state.turn())
        .unwrap();
    assert!(matches!(
        TilesRules::legal_commands(&state, waiting),
        LegalCommands::None
    ));
}

// ---------------------------------------------------------------------------
// The deadline, pausing, and terminal states
// ---------------------------------------------------------------------------

#[test]
fn the_turn_deadline_resolves_the_turn_deterministically() {
    let cfg = timed_config(60_000);
    let mut state = create(&seed(), SEATS, cfg);
    let expected = next_placement(&state).expect("a placement is available");
    let seat = state.turn();

    let outcome = apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
        &seed(),
        1,
    )
    .expect("the deadline is handled, not rejected");

    assert!(outcome
        .events
        .iter()
        .any(|event| matches!(event, Event::TurnAutoResolved { seat: s } if *s == seat)));
    // It played exactly the placement a first-legal-in-canonical-order player
    // would have played — reproducible by any observer.
    let Input::Player {
        command: Command::PlaceTile { at, rotation },
        ..
    } = expected
    else {
        unreachable!()
    };
    assert_eq!(
        state.board().get(at).map(|tile| tile.rotation),
        Some(rotation)
    );
    assert_eq!(state.turn(), SeatId(1));
}

#[test]
fn a_timer_this_version_never_set_is_ignored_rather_than_rejected() {
    let mut state = opening();
    let before = canonical_encode(&state).unwrap();
    let outcome = apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(99),
        },
        &seed(),
        1,
    )
    .expect("a stale timer is not an error");
    assert!(outcome.events.is_empty());
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn pausing_stops_play_and_resuming_restores_it() {
    let mut state = opening();
    let placement = next_placement(&state).unwrap();

    apply_at(&mut state, Input::Admin(AdminInput::Pause), &seed(), 1).expect("pausable = true");
    assert!(state.paused());
    assert_eq!(
        reject_code(&mut state, placement.clone(), 2),
        RuleErrorCode::WrongPhase
    );
    assert!(matches!(
        TilesRules::legal_commands(&state, state.turn()),
        LegalCommands::None
    ));

    // Pausing twice is a no-op rather than an error: effects must be safe to
    // re-run (doc 03 §7.1) and so must the input that provokes them.
    let again = apply_at(&mut state, Input::Admin(AdminInput::Pause), &seed(), 3).unwrap();
    assert!(again.events.is_empty());

    apply_at(&mut state, Input::Admin(AdminInput::Resume), &seed(), 4).expect("resumable");
    assert!(!state.paused());
    assert!(apply_at(&mut state, placement, &seed(), 5).is_ok());
}

#[test]
fn a_deadline_that_fires_while_paused_changes_nothing() {
    let mut state = create(&seed(), SEATS, timed_config(60_000));
    apply_at(&mut state, Input::Admin(AdminInput::Pause), &seed(), 1).unwrap();
    let before = canonical_encode(&state).unwrap();
    apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
        &seed(),
        2,
    )
    .unwrap();
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn an_operator_cancel_ends_the_match_and_later_input_is_refused() {
    let mut state = opening();
    let placement = next_placement(&state).unwrap();
    apply_at(
        &mut state,
        Input::Admin(AdminInput::Cancel {
            reason: AbortReason::OperatorCancelled,
        }),
        &seed(),
        1,
    )
    .expect("cancel is honoured");
    assert_eq!(state.status(), Status::Aborted);
    assert_eq!(state.drawn(), None);
    assert_eq!(
        reject_code(&mut state, placement, 2),
        RuleErrorCode::MatchOver
    );
}

#[test]
fn the_match_ends_when_the_bag_runs_dry_with_every_tile_accounted_for() {
    let (state, script) = drive(&seed(), SEATS, config(), 256);
    assert_eq!(state.status(), Status::Ended);
    assert_eq!(state.bag_remaining(), 0);
    assert_eq!(state.drawn(), None);
    assert_eq!(
        state.board().len() - 1 + state.discarded().len(),
        tabula_game_tiles::rules::BAG_SIZE,
        "every tile that left the bag is either on the board or in the discards"
    );
    assert_eq!(script.len(), state.board().len() - 1);
    // The validator agrees the terminal position is well formed.
    assert_eq!(state.check_invariants(), Ok(()));
}

// ---------------------------------------------------------------------------
// Randomness is consumed only at `create`
// ---------------------------------------------------------------------------

/// A differential over the *randomness* axis: replay the same script with every
/// `apply` given an RNG derived from a completely different match seed. If any
/// rule read `ctx.rng`, the two runs would diverge.
///
/// This is the property that makes contract R8 free for Tiles rather than
/// something to be maintained: a rejected input cannot disturb randomness that
/// no accepted input consumes either.
#[test]
fn no_input_after_create_consumes_randomness() {
    let script = drive(&seed(), SEATS, config(), 20).1;

    let mut with_own_seed = create(&seed(), SEATS, config());
    let mut with_foreign_seed = create(&seed(), SEATS, config());
    let foreign = MatchSeed::from_bytes(ALT_SEED);

    for (step, input) in script.into_iter().enumerate() {
        let index = step as u64 + 1;
        apply_at(&mut with_own_seed, input.clone(), &seed(), index).unwrap();
        apply_at(&mut with_foreign_seed, input, &foreign, index).unwrap();
    }

    assert_eq!(
        canonical_encode(&with_own_seed).unwrap(),
        canonical_encode(&with_foreign_seed).unwrap(),
        "apply() must not read ctx.rng: swapping the RNG changed the result"
    );
}

/// The control for the test above: the *create* seed does matter, so the two
/// oracles are not both trivially satisfied by a game that ignores randomness
/// entirely.
#[test]
fn the_create_seed_does_change_the_shuffle() {
    let a = create(&seed(), SEATS, config());
    let b = create(&MatchSeed::from_bytes(ALT_SEED), SEATS, config());
    assert_ne!(
        canonical_encode(&a).unwrap(),
        canonical_encode(&b).unwrap(),
        "two different match seeds must produce two different bags"
    );
}
