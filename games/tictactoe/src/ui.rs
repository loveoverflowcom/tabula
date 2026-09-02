//! Renderer-neutral Tic-tac-toe presentation. (doc 04 §5)
//!
//! The presenter consumes [`View`] and keeps only presentation-local state. The
//! rules state remains behind the `GamePresentation` boundary: pointer and keyboard
//! input are translated into a [`Command`] and sent back to the shell as an [`Intent`].

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use tabula_design::{Color, Theme};
use tabula_game_api::{
    A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId, GameRules, SeatId,
};
use tabula_presentation::{
    handle_navigation, Align, AssetPackRef, AudioCue, AudioCues, Border, Camera2D, Corners,
    FocusGraph, FocusId, FocusModality, FocusNode, FocusState, FrameCtx, GamePresentation,
    InputEvent, Intent, Layer, NavigationAction, Paint, PointerButton, PointerPhase,
    PointerPosition, Rect, RenderCmd, RenderList, RenderListBuilder, RenderListError,
    TextStyleToken, Viewport,
};

use crate::{Command, Event, Mark, Status, TicTacToeRules, View};

const STATUS_HEIGHT_FRACTION: f32 = 0.12;
const STATUS_MAX_HEIGHT: f32 = 48.0;

/// The validated, responsive geometry shared by board rendering and hit testing.
///
/// The board uses the smaller remaining content axis, so its rectangle is always
/// square and centered below a small status header. A cell index (0..9) is converted
/// to a rectangle only through this type, keeping drawing and pointer mapping on
/// the same coordinate calculation.
///
/// @ai.role proof-boundary
/// @ai.domain presentation.tictactoe-layout
/// @ai.pure true
/// @ai.invariant finite-square-board-layout
/// @ai.invariant viewport-square-fit
/// @ai.law cell-center-roundtrip
/// @ai.evidence tests::cell_mapping_round_trips_centers_and_rejects_edges_outside_the_board
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardLayout {
    board: Rect,
    status: Rect,
    cell_size: f32,
}

impl BoardLayout {
    /// Computes a square 3x3 board that fits inside a finite validated viewport.
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn from_viewport(viewport: Viewport) -> Self {
        let viewport_size = viewport.size();
        let status_height = (viewport_size.y * STATUS_HEIGHT_FRACTION).min(STATUS_MAX_HEIGHT);
        let status = Rect::new(Vec2::ZERO, Vec2::new(viewport_size.x, status_height))
            .expect("a finite positive viewport produces finite status geometry");
        let content_origin = Vec2::new(0.0, status_height);
        let content_size = Vec2::new(viewport_size.x, viewport_size.y - status_height);
        let side = content_size.x.min(content_size.y);
        let board_origin = content_origin + (content_size - Vec2::splat(side)) * 0.5;
        let board = Rect::new(board_origin, Vec2::splat(side))
            .expect("a finite positive viewport produces finite board geometry");
        Self {
            board,
            status,
            cell_size: side / 3.0,
        }
    }

    #[must_use]
    pub const fn board(self) -> Rect {
        self.board
    }

    #[must_use]
    pub const fn status(self) -> Rect {
        self.status
    }

    #[must_use]
    pub const fn cell_size(self) -> f32 {
        self.cell_size
    }

    /// Returns the rectangle for a board cell (0..9).
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn cell_rect(self, cell: u8) -> Option<Rect> {
        if cell >= 9 {
            return None;
        }
        let col = cell % 3;
        let row = cell / 3;
        let origin = self.board.origin()
            + Vec2::new(
                f32::from(col) * self.cell_size,
                f32::from(row) * self.cell_size,
            );
        Rect::new(origin, Vec2::splat(self.cell_size)).ok()
    }

    /// Maps a finite pointer to a board cell (0..9); edges outside the board
    /// intentionally return None.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::float_arithmetic
    )]
    pub fn cell_at(self, position: PointerPosition) -> Option<u8> {
        if self.cell_size <= 0.0 {
            return None;
        }
        let point = position.get();
        let origin = self.board.origin();
        let side = self.board.size().x;
        let relative = point - origin;
        if relative.x < 0.0 || relative.y < 0.0 || relative.x >= side || relative.y >= side {
            return None;
        }
        let col = (relative.x / self.cell_size).floor() as u8;
        let row = (relative.y / self.cell_size).floor() as u8;
        if col >= 3 || row >= 3 {
            return None;
        }
        Some(row * 3 + col)
    }
}

