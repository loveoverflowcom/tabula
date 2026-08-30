//! Bots. (doc 02 §6, doc 00 §6.5)
//!
//! # The one structural rule
//!
//! **A bot is not privileged.** `choose` takes `&R::View`, not `&R::State`. It
//! receives exactly what a human in that seat receives. Three consequences, all
//! of which we want:
//!
//! 1. A bot that plays well *proves the projection contains enough information
//!    to play*. That is a free correctness test on the security boundary.
//! 2. A bot cannot accidentally become a cheating oracle.
//! 3. Bots are testable with no server, and are excellent fuzz drivers — bot
//!    self-play is the highest-value test in the suite (doc 02 §11.3).
//!
//! Bots are **inputs, not authorities**: `Effect::RequestBotMove` → bot runner →
//! `Input::Player`. The command goes through the same `apply` and can be
//! rejected like any other.
//!
//! No ML in the MVP (doc 01 §8). Heuristics, and shallow search where the game
//! warrants it — chess gets a small alpha-beta as an *optional crate feature*,
//! never in the rules half.

use tabula_core::{BotLevel, DetRng, Millis, SeatId};

use crate::rules::GameRules;

pub trait GameBot<R: GameRules>: Send + Sync {
    fn level(&self) -> BotLevel;

    /// Deterministic given the same view and rng — that is what makes bot-vs-bot
    /// self-play reproducible, and reproducibility is what makes it a fuzzer.
    ///
    /// `None` means "no move available", not "pass" — if passing is a move, the
    /// game must have a `Command` for it.
    fn choose(&self, view: &R::View, seat: SeatId, rng: &mut DetRng) -> Option<R::Command>;

    /// Optional pacing so bots do not feel robotic.
    ///
    /// The platform honours it as a **delay, not a rule** — it never affects
    /// legality, and a bot that thinks past its deadline is simply late.
    fn think_time(&self, _view: &R::View) -> Millis {
        Millis(600)
    }
}
