//! Renderer-neutral Chess presentation. (doc 04 §5)
//!
//! The presenter consumes [`View`] and keeps only ephemeral interaction state. The
//! rules state remains behind the `GamePresentation` boundary: pointer and keyboard
//! input are translated into a [`Command`] and sent back to the shell as an [`Intent`].

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use tabula_design::{Color as SemanticTint, Theme};
use tabula_game_api::{A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId, GameRules};
use tabula_presentation::{
    handle_navigation, lerp_vec2, Align, AssetPackRef, Border, Camera2D, Corners, FocusGraph,
    FocusId, FocusModality, FocusNode, FocusState, FrameCtx, GamePresentation, InputEvent, Intent,
    Layer, MotionMode, MotionTimeline, NavigationAction, Paint, PointerButton, PointerPhase,
    PointerPosition, Rect, RenderCmd, RenderList, RenderListBuilder, RenderListError,
    TextStyleToken, Viewport,
};

use crate::{ChessRules, Color as ChessColor, Command, Piece, PieceKind, Square, Status, View};

const PROMOTION_CHOICES: [PromotionChoice; 4] = [
    PromotionChoice::Queen,
    PromotionChoice::Rook,
    PromotionChoice::Bishop,
    PromotionChoice::Knight,
];
const PROMOTION_BASE_FOCUS_ID: u32 = 100;
const STATUS_HEIGHT_FRACTION: f32 = 0.12;
const STATUS_MAX_HEIGHT: f32 = 48.0;
const IN_TRANSIT_PIECE_Z: i16 = 100;

/// The closed set of pieces a pawn may become at the end of a Chess move.
///
/// Keeping this separate from [`PieceKind`] makes Pawn and King unrepresentable
/// in the presentation-local promotion chooser.
///
/// @ai.role closed-domain
/// @ai.domain presentation.chess-promotion
/// @ai.invariant only-promotable-piece-kinds
/// @ai.evidence tests::promotion_choice_type_contains_exactly_the_four_upgrade_pieces
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromotionChoice {
    Queen,
    Rook,
    Bishop,
    Knight,
}

impl PromotionChoice {
    const fn piece_kind(self) -> PieceKind {
        match self {
            Self::Queen => PieceKind::Queen,
            Self::Rook => PieceKind::Rook,
            Self::Bishop => PieceKind::Bishop,
            Self::Knight => PieceKind::Knight,
        }
    }

    const fn action_id(self) -> &'static str {
        match self {
            Self::Queen => "promote-queen",
            Self::Rook => "promote-rook",
            Self::Bishop => "promote-bishop",
            Self::Knight => "promote-knight",
        }
    }
}

fn promotion_choice_focus_id(choice: PromotionChoice) -> FocusId {
    let index = match choice {
        PromotionChoice::Queen => 0,
        PromotionChoice::Rook => 1,
        PromotionChoice::Bishop => 2,
        PromotionChoice::Knight => 3,
    };
    FocusId::new(PROMOTION_BASE_FOCUS_ID + index)
}

fn focus_id_to_promotion_choice(id: FocusId) -> Option<PromotionChoice> {
    match id.get().checked_sub(PROMOTION_BASE_FOCUS_ID)? {
        0 => Some(PromotionChoice::Queen),
        1 => Some(PromotionChoice::Rook),
        2 => Some(PromotionChoice::Bishop),
        3 => Some(PromotionChoice::Knight),
        _ => None,
    }
}

/// Constructs the focus graph for all 64 board squares.
fn chess_board_focus_graph(layout: BoardLayout) -> FocusGraph {
    let mut nodes = Vec::with_capacity(64);
    for rank in 0..8_u8 {
        for file in 0..8_u8 {
            let square = Square::new(file + rank * 8).expect("valid board square");
            let id = FocusId::new(u32::from(square.0));
            let rect = layout.square_rect(square).expect("valid square geometry");
            let up = (rank < 7).then(|| FocusId::new(u32::from(square.0 + 8)));
            let down = (rank > 0).then(|| FocusId::new(u32::from(square.0 - 8)));
            let left = (file > 0).then(|| FocusId::new(u32::from(square.0 - 1)));
            let right = (file < 7).then(|| FocusId::new(u32::from(square.0 + 1)));
            nodes.push(FocusNode::with_neighbors(id, rect, up, down, left, right));
        }
    }
    FocusGraph::new(nodes).expect("board focus graph topology is valid")
}

