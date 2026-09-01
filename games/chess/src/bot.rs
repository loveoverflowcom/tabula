//! Projection-only chess bots. (doc 02 §6, doc 00 §6.5)
//!
//! The Easy policy searches exactly one reply past each legal move. Its private
//! simulation position is reconstructed from [`View`] and delegates every move
//! transition and reply list to the canonical rules contract. Search deliberately
//! omits unavailable repetition history and wall-clock clock state; neither is
//! part of the public bot projection's usable search information.
//! Draw offers and resignation are intentionally outside this move-only policy;
//! as before, a view with no legal moves produces `None`.
//!
//! @ai.role projection-bot
//! @ai.domain chess.bot
//! @ai.pure true
//! @ai.invariant bot-output-is-a-projected-legal-command
//! @ai.law exact-score-ties-use-deterministic-rng
//! @ai.evidence tests::bot::easy_bot_always_returns_a_legal_projected_command
//! @ai.evidence tests::bot::easy_bot_is_deterministic_for_identical_view_and_rng
//! @ai.evidence tests::bot::easy_equal_score_ties_use_rng_but_stay_with_the_best_moves

#![allow(clippy::doc_markdown)] // `@ai.*` schema values must remain bare machine-readable paths.

use tabula_core::{
    BotLevel, DetRng, InputIndex, LogicalTime, MatchSeed, Millis, OutcomeKind, SeatId, Viewer,
};
use tabula_game_api::{Budget, Ctx, GameBot, GameRules, Input};

use crate::{ChessRules, Color, Command, PieceKind, State, Status, View};

/// A fixed private context for non-authoritative search simulations.
///
/// Chess rule application currently does not draw randomness, but supplying a
/// separate deterministic context keeps simulation from consuming the caller's
/// tie-breaking stream if that ever changes.
const SEARCH_SEED: MatchSeed = MatchSeed::from_bytes([0; 32]);
const MATE_SCORE: i32 = 1_000_000;

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
        let color = view.you?;
        if Some(color) != Color::from_seat(seat) || view.legal_moves.is_empty() {
            return None;
        }

        match self.level {
            BotLevel::Trivial => choose_uniform(&view.legal_moves, rng),
            BotLevel::Easy => choose_best(easy_scores(view, color), rng),
            BotLevel::Medium | BotLevel::Hard => None,
        }
    }

    fn think_time(&self, _view: &View) -> Millis {
        match self.level {
            BotLevel::Trivial => Millis(200),
            BotLevel::Easy => Millis(500),
            BotLevel::Medium | BotLevel::Hard => Millis(600),
        }
    }
}

fn choose_uniform(commands: &[Command], rng: &mut DetRng) -> Option<Command> {
    let count = u32::try_from(commands.len()).ok()?;
    let index = usize::try_from(rng.below(count)).ok()?;
    commands.get(index).copied()
}

fn choose_best(scored: Vec<(i32, Command)>, rng: &mut DetRng) -> Option<Command> {
    let best_score = scored.iter().map(|(score, _)| *score).max()?;
    // The input projection and canonical move generator already provide stable
    // ordering. Keep that order until the one and only RNG draw below.
    let best_moves: Vec<_> = scored
        .into_iter()
        .filter_map(|(score, command)| (score == best_score).then_some(command))
        .collect();
    choose_uniform(&best_moves, rng)
}

/// Scores every original projected move using a projection-limited two-ply
/// position search.
fn easy_scores(view: &View, color: Color) -> Vec<(i32, Command)> {
    let state = state_from_view(view);
    view.legal_moves
        .iter()
        .copied()
        .filter_map(|command| {
            // Chess's legal projection currently contains only moves. Keep the
            // match explicit so a future non-move affordance cannot enter search.
            if !matches!(command, Command::Move { .. }) {
                return None;
            }
            let mut after_our_move = state.clone();
            apply_search_move(&mut after_our_move, color.seat(), command)
                .then(|| (score_after_reply(&after_our_move, color), command))
        })
        .collect()
}

/// Returns the worst position the opponent can leave after our candidate move.
fn score_after_reply(after_our_move: &State, our_color: Color) -> i32 {
    let opponent = our_color.other();
    let opponent_view = ChessRules::project(after_our_move, Viewer::Seat(opponent.seat()));
    let mut worst_score = i32::MAX;
    let mut found_reply = false;

    for reply in opponent_view.legal_moves {
        if !matches!(reply, Command::Move { .. }) {
            continue;
        }
        let mut after_reply = after_our_move.clone();
        if apply_search_move(&mut after_reply, opponent.seat(), reply) {
            found_reply = true;
            worst_score = worst_score.min(evaluate(&after_reply, our_color));
        }
    }

    if found_reply {
        worst_score
    } else {
        // This also evaluates checkmate/stalemate positions after our move.
        evaluate(after_our_move, our_color)
    }
}

