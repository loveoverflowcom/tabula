//! Projection control tests for a **perfect-information** game.
//!
//! Chess has no hidden information (`capabilities.hidden_information ==
//! false`), so — like tic-tac-toe's `tests/projection_control.rs` — it cannot
//! exercise the noninterference property this PR adds. See
//! `crates/tabula-testkit/tests/projection_noninterference.rs` for the real
//! hidden-information exercise. What this file checks:
//!
//! ```text
//! P1  projection determinism: same state + same viewer -> same projection
//! P3  a real, public state change (a move) remains observable to every
//!     viewer kind, including a spectator
//! ```

use smallvec::smallvec;
use tabula_core::{
    MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::Input;
use tabula_game_chess::{ChessRules, Command, Config};
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

fn move_to(seat: u8, from: u8, to: u8) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::Move {
            from,
            to,
            promotion: None,
        },
    }
}

fn scenario(inputs: Vec<Input<Command>>) -> Scenario<ChessRules> {
    Scenario {
        config: Config::default(),
        roster: roster(),
        seed: MatchSeed::from_bytes([11u8; 32]),
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
    let state = run_typed::<ChessRules>(&scenario(vec![move_to(0, 13, 21), move_to(1, 52, 36)]))
        .expect("fixture script is legal");

    for viewer in VIEWERS {
        assert_projection_noninterference::<ChessRules>(
            "chess determinism",
            &state,
            &state,
            viewer,
        );
    }
}

#[test]
fn a_move_changes_the_board_which_is_public_to_every_viewer() {
    let before = run_typed::<ChessRules>(&scenario(Vec::new())).expect("create succeeds");
    let after =
        run_typed::<ChessRules>(&scenario(vec![move_to(0, 13, 21)])).expect("e2-e4 is legal");

    for viewer in VIEWERS {
        assert_projection_differs::<ChessRules>("a played move is public", &before, &after, viewer);
    }
}
