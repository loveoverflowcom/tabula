//! Bot policies. `#[cfg(feature = "bots")]` — server-side and tests only.
//!
//! **A bot sees `View`, never `State`** (doc 00 §6.5). That is not a courtesy;
//! it is the structural guarantee that a bot cannot become a cheating oracle,
//! and it doubles as a proof that the projection contains enough information to
//! play the game.
//!
//! Tic-tac-toe is solved, so `Hard` can be perfect. Do not read that as a
//! template — chess gets a shallow alpha-beta as an *optional crate feature*,
//! and no game gets ML in the MVP (doc 01 §8).

use tabula_core::{BotLevel, DetRng, Millis, SeatId};
use tabula_game_api::GameBot;

use crate::{rules::TicTacToeRules, state::Command, state::Mark, state::Status, state::View};

#[derive(Debug)]
pub struct Perfect {
    level: BotLevel,
}

impl Perfect {
    #[must_use]
    pub fn new(level: BotLevel) -> Self {
        Self { level }
    }
}

impl GameBot<TicTacToeRules> for Perfect {
    fn level(&self) -> BotLevel {
        self.level
    }

    fn choose(&self, view: &View, seat: SeatId, rng: &mut DetRng) -> Option<Command> {
        if view.you != Some(seat) || view.turn != seat || !matches!(view.status, Status::Playing) {
            return None;
        }

        let mut cells: Vec<u8> = view
            .board
            .iter()
            .enumerate()
            .filter(|(_, mark)| mark.is_none())
            .filter_map(|(cell, _)| u8::try_from(cell).ok())
            .collect();
        if cells.is_empty() {
            return None;
        }

        let mark = mark_for_turn(view);
        match self.level {
            BotLevel::Trivial => {}
            BotLevel::Easy | BotLevel::Medium | BotLevel::Hard => {
                if let Some(cell) = completion(view.board, mark) {
                    cells = vec![cell];
                } else if let Some(cell) = completion(view.board, other(mark)) {
                    cells = vec![cell];
                }
            }
        }

        if matches!(self.level, BotLevel::Medium | BotLevel::Hard) && cells.len() > 1 {
            const PREFERRED: [u8; 9] = [4, 0, 2, 6, 8, 1, 3, 5, 7];
            if let Some(cell) = PREFERRED.into_iter().find(|cell| cells.contains(cell)) {
                cells = vec![cell];
            }
        }

        let index = usize::try_from(rng.below(u32::try_from(cells.len()).ok()?)).ok()?;
        cells
            .get(index)
            .copied()
            .map(|cell| Command::Place { cell })
    }

    fn think_time(&self, _view: &View) -> Millis {
        // Pacing so bots do not feel robotic. The platform honours this as a
        // delay, never as a rule — a late bot is late, not illegal.
        match self.level {
            BotLevel::Trivial => Millis(200),
            _ => Millis(700),
        }
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

fn mark_for_turn(view: &View) -> Mark {
    let x_count = view
        .board
        .iter()
        .filter(|mark| **mark == Some(Mark::X))
        .count();
    let o_count = view
        .board
        .iter()
        .filter(|mark| **mark == Some(Mark::O))
        .count();
    if x_count == o_count {
        Mark::X
    } else {
        Mark::O
    }
}

fn other(mark: Mark) -> Mark {
    match mark {
        Mark::X => Mark::O,
        Mark::O => Mark::X,
    }
}

fn completion(board: [Option<Mark>; 9], mark: Mark) -> Option<u8> {
    LINES.iter().find_map(|line| {
        let empty = line.iter().find(|cell| board[**cell].is_none());
        let marks = line
            .iter()
            .filter(|cell| board[**cell] == Some(mark))
            .count();
        (marks == 2 && empty.is_some())
            .then(|| u8::try_from(*empty?).ok())
            .flatten()
    })
}