/// Constructs the focus graph for the 3x3 board grid.
fn tictactoe_focus_graph(layout: BoardLayout) -> FocusGraph {
    let mut nodes = Vec::with_capacity(9);
    for row in 0..3_u8 {
        for col in 0..3_u8 {
            let cell = row * 3 + col;
            let id = FocusId::new(u32::from(cell));
            let rect = layout.cell_rect(cell).expect("valid cell geometry");
            let up = (row > 0).then(|| FocusId::new(u32::from((row - 1) * 3 + col)));
            let down = (row < 2).then(|| FocusId::new(u32::from((row + 1) * 3 + col)));
            let left = (col > 0).then(|| FocusId::new(u32::from(row * 3 + (col - 1))));
            let right = (col < 2).then(|| FocusId::new(u32::from(row * 3 + (col + 1))));
            nodes.push(FocusNode::with_neighbors(id, rect, up, down, left, right));
        }
    }
    FocusGraph::new(nodes).expect("tictactoe focus graph topology is valid")
}

/// Presentation-local state that is never sent to rules or treated as authoritative truth.
///
/// @ai.role presentation-state
/// @ai.domain presentation.tictactoe-local
/// @ai.invariant no-authoritative-game-state
#[derive(Clone, Debug, PartialEq)]
pub struct TicTacToeLocal {
    hover: Option<u8>,
    last_placed: Option<u8>,
    focus: FocusState,
    viewport: Viewport,
}

impl Default for TicTacToeLocal {
    fn default() -> Self {
        Self {
            hover: None,
            last_placed: None,
            focus: FocusState::new(Some(FocusId::new(0)), FocusModality::Pointer, true),
            viewport: Viewport::new(Vec2::splat(1.0)).expect("unit viewport is valid"),
        }
    }
}

impl TicTacToeLocal {
    #[must_use]
    pub const fn hover(&self) -> Option<u8> {
        self.hover
    }

    #[must_use]
    pub const fn last_placed(&self) -> Option<u8> {
        self.last_placed
    }

    #[must_use]
    pub const fn focus(&self) -> &FocusState {
        &self.focus
    }

    pub fn focus_mut(&mut self) -> &mut FocusState {
        &mut self.focus
    }

    #[must_use]
    pub fn cursor(&self) -> Option<u8> {
        self.focus
            .current()
            .and_then(|id| u8::try_from(id.get()).ok())
            .filter(|&c| c < 9)
    }

    pub const fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    pub fn clear_hover(&mut self) {
        self.hover = None;
    }
}

/// The standard Tic-tac-toe presenter for the Phase 2 local and future online clients.
#[derive(Debug, Default)]
pub struct TicTacToePresentation;

impl GamePresentation for TicTacToePresentation {
    type Rules = TicTacToeRules;
    type Local = TicTacToeLocal;

    fn asset_pack() -> AssetPackRef {
        AssetPackRef::from_static("tictactoe", "0.2.0")
    }

    fn present(view: &View, local: &TicTacToeLocal, frame: &FrameCtx) -> RenderList {
        build_render_list(view, local, frame).unwrap_or_else(|_| {
            RenderListBuilder::new(Camera2D::default())
                .finish()
                .expect("the empty render list is valid")
        })
    }

    fn on_view_event(
        event: &<TicTacToeRules as GameRules>::ViewEvent,
        local: &mut TicTacToeLocal,
        _frame: &FrameCtx,
    ) -> AudioCues {
        match event {
            Event::Placed { cell, .. } => {
                local.last_placed = Some(*cell);
                let mut cues = AudioCues::new();
                cues.push(AudioCue::from_static("place"));
                cues
            }
            Event::Ended { .. } => {
                let mut cues = AudioCues::new();
                cues.push(AudioCue::from_static("game-end"));
                cues
            }
        }
    }