/// Constructs the focus graph for the 4 horizontal promotion choices.
fn chess_promotion_focus_graph(layout: BoardLayout) -> FocusGraph {
    let mut nodes = Vec::with_capacity(4);
    for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
        let id = promotion_choice_focus_id(choice);
        let rect = promotion_choice_rect(layout, index).expect("valid choice geometry");
        let left = (index > 0).then(|| promotion_choice_focus_id(PROMOTION_CHOICES[index - 1]));
        let right = (index + 1 < PROMOTION_CHOICES.len())
            .then(|| promotion_choice_focus_id(PROMOTION_CHOICES[index + 1]));
        nodes.push(FocusNode::with_neighbors(id, rect, None, None, left, right));
    }
    FocusGraph::new(nodes).expect("promotion focus graph topology is valid")
}

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
        selected: PromotionChoice,
    },
}

/// Ephemeral animation description for a piece in transit following a [`crate::ViewEvent::Moved`].
///
/// This state is presentation-only: it does not affect rules, authority, or command formation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChessMoveAnimation {
    pub from: Square,
    pub to: Square,
    /// The mover's color is needed to preserve pawn identity during promotion playback.
    pub color: ChessColor,
    /// The canonical promotion choice, if this focal move promotes a pawn.
    pub promotion: Option<PieceKind>,
    pub timeline: MotionTimeline,
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
    focus: FocusState,
    move_animation: Option<ChessMoveAnimation>,
    viewport: Viewport,
}

