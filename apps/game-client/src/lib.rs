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
/// @ai.evidence tests::local_hotseat_applies_a_presenter_move_through_canonical_rules
#[allow(clippy::doc_markdown)]
#[derive(Debug)]
pub struct LocalChessMatch {
    state: State,
    view: View,
    local: ChessLocal,
    seed: MatchSeed,
    input_index: u64,
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
            input_index: 0,
        }
    }

    /// Processes one frame-normalized input and applies any emitted intent.
    ///
    /// Rejected commands are harmless: the rules contract leaves `state` and
    /// the input index unchanged, while the presenter has already cleared the
    /// attempted selection and can accept the next click.
    pub fn handle_input(&mut self, input: &InputEvent, frame: &FrameCtx) {
        self.local.set_viewport(frame.viewport());
        let Some(intent) = ChessPresentation::on_input(input, &self.view, &mut self.local) else {
            return;
        };
        let seat = self.view.you.unwrap_or(self.view.turn).seat();
        let input_index = InputIndex(self.input_index);
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
            return;
        };

        for event in &outcome.events {
            if let Some(view_event) = ChessRules::view_event(&self.state, event, Viewer::Seat(seat))
            {
                ChessPresentation::on_view_event(&view_event, &mut self.local, frame);
            }
        }
        self.input_index = self.input_index.saturating_add(1);
        self.view = ChessRules::project(&self.state, Viewer::Seat(self.state.turn.seat()));
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

        local_match.handle_input(&click(layout, 12), &frame);
        assert_eq!(local_match.view().board[12].unwrap().kind, PieceKind::Pawn);
        local_match.handle_input(&click(layout, 28), &frame);

        assert_eq!(local_match.view().board[12], None);
        assert_eq!(local_match.view().board[28].unwrap().kind, PieceKind::Pawn);
        assert_eq!(local_match.view().turn, ChessColor::Black);
    }
}