/// Reconstructs only the public position facts exposed by [`View`].
///
/// The synthetic one-entry repetition vector is enough for the private rules
/// simulation to have a current position key, but it is not recovered history.
/// Consequently this search is intentionally not a repetition-aware oracle.
fn state_from_view(view: &View) -> State {
    let mut state = State {
        board: view.board,
        turn: view.turn,
        castling: view.castling,
        en_passant: view.en_passant,
        halfmove_clock: view.halfmove_clock,
        fullmove_number: view.fullmove_number,
        repetition: Vec::new(),
        status: view.status.clone(),
        draw_offer: view.draw_offer,
        // Search is intentionally position-only. A bot cannot infer elapsed
        // wall-clock time or repetition history from a projection and should
        // not alter its move policy based on a non-authoritative approximation.
        clock: None,
    };
    let current_key = state.position_key();
    state.repetition.push(current_key);
    state
}

/// Applies a simulated move through the same public rule transition as a human
/// command. The returned command remains the original projected command.
fn apply_search_move(state: &mut State, seat: SeatId, command: Command) -> bool {
    let mut simulation_rng = DetRng::for_input(&SEARCH_SEED, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime(0),
        index: InputIndex(0),
        rng: &mut simulation_rng,
        budget: Budget::default(),
    };
    ChessRules::apply(state, Input::Player { seat, command }, &mut ctx).is_ok()
}

/// Static integer evaluation from one explicit bot perspective.
fn evaluate(state: &State, perspective: Color) -> i32 {
    if let Status::Ended { outcome } = &state.status {
        // Terminal utility dominates every material or positional heuristic.
        // In particular, a drawn position is exactly neutral even when its
        // board would otherwise look winning or losing.
        return terminal_score(outcome, perspective);
    }

    let board_score: i32 = state
        .board
        .iter()
        .enumerate()
        .filter_map(|(square, piece)| piece.map(|piece| (square, piece)))
        .map(|(square, piece)| {
            let sign = if piece.color == perspective { 1 } else { -1 };
            sign * (piece_value(piece.kind) + piece_square_value(piece.kind, piece.color, square))
        })
        .sum();

    board_score
}

fn terminal_score(outcome: &tabula_core::MatchOutcome, perspective: Color) -> i32 {
    if !matches!(outcome.kind(), OutcomeKind::Decisive) {
        return 0;
    }
    let Some(winner) = outcome
        .standings()
        .iter()
        .find(|standing| standing.rank == 0)
        .map(|standing| standing.seat)
    else {
        return 0;
    };
    if winner == perspective.seat() {
        MATE_SCORE
    } else {
        -MATE_SCORE
    }
}

const fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight => 320,
        PieceKind::Bishop => 330,
        PieceKind::Rook => 500,
        PieceKind::Queen => 900,
        PieceKind::King => 0,
    }
}

/// A deliberately small, integer-only piece-square heuristic.
///
/// The coordinates are expressed from White's side of the board. Black mirrors
/// the rank so both colors receive the same advancement/centralization preference.
fn piece_square_value(kind: PieceKind, color: Color, square: usize) -> i32 {
    let oriented_square = if color == Color::White {
        square
    } else {
        square ^ 0x38
    };
    let file = oriented_square % 8;
    let rank_index = oriented_square / 8;
    let file_center = 3usize.saturating_sub(file.abs_diff(3).min(file.abs_diff(4)));
    let rank_center = 3usize.saturating_sub(rank_index.abs_diff(3).min(rank_index.abs_diff(4)));
    let rank = i32::try_from(rank_index).unwrap_or(0);
    let center = i32::try_from(file_center + rank_center).unwrap_or(0);

    match kind {
        PieceKind::Pawn => rank * 6 + center * 2,
        PieceKind::Knight => center * 16,
        PieceKind::Bishop => center * 8,
        PieceKind::Rook => rank * 2 + center * 2,
        PieceKind::Queen => center * 4,
        // The simple Easy policy values king safety over activity.
        PieceKind::King => -center * 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::canonical_encode;

    #[test]
    fn easy_bot_cannot_distinguish_unprojected_repetition_history() {
        let first = State::initial();
        let mut second = first.clone();
        second.repetition.push(crate::PositionKey(0xfeed_face));
        let first_view = ChessRules::project(&first, Viewer::Seat(SeatId(0)));
        let second_view = ChessRules::project(&second, Viewer::Seat(SeatId(0)));

        assert_ne!(
            canonical_encode(&first).unwrap(),
            canonical_encode(&second).unwrap()
        );
        assert_eq!(
            canonical_encode(&first_view).unwrap(),
            canonical_encode(&second_view).unwrap()
        );

        let mut first_rng = DetRng::for_input(&SEARCH_SEED, InputIndex(7));
        let mut second_rng = DetRng::for_input(&SEARCH_SEED, InputIndex(7));
        assert_eq!(
            ChessBot::new(BotLevel::Easy).choose(&first_view, SeatId(0), &mut first_rng,),
            ChessBot::new(BotLevel::Easy).choose(&second_view, SeatId(0), &mut second_rng,)
        );
    }
}
