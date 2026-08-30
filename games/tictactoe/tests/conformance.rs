//! The mandatory conformance suite. (doc 02 §11)
//!
//! One fixture. The full suite. **A game may not be registered until it
//! passes.**
//!
//! If you are reading this in a new game crate, this file is the template:
//! implement [`GameTestFixture`] against your own `GameModule`, call
//! [`tabula_testkit::conformance!`], and stop. Game-specific rule tests go in
//! a sibling file (`tests/rules.rs`) or `tests/determinism.rs`, so a
//! conformance failure is never confused with a gameplay-logic failure.

use smallvec::smallvec;
use tabula_core::{MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId};
use tabula_game_api::Input;
use tabula_game_tictactoe::{Command, Config, TicTacToeModule};
use tabula_testkit::{GameTestFixture, InvalidCommandScenario, TerminalScenario};

struct TicTacToeFixture;

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

/// X takes the top row; O blocks nothing useful. Ends in a decisive win, so
/// this single script doubles as the terminal scenario below.
fn winning_script() -> Vec<Input<Command>> {
    vec![
        place(0, 0),
        place(1, 3),
        place(0, 1),
        place(1, 4),
        place(0, 2), // X completes the top row
    ]
}

impl GameTestFixture for TicTacToeFixture {
    type Module = TicTacToeModule;

    fn config() -> Config {
        Config {
            move_timeout_ms: 30_000,
        }
    }

    fn roster() -> SeatRoster {
        roster()
    }

    fn seed() -> MatchSeed {
        MatchSeed::from_bytes([42u8; 32])
    }

    fn deterministic_script() -> Vec<Input<Command>> {
        winning_script()
    }

    fn invalid_command() -> Option<InvalidCommandScenario<Command>> {
        Some(InvalidCommandScenario {
            setup: vec![place(0, 0)],
            invalid: place(0, 4), // NotYourTurn: seat 0 acting twice in a row
            probe: place(1, 4),   // legal: it is seat 1's turn
        })
    }

    fn terminal() -> Option<TerminalScenario<Command>> {
        Some(TerminalScenario {
            script: winning_script(),
            post_terminal: place(1, 5), // the match already ended
        })
    }

    // No `randomness()`: tic-tac-toe never draws from `ctx.rng` at all, which
    // is exactly why it is the reference fixture (doc 02 §10) — any
    // divergence in the suite is a bug in the kernel or the harness, never
    // in a shuffle.
}

tabula_testkit::conformance!(TicTacToeFixture);
