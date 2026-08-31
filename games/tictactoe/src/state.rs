//! `State`, `Command`, `Event`, `View`, `ViewEvent`, `Config`. (doc 02 §10.2)

use serde::{Deserialize, Serialize};
use tabula_core::{MatchOutcome, SeatId};

/// Canonical, full-information state. Server-only, never on the wire (I-5).
///
/// `moves` is derived from `board`, and the two actual roster seats travel with
/// the state. Those choices remove two independent ways to forge a contradictory
/// position: stale move counts and a hidden `SeatId(0/1)` assumption.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "RawState")]
pub struct State {
    pub(crate) board: [Option<Mark>; 9],
    pub(crate) seats: [SeatId; 2],
    pub(crate) turn: SeatId,
    pub(crate) status: Status,
    pub(crate) move_timeout_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct RawState {
    board: [Option<Mark>; 9],
    seats: [SeatId; 2],
    turn: SeatId,
    status: Status,
    move_timeout_ms: u64,
}

/// A rejected snapshot is corrupt rather than a legal alternative game state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("tic-tac-toe seats must be distinct")]
    DuplicateSeats,
    #[error("tic-tac-toe turn must be one of the roster seats")]
    TurnOutsideRoster,
    #[error("tic-tac-toe board has an impossible mark count")]
    ImpossibleMarkCount,
    #[error("tic-tac-toe board has simultaneous winning lines")]
    SimultaneousWinners,
    #[error("tic-tac-toe playing state is terminal or has the wrong turn")]
    InvalidPlaying,
    #[error("tic-tac-toe won state has no valid winning line")]
    InvalidWin,
    #[error("tic-tac-toe terminal state records the wrong final mover")]
    InvalidTerminalTurn,
    #[error("tic-tac-toe forfeited state must be a non-terminal position with the resigning seat on turn")]
    InvalidForfeit,
    #[error("tic-tac-toe aborted state must be a non-terminal position")]
    InvalidAbort,
    #[error("tic-tac-toe draw state is not a full non-winning board")]
    InvalidDraw,
    #[error("tic-tac-toe move timeout must be at least five seconds")]
    InvalidMoveTimeout,
}

impl State {
    pub(crate) fn new(seats: [SeatId; 2], move_timeout_ms: u64) -> Result<Self, StateError> {
        Self::from_parts([None; 9], seats, seats[0], Status::Playing, move_timeout_ms)
    }

    fn from_parts(
        board: [Option<Mark>; 9],
        seats: [SeatId; 2],
        turn: SeatId,
        status: Status,
        move_timeout_ms: u64,
    ) -> Result<Self, StateError> {
        if seats[0] == seats[1] {
            return Err(StateError::DuplicateSeats);
        }
        if !seats.contains(&turn) {
            return Err(StateError::TurnOutsideRoster);
        }
        if move_timeout_ms < 5_000 {
            return Err(StateError::InvalidMoveTimeout);
        }

        let x_count = board.iter().filter(|cell| **cell == Some(Mark::X)).count();
        let o_count = board.iter().filter(|cell| **cell == Some(Mark::O)).count();
        if o_count > x_count || x_count > o_count + 1 {
            return Err(StateError::ImpossibleMarkCount);
        }

        let x_won = has_line(&board, Mark::X);
        let o_won = has_line(&board, Mark::O);
        if x_won && o_won {
            return Err(StateError::SimultaneousWinners);
        }
        let expected_playing_turn = if x_count == o_count {
            seats[0]
        } else {
            seats[1]
        };
        let terminal_mover = if x_count == o_count {
            seats[1]
        } else {
            seats[0]
        };

        match status {
            Status::Playing => {
                if x_won
                    || o_won
                    || x_count + o_count == board.len()
                    || turn != expected_playing_turn
                {
                    return Err(StateError::InvalidPlaying);
                }
            }
            Status::Won(winner) => {
                let valid_winner = match winner {
                    seat if seat == seats[0] => x_won && x_count == o_count + 1,
                    seat if seat == seats[1] => o_won && x_count == o_count,
                    _ => false,
                };
                if !valid_winner {
                    return Err(StateError::InvalidWin);
                }
                if turn != winner || turn != terminal_mover {
                    return Err(StateError::InvalidTerminalTurn);
                }
            }
            Status::Forfeited(winner) => {
                if !seats.contains(&winner)
                    || x_won
                    || o_won
                    || x_count + o_count == board.len()
                    || turn != expected_playing_turn
                    || winner == turn
                {
                    return Err(StateError::InvalidForfeit);
                }
            }
            Status::Drawn => {
                if x_won || o_won || x_count + o_count != board.len() {
                    return Err(StateError::InvalidDraw);
                }
                if turn != terminal_mover {
                    return Err(StateError::InvalidTerminalTurn);
                }
            }
            Status::Aborted => {
                if x_won
                    || o_won
                    || x_count + o_count == board.len()
                    || turn != expected_playing_turn
                {
                    return Err(StateError::InvalidAbort);
                }
            }
        }

        Ok(Self {
            board,
            seats,
            turn,
            status,
            move_timeout_ms,
        })
    }

