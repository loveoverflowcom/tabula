//! Projection control tests for a **perfect-information** game.
//!
//! Tic-tac-toe has no hidden information (`capabilities.hidden_information ==
//! false`), so it cannot exercise the noninterference property this PR adds
//! (there is nothing secret to scramble) — see
//! `crates/tabula-testkit/tests/projection_noninterference.rs` for the real
//! hidden-information exercise. What tic-tac-toe *can* honestly demonstrate,
//! and what this file checks:
//!
//! ```text
//! P1  projection determinism: same state + same viewer -> same projection
//! P3  a real, public state change remains observable (the board and the
//!     turn are both public here, so every viewer kind must see it)
//! ```
//!
//! This is **not** a secrecy proof for tic-tac-toe — there is no secrecy to
//! prove — only the determinism half of the ledger, plus a sanity control
//! that the harness is not vacuously passing.

use smallvec::smallvec;
use tabula_core::{
    MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::Input;
use tabula_game_tictactoe::{Command, Config, TicTacToeRules};
use tabula_testkit::determinism::{run_typed, Scenario};
use tabula_testkit::{assert_projection_differs, assert_projection_noninterference};

fn roster() -> SeatRoster {
    SeatRoster::new(smallvec![
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
    ])
    .expect("fixture seats are unique")
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
        seed: MatchSeed::from_bytes([7u8; 32]),
        inputs,
    }
}

const VIEWERS: [Viewer; 3] = [
    Viewer::Seat(SeatId(0)),
    Viewer::Seat(SeatId(1)),
    Viewer::Spectator(SpectatorTier::Live),
];

#[test]
fn projection_is_deterministic_for_a_fixed_reachable_state() {
    let state = run_typed::<TicTacToeRules>(&scenario(vec![place(0, 0), place(1, 4)]))
        .expect("fixture script is legal");

    for viewer in VIEWERS {
        assert_projection_noninterference::<TicTacToeRules>(
            "tictactoe determinism",
            &state,
            &state,
            viewer,
        );
    }
}

#[test]
fn a_move_changes_the_board_which_is_public_to_every_viewer() {
    // Perfect information: the board and whose turn it is are public. This is
    // the positive control (P3) for a real game — every viewer kind must be
    // able to tell these two states apart, since nothing here is secret.
    let before = run_typed::<TicTacToeRules>(&scenario(Vec::new())).expect("create succeeds");
    let after = run_typed::<TicTacToeRules>(&scenario(vec![place(0, 4)])).expect("center is legal");

    for viewer in VIEWERS {
        assert_projection_differs::<TicTacToeRules>(
            "a placed mark is public",
            &before,
            &after,
            viewer,
        );
    }
}
