//! Projection-only chess bots. (doc 02 §6, doc 00 §6.5)

use tabula_core::{BotLevel, DetRng, Millis, SeatId};
use tabula_game_api::GameBot;

use crate::{ChessRules, Color, Command, PieceKind, View};

/// A deterministic lightweight bot for auto-fill and self-play.
#[derive(Debug)]
pub struct ChessBot {
    level: BotLevel,
}

impl ChessBot {
    #[must_use]
    pub const fn new(level: BotLevel) -> Self {
        Self { level }
    }
}

impl GameBot<ChessRules> for ChessBot {
    fn level(&self) -> BotLevel {
        self.level
    }

    fn choose(&self, view: &View, seat: SeatId, rng: &mut DetRng) -> Option<Command> {
        if view.you != Color::from_seat(seat) || view.legal_moves.is_empty() {
            return None;
        }
        let candidates: Vec<_> = match self.level {
            BotLevel::Trivial => view.legal_moves.clone(),
            BotLevel::Easy => best_captures(view),
            BotLevel::Medium | BotLevel::Hard => return None,
        };
        let index = usize::try_from(rng.below(u32::try_from(candidates.len()).ok()?)).ok()?;
        candidates.get(index).copied()
    }

    fn think_time(&self, _view: &View) -> Millis {
        match self.level {
            BotLevel::Trivial => Millis(200),
            BotLevel::Easy => Millis(500),
            BotLevel::Medium | BotLevel::Hard => Millis(600),
        }
    }
}

fn best_captures(view: &View) -> Vec<Command> {
    let Some(color) = view.you else {
        return Vec::new();
    };
    let scored: Vec<_> = view
        .legal_moves
        .iter()
        .copied()
        .map(|command| (capture_value(view, color, command), command))
        .collect();
    let best = scored.iter().map(|(value, _)| *value).max().unwrap_or(0);
    scored
        .into_iter()
        .filter_map(|(value, command)| (value == best).then_some(command))
        .collect()
}

fn capture_value(view: &View, color: Color, command: Command) -> u8 {
    let Command::Move { to, .. } = command else {
        return 0;
    };
    if let Some(piece) = view.board.get(usize::from(to)).copied().flatten() {
        return if piece.color == color {
            0
        } else {
            piece_value(piece.kind)
        };
    }
    u8::from(
        view.en_passant == crate::Square::new(to)
            && view.board.get(usize::from(to)).is_some_and(Option::is_none),
    )
}

const fn piece_value(kind: PieceKind) -> u8 {
    match kind {
        PieceKind::Pawn => 1,
        PieceKind::Knight | PieceKind::Bishop => 3,
        PieceKind::Rook => 5,
        PieceKind::Queen => 9,
        PieceKind::King => 0,
    }
}