    fn on_input(
        input: &InputEvent,
        _view: &View,
        local: &mut TicTacToeLocal,
    ) -> Option<Intent<Command>> {
        let layout = BoardLayout::from_viewport(local.viewport);
        match input {
            InputEvent::Pointer {
                position,
                button,
                phase,
            } => match phase {
                PointerPhase::Down | PointerPhase::Move => {
                    local.hover = layout.cell_at(*position);
                    if let Some(cell) = local.hover {
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(cell))));
                    }
                    None
                }
                PointerPhase::Cancel => {
                    local.hover = None;
                    None
                }
                PointerPhase::Up if *button == PointerButton::Primary => {
                    let cell = layout.cell_at(*position);
                    local.hover = cell;
                    if let Some(cell) = cell {
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(cell))));
                        Some(Intent::new(Command::Place { cell }))
                    } else {
                        None
                    }
                }
                PointerPhase::Up => None,
            },
            InputEvent::Key { .. } | InputEvent::Focus(_) => {
                let graph = tictactoe_focus_graph(layout);
                if local.focus.current().is_none()
                    || !graph.contains(local.focus.current().unwrap())
                {
                    local
                        .focus
                        .set_current(Some(graph.first_id().unwrap_or(FocusId::new(0))));
                }

                match handle_navigation(&graph, &mut local.focus, input) {
                    NavigationAction::None | NavigationAction::Cancel => None,
                    NavigationAction::FocusChanged(focus_id) => {
                        local.hover = u8::try_from(focus_id.get()).ok().filter(|&c| c < 9);
                        None
                    }
                    NavigationAction::Activate(focus_id) => {
                        let cell = u8::try_from(focus_id.get()).ok().filter(|&c| c < 9)?;
                        Some(Intent::new(Command::Place { cell }))
                    }
                }
            }
        }
    }

    fn a11y(view: &View, local: &TicTacToeLocal) -> A11yDescription {
        tictactoe_a11y(view, local)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::float_arithmetic,
    clippy::too_many_lines
)]
fn build_render_list(
    view: &View,
    local: &TicTacToeLocal,
    frame: &FrameCtx,
) -> Result<RenderList, RenderListError> {
    let theme = frame.theme();
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut builder = RenderListBuilder::new(Camera2D::default());

    // 1. Board background cells (Layer::BOARD)
    for cell in 0..9_u8 {
        let rect = layout
            .cell_rect(cell)
            .ok_or(RenderListError::InvalidGeometry)?;
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(0.0)?,
            fill: Some(Paint::Solid(theme.color.surface_container)),
            border: Some(Border::new(1.0, theme.color.outline)?),
            layer: Layer::BOARD,
            z: i16::from(cell),
        })?;
    }

    // 2. Overlays (Layer::OVERLAY): last action, legal target, hover, focus
    for cell in 0..9_u8 {
        let rect = layout
            .cell_rect(cell)
            .ok_or(RenderListError::InvalidGeometry)?;
        let is_last_placed = local.last_placed == Some(cell);
        let is_empty = view.board[usize::from(cell)].is_none();
        let is_on_turn = matches!(view.status, Status::Playing) && view.you == Some(view.turn);
        let is_hovered = local.hover == Some(cell);
        let is_focused = local.focus.is_focus_visible()
            && local.focus.current() == Some(FocusId::new(u32::from(cell)));

        if is_last_placed {
            builder.push(outline(
                rect,
                theme.color.last_action,
                Layer::OVERLAY,
                0,
                &theme,
            )?)?;
        }

        if is_empty && is_on_turn {
            let marker_size = Vec2::splat(layout.cell_size() * 0.16);
            builder.push(RenderCmd::Rect {
                rect: Rect::new(
                    rect.origin() + (rect.size() - marker_size) * 0.5,
                    marker_size,
                )?,
                radii: Corners::uniform(marker_size.x * 0.5)?,
                fill: Some(Paint::Solid(theme.color.legal_target)),
                border: None,
                layer: Layer::OVERLAY,
                z: i16::from(cell),
            })?;
        }

        if is_hovered {
            let tint = if is_empty && is_on_turn {
                theme.color.legal_target
            } else {
                theme.color.surface_container_high
            };
            builder.push(outline(rect, tint, Layer::OVERLAY, 100, &theme)?)?;
        }

        if is_focused {
            builder.push(outline(
                rect,
                theme.focus.ring_color,
                Layer::OVERLAY,
                150,
                &theme,
            )?)?;
        }
    }

    // 3. Marks (Layer::PIECES)
    for (index, mark) in view.board.iter().enumerate() {
        let Some(mark) = mark else {
            continue;
        };
        let cell = u8::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?;
        let rect = layout
            .cell_rect(cell)
            .ok_or(RenderListError::InvalidGeometry)?;
        let style = TextStyleToken::DisplayLg;
        let line_height = theme.text_style(style).line_height().get();
        let (text, color) = match mark {
            Mark::X => ("X", theme.color.seat_marker[0]),
            Mark::O => ("O", theme.color.seat_marker[1]),
        };
        builder.push(RenderCmd::Text {
            text: text.to_owned(),
            at: rect.origin()
                + Vec2::new(rect.size().x * 0.5, rect.size().y * 0.5 - line_height * 0.5),
            style,
            align: Align::Center,
            max_width: None,
            color,
            layer: Layer::PIECES,
            z: i16::from(cell),
        })?;
    }

    // 4. Status HUD (Layer::HUD)
    let status = status_text(view);
    let status_rect = layout.status();
    let status_line_height = theme
        .text_style(TextStyleToken::TitleLg)
        .line_height()
        .get();
    builder.push(RenderCmd::Text {
        text: status,
        at: status_rect.origin()
            + Vec2::new(
                status_rect.size().x * 0.5,
                status_rect.size().y * 0.5 - status_line_height * 0.5,
            ),
        style: TextStyleToken::TitleLg,
        align: Align::Center,
        max_width: None,
        color: theme.color.on_surface,
        layer: Layer::HUD,
        z: 0,
    })?;

    builder.finish()
}

