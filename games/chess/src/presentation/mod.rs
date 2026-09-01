//! Renderer-neutral Chess presentation. (doc 04 §5)
//!
//! The presenter consumes [`View`] and keeps only ephemeral interaction state. The
//! rules state remains behind the `GamePresentation` boundary: pointer input is
//! translated into a [`Command`] and sent back to the shell as an [`Intent`].

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use tabula_design::{Color as SemanticTint, Theme};
use tabula_game_api::{A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId, GameRules};
use tabula_presentation::{
    Align, AssetPackRef, Border, Camera2D, Corners, FrameCtx, GamePresentation, InputEvent, Intent,
    Key, Layer, Paint, PointerButton, PointerPhase, PointerPosition, Rect, RenderCmd, RenderList,
    RenderListBuilder, RenderListError, TextStyleToken, Viewport,
};

use crate::{ChessRules, Color as ChessColor, Command, Piece, PieceKind, Square, Status, View};

const PROMOTION_KINDS: [PieceKind; 4] = [
    PieceKind::Queen,
    PieceKind::Rook,
    PieceKind::Bishop,
    PieceKind::Knight,
];
const STATUS_HEIGHT_FRACTION: f32 = 0.12;
const STATUS_MAX_HEIGHT: f32 = 48.0;

/// The validated, responsive geometry shared by board rendering and hit testing.
///
/// The board uses the smaller remaining content axis, so its rectangle is always
/// square and centered below a small status header. A `Square` is converted to a
/// rectangle only through this type, which keeps rendering and pointer mapping
/// on the same coordinate calculation.
///
/// @ai.role proof-boundary
/// @ai.domain presentation.chess-layout
/// @ai.pure true
/// @ai.invariant finite-square-board-layout
/// @ai.invariant viewport-square-fit
/// @ai.law square-center-roundtrip
/// @ai.evidence tests::board_is_64_square_square_and_fits_both_viewport_orientations
/// @ai.evidence tests::square_mapping_round_trips_centers_and_rejects_edges_outside_the_board
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardLayout {
    board: Rect,
    status: Rect,
    square_size: f32,
}

impl BoardLayout {
    /// Computes a square board that fits inside a finite validated viewport.
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
            square_size: side / 8.0,
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
    pub const fn square_size(self) -> f32 {
        self.square_size
    }

    /// Returns the rectangle for a representable board square.
    ///
    /// The public `Square` tuple remains defensively checked here because this
    /// is a presentation boundary and callers may hold a value from a wire DTO.
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn square_rect(self, square: Square) -> Option<Rect> {
        if square.0 >= 64 {
            return None;
        }
        let row = 7 - square.rank();
        let origin = self.board.origin()
            + Vec2::new(
                f32::from(square.file()) * self.square_size,
                f32::from(row) * self.square_size,
            );
        Rect::new(origin, Vec2::splat(self.square_size)).ok()
    }

    /// Maps a finite pointer to a board square; the right and bottom edges are
    /// intentionally outside the board so no coordinate maps to file/rank 8.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::float_arithmetic
    )]
    pub fn square_at(self, position: PointerPosition) -> Option<Square> {
        if self.square_size <= 0.0 {
            return None;
        }
        let point = position.get();
        let origin = self.board.origin();
        let side = self.board.size().x;
        let relative = point - origin;
        if relative.x < 0.0 || relative.y < 0.0 || relative.x >= side || relative.y >= side {
            return None;
        }
        let file = (relative.x / self.square_size).floor() as u8;
        let row = (relative.y / self.square_size).floor() as u8;
        if file >= 8 || row >= 8 {
            return None;
        }
        Square::new(file + (7 - row) * 8)
    }
}

/// Mutually exclusive interaction modes owned by the client only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interaction {
    #[default]
    Idle,
    Selected {
        square: Square,
    },
    Promotion {
        from: Square,
        to: Square,
    },
}

/// Chess presentation state that is never sent to rules or treated as truth.
///
/// @ai.role presentation-state
/// @ai.domain presentation.chess-local
/// @ai.invariant no-authoritative-game-state
/// @ai.evidence tests::pointer_selection_is_local_and_valid_destination_emits_one_command
#[derive(Clone, Debug, PartialEq)]
pub struct ChessLocal {
    interaction: Interaction,
    hover: Option<Square>,
    last_move: Option<(Square, Square)>,
    cursor: Square,
    viewport: Viewport,
}

