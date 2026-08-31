//! The generic game-contract proof, supplied with a concrete chess fixture.

use smallvec::smallvec;
use tabula_core::{MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId};
use tabula_game_api::Input;
use tabula_game_chess::{ChessModule, Command, Config};
use tabula_testkit::{GameTestFixture, InvalidCommandScenario, TerminalScenario};

struct ChessFixture;

fn roster() -> SeatRoster {
    SeatRoster::new(smallvec![
        SeatEntry {
            seat: SeatId(0),
            occupant: Occupant::Human(UserId(1)),
            team: None
        },
        SeatEntry {
            seat: SeatId(1),
            occupant: Occupant::Human(UserId(2)),
            team: None
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

/// Fool's mate: a real terminal chess sequence with no test-only command.
fn fools_mate() -> Vec<Input<Command>> {
    vec![
        move_to(0, 13, 21),
        move_to(1, 52, 36),
        move_to(0, 14, 30),
        move_to(1, 59, 31),
    ]
}

impl GameTestFixture for ChessFixture {
    type Module = ChessModule;

    fn config() -> Config {
        Config::default()
    }
    fn roster() -> SeatRoster {
        roster()
    }
    fn seed() -> MatchSeed {
        MatchSeed::from_bytes([19; 32])
    }
    fn deterministic_script() -> Vec<Input<Command>> {
        fools_mate()
    }
    fn invalid_command() -> Option<InvalidCommandScenario<Command>> {
        Some(InvalidCommandScenario {
            setup: vec![move_to(0, 13, 21)],
            invalid: move_to(0, 12, 28),
            probe: move_to(1, 52, 36),
        })
    }
    fn terminal() -> Option<TerminalScenario<Command>> {
        Some(TerminalScenario {
            script: fools_mate(),
            post_terminal: move_to(0, 4, 12),
        })
    }
}

tabula_testkit::conformance!(ChessFixture);