fn status_text(view: &View) -> String {
    match view.status {
        Status::Playing => {
            let mark_str = if view.turn == SeatId(0) { "X" } else { "O" };
            if view.you == Some(view.turn) {
                format!("Your turn ({mark_str})")
            } else {
                format!("{mark_str} to move")
            }
        }
        Status::Won(winner) => {
            let mark_str = if winner == SeatId(0) { "X" } else { "O" };
            format!("{mark_str} won!")
        }
        Status::Forfeited(winner) => {
            let mark_str = if winner == SeatId(0) { "X" } else { "O" };
            format!("{mark_str} won by forfeit")
        }
        Status::Drawn => String::from("Game drawn"),
        Status::Aborted => String::from("Game aborted"),
    }
}

fn tictactoe_a11y(view: &View, _local: &TicTacToeLocal) -> A11yDescription {
    let items = view
        .board
        .iter()
        .enumerate()
        .filter_map(|(index, mark)| {
            let cell = u8::try_from(index).ok()?;
            if cell >= 9 {
                return None;
            }
            let label = mark.map_or_else(
                || String::from("Empty cell"),
                |mark| match mark {
                    Mark::X => String::from("Mark X"),
                    Mark::O => String::from("Mark O"),
                },
            );
            let state = if mark.is_some() {
                String::from("occupied")
            } else {
                String::from("empty")
            };
            let activates = if mark.is_none()
                && matches!(view.status, Status::Playing)
                && view.you == Some(view.turn)
            {
                Some(ActionId(String::from("place-cell")))
            } else {
                None
            };
            let row = cell / 3 + 1;
            let col = cell % 3 + 1;
            Some(A11yItem {
                label,
                position: format!("row {row} column {col}"),
                state,
                activates,
            })
        })
        .collect();

    A11yDescription {
        status: status_text(view),
        regions: vec![A11yRegion {
            label: String::from("TicTacToe board"),
            items,
        }],
        actions: vec![A11yAction {
            id: ActionId(String::from("place-cell")),
            label: String::from("Place mark in cell"),
            enabled: matches!(view.status, Status::Playing) && view.you == Some(view.turn),
        }],
    }
}