impl Default for ChessLocal {
    fn default() -> Self {
        Self {
            interaction: Interaction::Idle,
            hover: None,
            last_move: None,
            cursor: Square::new(0).expect("a1 is a representable square"),
            viewport: Viewport::new(Vec2::splat(1.0)).expect("unit viewport is valid"),
        }
    }
}

impl ChessLocal {
    #[must_use]
    pub const fn interaction(&self) -> Interaction {
        self.interaction
    }

    #[must_use]
    pub const fn hover(&self) -> Option<Square> {
        self.hover
    }

    #[must_use]
    pub const fn last_move(&self) -> Option<(Square, Square)> {
        self.last_move
    }

    #[must_use]
    pub const fn cursor(&self) -> Square {
        self.cursor
    }

    /// Records the current logical viewport for pointer hit testing.
    ///
    /// Viewport size is presentation-local input context, not game state. The
    /// client updates it before draining each frame, so a resize between two
    /// clicks uses the new board geometry.
    pub const fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    fn clear_interaction(&mut self) {
        self.interaction = Interaction::Idle;
    }
}

/// The standard Chess presenter for the Phase 2 local and future online clients.
#[derive(Debug, Default)]
pub struct ChessPresentation;

impl GamePresentation for ChessPresentation {
    type Rules = ChessRules;
    type Local = ChessLocal;

    fn asset_pack() -> AssetPackRef {
        AssetPackRef::new("chess", "0.1.0")
    }

    fn present(view: &View, local: &ChessLocal, frame: &FrameCtx) -> RenderList {
        build_render_list(view, local, frame).unwrap_or_else(|_| {
            // `View` is produced by the rules boundary and the frame is built
            // from validated facts. This fallback keeps a future malformed
            // client projection from taking down the render loop.
            RenderListBuilder::new(Camera2D::default())
                .finish()
                .expect("the empty render list is valid")
        })
    }

    fn on_view_event(
        event: &<ChessRules as GameRules>::ViewEvent,
        local: &mut ChessLocal,
        _frame: &FrameCtx,
    ) {
        match event {
            crate::ViewEvent::Moved { from, to, .. } => {
                local.last_move = Some((*from, *to));
                local.clear_interaction();
            }
            crate::ViewEvent::Ended { .. } => local.clear_interaction(),
            crate::ViewEvent::ClockUpdated { .. }
            | crate::ViewEvent::DrawOffered { .. }
            | crate::ViewEvent::DrawDeclined { .. } => {}
        }
    }

    fn on_input(
        input: &InputEvent,
        view: &View,
        local: &mut ChessLocal,
    ) -> Option<Intent<Command>> {
        // The input contract is already normalized, so the presenter only
        // performs local hit testing and command translation here.
        match input {
            InputEvent::Pointer {
                position,
                button,
                phase,
            } => {
                let layout = BoardLayout::from_viewport(local.viewport);
                match phase {
                    PointerPhase::Move | PointerPhase::Down => {
                        local.hover = layout.square_at(*position);
                        local.cursor = local.hover.unwrap_or(local.cursor);
                        None
                    }
                    PointerPhase::Cancel => {
                        local.hover = None;
                        local.clear_interaction();
                        None
                    }
                    PointerPhase::Up if *button == PointerButton::Primary => {
                        let square = layout.square_at(*position);
                        local.hover = square;
                        if let Some(square) = square {
                            local.cursor = square;
                        }
                        click_square(view, local, layout, square, Some(*position))
                    }
                    PointerPhase::Up => None,
                }
            }
            InputEvent::Key { key, pressed: true } => {
                let layout = BoardLayout::from_viewport(local.viewport);
                match key {
                    Key::Escape => {
                        local.clear_interaction();
                        None
                    }
                    Key::ArrowUp => {
                        move_cursor(local, 0, 1);
                        None
                    }
                    Key::ArrowDown => {
                        move_cursor(local, 0, -1);
                        None
                    }
                    Key::ArrowLeft => {
                        move_cursor(local, -1, 0);
                        None
                    }
                    Key::ArrowRight => {
                        move_cursor(local, 1, 0);
                        None
                    }
                    Key::Enter | Key::Space => {
                        if let Interaction::Promotion { from, to } = local.interaction {
                            local.interaction = Interaction::Idle;
                            Some(Intent::new(Command::Move {
                                from: from.0,
                                to: to.0,
                                promotion: Some(PROMOTION_KINDS[0]),
                            }))
                        } else {
                            click_square(view, local, layout, Some(local.cursor), None)
                        }
                    }
                    Key::Tab => None,
                }
            }
            InputEvent::Key { pressed: false, .. } | InputEvent::Focus(_) => None,
        }
    }

