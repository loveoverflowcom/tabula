//! # `tabula-game-client` — the Macroquad gameplay runtime
//!
//! Phase 2 is a local hot-seat vertical slice. The application shell owns the
//! canonical state and deterministic rule context; the game presenter sees
//! only the current projection and its own ephemeral local state.
//!
//! ```text
//! renderer input → presenter → Intent<Command> → ChessRules::apply
//!                                      ↓
//!                         project(state, viewer) → presenter → RenderList
//! ```
//!
//! The frame clock is used only for presentation timing. The local match sends
//! `LogicalTime::ZERO` to untimed rules, so wall-clock time cannot affect the
//! canonical match.

#![forbid(unsafe_code)]

use local_game::{
    presentation::{ChessLocal, ChessPresentation},
    ChessRules, Config, State, View,
};
use tabula_core::{
    DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
    Viewer,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input};
use tabula_game_chess as local_game; // xtask-allow-game-id: direct Phase 2 local vertical slice; not a game-id branch.
use tabula_presentation::{FrameCtx, GamePresentation, InputEvent, RenderList};

/// A deterministic two-seat shell for a local hot-seat match.
///
/// `state` is intentionally private: only this imperative shell applies rules.
/// The presenter receives `view`, never the canonical state, and emits intents
/// that come back through [`ChessRules::apply`].
///
/// @ai.role imperative-shell
/// @ai.domain client.local-match
/// @ai.pure false
/// @ai.invariant projection-only-presenter-input
/// @ai.invariant deterministic-untimed-application
/// @ai.invariant input-index-per-attempt
/// @ai.evidence tests::local_hotseat_applies_a_presenter_move_through_canonical_rules
/// @ai.evidence tests::every_emitted_command_consumes_a_distinct_input_index
#[allow(clippy::doc_markdown)]
#[derive(Debug)]
pub struct LocalChessMatch {
    state: State,
    view: View,
    local: ChessLocal,
    seed: MatchSeed,
    next_input_index: u64,
}

/// Failure raised when the finite canonical input-index domain is exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalChessMatchError {
    InputIndexExhausted,
}

impl LocalChessMatch {
    /// Creates an untimed standard match with a fixed local seed.
    ///
    /// A fixed seed is sufficient because the standard opening position does
    /// not draw randomness. A future local randomised game should receive its
    /// seed from the shell in the same way as the server.
    #[must_use]
    pub fn new() -> Self {
        let seed = MatchSeed::from_bytes([0; 32]);
        let roster = local_roster();
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let init = ChessRules::create(&Config::default(), &roster, &mut ctx)
            .expect("the local two-seat standard match configuration is valid");
        let state = init.state;
        let view = ChessRules::project(&state, Viewer::Seat(SeatId(0)));
        Self {
            state,
            view,
            local: ChessLocal::default(),
            seed,
            // Match creation owns index 0; player attempts begin at index 1.
            next_input_index: 1,
        }
    }

    /// Processes one frame-normalized input and applies any emitted intent.
    ///
    /// Rejected commands are harmless: the rules contract leaves `state` and
    /// `view` unchanged. The attempted command still consumes its input-log
    /// index and RNG domain, matching the runtime replay contract.
    pub fn handle_input(
        &mut self,
        input: &InputEvent,
        frame: &FrameCtx,
    ) -> Result<(), LocalChessMatchError> {
        self.local.set_viewport(frame.viewport());
        let Some(intent) = ChessPresentation::on_input(input, &self.view, &mut self.local) else {
            return Ok(());
        };
        let seat = self.view.you.unwrap_or(self.view.turn).seat();
        let input_index = InputIndex(self.next_input_index);
        self.next_input_index = self
            .next_input_index
            .checked_add(1)
            .ok_or(LocalChessMatchError::InputIndexExhausted)?;
        let mut rng = DetRng::for_input(&self.seed, input_index);
        let mut ctx = Ctx {
            // This match uses the default no-clock configuration. Presentation
            // time never crosses into the deterministic rules context.
            now: LogicalTime::ZERO,
            index: input_index,
            rng: &mut rng,
            budget: Budget::default(),
        };
        let result = ChessRules::apply(
            &mut self.state,
            Input::Player {
                seat,
                command: intent.into_command(),
            },
            &mut ctx,
        );
        let Ok(outcome) = result else {
            return Ok(());
        };

        for event in &outcome.events {
            if let Some(view_event) = ChessRules::view_event(&self.state, event, Viewer::Seat(seat))
            {
                ChessPresentation::on_view_event(&view_event, &mut self.local, frame);
            }
        }
        self.view = ChessRules::project(&self.state, Viewer::Seat(self.state.turn.seat()));
        Ok(())
    }

