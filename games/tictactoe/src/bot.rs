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

use crate::{rules::TicTacToeRules, state::Command, state::View};

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

    fn choose(&self, _view: &View, _seat: SeatId, _rng: &mut DetRng) -> Option<Command> {
        // TODO(phase 0): implement per level.
        //
        //   Trivial — uniform random legal cell. Free for any game that
        //             implements `legal_commands`; this is the one to write
        //             first because self-play (the primary fuzzer) only needs it.
        //   Easy    — random, but take an immediate win and block an immediate loss.
        //   Medium  — the above plus centre/corner preference.
        //   Hard    — full minimax. The board is 9 cells; it is instant.
        //
        // MUST be deterministic given `(view, rng)`. That is what makes
        // bot-vs-bot self-play reproducible, and reproducibility is the entire
        // reason self-play works as a fuzzer (doc 02 §11.3).
        todo!("doc 02 §6")
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