impl Default for ChessLocal {
    fn default() -> Self {
        Self {
            interaction: Interaction::Idle,
            hover: None,
            last_move: None,
            focus: FocusState::new(Some(FocusId::new(0)), FocusModality::Pointer, true),
            move_animation: None,
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
    pub const fn focus(&self) -> &FocusState {
        &self.focus
    }

    pub fn focus_mut(&mut self) -> &mut FocusState {
        &mut self.focus
    }

    #[must_use]
    pub fn cursor(&self) -> Square {
        self.focus
            .current()
            .and_then(|id| u8::try_from(id.get()).ok())
            .and_then(Square::new)
            .unwrap_or(Square(0))
    }

    #[must_use]
    pub const fn move_animation(&self) -> Option<&ChessMoveAnimation> {
        self.move_animation.as_ref()
    }

    /// Records the current logical viewport for pointer hit testing.
    ///
    /// Viewport size is presentation-local input context, not game state. The
    /// client updates it before draining each frame, so a resize between two
    /// clicks uses the new board geometry.
    pub const fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    pub fn clear_interaction(&mut self) {
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
        frame: &FrameCtx,
    ) {
        match event {
            crate::ViewEvent::Moved {
                seat,
                from,
                to,
                promotion,
                ..
            } => {
                local.last_move = Some((*from, *to));
                local.clear_interaction();
                let timeline = MotionTimeline::from_profile(
                    frame.now_ms(),
                    frame.theme().motion.piece_move,
                    &frame.theme(),
                    MotionMode::Full,
                );
                if let Some(color) = ChessColor::from_seat(*seat) {
                    local.move_animation = Some(ChessMoveAnimation {
                        from: *from,
                        to: *to,
                        color,
                        promotion: *promotion,
                        timeline,
                    });
                } else {
                    local.move_animation = None;
                }
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
        let layout = BoardLayout::from_viewport(local.viewport);
        match input {
            InputEvent::Pointer {
                position,
                button,
                phase,
            } => match phase {
                PointerPhase::Move | PointerPhase::Down => {
                    local.hover = layout.square_at(*position);
                    if let Some(square) = local.hover {
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(square.0))));
                    }
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
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(square.0))));
                    }
                    click_square(view, local, layout, square, Some(*position))
                }
                PointerPhase::Up => None,
            },
            InputEvent::Key { .. } | InputEvent::Focus(_) => {
                let is_promotion = matches!(local.interaction, Interaction::Promotion { .. });
                let graph = if is_promotion {
                    chess_promotion_focus_graph(layout)
                } else {
                    chess_board_focus_graph(layout)
                };

                // Reconcile focus if active mode switched or current focus is invalid for this graph
                if local.focus.current().is_none()
                    || !graph.contains(local.focus.current().unwrap())
                {
                    let default_id =
                        if let Interaction::Promotion { selected, .. } = local.interaction {
                            promotion_choice_focus_id(selected)
                        } else {
                            graph.first_id().unwrap_or(FocusId::new(0))
                        };
                    local.focus.set_current(Some(default_id));
                }

                match handle_navigation(&graph, &mut local.focus, input) {
                    NavigationAction::None => None,
                    NavigationAction::Cancel => {
                        local.clear_interaction();
                        None
                    }
                    NavigationAction::FocusChanged(focus_id) => {
                        if let Interaction::Promotion { from, to, .. } = local.interaction {
                            if let Some(choice) = focus_id_to_promotion_choice(focus_id) {
                                local.interaction = Interaction::Promotion {
                                    from,
                                    to,
                                    selected: choice,
                                };
                            }
                        }
                        None
                    }
                    NavigationAction::Activate(focus_id) => {
                        if let Interaction::Promotion { from, to, selected } = local.interaction {
                            let choice = focus_id_to_promotion_choice(focus_id).unwrap_or(selected);
                            local.clear_interaction();
                            Some(promotion_intent(from, to, choice))
                        } else if let Some(square) =
                            u8::try_from(focus_id.get()).ok().and_then(Square::new)
                        {
                            click_square(view, local, layout, Some(square), None)
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }

    fn a11y(view: &View, local: &ChessLocal) -> A11yDescription {
        chess_a11y(view, local)
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
    if let Interaction::Promotion { from, to, .. } = local.interaction {
        let selected_promotion = pointer.and_then(|position| {
            PROMOTION_CHOICES
                .iter()
                .copied()
                .enumerate()
                .find_map(|(index, choice)| {
                    let rect = promotion_choice_rect(layout, index)?;
                    rect.contains(position.get()).then_some(choice)
                })
        });
        if let Some(promotion) = selected_promotion {
            local.clear_interaction();
            return Some(promotion_intent(from, to, promotion));
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
                local.interaction = Interaction::Promotion {
                    from,
                    to: square,
                    selected: PromotionChoice::Queen,
                };
                // Select the logical default without changing modality. Pointer activation must
                // remain pointer-modality; keyboard activation has already selected keyboard
                // modality in `handle_navigation`.
                local
                    .focus
                    .set_current(Some(promotion_choice_focus_id(PromotionChoice::Queen)));
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

fn promotion_intent(from: Square, to: Square, choice: PromotionChoice) -> Intent<Command> {
    Intent::new(Command::Move {
        from: from.0,
        to: to.0,
        promotion: Some(choice.piece_kind()),
    })
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

#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
fn promotion_choice_rect(layout: BoardLayout, index: usize) -> Option<Rect> {
    if index >= PROMOTION_CHOICES.len() || layout.square_size() <= 0.0 {
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

    let is_promotion = matches!(local.interaction, Interaction::Promotion { .. });

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
        let is_focused = local.focus.is_focus_visible()
            && !is_promotion
            && local.focus.current() == Some(FocusId::new(u32::from(square.0)));
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

    let move_sample = local
        .move_animation
        .as_ref()
        .map(|anim| (anim, anim.timeline.sample(frame.now_ms())));

    let in_transit_dest = move_sample.and_then(
        |(anim, sample)| {
            if sample.done {
                None
            } else {
                Some(anim.to)
            }
        },
    );

    for (index, piece) in view.board.iter().enumerate() {
        let Some(piece) = piece else {
            continue;
        };
        let square =
            Square::new(u8::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?)
                .ok_or(RenderListError::InvalidGeometry)?;
        if in_transit_dest == Some(square) {
            // Avoid double-drawing destination piece while animation is in flight
            continue;
        }
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

    if let Some((anim, sample)) = move_sample {
        if !sample.done {
            let piece = anim
                .promotion
                .map(|_| Piece {
                    color: anim.color,
                    kind: PieceKind::Pawn,
                })
                .or_else(|| view.board.get(usize::from(anim.to.0)).copied().flatten());
            if let Some(piece) = piece {
                let from_rect = layout
                    .square_rect(anim.from)
                    .ok_or(RenderListError::InvalidGeometry)?;
                let to_rect = layout
                    .square_rect(anim.to)
                    .ok_or(RenderListError::InvalidGeometry)?;
                let current_origin = lerp_vec2(from_rect.origin(), to_rect.origin(), sample.factor);
                let style = TextStyleToken::DisplaySm;
                let line_height = theme.text_style(style).line_height().get();
                builder.push(RenderCmd::Text {
                    text: piece_glyph(piece).to_owned(),
                    at: current_origin
                        + Vec2::new(
                            from_rect.size().x * 0.5,
                            from_rect.size().y * 0.5 - line_height * 0.5,
                        ),
                    style,
                    align: Align::Center,
                    max_width: None,
                    color: piece_color(piece, &theme),
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                })?;
            }
        }
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

    if is_promotion {
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
        let selected = match local.interaction {
            Interaction::Promotion { selected, .. } => selected,
            Interaction::Idle | Interaction::Selected { .. } => PromotionChoice::Queen,
        };
        for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
            let Some(rect) = promotion_choice_rect(layout, index) else {
                continue;
            };
            let is_selected = choice == selected;
            let is_focused = local.focus.is_focus_visible()
                && local.focus.current() == Some(promotion_choice_focus_id(choice));
            let border_color = if is_focused {
                theme.focus.ring_color
            } else if is_selected {
                theme.color.selected
            } else {
                theme.color.primary
            };
            builder.push(RenderCmd::Rect {
                rect,
                radii: Corners::uniform(theme.shape.sm.get())?,
                fill: Some(Paint::Solid(if is_selected {
                    theme.color.primary
                } else {
                    theme.color.surface_container_high
                })),
                border: Some(Border::new(theme.focus.ring_width.get(), border_color)?),
                layer: Layer::MODAL,
                z: i16::try_from(index + 1).map_err(|_| RenderListError::InvalidGeometry)?,
            })?;
            builder.push(RenderCmd::Text {
                text: piece_glyph(Piece {
                    color: promotion_color,
                    kind: choice.piece_kind(),
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
                color: if is_selected {
                    theme.color.on_primary
                } else {
                    theme.color.on_surface
                },
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

fn chess_a11y(view: &View, local: &ChessLocal) -> A11yDescription {
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

    let promotion_active = matches!(local.interaction, Interaction::Promotion { .. });
    let mut description = A11yDescription {
        status: status_text(view),
        regions: vec![A11yRegion {
            label: String::from("Chess board"),
            items,
        }],
        actions: vec![A11yAction {
            id: ActionId(String::from("move-square")),
            label: String::from("Select a piece and move it"),
            enabled: matches!(view.status, Status::Playing)
                && view.you == Some(view.turn)
                && !promotion_active,
        }],
    };

    if let Interaction::Promotion { selected, .. } = local.interaction {
        description.status = format!(
            "{} — choose promotion, {} selected",
            description.status,
            piece_name(selected.piece_kind())
        );
        description.regions.push(A11yRegion {
            label: String::from("Promotion choices"),
            items: PROMOTION_CHOICES
                .iter()
                .copied()
                .enumerate()
                .map(|(index, choice)| A11yItem {
                    label: format!("Promote to {}", piece_name(choice.piece_kind())),
                    position: format!("choice {}", index + 1),
                    state: if choice == selected {
                        String::from("selected")
                    } else {
                        String::from("available")
                    },
                    activates: Some(ActionId(String::from(choice.action_id()))),
                })
                .collect(),
        });
        description
            .actions
            .extend(PROMOTION_CHOICES.iter().copied().map(|choice| A11yAction {
                id: ActionId(String::from(choice.action_id())),
                label: format!("Promote to {}", piece_name(choice.piece_kind())),
                enabled: true,
            }));
    }

    description
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
    use tabula_presentation::Key;
    use tabula_testkit::assert_render_list_snapshot;

    fn viewport(width: f32, height: f32) -> Viewport {
        Viewport::new(Vec2::new(width, height)).expect("test viewport is valid")
    }

    fn frame(width: f32, height: f32) -> FrameCtx {
        frame_at(width, height, 0)
    }

    fn frame_at(width: f32, height: f32, now_ms: u64) -> FrameCtx {
        frame_with_theme(
            width,
            height,
            now_ms,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    fn frame_with_theme(width: f32, height: f32, now_ms: u64, theme: &Theme) -> FrameCtx {
        FrameCtx::new(
            viewport(width, height),
            tabula_presentation::Dpi::new(1.0).expect("test DPI is valid"),
            now_ms,
            *theme,
        )
    }

    fn view(state: &crate::State) -> View {
        ChessRules::project(state, Viewer::Seat(SeatId(0)))
    }

    fn pointer(layout: BoardLayout, square: Square) -> PointerPosition {
        clicked_center(layout.square_rect(square).expect("test square is valid"))
    }

    #[allow(clippy::float_arithmetic)]
    fn piece_position(layout: BoardLayout, square: Square) -> Vec2 {
        let rect = layout.square_rect(square).expect("test square is valid");
        let line_height = Theme::by_kind(tabula_design::ThemeKind::Light)
            .text_style(TextStyleToken::DisplaySm)
            .line_height()
            .get();
        rect.origin() + Vec2::new(rect.size().x * 0.5, rect.size().y * 0.5 - line_height * 0.5)
    }

    fn key(key: Key) -> InputEvent {
        InputEvent::Key { key, pressed: true }
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
                to: Square(60),
                selected: PromotionChoice::Queen,
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
    fn pointer_opened_promotion_keeps_pointer_modality_and_hides_focus_ring() {
        let (view, _layout, local) = promotion_fixture();
        assert_eq!(local.focus().modality(), FocusModality::Pointer);
        assert!(!local.focus().is_focus_visible());

        let mut theme = Theme::by_kind(tabula_design::ThemeKind::Light);
        theme.focus.ring_color = theme.color.danger;
        let rendered =
            ChessPresentation::present(&view, &local, &frame_with_theme(640.0, 640.0, 0, &theme));
        assert!(!rendered.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Rect {
                    layer: Layer::MODAL,
                    border: Some(border),
                    ..
                } if border.color() == theme.focus.ring_color
            )
        }));
    }

    #[test]
    fn keyboard_opened_promotion_uses_keyboard_modality_and_shows_focus_ring() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(52)));

        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(60)));
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());

        assert_eq!(
            local.interaction(),
            Interaction::Promotion {
                from: Square(52),
                to: Square(60),
                selected: PromotionChoice::Queen,
            }
        );
        assert_eq!(local.focus().modality(), FocusModality::Keyboard);
        assert!(local.focus().is_focus_visible());
        assert_eq!(
            local.focus().current(),
            Some(promotion_choice_focus_id(PromotionChoice::Queen))
        );

        let mut theme = Theme::by_kind(tabula_design::ThemeKind::Light);
        theme.focus.ring_color = theme.color.danger;
        let rendered =
            ChessPresentation::present(&view, &local, &frame_with_theme(640.0, 640.0, 0, &theme));
        assert!(rendered.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Rect {
                    layer: Layer::MODAL,
                    border: Some(border),
                    z: 1,
                    ..
                } if border.color() == theme.focus.ring_color
            )
        }));
    }

    fn promotion_fixture() -> (View, BoardLayout, ChessLocal) {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        assert!(click(&view, &mut local, layout, Square::new(52).unwrap()).is_none());
        assert!(click(&view, &mut local, layout, Square::new(60).unwrap()).is_none());
        (view, layout, local)
    }

    #[test]
    fn promotion_choice_type_contains_exactly_the_four_upgrade_pieces() {
        assert_eq!(
            PROMOTION_CHOICES.map(PromotionChoice::piece_kind),
            [
                PieceKind::Queen,
                PieceKind::Rook,
                PieceKind::Bishop,
                PieceKind::Knight,
            ]
        );
    }

    #[test]
    fn every_pointer_promotion_choice_emits_the_matching_command() {
        for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
            let (view, layout, mut local) = promotion_fixture();
            let event = InputEvent::Pointer {
                position: clicked_center(promotion_choice_rect(layout, index).unwrap()),
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            };
            local.set_viewport(viewport(640.0, 640.0));
            assert_eq!(
                ChessPresentation::on_input(&event, &view, &mut local)
                    .expect("pointer promotion choice emits a command")
                    .into_command(),
                Command::Move {
                    from: 52,
                    to: 60,
                    promotion: Some(choice.piece_kind()),
                }
            );
        }
    }

    #[test]
    fn keyboard_promotion_navigation_is_clamped_and_commits_the_selected_piece() {
        for (steps, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
            let (view, _layout, mut local) = promotion_fixture();
            for _ in 0..steps {
                assert!(
                    ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut local).is_none()
                );
            }
            assert_eq!(
                local.interaction(),
                Interaction::Promotion {
                    from: Square(52),
                    to: Square(60),
                    selected: choice,
                }
            );
            let commit_key = if steps % 2 == 0 {
                Key::Enter
            } else {
                Key::Space
            };
            assert_eq!(
                ChessPresentation::on_input(&key(commit_key), &view, &mut local)
                    .expect("keyboard promotion choice emits a command")
                    .into_command(),
                Command::Move {
                    from: 52,
                    to: 60,
                    promotion: Some(choice.piece_kind()),
                }
            );
            assert_eq!(local.interaction(), Interaction::Idle);
        }

        let (view, _layout, mut local) = promotion_fixture();
        for _ in 0..PROMOTION_CHOICES.len() {
            ChessPresentation::on_input(&key(Key::ArrowLeft), &view, &mut local);
        }
        assert_eq!(
            local.interaction(),
            Interaction::Promotion {
                from: Square(52),
                to: Square(60),
                selected: PromotionChoice::Queen,
            }
        );

        let (view, _layout, mut local) = promotion_fixture();
        for _ in 0..PROMOTION_CHOICES.len() {
            ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut local);
        }
        assert_eq!(
            local.interaction(),
            Interaction::Promotion {
                from: Square(52),
                to: Square(60),
                selected: PromotionChoice::Knight,
            }
        );
    }

    #[test]
    fn pointer_and_keyboard_promotion_selection_share_command_construction() {
        let (view, layout, mut pointer_local) = promotion_fixture();
        let pointer_event = InputEvent::Pointer {
            position: clicked_center(promotion_choice_rect(layout, 2).unwrap()),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        pointer_local.set_viewport(viewport(640.0, 640.0));
        let pointer_command =
            ChessPresentation::on_input(&pointer_event, &view, &mut pointer_local)
                .unwrap()
                .into_command();

        let (view, _layout, mut keyboard_local) = promotion_fixture();
        ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut keyboard_local);
        ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut keyboard_local);
        let keyboard_command =
            ChessPresentation::on_input(&key(Key::Enter), &view, &mut keyboard_local)
                .unwrap()
                .into_command();

        assert_eq!(pointer_command, keyboard_command);
    }

    #[test]
    fn promotion_animation_moves_a_pawn_and_finishes_as_the_promoted_piece() {
        let mut state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let mut local = ChessLocal::default();
        let start_frame = frame_at(640.0, 640.0, 1000);
        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 52,
                to: 60,
                promotion: Some(PieceKind::Queen),
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &start_frame);
            }
        }

        let animation = local
            .move_animation()
            .expect("promotion event creates a focal animation");
        assert_eq!(animation.promotion, Some(PieceKind::Queen));
        assert_eq!(animation.color, ChessColor::White);

        let current_view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let intermediate =
            ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, 1100));
        assert!(intermediate.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Text {
                    text,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if text == "P"
            )
        }));
        assert!(!intermediate.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Text {
                    text,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if text == "Q"
            )
        }));

        let terminal =
            ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, 1280));
        let destination_position = piece_position(layout, Square(60));
        assert!(terminal.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Text {
                    text,
                    at,
                    layer: Layer::PIECES,
                    ..
                } if text == "Q" && *at == destination_position
            )
        }));
    }

    #[test]
    fn escape_cancels_promotion_without_emitting_a_command() {
        let (view, _layout, mut local) = promotion_fixture();
        assert!(ChessPresentation::on_input(&key(Key::Escape), &view, &mut local).is_none());
        assert_eq!(local.interaction(), Interaction::Idle);
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
                RenderCmd::Text {
                    text,
                    layer: Layer::MODAL,
                    ..
                } if text == glyph
            )));
        }
    }

    #[test]
    fn chess_board_focus_uses_shared_directional_navigation() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        // Start focus at e4 (square 28: file 4, rank 3)
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(28)));

        // ArrowUp: rank 3 -> 4 => e5 (square 36)
        ChessPresentation::on_input(&key(Key::ArrowUp), &view, &mut local);
        assert_eq!(local.cursor(), Square(36));

        // ArrowDown: rank 4 -> 3 => e4 (square 28)
        ChessPresentation::on_input(&key(Key::ArrowDown), &view, &mut local);
        assert_eq!(local.cursor(), Square(28));

        // ArrowLeft: file 4 -> 3 => d4 (square 27)
        ChessPresentation::on_input(&key(Key::ArrowLeft), &view, &mut local);
        assert_eq!(local.cursor(), Square(27));

        // ArrowRight: file 3 -> 4 => e4 (square 28)
        ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut local);
        assert_eq!(local.cursor(), Square(28));
    }

    #[test]
    fn board_boundaries_clamp_directional_arrows() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        // a1 (square 0): Left and Down must stay at a1
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(0)));
        ChessPresentation::on_input(&key(Key::ArrowLeft), &view, &mut local);
        assert_eq!(local.cursor(), Square(0));
        ChessPresentation::on_input(&key(Key::ArrowDown), &view, &mut local);
        assert_eq!(local.cursor(), Square(0));

        // h8 (square 63): Right and Up must stay at h8
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(63)));
        ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut local);
        assert_eq!(local.cursor(), Square(63));
        ChessPresentation::on_input(&key(Key::ArrowUp), &view, &mut local);
        assert_eq!(local.cursor(), Square(63));
    }

    #[test]
    fn tab_navigation_traverses_all_board_squares() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        local.focus_mut().set_keyboard_focus(Some(FocusId::new(0)));
        for expected in 1..64_u8 {
            ChessPresentation::on_input(&key(Key::Tab), &view, &mut local);
            assert_eq!(local.cursor(), Square(expected));
        }
        // Cycles back to 0
        ChessPresentation::on_input(&key(Key::Tab), &view, &mut local);
        assert_eq!(local.cursor(), Square(0));
    }

    #[test]
    fn pointer_and_keyboard_activation_build_the_same_move() {
        let state = crate::State::initial();
        let view = view(&state);

        // Pointer path: click e2 (12), click e4 (28)
        let mut pointer_local = ChessLocal::default();
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        assert!(click(&view, &mut pointer_local, layout, Square(12)).is_none());
        let pointer_intent = click(&view, &mut pointer_local, layout, Square(28)).unwrap();

        // Keyboard path: focus e2 (12), Enter, focus e4 (28), Space
        let mut keyboard_local = ChessLocal::default();
        keyboard_local.set_viewport(viewport(640.0, 640.0));
        keyboard_local
            .focus_mut()
            .set_keyboard_focus(Some(FocusId::new(12)));
        assert!(
            ChessPresentation::on_input(&key(Key::Enter), &view, &mut keyboard_local).is_none()
        );
        assert_eq!(
            keyboard_local.interaction(),
            Interaction::Selected { square: Square(12) }
        );

        keyboard_local
            .focus_mut()
            .set_keyboard_focus(Some(FocusId::new(28)));
        let keyboard_intent =
            ChessPresentation::on_input(&key(Key::Space), &view, &mut keyboard_local).unwrap();

        assert_eq!(
            pointer_intent.into_command(),
            keyboard_intent.into_command()
        );
    }

    #[test]
    fn focus_is_local_and_never_mutates_authoritative_state() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let before = canonical_encode(&state).unwrap();
        for _ in 0..10 {
            ChessPresentation::on_input(&key(Key::ArrowRight), &view, &mut local);
            ChessPresentation::on_input(&key(Key::ArrowUp), &view, &mut local);
            ChessPresentation::on_input(&key(Key::Tab), &view, &mut local);
        }
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }

    #[test]
    fn keyboard_focus_renders_semantic_focus_ring() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        // When pointer modality is active, no focus ring
        let rendered_pointer = ChessPresentation::present(&view, &local, &frame(640.0, 640.0));
        let theme = Theme::by_kind(tabula_design::ThemeKind::Light);
        assert!(!rendered_pointer.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                border: Some(border),
                z: 150,
                ..
            } if border.color() == theme.focus.ring_color
        )));

        // Navigate with keyboard -> focus visible
        ChessPresentation::on_input(&key(Key::Tab), &view, &mut local);
        let rendered_kb = ChessPresentation::present(&view, &local, &frame(640.0, 640.0));
        assert!(rendered_kb.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                border: Some(border),
                z: 150,
                ..
            } if border.color() == theme.focus.ring_color
        )));
    }

    #[test]
    fn chess_move_animation_is_driven_by_view_event() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );

        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &frame);
            }
        }

        let anim = local
            .move_animation()
            .expect("Moved view event starts move animation");
        assert_eq!(anim.from, Square(12));
        assert_eq!(anim.to, Square(28));
        assert_eq!(anim.timeline.started_at_ms(), 1000);
    }

    #[test]
    fn chess_move_animation_has_a_real_in_flight_sample() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &frame);
            }
        }

        let animation = local
            .move_animation()
            .expect("a real move creates a focal animation");
        let sample = animation.timeline.sample(1100);
        assert!(!sample.done);
        assert!(sample.factor.is_finite());
        assert_eq!(
            animation.timeline.duration_ms(),
            u64::from(frame.theme().motion.piece_move.duration.milliseconds())
        );
    }

    #[test]
    fn chess_animation_never_mutates_authoritative_state() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &frame);
            }
        }

        let before = canonical_encode(&state).unwrap();
        for t in [1000, 1050, 1100, 1150, 1280, 1500] {
            let _ = ChessPresentation::present(&view(&state), &local, &frame_at(640.0, 640.0, t));
        }
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }

    #[test]
    fn chess_final_render_is_identical_after_sparse_or_dense_sampling() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let start_frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &start_frame);
            }
        }
        let current_view = view(&state);

        // Path A: sample final directly (terminal time >= 1280)
        let final_frame = frame_at(640.0, 640.0, 1280);
        let direct_render = ChessPresentation::present(&current_view, &local, &final_frame);

        // Path B: sample at intermediate frames first
        for t in [1016, 1032, 1048, 1064, 1080, 1150, 1200] {
            let _ = ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, t));
        }
        let sequential_render = ChessPresentation::present(&current_view, &local, &final_frame);

        assert_eq!(direct_render, sequential_render);
    }

    #[test]
    fn animation_does_not_gate_input_or_intent_creation() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &frame);
            }
        }
        let current_view = ChessRules::project(&state, Viewer::Seat(SeatId(1)));
        assert!(
            !local
                .move_animation()
                .expect("the first move is animating")
                .timeline
                .sample(1100)
                .done
        );

        // Black can form a valid intent while White's focal animation is in flight.
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        assert!(click(&current_view, &mut local, layout, Square(52)).is_none());
        let intent = click(&current_view, &mut local, layout, Square(36))
            .expect("active animation must not gate a valid second intent");
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 52,
                to: 36,
                promotion: None,
            }
        );
        assert_eq!(local.interaction(), Interaction::Idle);
    }

    #[test]
    fn piece_in_transit_is_not_double_rendered() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        let start_frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &start_frame);
            }
        }
        let current_view = view(&state);

        // Mid-animation frame at t=1100 (duration is 280ms, so 1100 is in flight)
        let mid_render =
            ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, 1100));
        let piece_texts = mid_render
            .commands()
            .iter()
            .filter(|cmd| {
                matches!(
                    cmd,
                    RenderCmd::Text {
                        layer: Layer::PIECES,
                        ..
                    }
                )
            })
            .count();

        // Exactly 32 pieces are rendered (31 stationary + 1 in transit).
        assert_eq!(piece_texts, 32);

        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let start_position = piece_position(layout, Square(12));
        let destination_position = piece_position(layout, Square(28));
        let focal_position = mid_render
            .commands()
            .iter()
            .find_map(|command| match command {
                RenderCmd::Text {
                    text,
                    at,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if text == "P" => Some(*at),
                _ => None,
            });
        let focal_position = focal_position.expect("one focal pawn is rendered in transit");
        assert_ne!(focal_position, start_position);
        assert_ne!(focal_position, destination_position);
        assert!(!mid_render.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Text {
                    text,
                    at,
                    layer: Layer::PIECES,
                    z: 28,
                    ..
                } if text == "P" && *at == destination_position
            )
        }));
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
            RenderCmd::Text {
                text,
                layer: Layer::HUD,
                ..
            } if text.starts_with("Game over")
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
        let description = ChessPresentation::a11y(&view(&state), &ChessLocal::default());
        assert!(description.status.contains("White"));
        assert_eq!(description.regions[0].items.len(), 64);
        assert!(description.regions[0]
            .items
            .iter()
            .any(|item| item.position == "e2" && item.label == "White pawn"));
        assert!(description.actions[0].enabled);
    }

    #[test]
    fn a11y_describes_the_active_promotion_selection() {
        let (view, _layout, local) = promotion_fixture();
        let description = ChessPresentation::a11y(&view, &local);
        assert!(description.status.contains("choose promotion"));
        let choices = &description.regions[1];
        assert_eq!(choices.label, "Promotion choices");
        assert_eq!(choices.items.len(), PROMOTION_CHOICES.len());
        assert_eq!(choices.items[0].state, "selected");
        assert!(choices.items[1..]
            .iter()
            .all(|item| item.state == "available"));
        assert_eq!(description.actions.len(), 1 + PROMOTION_CHOICES.len());
        assert!(description.actions[1..].iter().all(|action| action.enabled));
    }

    #[test]
    fn golden_chess_initial_640x640_light() {
        let state = crate::State::initial();
        let view = view(&state);
        let local = ChessLocal::default();
        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_initial_640x640_light", list);
    }

    #[test]
    fn golden_chess_initial_320x640_responsive() {
        let state = crate::State::initial();
        let view = view(&state);
        let local = ChessLocal::default();
        let frame = frame_with_theme(
            320.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_initial_320x640_responsive", list);
    }

    #[test]
    fn golden_chess_selected_focus_overlay_dark() {
        let state = crate::State::initial();
        let view = view(&state);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));

        // Select e2 (Square(12))
        assert!(click(&view, &mut local, layout, Square(12)).is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );

        // Focus on e4 (Square(28)) with keyboard modality
        local.focus_mut().set_keyboard_focus(Some(FocusId::new(28)));

        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Dark),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_selected_focus_overlay_dark", list);
    }

    #[test]
    fn golden_chess_move_animation_midflight() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));
        let start_frame = frame_at(640.0, 640.0, 1000);

        let outcome = legal_apply(
            &mut state,
            0,
            0,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        for event in outcome.events {
            if let Some(view_event) =
                ChessRules::view_event(&state, &event, Viewer::Seat(SeatId(0)))
            {
                ChessPresentation::on_view_event(&view_event, &mut local, &start_frame);
            }
        }

        let current_view = view(&state);
        let sample_frame = frame_at(640.0, 640.0, 1100);
        let list = ChessPresentation::present(&current_view, &local, &sample_frame);
        assert_render_list_snapshot!("chess_move_animation_midflight", list);
    }

    #[test]
    fn golden_chess_promotion_chooser_modal() {
        let (view, _layout, mut local) = promotion_fixture();
        local.set_viewport(viewport(640.0, 640.0));
        local
            .focus_mut()
            .set_keyboard_focus(Some(promotion_choice_focus_id(PromotionChoice::Queen)));

        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_promotion_chooser_modal", list);
    }

    #[test]
    fn golden_chess_terminal_checkmate_hud() {
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
        let local = ChessLocal::default();
        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_terminal_checkmate_hud", list);
    }
}