    fn a11y(view: &View) -> A11yDescription {
        chess_a11y(view)
    }
}

#[allow(clippy::float_arithmetic)]
fn click_square(
    view: &View,
    local: &mut ChessLocal,
    layout: BoardLayout,
    square: Option<Square>,
    pointer: Option<PointerPosition>,
) -> Option<Intent<Command>> {
    if let Interaction::Promotion { from, to } = local.interaction {
        let selected_promotion = pointer.and_then(|position| {
            PROMOTION_KINDS
                .iter()
                .copied()
                .enumerate()
                .find_map(|(index, kind)| {
                    let choice = promotion_choice_rect(layout, index)?;
                    choice.contains(position.get()).then_some(kind)
                })
        });
        if let Some(promotion) = selected_promotion {
            local.clear_interaction();
            return Some(Intent::new(Command::Move {
                from: from.0,
                to: to.0,
                promotion: Some(promotion),
            }));
        }
        local.clear_interaction();
        return None;
    }

    let Some(square) = square else {
        local.clear_interaction();
        return None;
    };
    if !matches!(view.status, Status::Playing) || view.you != Some(view.turn) {
        local.clear_interaction();
        return None;
    }

    let own_piece = |candidate: Square| {
        view.board
            .get(usize::from(candidate.0))
            .and_then(|piece| *piece)
            .is_some_and(|piece| Some(piece.color) == view.you)
    };

    match local.interaction {
        Interaction::Idle => {
            if own_piece(square) {
                local.interaction = Interaction::Selected { square };
            }
            None
        }
        Interaction::Selected { square: from } => {
            if square == from {
                local.clear_interaction();
                return None;
            }
            if own_piece(square) {
                local.interaction = Interaction::Selected { square };
                return None;
            }
            if has_promotion_command(view, from, square) {
                local.interaction = Interaction::Promotion { from, to: square };
                return None;
            }
            local.clear_interaction();
            Some(Intent::new(Command::Move {
                from: from.0,
                to: square.0,
                promotion: None,
            }))
        }
        Interaction::Promotion { .. } => None,
    }
}

fn has_promotion_command(view: &View, from: Square, to: Square) -> bool {
    view.legal_moves.iter().any(|command| {
        matches!(
            command,
            Command::Move {
                from: command_from,
                to: command_to,
                promotion: Some(_),
            } if *command_from == from.0 && *command_to == to.0
        )
    })
}

fn move_cursor(local: &mut ChessLocal, file_delta: i8, rank_delta: i8) {
    let file = u8::try_from((i16::from(local.cursor.file()) + i16::from(file_delta)).clamp(0, 7))
        .expect("clamped cursor file fits u8");
    let rank = u8::try_from((i16::from(local.cursor.rank()) + i16::from(rank_delta)).clamp(0, 7))
        .expect("clamped cursor rank fits u8");
    local.cursor = Square::new(file + rank * 8).expect("clamped cursor is a board square");
}

#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
fn promotion_choice_rect(layout: BoardLayout, index: usize) -> Option<Rect> {
    if index >= PROMOTION_KINDS.len() || layout.square_size() <= 0.0 {
        return None;
    }
    let button_size = layout.square_size() * 0.9;
    let gap = layout.square_size() * 0.1;
    let total_width = button_size * 4.0 + gap * 3.0;
    let origin = layout.board().origin()
        + (layout.board().size() - Vec2::new(total_width, button_size)) * 0.5
        + Vec2::new((button_size + gap) * index as f32, 0.0);
    Rect::new(origin, Vec2::splat(button_size)).ok()
}