    #[must_use]
    pub fn board(&self) -> &[Option<Mark>; 9] {
        &self.board
    }

    #[must_use]
    pub const fn turn(&self) -> SeatId {
        self.turn
    }

    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }

    #[must_use]
    pub const fn seats(&self) -> [SeatId; 2] {
        self.seats
    }

    #[must_use]
    pub(crate) fn moves(&self) -> u8 {
        self.board
            .iter()
            .fold(0, |count, cell| count + u8::from(cell.is_some()))
    }

    #[must_use]
    pub(crate) fn is_seated(&self, seat: SeatId) -> bool {
        self.seats.contains(&seat)
    }

    #[must_use]
    pub(crate) fn other(&self, seat: SeatId) -> SeatId {
        debug_assert!(self.is_seated(seat));
        if seat == self.seats[0] {
            self.seats[1]
        } else {
            self.seats[0]
        }
    }

    #[must_use]
    pub(crate) fn mark_for(&self, seat: SeatId) -> Mark {
        debug_assert!(self.is_seated(seat));
        if seat == self.seats[0] {
            Mark::X
        } else {
            Mark::O
        }
    }
}

impl TryFrom<RawState> for State {
    type Error = StateError;

    fn try_from(raw: RawState) -> Result<Self, Self::Error> {
        Self::from_parts(
            raw.board,
            raw.seats,
            raw.turn,
            raw.status,
            raw.move_timeout_ms,
        )
    }
}

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

fn has_line(board: &[Option<Mark>; 9], mark: Mark) -> bool {
    LINES.iter().any(|[a, b, c]| {
        board[*a] == Some(mark) && board[*b] == Some(mark) && board[*c] == Some(mark)
    })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    X,
    O,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Playing,
    Won(SeatId),
    Forfeited(SeatId),
    Drawn,
    Aborted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Place { cell: u8 },
    Resign,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Placed { seat: SeatId, cell: u8, mark: Mark },
    Ended { outcome: MatchOutcome },
}

#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub board: [Option<Mark>; 9],
    pub turn: SeatId,
    pub status: Status,
    pub you: Option<SeatId>,
}

pub type ViewEvent = Event;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Per-move deadline. `0` means the documented default; every nonzero
    /// value must meet [`super::rules::MIN_MOVE_TIMEOUT`].
    pub move_timeout_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{canonical_decode, canonical_encode};

    #[test]
    fn state_deserialization_rejects_impossible_snapshots() {
        let invalid_win = RawState {
            board: [None; 9],
            seats: [SeatId(7), SeatId(42)],
            turn: SeatId(7),
            status: Status::Won(SeatId(7)),
            move_timeout_ms: 5_000,
        };
        let wrong_turn = RawState {
            board: [None; 9],
            seats: [SeatId(7), SeatId(42)],
            turn: SeatId(42),
            status: Status::Playing,
            move_timeout_ms: 5_000,
        };
        let duplicate_seats = RawState {
            board: [None; 9],
            seats: [SeatId(7), SeatId(7)],
            turn: SeatId(7),
            status: Status::Playing,
            move_timeout_ms: 5_000,
        };
        for raw in [invalid_win, wrong_turn, duplicate_seats] {
            let bytes = canonical_encode(&raw).unwrap();
            assert!(canonical_decode::<State>(&bytes).is_err());
        }
    }

    #[test]
    fn terminal_snapshot_validation_rejects_unreachable_turns_and_wins() {
        let seats = [SeatId(7), SeatId(42)];
        let x_wins = [
            Some(Mark::X),
            Some(Mark::X),
            Some(Mark::X),
            Some(Mark::O),
            Some(Mark::O),
            None,
            None,
            None,
            None,
        ];
        let valid_draw = [
            Some(Mark::X),
            Some(Mark::O),
            Some(Mark::X),
            Some(Mark::X),
            Some(Mark::O),
            Some(Mark::O),
            Some(Mark::O),
            Some(Mark::X),
            Some(Mark::X),
        ];
        let invalid = [
            RawState {
                board: x_wins,
                seats,
                turn: SeatId(42),
                status: Status::Won(SeatId(7)),
                move_timeout_ms: 5_000,
            },
            RawState {
                board: x_wins,
                seats,
                turn: SeatId(42),
                status: Status::Forfeited(SeatId(7)),
                move_timeout_ms: 5_000,
            },
            RawState {
                board: x_wins,
                seats,
                turn: SeatId(42),
                status: Status::Aborted,
                move_timeout_ms: 5_000,
            },
            RawState {
                board: valid_draw,
                seats,
                turn: SeatId(42),
                status: Status::Drawn,
                move_timeout_ms: 5_000,
            },
        ];

        for raw in invalid {
            let bytes = canonical_encode(&raw).unwrap();
            assert!(canonical_decode::<State>(&bytes).is_err());
        }
    }
}
