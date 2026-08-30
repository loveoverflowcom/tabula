//! The deterministic-kernel contract, exercised against the reference game.
//! (doc 02 §11.1, ADR-026)
//!
//! These are the invariant tests, not gameplay tests. Tic-tac-toe is the fixture
//! because it has **no randomness at all**, which makes it the right control
//! group: any divergence here is a bug in the kernel or the harness, never in a
//! shuffle. (doc 02 §10)
//!
//! When `conformance!` is implemented (doc 02 §11.1) it emits these same
//! assertions for every game. This file is what proves the harness underneath it
//! actually works, and that the contract is usable from a game author's seat.

use smallvec::smallvec;
use tabula_core::{MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId};
use tabula_game_api::{AdminInput, Input};
use tabula_game_tictactoe::{Command, Config, TicTacToeRules};
use tabula_testkit::determinism::{
    assert_deterministic, assert_deterministic_across_snapshot,
    assert_rejection_does_not_disturb_rng, assert_state_roundtrip, assert_transactional_on_error,
    assert_version_monotonic, run, Scenario,
};

const SEED: MatchSeed = MatchSeed::from_bytes([42u8; 32]);

fn roster() -> SeatRoster {
    SeatRoster {
        seats: smallvec![
            SeatEntry {
                seat: SeatId(0),
                occupant: Occupant::Human(UserId(1)),
                team: None,
            },
            SeatEntry {
                seat: SeatId(1),
                occupant: Occupant::Human(UserId(2)),
                team: None,
            },
        ],
    }
}

fn place(seat: u8, cell: u8) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::Place { cell },
    }
}

fn scenario(inputs: Vec<Input<Command>>) -> Scenario<TicTacToeRules> {
    Scenario {
        config: Config {
            move_timeout_ms: 30_000,
        },
        roster: roster(),
        seed: SEED.clone(),
        inputs,
    }
}

/// A full game: X takes the top row, O blocks nothing useful.
fn decisive_game() -> Scenario<TicTacToeRules> {
    scenario(vec![
        place(0, 0),
        place(1, 3),
        place(0, 1),
        place(1, 4),
        place(0, 2), // X wins the top row
    ])
}