#[allow(clippy::float_arithmetic)]
fn promotion_panel_rect(layout: BoardLayout) -> Option<Rect> {
    let first = promotion_choice_rect(layout, 0)?;
    let last = promotion_choice_rect(layout, 3)?;
    let gap = layout.square_size() * 0.1;
    let origin = first.origin() - Vec2::splat(gap);
    let far_corner = last.origin() + last.size() + Vec2::splat(gap);
    Rect::new(origin, far_corner - origin).ok()
}

#[cfg(test)]
fn clicked_center(rect: Rect) -> PointerPosition {
    PointerPosition::new(rect.origin() + rect.size() * 0.5)
        .expect("a validated rectangle has a finite center")
}

#[allow(
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::too_many_lines
)]
fn build_render_list(
    view: &View,
    local: &ChessLocal,
    frame: &FrameCtx,
) -> Result<RenderList, RenderListError> {
    let theme = frame.theme();
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut builder = RenderListBuilder::new(Camera2D::default());

    for row in 0..8_u8 {
        for file in 0..8_u8 {
            let square =
                Square::new(file + (7 - row) * 8).ok_or(RenderListError::InvalidGeometry)?;
            let rect = layout
                .square_rect(square)
                .ok_or(RenderListError::InvalidGeometry)?;
            let light_square = (usize::from(square.file()) + usize::from(square.rank())) % 2 == 1;
            builder.push(RenderCmd::Rect {
                rect,
                radii: Corners::uniform(0.0)?,
                fill: Some(Paint::Solid(if light_square {
                    theme.color.surface_container
                } else {
                    theme.color.surface_container_high
                })),
                border: None,
                layer: Layer::BOARD,
                z: i16::from(square.0),
            })?;
        }
    }

    for square in 0..64_u8 {
        let square = Square::new(square).ok_or(RenderListError::InvalidGeometry)?;
        let is_selected = matches!(
            local.interaction,
            Interaction::Selected { square: selected } if selected == square
        );
        let is_last_move = local
            .last_move
            .is_some_and(|(from, to)| from == square || to == square);
        let is_legal_destination = matches!(
            local.interaction,
            Interaction::Selected { square: from } if legal_destination(view, from, square)
        );
        let rect = layout
            .square_rect(square)
            .ok_or(RenderListError::InvalidGeometry)?;

        if is_last_move {
            builder.push(outline(
                rect,
                theme.color.last_action,
                Layer::OVERLAY,
                0,
                &theme,
            )?)?;
        }
        if is_legal_destination {
            let marker_size = Vec2::splat(layout.square_size() * 0.22);
            builder.push(RenderCmd::Rect {
                rect: Rect::new(
                    rect.origin() + (rect.size() - marker_size) * 0.5,
                    marker_size,
                )?,
                radii: Corners::uniform(marker_size.x * 0.5)?,
                fill: Some(Paint::Solid(theme.color.legal_target)),
                border: None,
                layer: Layer::OVERLAY,
                z: i16::from(square.0),
            })?;
        }
        if is_selected {
            builder.push(outline(
                rect,
                theme.color.selected,
                Layer::OVERLAY,
                100,
                &theme,
            )?)?;
        }
    }

    for (index, piece) in view.board.iter().enumerate() {
        let Some(piece) = piece else {
            continue;
        };
        let square =
            Square::new(u8::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?)
                .ok_or(RenderListError::InvalidGeometry)?;
        let rect = layout
            .square_rect(square)
            .ok_or(RenderListError::InvalidGeometry)?;
        let style = TextStyleToken::DisplaySm;
        let line_height = theme.text_style(style).line_height().get();
        builder.push(RenderCmd::Text {
            text: piece_glyph(*piece).to_owned(),
            at: rect.origin()
                + Vec2::new(rect.size().x * 0.5, rect.size().y * 0.5 - line_height * 0.5),
            style,
            align: Align::Center,
            max_width: None,
            color: piece_color(*piece, &theme),
            layer: Layer::PIECES,
            z: i16::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?,
        })?;
    }

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

    if matches!(local.interaction, Interaction::Promotion { .. }) {
        let promotion_color = view.turn;
        if let Some(panel) = promotion_panel_rect(layout) {
            builder.push(RenderCmd::Rect {
                rect: panel,
                radii: Corners::uniform(theme.shape.md.get())?,
                fill: Some(Paint::Solid(theme.color.surface_container)),
                border: Some(Border::new(
                    theme.focus.ring_width.get(),
                    theme.color.outline,
                )?),
                layer: Layer::MODAL,
                z: 0,
            })?;
        }
        for (index, kind) in PROMOTION_KINDS.iter().copied().enumerate() {
            let Some(rect) = promotion_choice_rect(layout, index) else {
                continue;
            };
            builder.push(RenderCmd::Rect {
                rect,
                radii: Corners::uniform(theme.shape.sm.get())?,
                fill: Some(Paint::Solid(theme.color.surface_container_high)),
                border: Some(Border::new(
                    theme.focus.ring_width.get(),
                    theme.color.primary,
                )?),
                layer: Layer::MODAL,
                z: i16::try_from(index + 1).map_err(|_| RenderListError::InvalidGeometry)?,
            })?;
            builder.push(RenderCmd::Text {
                text: piece_glyph(Piece {
                    color: promotion_color,
                    kind,
                })
                .to_owned(),
                at: rect.origin()
                    + Vec2::new(
                        rect.size().x * 0.5,
                        rect.size().y * 0.5
                            - theme
                                .text_style(TextStyleToken::TitleLg)
                                .line_height()
                                .get()
                                * 0.5,
                    ),
                style: TextStyleToken::TitleLg,
                align: Align::Center,
                max_width: None,
                color: theme.color.on_surface,
                layer: Layer::MODAL,
                z: i16::try_from(index + 5).map_err(|_| RenderListError::InvalidGeometry)?,
            })?;
        }
    }

    builder.finish()
}