    /// Builds the renderer-neutral frame from the latest projection.
    #[must_use]
    pub fn present(&self, frame: &FrameCtx) -> RenderList {
        ChessPresentation::present(&self.view, &self.local, frame)
    }

    /// Exposes the authoritative projection for shell diagnostics and tests.
    #[must_use]
    pub const fn view(&self) -> &View {
        &self.view
    }
}

impl Default for LocalChessMatch {
    fn default() -> Self {
        Self::new()
    }
}

fn local_roster() -> SeatRoster {
    SeatRoster::new(
        [
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
        ]
        .into_iter()
        .collect(),
    )
    .expect("local seats are unique")
}

#[cfg(test)]
mod tests {
    use super::local_game::{presentation::BoardLayout, Color as ChessColor, PieceKind, Square};
    use super::*;
    use glam::Vec2;
    use tabula_presentation::{Dpi, PointerButton, PointerPhase, PointerPosition, Viewport};

    fn frame() -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            0,
            tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    fn click(layout: BoardLayout, square: u8) -> InputEvent {
        let square = Square::new(square).expect("test square is valid");
        let rect = layout
            .square_rect(square)
            .expect("test square has geometry");
        InputEvent::Pointer {
            position: PointerPosition::new(rect.origin() + rect.size() * 0.5)
                .expect("test pointer is finite"),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        }
    }

    #[test]
    fn local_hotseat_applies_a_presenter_move_through_canonical_rules() {
        let frame = frame();
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut local_match = LocalChessMatch::new();

        local_match
            .handle_input(&click(layout, 12), &frame)
            .expect("source selection has an available input index");
        assert_eq!(local_match.view().board[12].unwrap().kind, PieceKind::Pawn);
        local_match
            .handle_input(&click(layout, 28), &frame)
            .expect("legal move has an available input index");

        assert_eq!(local_match.view().board[12], None);
        assert_eq!(local_match.view().board[28].unwrap().kind, PieceKind::Pawn);
        assert_eq!(local_match.view().turn, ChessColor::Black);
    }

    #[test]
    fn every_emitted_command_consumes_a_distinct_input_index() {
        let frame = frame();
        let layout = BoardLayout::from_viewport(frame.viewport());
        let e2 = click(layout, 12);
        let e5 = click(layout, 36);
        let e4 = click(layout, 28);
        let mut local_match = LocalChessMatch::new();

        assert_eq!(local_match.next_input_index, 1);

        local_match
            .handle_input(&e2, &frame)
            .expect("first source selection has an available input index");
        assert_eq!(local_match.next_input_index, 1);
        local_match
            .handle_input(&e5, &frame)
            .expect("first rejected command still has an input index");
        assert_eq!(local_match.next_input_index, 2);
        assert_eq!(local_match.view().board[12].unwrap().kind, PieceKind::Pawn);
        assert_eq!(local_match.view().board[28], None);

        local_match
            .handle_input(&e2, &frame)
            .expect("second source selection has an available input index");
        local_match
            .handle_input(&e5, &frame)
            .expect("second rejected command still has an input index");
        assert_eq!(local_match.next_input_index, 3);

        local_match
            .handle_input(&e2, &frame)
            .expect("valid source selection has an available input index");
        local_match
            .handle_input(&e4, &frame)
            .expect("valid command has an available input index");
        assert_eq!(local_match.next_input_index, 4);
        assert_eq!(local_match.view().board[12], None);
        assert_eq!(local_match.view().board[28].unwrap().kind, PieceKind::Pawn);
    }

    #[test]
    fn exhausted_input_index_stops_before_reusing_an_rng_domain() {
        let frame = frame();
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut local_match = LocalChessMatch::new();
        local_match.next_input_index = u64::MAX;

        local_match
            .handle_input(&click(layout, 12), &frame)
            .expect("source selection does not consume an input index");
        assert_eq!(
            local_match.handle_input(&click(layout, 28), &frame),
            Err(LocalChessMatchError::InputIndexExhausted)
        );
        assert_eq!(local_match.next_input_index, u64::MAX);
    }
}