fn outline(
    rect: Rect,
    color: Color,
    layer: Layer,
    z: i16,
    theme: &Theme,
) -> Result<RenderCmd, RenderListError> {
    Ok(RenderCmd::Rect {
        rect,
        radii: Corners::uniform(0.0)?,
        fill: None,
        border: Some(Border::new(theme.focus.ring_width.get(), color)?),
        layer,
        z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{
        DetRng, InputIndex, LogicalTime, MatchSeed, SeatEntry, SeatRoster, UserId, Viewer,
    };
    use tabula_design::ThemeKind;
    use tabula_game_api::{Budget, Ctx, Input};
    use tabula_presentation::{Dpi, Key, PointerPhase};
    use tabula_testkit::assert_render_list_snapshot;

    fn test_roster() -> SeatRoster {
        SeatRoster::new(
            [
                SeatEntry {
                    seat: SeatId(0),
                    occupant: tabula_core::Occupant::Human(UserId(1)),
                    team: None,
                },
                SeatEntry {
                    seat: SeatId(1),
                    occupant: tabula_core::Occupant::Human(UserId(2)),
                    team: None,
                },
            ]
            .into_iter()
            .collect(),
        )
        .expect("test roster is valid")
    }

    fn initial_view() -> View {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let init = TicTacToeRules::create(&crate::Config::default(), &test_roster(), &mut ctx)
            .expect("create is valid");
        TicTacToeRules::project(&init.state, Viewer::Seat(SeatId(0)))
    }

    fn midgame_view() -> View {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let mut state = TicTacToeRules::create(&crate::Config::default(), &test_roster(), &mut ctx)
            .expect("create is valid")
            .state;
        let mut apply_rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(1));
        let mut apply_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(1),
            rng: &mut apply_rng,
            budget: Budget::default(),
        };
        TicTacToeRules::apply(
            &mut state,
            Input::Player {
                seat: SeatId(0),
                command: Command::Place { cell: 0 },
            },
            &mut apply_ctx,
        )
        .expect("move 1 accepted");
        let mut apply_rng2 = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(2));
        let mut apply_ctx2 = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(2),
            rng: &mut apply_rng2,
            budget: Budget::default(),
        };
        TicTacToeRules::apply(
            &mut state,
            Input::Player {
                seat: SeatId(1),
                command: Command::Place { cell: 4 },
            },
            &mut apply_ctx2,
        )
        .expect("move 2 accepted");
        TicTacToeRules::project(&state, Viewer::Seat(SeatId(0)))
    }

    fn won_view() -> View {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let mut state = TicTacToeRules::create(&crate::Config::default(), &test_roster(), &mut ctx)
            .expect("create is valid")
            .state;
        for (i, (seat, cell)) in [(0, 0), (1, 3), (0, 1), (1, 4), (0, 2)].iter().enumerate() {
            let mut apply_rng =
                DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(i as u64 + 1));
            let mut apply_ctx = Ctx {
                now: LogicalTime::ZERO,
                index: InputIndex(i as u64 + 1),
                rng: &mut apply_rng,
                budget: Budget::default(),
            };
            TicTacToeRules::apply(
                &mut state,
                Input::Player {
                    seat: SeatId(*seat),
                    command: Command::Place { cell: *cell },
                },
                &mut apply_ctx,
            )
            .expect("move accepted");
        }
        TicTacToeRules::project(&state, Viewer::Seat(SeatId(0)))
    }

    fn drawn_view() -> View {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let mut state = TicTacToeRules::create(&crate::Config::default(), &test_roster(), &mut ctx)
            .expect("create is valid")
            .state;
        for (i, (seat, cell)) in [
            (0, 0),
            (1, 1),
            (0, 2),
            (1, 4),
            (0, 3),
            (1, 5),
            (0, 7),
            (1, 6),
            (0, 8),
        ]
        .iter()
        .enumerate()
        {
            let mut apply_rng =
                DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(i as u64 + 1));
            let mut apply_ctx = Ctx {
                now: LogicalTime::ZERO,
                index: InputIndex(i as u64 + 1),
                rng: &mut apply_rng,
                budget: Budget::default(),
            };
            TicTacToeRules::apply(
                &mut state,
                Input::Player {
                    seat: SeatId(*seat),
                    command: Command::Place { cell: *cell },
                },
                &mut apply_ctx,
            )
            .expect("move accepted");
        }
        TicTacToeRules::project(&state, Viewer::Seat(SeatId(0)))
    }

    fn frame_for(width: f32, height: f32, theme: ThemeKind) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::new(width, height)).expect("viewport is valid"),
            Dpi::new(1.0).expect("dpi is valid"),
            0,
            Theme::by_kind(theme),
        )
    }

    #[test]
    fn cell_mapping_round_trips_centers_and_rejects_edges_outside_the_board() {
        let layout = BoardLayout::from_viewport(Viewport::new(Vec2::splat(600.0)).unwrap());
        for cell in 0..9_u8 {
            let rect = layout.cell_rect(cell).expect("valid cell rect");
            let center = PointerPosition::new(rect.origin() + rect.size() * 0.5).unwrap();
            assert_eq!(layout.cell_at(center), Some(cell));
        }

        assert_eq!(layout.cell_rect(9), None);
        assert_eq!(layout.cell_rect(255), None);

        // Outside coordinates
        assert_eq!(
            layout.cell_at(PointerPosition::new(Vec2::new(-10.0, 100.0)).unwrap()),
            None
        );
        assert_eq!(
            layout.cell_at(PointerPosition::new(Vec2::new(100.0, -10.0)).unwrap()),
            None
        );
        assert_eq!(
            layout.cell_at(PointerPosition::new(Vec2::new(700.0, 100.0)).unwrap()),
            None
        );
        assert_eq!(
            layout.cell_at(PointerPosition::new(Vec2::new(100.0, 700.0)).unwrap()),
            None
        );
    }

    #[test]
    fn pointer_hit_testing_for_all_nine_cells() {
        let view = initial_view();
        let mut local = TicTacToeLocal::default();
        local.set_viewport(Viewport::new(Vec2::splat(600.0)).unwrap());
        let layout = BoardLayout::from_viewport(Viewport::new(Vec2::splat(600.0)).unwrap());

        for cell in 0..9_u8 {
            let rect = layout.cell_rect(cell).unwrap();
            let center = PointerPosition::new(rect.origin() + rect.size() * 0.5).unwrap();

            // Down sets hover and returns None
            let down_event = InputEvent::Pointer {
                position: center,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            };
            assert_eq!(
                TicTacToePresentation::on_input(&down_event, &view, &mut local),
                None
            );
            assert_eq!(local.hover(), Some(cell));

            // Up emits Intent with Place command
            let up_event = InputEvent::Pointer {
                position: center,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            };
            let intent = TicTacToePresentation::on_input(&up_event, &view, &mut local);
            assert_eq!(intent, Some(Intent::new(Command::Place { cell })));
        }
    }

    #[test]
    fn outside_board_click_emits_no_command() {
        let view = initial_view();
        let mut local = TicTacToeLocal::default();
        local.set_viewport(Viewport::new(Vec2::splat(600.0)).unwrap());

        let outside = PointerPosition::new(Vec2::new(10.0, 10.0)).unwrap();
        let up_event = InputEvent::Pointer {
            position: outside,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        assert_eq!(
            TicTacToePresentation::on_input(&up_event, &view, &mut local),
            None
        );
    }

    #[test]
    fn keyboard_focus_navigation_and_activation() {
        let view = initial_view();
        let mut local = TicTacToeLocal::default();
        local.set_viewport(Viewport::new(Vec2::splat(600.0)).unwrap());

        // Navigate right from 0 to 1
        let right = InputEvent::Key {
            key: Key::ArrowRight,
            pressed: true,
        };
        assert_eq!(
            TicTacToePresentation::on_input(&right, &view, &mut local),
            None
        );
        assert_eq!(local.cursor(), Some(1));

        // Navigate down from 1 to 4
        let down = InputEvent::Key {
            key: Key::ArrowDown,
            pressed: true,
        };
        assert_eq!(
            TicTacToePresentation::on_input(&down, &view, &mut local),
            None
        );
        assert_eq!(local.cursor(), Some(4));

        // Activate with Enter on cell 4
        let enter = InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        };
        assert_eq!(
            TicTacToePresentation::on_input(&enter, &view, &mut local),
            Some(Intent::new(Command::Place { cell: 4 }))
        );
    }

    #[test]
    fn view_events_update_local_last_placed_and_audio_cues() {
        let mut local = TicTacToeLocal::default();
        let frame = frame_for(600.0, 600.0, ThemeKind::Light);

        let placed = Event::Placed {
            seat: SeatId(0),
            cell: 4,
            mark: Mark::X,
        };
        let cues = TicTacToePresentation::on_view_event(&placed, &mut local, &frame);
        assert_eq!(local.last_placed(), Some(4));
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].id(), "place");

        let ended = Event::Ended {
            outcome: tabula_core::MatchOutcome::new_for_seats(
                tabula_core::OutcomeKind::Draw,
                smallvec::smallvec![
                    tabula_core::Standing {
                        seat: SeatId(0),
                        rank: 0,
                        score: 0,
                    },
                    tabula_core::Standing {
                        seat: SeatId(1),
                        rank: 0,
                        score: 0,
                    },
                ],
                "draw".into(),
                &[SeatId(0), SeatId(1)],
            )
            .unwrap(),
        };
        let ended_cues = TicTacToePresentation::on_view_event(&ended, &mut local, &frame);
        assert_eq!(ended_cues.len(), 1);
        assert_eq!(ended_cues[0].id(), "game-end");
    }

    #[test]
    fn a11y_exposes_all_nine_cells_occupancy_and_actions() {
        let view = midgame_view();
        let local = TicTacToeLocal::default();
        let desc = TicTacToePresentation::a11y(&view, &local);

        assert_eq!(desc.regions.len(), 1);
        assert_eq!(desc.regions[0].items.len(), 9);
        assert_eq!(desc.regions[0].items[0].state, "occupied");
        assert_eq!(desc.regions[0].items[0].label, "Mark X");
        assert_eq!(desc.regions[0].items[4].state, "occupied");
        assert_eq!(desc.regions[0].items[4].label, "Mark O");
        assert_eq!(desc.regions[0].items[1].state, "empty");
        assert_eq!(desc.regions[0].items[1].label, "Empty cell");
        assert_eq!(desc.actions.len(), 1);
        assert!(desc.actions[0].enabled);
    }

    #[test]
    fn golden_tictactoe_initial_640x640_light() {
        let view = initial_view();
        let local = TicTacToeLocal::default();
        let frame = frame_for(640.0, 640.0, ThemeKind::Light);
        let list = TicTacToePresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("tictactoe_initial_640x640_light", list);
    }

    #[test]
    fn golden_tictactoe_initial_320x640_responsive() {
        let view = initial_view();
        let local = TicTacToeLocal::default();
        let frame = frame_for(320.0, 640.0, ThemeKind::Light);
        let list = TicTacToePresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("tictactoe_initial_320x640_responsive", list);
    }

    #[test]
    fn golden_tictactoe_midgame_focus_and_hover_dark() {
        let view = midgame_view();
        let mut local = TicTacToeLocal::default();
        local.set_viewport(Viewport::new(Vec2::splat(640.0)).unwrap());
        local.hover = Some(2);
        local.last_placed = Some(4);
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(2)));
        let frame = frame_for(640.0, 640.0, ThemeKind::Dark);
        let list = TicTacToePresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("tictactoe_midgame_focus_and_hover_dark", list);
    }

    #[test]
    fn golden_tictactoe_won_state_light() {
        let view = won_view();
        let local = TicTacToeLocal::default();
        let frame = frame_for(640.0, 640.0, ThemeKind::Light);
        let list = TicTacToePresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("tictactoe_won_state_light", list);
    }

    #[test]
    fn golden_tictactoe_drawn_state_dark() {
        let view = drawn_view();
        let local = TicTacToeLocal::default();
        let frame = frame_for(640.0, 640.0, ThemeKind::Dark);
        let list = TicTacToePresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("tictactoe_drawn_state_dark", list);
    }
}