fn outline(
    rect: Rect,
    color: SemanticTint,
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

fn legal_destination(view: &View, from: Square, to: Square) -> bool {
    view.legal_moves.iter().any(|command| {
        matches!(
            command,
            Command::Move {
                from: command_from,
                to: command_to,
                ..
            } if *command_from == from.0 && *command_to == to.0
        )
    })
}

fn status_text(view: &View) -> String {
    match &view.status {
        Status::Playing => {
            if view.you == Some(view.turn) {
                format!("Your turn — {}", color_name(view.turn))
            } else {
                format!("{} to move", color_name(view.turn))
            }
        }
        Status::Ended { outcome } => format!("Game over — {}", outcome.summary()),
    }
}

fn chess_a11y(view: &View) -> A11yDescription {
    let items = view
        .board
        .iter()
        .enumerate()
        .filter_map(|(index, piece)| {
            let square = Square::new(u8::try_from(index).ok()?)?;
            let label = piece.map_or_else(
                || String::from("Empty square"),
                |piece| format!("{} {}", color_name(piece.color), piece_name(piece.kind)),
            );
            let state = if piece.is_some() {
                String::from("occupied")
            } else {
                String::from("empty")
            };
            let activates = piece
                .filter(|piece| view.you == Some(piece.color) && view.you == Some(view.turn))
                .map(|_| ActionId(String::from("move-square")));
            Some(A11yItem {
                label,
                position: square_name(square),
                state,
                activates,
            })
        })
        .collect();

    A11yDescription {
        status: status_text(view),
        regions: vec![A11yRegion {
            label: String::from("Chess board"),
            items,
        }],
        actions: vec![A11yAction {
            id: ActionId(String::from("move-square")),
            label: String::from("Select a piece and move it"),
            enabled: matches!(view.status, Status::Playing) && view.you == Some(view.turn),
        }],
    }
}

fn piece_glyph(piece: Piece) -> &'static str {
    match (piece.color, piece.kind) {
        (ChessColor::White, PieceKind::Pawn) => "P",
        (ChessColor::White, PieceKind::Knight) => "N",
        (ChessColor::White, PieceKind::Bishop) => "B",
        (ChessColor::White, PieceKind::Rook) => "R",
        (ChessColor::White, PieceKind::Queen) => "Q",
        (ChessColor::White, PieceKind::King) => "K",
        (ChessColor::Black, PieceKind::Pawn) => "p",
        (ChessColor::Black, PieceKind::Knight) => "n",
        (ChessColor::Black, PieceKind::Bishop) => "b",
        (ChessColor::Black, PieceKind::Rook) => "r",
        (ChessColor::Black, PieceKind::Queen) => "q",
        (ChessColor::Black, PieceKind::King) => "k",
    }
}