/// Legal play interleaved with rejections of every kind the game can produce.
fn hostile_game() -> Scenario<TicTacToeRules> {
    scenario(vec![
        place(1, 0),                     // NotYourTurn — seat 0 opens
        place(0, 0),                     // ok
        place(0, 4),                     // NotYourTurn — still seat 0 asking
        place(1, 0),                     // IllegalMove — occupied
        place(1, 99),                    // IllegalMove — off the board
        place(9, 5),                     // NoSuchSeat — outside the roster
        place(1, 4),                     // ok
        Input::Admin(AdminInput::Pause), // Unsupported — pausable = false
        place(0, 8),                     // ok
    ])
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

#[test]
fn same_config_and_seed_give_the_same_initial_state() {
    let empty = scenario(vec![]);
    let a = run::<TicTacToeRules>(&empty).unwrap();
    let b = run::<TicTacToeRules>(&empty).unwrap();

    assert_eq!(a.final_state, b.final_state, "initial state bytes differed");
    assert_eq!(a.final_hash, b.final_hash, "initial state hash differed");
    assert_eq!(
        a.effects_encoded, b.effects_encoded,
        "create effects differed"
    );
}

// ---------------------------------------------------------------------------
// Transition determinism (I-2) and event ordering (R7)
// ---------------------------------------------------------------------------

#[test]
fn decisive_game_is_deterministic() {
    assert_deterministic::<TicTacToeRules>(&decisive_game());
}

#[test]
fn hostile_game_is_deterministic() {
    assert_deterministic::<TicTacToeRules>(&hostile_game());
}

#[test]
fn event_order_is_observable_and_stable() {
    // The winning move emits Placed THEN Ended. That order is part of the
    // contract: the log stores it verbatim and replay reproduces it.
    let trace = run::<TicTacToeRules>(&decisive_game()).unwrap();
    let events = &trace.events_encoded;

    assert_eq!(events.len(), 6, "5 placements + 1 ended");
    assert_ne!(
        events[4], events[5],
        "the last two events of the winning move must be distinct (Placed, Ended)"
    );

    let again = run::<TicTacToeRules>(&decisive_game()).unwrap();
    assert_eq!(*events, again.events_encoded, "event order is not stable");
}

// ---------------------------------------------------------------------------
// Rejection semantics (R2, R8, I-7)
// ---------------------------------------------------------------------------

#[test]
fn rejected_inputs_leave_state_byte_identical() {
    assert_transactional_on_error::<TicTacToeRules>(&hostile_game());
}

#[test]
fn every_rejection_kind_is_exercised() {
    // A transactionality test that never rejects anything asserts nothing. This
    // guards the guard: all four rejection codes tic-tac-toe can produce must
    // actually occur in the hostile scenario.
    use tabula_core::RuleErrorCode::{IllegalMove, NoSuchSeat, NotYourTurn, Unsupported};

    let trace = run::<TicTacToeRules>(&hostile_game()).unwrap();
    let codes: Vec<_> = trace.rejections.iter().map(|(_, c)| *c).collect();

    assert_eq!(
        codes,
        vec![
            NotYourTurn, // 1: seat 1 cannot open
            NotYourTurn, // 3: seat 0 already moved
            IllegalMove, // 4: cell occupied
            IllegalMove, // 5: cell off the board
            NoSuchSeat,  // 6: seat outside the roster
            Unsupported, // 8: pausable = false
        ],
        "rejection codes or their order changed"
    );
}

#[test]
fn state_version_tracks_accepted_inputs_only() {
    assert_version_monotonic::<TicTacToeRules>(&hostile_game());

    let trace = run::<TicTacToeRules>(&hostile_game()).unwrap();
    assert_eq!(
        trace.final_version.0, 3,
        "3 of the 9 hostile inputs were legal"
    );
}

#[test]
fn a_rejection_does_not_disturb_the_next_input() {
    // R8: the RNG stream is a pure function of (seed, index), so a rejected
    // apply — however much it drew — cannot shift what comes next.
    let base = scenario(vec![place(0, 0), place(1, 4)]);
    assert_rejection_does_not_disturb_rng::<TicTacToeRules>(
        &base,
        &place(0, 0), // rejected: cell already occupied
        &place(0, 8), // probe: legal, seat 0's turn
    );
}

#[test]
fn hostile_input_never_panics() {
    // R3. Out-of-range seats and cells, commands in a finished match, timers that
    // were never set, and admin inputs the game does not support.
    let mut inputs = vec![place(0, 0), place(1, 1)];
    for seat in [0u8, 1, 2, 99, 255] {
        for cell in [0u8, 8, 9, 200, 255] {
            inputs.push(place(seat, cell));
        }
        inputs.push(Input::Player {
            seat: SeatId(seat),
            command: Command::Resign,
        });
    }
    inputs.push(Input::Timer {
        timer: tabula_core::TimerId(9999),
    });
    inputs.push(Input::Admin(AdminInput::Resume));

    // The assertion is that this returns at all.
    let trace = run::<TicTacToeRules>(&scenario(inputs)).unwrap();
    assert!(trace.final_version.0 >= 2);
}

// ---------------------------------------------------------------------------
// Serialization round-trip and snapshots (I-8)
// ---------------------------------------------------------------------------

#[test]
fn state_survives_a_canonical_round_trip() {
    assert_state_roundtrip::<TicTacToeRules>(&decisive_game());
    assert_state_roundtrip::<TicTacToeRules>(&hostile_game());
}

#[test]
fn snapshot_and_resume_matches_an_uninterrupted_run() {
    for at in 0..5 {
        assert_deterministic_across_snapshot::<TicTacToeRules>(&decisive_game(), at);
        assert_deterministic_across_snapshot::<TicTacToeRules>(&hostile_game(), at);
    }
}

// ---------------------------------------------------------------------------
// Replay (I-8)
// ---------------------------------------------------------------------------

#[test]
fn repeated_independent_replays_agree() {
    // A fixed command sequence with a fixed seed must produce the same final
    // result over repeated independent executions — the property every stored
    // replay depends on.
    let expected = run::<TicTacToeRules>(&decisive_game()).unwrap();
    for run_no in 0..32 {
        let actual = run::<TicTacToeRules>(&decisive_game()).unwrap();
        assert_eq!(
            expected, actual,
            "replay {run_no} diverged from the first execution"
        );
    }
}

#[test]
fn the_state_hash_is_not_a_placeholder() {
    // Guards against a regression to the constant-zero hash this contract
    // replaced: distinct positions must produce distinct hashes, and no hash may
    // be all-zero.
    let opening = run::<TicTacToeRules>(&scenario(vec![place(0, 0)])).unwrap();
    let different = run::<TicTacToeRules>(&scenario(vec![place(0, 4)])).unwrap();

    assert_ne!(opening.final_hash, different.final_hash);
    assert_ne!(opening.final_hash.0, [0u8; 32]);

    // Every checkpoint in a real game is distinct — the board never repeats.
    let game = run::<TicTacToeRules>(&decisive_game()).unwrap();
    let mut seen: Vec<_> = game.checkpoints.iter().map(|(_, h)| h.0).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(before, seen.len(), "two distinct positions hashed alike");
}