fn piece_color(piece: Piece, theme: &Theme) -> SemanticTint {
    match piece.color {
        ChessColor::White => theme.color.on_surface,
        ChessColor::Black => theme.color.on_surface_variant,
    }
}

fn color_name(color: ChessColor) -> &'static str {
    match color {
        ChessColor::White => "White",
        ChessColor::Black => "Black",
    }
}

fn piece_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pawn",
        PieceKind::Knight => "knight",
        PieceKind::Bishop => "bishop",
        PieceKind::Rook => "rook",
        PieceKind::Queen => "queen",
        PieceKind::King => "king",
    }
}

fn square_name(square: Square) -> String {
    let file = char::from(b'a' + square.file());
    format!("{file}{}", square.rank() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{
        canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, SeatId, Viewer,
    };
    use tabula_game_api::{Budget, Ctx, Input, Outcome};

    fn viewport(width: f32, height: f32) -> Viewport {
        Viewport::new(Vec2::new(width, height)).expect("test viewport is valid")
    }

    fn frame(width: f32, height: f32) -> FrameCtx {
        FrameCtx::new(
            viewport(width, height),
            tabula_presentation::Dpi::new(1.0).expect("test DPI is valid"),
            0,
            Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    fn view(state: &crate::State) -> View {
        ChessRules::project(state, Viewer::Seat(SeatId(0)))
    }

    fn pointer(layout: BoardLayout, square: Square) -> PointerPosition {
        clicked_center(layout.square_rect(square).expect("test square is valid"))
    }

    fn click(
        view: &View,
        local: &mut ChessLocal,
        layout: BoardLayout,
        square: Square,
    ) -> Option<Intent<Command>> {
        click_at(view, local, viewport(640.0, 640.0), layout, square)
    }

    fn click_at(
        view: &View,
        local: &mut ChessLocal,
        viewport: Viewport,
        layout: BoardLayout,
        square: Square,
    ) -> Option<Intent<Command>> {
        let event = InputEvent::Pointer {
            position: pointer(layout, square),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        local.set_viewport(viewport);
        ChessPresentation::on_input(&event, view, local)
    }

    fn legal_apply(
        state: &mut crate::State,
        seat: u8,
        index: u64,
        command: Command,
    ) -> Outcome<ChessRules> {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([9; 32]), InputIndex(index));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(index),
            rng: &mut rng,
            budget: Budget::default(),
        };
        ChessRules::apply(
            state,
            Input::Player {
                seat: SeatId(seat),
                command,
            },
            &mut ctx,
        )
        .expect("test command is legal")
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn board_is_64_square_square_and_fits_both_viewport_orientations() {
        for (width, height) in [(320.0, 640.0), (640.0, 320.0)] {
            let layout = BoardLayout::from_viewport(viewport(width, height));
            let board = layout.board();
            let status = layout.status();
            assert_eq!(board.size().x, board.size().y);
            assert!(board.origin().x >= 0.0 && board.origin().y >= 0.0);
            assert!(board.origin().x + board.size().x <= width);
            assert!(board.origin().y + board.size().y <= height);
            assert!(board.origin().y >= status.origin().y + status.size().y);
            assert_eq!(
                (0..64)
                    .filter_map(Square::new)
                    .filter_map(|square| layout.square_rect(square))
                    .count(),
                64
            );
        }
    }

    #[test]
    fn square_mapping_round_trips_centers_and_rejects_edges_outside_the_board() {
        let layout = BoardLayout::from_viewport(viewport(800.0, 500.0));
        for value in 0..64 {
            let square = Square::new(value).unwrap();
            assert_eq!(layout.square_at(pointer(layout, square)), Some(square));
        }
        let board = layout.board();
        assert_eq!(
            layout.square_at(PointerPosition::new(board.origin() - Vec2::ONE).unwrap()),
            None
        );
        assert_eq!(
            layout.square_at(
                PointerPosition::new(board.origin() + Vec2::new(board.size().x, 1.0)).unwrap()
            ),
            None
        );
        assert_eq!(
            layout.square_at(
                PointerPosition::new(board.origin() + Vec2::new(1.0, board.size().y)).unwrap()
            ),
            None
        );
    }

    #[test]
    fn resizing_between_clicks_keeps_hit_testing_on_the_current_board() {
        let state = crate::State::initial();
        let view = view(&state);
        let first_viewport = viewport(800.0, 500.0);
        let first_layout = BoardLayout::from_viewport(first_viewport);
        let resized_viewport = viewport(320.0, 640.0);
        let resized_layout = BoardLayout::from_viewport(resized_viewport);
        let mut local = ChessLocal::default();

        assert!(click_at(
            &view,
            &mut local,
            first_viewport,
            first_layout,
            Square::new(12).unwrap()
        )
        .is_none());
        let intent = click_at(
            &view,
            &mut local,
            resized_viewport,
            resized_layout,
            Square::new(28).unwrap(),
        )
        .expect("the resized destination still resolves to e4");
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None
            }
        );
    }

    #[test]
    fn initial_position_renders_all_board_squares_and_pieces_deterministically() {
        let state = crate::State::initial();
        let view = view(&state);
        let local = ChessLocal::default();
        let first = ChessPresentation::present(&view, &local, &frame(800.0, 500.0));
        let second = ChessPresentation::present(&view, &local, &frame(800.0, 500.0));
        assert_eq!(first, second);
        assert_eq!(
            first
                .commands()
                .iter()
                .filter(|command| matches!(
                    command,
                    RenderCmd::Rect {
                        layer: Layer::BOARD,
                        ..
                    }
                ))
                .count(),
            64
        );
        assert_eq!(
            first
                .commands()
                .iter()
                .filter(|command| matches!(
                    command,
                    RenderCmd::Text {
                        layer: Layer::PIECES,
                        ..
                    }
                ))
                .count(),
            32
        );
    }

    #[test]
    fn pointer_selection_is_local_and_valid_destination_emits_one_command() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();

        assert!(click(&view, &mut local, layout, Square::new(20).unwrap()).is_none());
        assert_eq!(local.interaction(), Interaction::Idle);
        assert!(click(&view, &mut local, layout, Square::new(52).unwrap()).is_none());
        assert_eq!(local.interaction(), Interaction::Idle);

        let before = canonical_encode(&state).unwrap();
        assert!(click(&view, &mut local, layout, Square::new(12).unwrap()).is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );
        assert_eq!(canonical_encode(&state).unwrap(), before);

        assert!(click(&view, &mut local, layout, Square::new(14).unwrap()).is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(14) }
        );
        assert!(click(&view, &mut local, layout, Square::new(14).unwrap()).is_none());
        assert_eq!(local.interaction(), Interaction::Idle);

        assert!(click(&view, &mut local, layout, Square::new(12).unwrap()).is_none());
        let intent = click(&view, &mut local, layout, Square::new(28).unwrap())
            .expect("a selected source and destination produce one intent");
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None
            }
        );
        assert_eq!(local.interaction(), Interaction::Idle);

        let cancel = InputEvent::Pointer {
            position: pointer(layout, Square::new(12).unwrap()),
            button: PointerButton::Primary,
            phase: PointerPhase::Cancel,
        };
        local.set_viewport(viewport(640.0, 640.0));
        assert!(ChessPresentation::on_input(&cancel, &view, &mut local).is_none());
        assert_eq!(local.interaction(), Interaction::Idle);
    }

    #[test]
    fn promotion_choice_comes_from_the_canonical_legal_move_projection() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        assert!(click(&view, &mut local, layout, Square::new(52).unwrap()).is_none());
        assert!(click(&view, &mut local, layout, Square::new(60).unwrap()).is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Promotion {
                from: Square(52),
                to: Square(60)
            }
        );
        let choice = promotion_choice_rect(layout, 0).unwrap();
        let event = InputEvent::Pointer {
            position: clicked_center(choice),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        local.set_viewport(viewport(640.0, 640.0));
        let intent = ChessPresentation::on_input(&event, &view, &mut local)
            .expect("promotion selection emits one command");
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 52,
                to: 60,
                promotion: Some(PieceKind::Queen)
            }
        );
    }

    #[test]
    fn black_promotion_chooser_uses_black_piece_glyphs() {
        let state = crate::State::from_fen("7k/8/8/8/8/8/4p3/K7 b - - 0 1").unwrap();
        let view = ChessRules::project(&state, Viewer::Seat(SeatId(1)));
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        assert!(click(&view, &mut local, layout, Square::new(12).unwrap()).is_none());
        assert!(click(&view, &mut local, layout, Square::new(4).unwrap()).is_none());
        assert!(matches!(local.interaction(), Interaction::Promotion { .. }));

        let rendered = ChessPresentation::present(&view, &local, &frame(640.0, 640.0));
        for glyph in ["q", "r", "b", "n"] {
            assert!(rendered.commands().iter().any(|command| matches!(
                command,
                RenderCmd::Text { text, layer: Layer::MODAL, .. } if text == glyph
            )));
        }
    }

    #[test]
    fn end_to_end_projection_presentation_intent_and_rules_apply() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let initial_view = view(&state);

        assert!(click(&initial_view, &mut local, layout, Square::new(12).unwrap()).is_none());
        let intent = click(&initial_view, &mut local, layout, Square::new(28).unwrap())
            .expect("e2-e4 is translated into an intent");
        let outcome = legal_apply(&mut state, 0, 0, intent.into_command());
        assert!(matches!(
            outcome.events.as_slice(),
            [crate::Event::Moved {
                from: Square(12),
                to: Square(28),
                ..
            }]
        ));

        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &frame(640.0, 640.0));
            }
        }
        let next_view = ChessRules::project(&state, Viewer::Seat(SeatId(1)));
        assert_eq!(next_view.board[12], None);
        assert_eq!(
            next_view.board[28],
            Some(Piece {
                color: ChessColor::White,
                kind: PieceKind::Pawn
            })
        );
        assert_eq!(local.last_move(), Some((Square(12), Square(28))));
    }

    #[test]
    fn terminal_projection_renders_result_and_disables_input() {
        let mut state = crate::State::initial();
        for (index, (seat, command)) in [
            (
                0,
                Command::Move {
                    from: 13,
                    to: 21,
                    promotion: None,
                },
            ),
            (
                1,
                Command::Move {
                    from: 52,
                    to: 36,
                    promotion: None,
                },
            ),
            (
                0,
                Command::Move {
                    from: 14,
                    to: 30,
                    promotion: None,
                },
            ),
            (
                1,
                Command::Move {
                    from: 59,
                    to: 31,
                    promotion: None,
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let _ = legal_apply(&mut state, seat, index as u64, command);
        }

        let view = view(&state);
        assert!(matches!(view.status, Status::Ended { .. }));
        let rendered =
            ChessPresentation::present(&view, &ChessLocal::default(), &frame(640.0, 640.0));
        assert!(rendered.commands().iter().any(|command| matches!(
            command,
            RenderCmd::Text { text, layer: Layer::HUD, .. } if text.starts_with("Game over")
        )));

        let mut local = ChessLocal::default();
        assert!(click(
            &view,
            &mut local,
            BoardLayout::from_viewport(viewport(640.0, 640.0)),
            Square::new(6).unwrap()
        )
        .is_none());
        assert_eq!(local.interaction(), Interaction::Idle);
    }

    #[test]
    fn a11y_describes_every_square_from_the_projection() {
        let state = crate::State::initial();
        let description = ChessPresentation::a11y(&view(&state));
        assert!(description.status.contains("White"));
        assert_eq!(description.regions[0].items.len(), 64);
        assert!(description.regions[0]
            .items
            .iter()
            .any(|item| item.position == "e2" && item.label == "White pawn"));
        assert!(description.actions[0].enabled);
    }
}
