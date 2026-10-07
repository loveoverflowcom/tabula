//! Renderer-neutral Chess presentation. (doc 04 §5)
//!
//! The presenter consumes [`View`] and keeps only ephemeral interaction state. The
//! rules state remains behind the `GamePresentation` boundary: pointer and keyboard
//! input are translated into a [`Command`] and sent back to the shell as an [`Intent`].

#![allow(clippy::doc_markdown)]

/// Bounded licensed Chessnut artwork, resolved through the renderer asset pack.
pub mod assets;

mod hud;
mod material;
mod motion;

pub use motion::ChessMoveAnimation;

use glam::Vec2;
use tabula_design::{Color as SemanticTint, Theme};
use tabula_game_api::{A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId, GameRules};
use tabula_presentation::{
    handle_navigation, ActionButton, Align, AssetPackRef, AudioCue, AudioCues, Border,
    ButtonInteraction, ButtonShape, ButtonTone, Camera2D, Corners, FocusGraph, FocusId,
    FocusModality, FocusNode, FocusState, FrameCtx, GamePresentation, InputEvent, Intent, Key,
    Layer, NavigationAction, Paint, PointerButton, PointerPhase, PointerPosition, Rect, RenderCmd,
    RenderList, RenderListBuilder, RenderListError, TextStyleToken, Viewport,
};

use crate::{
    ChessRules, ClockControl, Color as ChessColor, Command, Piece, PieceKind, Square, Status, View,
};

const PROMOTION_CHOICES: [PromotionChoice; 4] = [
    PromotionChoice::Queen,
    PromotionChoice::Rook,
    PromotionChoice::Bishop,
    PromotionChoice::Knight,
];
const PROMOTION_BASE_FOCUS_ID: u32 = 100;
const PROMOTION_CANCEL_FOCUS_ID: FocusId = FocusId::new(104);
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
            let increasing_rank = (rank < 7).then(|| FocusId::new(u32::from(square.0 + 8)));
            let decreasing_rank = (rank > 0).then(|| FocusId::new(u32::from(square.0 - 8)));
            let decreasing_file = (file > 0).then(|| FocusId::new(u32::from(square.0 - 1)));
            let increasing_file = (file < 7).then(|| FocusId::new(u32::from(square.0 + 1)));
            let (up, down, left, right) = if layout.flipped {
                (
                    decreasing_rank,
                    increasing_rank,
                    increasing_file,
                    decreasing_file,
                )
            } else {
                (
                    increasing_rank,
                    decreasing_rank,
                    decreasing_file,
                    increasing_file,
                )
            };
            nodes.push(FocusNode::with_neighbors(id, rect, up, down, left, right));
        }
    }
    FocusGraph::new(nodes).expect("board focus graph topology is valid")
}

/// Constructs the focus graph for the 4 horizontal promotion choices.
fn chess_promotion_focus_graph(layout: BoardLayout, target: Square) -> FocusGraph {
    let mut nodes = Vec::with_capacity(4);
    for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
        let id = promotion_choice_focus_id(choice);
        let Some(rect) = promotion_choice_rect(layout, target, index) else {
            continue;
        };
        let left = (index > 0).then(|| promotion_choice_focus_id(PROMOTION_CHOICES[index - 1]));
        let right = (index + 1 < PROMOTION_CHOICES.len())
            .then(|| promotion_choice_focus_id(PROMOTION_CHOICES[index + 1]));
        nodes.push(FocusNode::with_neighbors(id, rect, None, None, left, right));
    }
    if let Some(rect) = promotion_cancel_rect(layout, target) {
        nodes.push(FocusNode::new(PROMOTION_CANCEL_FOCUS_ID, rect));
    }
    FocusGraph::new(nodes).expect("promotion focus graph topology is valid")
}

/// The validated, responsive geometry shared by board rendering and hit testing.
///
/// The board uses the smaller remaining content axis, so its rectangle is always
/// square between two board-aligned player bars, beside a bounded status rail on
/// wide or short landscape layouts and above compact status/actions on portrait layouts. A `Square` is converted to a
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
    viewport: Viewport,
    board: Rect,
    status: Rect,
    square_size: f32,
    flipped: bool,
    top_player: Rect,
    bottom_player: Rect,
    controls: Rect,
    title: Rect,
    table: Rect,
    compact_controls: bool,
}

impl BoardLayout {
    /// Computes a square board that fits inside a finite validated viewport.
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn from_viewport(viewport: Viewport) -> Self {
        Self::oriented(viewport, false)
    }

    /// Reverses board geometry only; square identities and command ownership stay fixed.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::similar_names,
        clippy::too_many_lines
    )]
    pub fn oriented(viewport: Viewport, flipped: bool) -> Self {
        let size = viewport.size();
        let margin = (size.x * 0.02).min(size.y * 0.025).min(16.0);
        let gap = (size.y * 0.01).min(8.0);
        let title_h = (size.y * 0.05).min(32.0);
        let player_h = if size.y >= 450.0 {
            44.0
        } else if size.y >= 160.0 {
            24.0
        } else {
            (size.y * 0.075).min(24.0)
        };
        let rail = (size.x >= 760.0 && size.y >= 420.0) || (size.x >= 600.0 && size.y < 420.0);
        let rail_w = if rail { (size.x * 0.3).min(240.0) } else { 0.0 };
        let game_w = (size.x - margin * 2.0 - rail_w - if rail { gap * 2.0 } else { 0.0 }).max(0.0);
        let coordinate = (game_w * 0.025).min(12.0);
        let status_h = if rail {
            0.0
        } else {
            (size.y * 0.08).min(if size.y >= 700.0 { 64.0 } else { 48.0 })
        };
        let rail_toolbar = rail && size.y < 450.0;
        let usable_controls = size.y >= 200.0 && size.x >= 280.0;
        let mut controls_h = if usable_controls && !rail_toolbar {
            44.0
        } else {
            0.0
        };
        let fixed_height =
            margin * 2.0 + title_h + player_h * 2.0 + status_h + gap * 6.0 + coordinate * 2.0;
        let mut side = (game_w - coordinate * 2.0)
            .max(0.0)
            .min((size.y - fixed_height - controls_h).max(0.0))
            .min(640.0);
        let compact_actions = !rail_toolbar && (size.x < 760.0 || side < 464.0);
        // Reserve the maximum projected action combination, rather than resizing
        // the board when draw eligibility or the controlled seat changes.
        if usable_controls && !rail_toolbar && !compact_actions {
            let columns = ((side + 4.0) / 76.0).floor().max(1.0);
            controls_h = (6.0 / columns).ceil() * 48.0 - 4.0;
            side = (game_w - coordinate * 2.0)
                .max(0.0)
                .min((size.y - fixed_height - controls_h).max(0.0))
                .min(640.0);
        }
        let content_w = side + coordinate * 2.0 + if rail { rail_w + gap * 2.0 } else { 0.0 };
        let left = (size.x - content_w) * 0.5 + coordinate;
        let title = Rect::new(Vec2::new(left, margin), Vec2::new(side, title_h))
            .expect("validated viewport gives finite title geometry");
        let top_player = Rect::new(
            Vec2::new(left, margin + title_h + gap),
            Vec2::new(side, player_h),
        )
        .expect("validated viewport gives finite player geometry");
        let board_y = top_player.origin().y + player_h + gap + coordinate;
        let board = Rect::new(Vec2::new(left, board_y), Vec2::splat(side))
            .expect("validated viewport gives finite square geometry");
        let bottom_player = Rect::new(
            Vec2::new(left, board_y + side + coordinate + gap),
            Vec2::new(side, player_h),
        )
        .expect("validated viewport gives finite player geometry");
        let status_y = bottom_player.origin().y + player_h + gap;
        let status = if rail {
            Rect::new(
                Vec2::new(left + side + coordinate + gap * 2.0, top_player.origin().y),
                Vec2::new(
                    rail_w,
                    (size.y - margin - top_player.origin().y).clamp(0.0, 360.0),
                ),
            )
        } else {
            Rect::new(Vec2::new(left, status_y), Vec2::new(side, status_h))
        }
        .expect("validated viewport gives finite status geometry");
        let controls = if rail_toolbar {
            Rect::new(
                status.origin() + Vec2::new(0.0, 32.0),
                Vec2::new(status.size().x, (status.size().y - 32.0).max(0.0)),
            )
        } else {
            Rect::new(
                Vec2::new(
                    left,
                    if rail {
                        status_y
                    } else {
                        status_y + status_h + gap
                    },
                ),
                Vec2::new(side, controls_h),
            )
        }
        .expect("validated viewport gives finite controls geometry");
        let table = Rect::new(
            board.origin() - Vec2::splat(coordinate),
            board.size() + Vec2::splat(coordinate * 2.0),
        )
        .expect("validated viewport gives finite table geometry");
        Self {
            viewport,
            board,
            status,
            square_size: side / 8.0,
            flipped,
            top_player,
            bottom_player,
            controls,
            title,
            table,
            compact_controls: compact_actions,
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
        let row = if self.flipped {
            square.rank()
        } else {
            7 - square.rank()
        };
        let file = if self.flipped {
            7 - square.file()
        } else {
            square.file()
        };
        let origin = self.board.origin()
            + Vec2::new(
                f32::from(file) * self.square_size,
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
        if self.flipped {
            Square::new(7 - file + row * 8)
        } else {
            Square::new(file + (7 - row) * 8)
        }
    }

    /// The deterministic pointer distance required to transition from a pressed
    /// piece to an active drag.
    ///
    /// Deriving this threshold as a fraction of the responsive square size rather
    /// than fixed device pixels ensures that drag activation feels consistent across
    /// compact mobile viewports and large desktop screens without depending on OS
    /// gesture frameworks or platform APIs.
    ///
    /// @ai.role pure-calculation
    /// @ai.domain presentation.chess-layout
    /// @ai.pure true
    /// @ai.invariant deterministic-drag-threshold
    /// @ai.evidence tests::movement_below_drag_threshold_remains_tap_candidate
    /// @ai.evidence tests::movement_above_drag_threshold_enters_dragging
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn drag_threshold(self) -> f32 {
        self.square_size * 0.15
    }

    /// Checks whether the Euclidean distance between two pointer coordinates exceeds
    /// the drag activation threshold for this board layout.
    ///
    /// @ai.role pure-calculation
    /// @ai.domain presentation.chess-layout
    /// @ai.pure true
    /// @ai.invariant deterministic-drag-threshold-check
    /// @ai.evidence tests::movement_below_drag_threshold_remains_tap_candidate
    /// @ai.evidence tests::movement_above_drag_threshold_enters_dragging
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn exceeds_drag_threshold(self, start: PointerPosition, current: PointerPosition) -> bool {
        let delta = current.get() - start.get();
        let threshold = self.drag_threshold();
        delta.length_squared() >= threshold * threshold
    }
}

/// Mutually exclusive interaction modes owned by the client only.
///
/// @ai.role closed-domain
/// @ai.domain presentation.chess-interaction
/// @ai.invariant valid-drag-and-selection-state
/// @ai.evidence tests::pointer_selection_is_local_and_valid_destination_emits_one_command
/// @ai.evidence tests::pointer_down_on_movable_piece_enters_pressed_state
/// @ai.evidence tests::movement_above_drag_threshold_enters_dragging
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Interaction {
    #[default]
    Idle,
    Selected {
        square: Square,
    },
    Pressed {
        from: Square,
        down_at: PointerPosition,
        was_selected: bool,
    },
    Dragging {
        from: Square,
        pointer: PointerPosition,
        over: Option<Square>,
    },
    Promotion {
        from: Square,
        to: Square,
        selected: PromotionChoice,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ActivationKeys {
    enter_held: bool,
    space_held: bool,
    pointer_cancelled: bool,
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
    promotion_buttons: ButtonInteraction,
    promotion_observed: Option<hud::VisiblePosition>,
    move_animation: Option<ChessMoveAnimation>,
    viewport: Viewport,
    flipped: bool,
    hud_buttons: ButtonInteraction,
    confirmation: Option<hud::ControlConfirmation>,
    hot_seat_controls: bool,
    control_color: Option<ChessColor>,
    move_history: Vec<hud::ObservedMove>,
    activation_keys: ActivationKeys,
    reduced_motion: bool,
    hud_menu: hud::ActionMenu,
}

impl Default for ChessLocal {
    fn default() -> Self {
        Self {
            interaction: Interaction::Idle,
            hover: None,
            last_move: None,
            focus: FocusState::new(Some(FocusId::new(0)), FocusModality::Pointer, true),
            promotion_buttons: ButtonInteraction::default(),
            promotion_observed: None,
            move_animation: None,
            viewport: Viewport::new(Vec2::splat(1.0)).expect("unit viewport is valid"),
            flipped: false,
            hud_buttons: ButtonInteraction::default(),
            confirmation: None,
            hot_seat_controls: false,
            control_color: None,
            move_history: Vec::new(),
            activation_keys: ActivationKeys::default(),
            reduced_motion: false,
            hud_menu: hud::ActionMenu::Closed,
        }
    }
}

impl ChessLocal {
    /// Applies the shell's local motion preference without changing the projection.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
        if reduced {
            self.move_animation = None;
        }
    }

    /// Enables local seat selection only for the shell's admitted hot-seat session.
    pub fn set_hot_seat_controls(&mut self, enabled: bool) {
        self.hot_seat_controls = enabled;
        if !enabled {
            self.control_color = None;
        }
    }

    /// Local hot-seat viewing choice; a network client must never enable this control.
    #[must_use]
    pub fn viewer_override(&self) -> Option<tabula_core::Viewer> {
        if !self.hot_seat_controls {
            return None;
        }
        self.control_color
            .map(|color| tabula_core::Viewer::Seat(color.seat()))
    }

    /// Current local board orientation. This never changes the controlling seat.
    #[must_use]
    pub const fn is_flipped(&self) -> bool {
        self.flipped
    }

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
        self.hud_menu = hud::ActionMenu::Closed;
        self.interaction = Interaction::Idle;
        self.promotion_buttons = ButtonInteraction::default();
        self.promotion_observed = None;
    }
}

/// The standard Chess presenter for the Phase 2 local and future online clients.
#[derive(Debug, Default)]
pub struct ChessPresentation;

impl GamePresentation for ChessPresentation {
    type Rules = ChessRules;
    type Local = ChessLocal;

    fn asset_pack() -> AssetPackRef {
        assets::asset_pack()
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
    ) -> AudioCues {
        match event {
            crate::ViewEvent::Moved {
                seat,
                from,
                to,
                promotion,
                captured,
            } => {
                local.last_move = Some((*from, *to));
                if let Some(color) = ChessColor::from_seat(*seat) {
                    hud::record_move(local, color, *from, *to, *promotion, *captured);
                }
                local.control_color = None;
                local.confirmation = None;
                local.clear_interaction();
                local.move_animation = ChessColor::from_seat(*seat).and_then(|color| {
                    ChessMoveAnimation::start(
                        *from,
                        *to,
                        color,
                        *promotion,
                        *captured,
                        frame.now_ms(),
                        local,
                        frame,
                    )
                });
                one_cue(if captured.is_some() {
                    "capture"
                } else {
                    "move"
                })
            }
            crate::ViewEvent::Ended { .. } => {
                local.clear_interaction();
                local.confirmation = None;
                one_cue("game-end")
            }
            crate::ViewEvent::ClockUpdated { .. }
            | crate::ViewEvent::DrawOffered { .. }
            | crate::ViewEvent::DrawDeclined { .. } => AudioCues::new(),
        }
    }

    fn on_view_event_with_projection(
        event: &crate::ViewEvent,
        previous: Option<&View>,
        current: &View,
        local: &mut ChessLocal,
        frame: &FrameCtx,
    ) -> AudioCues {
        let cues = Self::on_view_event(event, local, frame);
        if matches!(event, crate::ViewEvent::Moved { .. }) {
            local.move_animation = local.move_animation.and_then(|animation| {
                previous.and_then(|prior| animation.with_projection(prior, current))
            });
        }
        cues
    }

    fn on_command_rejected(local: &mut ChessLocal) {
        local.move_animation = None;
        local.clear_interaction();
        local.confirmation = None;
    }

    #[allow(clippy::too_many_lines)]
    fn on_input(
        input: &InputEvent,
        view: &View,
        local: &mut ChessLocal,
    ) -> Option<Intent<Command>> {
        let layout = BoardLayout::oriented(local.viewport, local.flipped);
        if matches!(
            input,
            InputEvent::Key { pressed: true, .. }
                | InputEvent::Pointer {
                    phase: PointerPhase::Down | PointerPhase::Cancel,
                    ..
                }
                | InputEvent::Focus(false)
        ) {
            // New input uses the current projection immediately, even mid-composition.
            local.move_animation = None;
        }
        if matches!(input, InputEvent::Focus(false)) {
            local.hud_menu = hud::ActionMenu::Closed;
            if let Interaction::Pressed { from, .. } | Interaction::Dragging { from, .. } =
                local.interaction
            {
                local.interaction = Interaction::Selected { square: from };
            }
            local.hud_buttons = ButtonInteraction::default();
            local.promotion_buttons = ButtonInteraction::default();
            local.activation_keys.enter_held = false;
            local.activation_keys.space_held = false;
            local.activation_keys.pointer_cancelled = true;
        }
        if !matches!(input, InputEvent::Focus(_)) && !local.focus.is_window_focused() {
            return None;
        }
        if let InputEvent::Pointer { phase, .. } = input {
            if *phase == PointerPhase::Down {
                local.activation_keys.pointer_cancelled = false;
            }
            if *phase == PointerPhase::Up && local.activation_keys.pointer_cancelled {
                return None;
            }
        }
        if let InputEvent::Key { key, pressed } = input {
            if matches!(key, Key::Enter | Key::Space) {
                let held = if *key == Key::Enter {
                    &mut local.activation_keys.enter_held
                } else {
                    &mut local.activation_keys.space_held
                };
                let repeated = *held;
                *held = *pressed;
                if repeated && *pressed {
                    return None;
                }
            }
        }
        if let hud::HudInput::Handled(result) = hud::on_input(input, view, local, layout) {
            return result;
        }
        if let Interaction::Promotion { from, to, selected } = local.interaction {
            let graph = chess_promotion_focus_graph(layout, to);
            if !local.focus.current().is_some_and(|id| graph.contains(id)) {
                local
                    .focus
                    .set_current(Some(promotion_choice_focus_id(selected)));
            }
            let buttons = promotion_buttons(view, local, layout);
            let action =
                local
                    .promotion_buttons
                    .on_input(input, &buttons, &graph, &mut local.focus);
            return match action {
                NavigationAction::Activate(PROMOTION_CANCEL_FOCUS_ID)
                | NavigationAction::Cancel => {
                    local.clear_interaction();
                    local
                        .focus
                        .set_current(Some(FocusId::new(u32::from(from.0))));
                    None
                }
                NavigationAction::Activate(id) => {
                    let choice = focus_id_to_promotion_choice(id)?;
                    local.clear_interaction();
                    local.focus.set_current(Some(FocusId::new(u32::from(to.0))));
                    Some(promotion_intent(from, to, choice))
                }
                NavigationAction::FocusChanged(id) => {
                    if let Some(choice) = focus_id_to_promotion_choice(id) {
                        local.interaction = Interaction::Promotion {
                            from,
                            to,
                            selected: choice,
                        };
                    }
                    None
                }
                NavigationAction::None => None,
            };
        }
        match input {
            InputEvent::Pointer {
                position,
                button,
                phase,
            } => match phase {
                PointerPhase::Down => {
                    if *button == PointerButton::Primary
                        && matches!(
                            local.interaction,
                            Interaction::Pressed { .. } | Interaction::Dragging { .. }
                        )
                    {
                        return None;
                    }
                    local.hover = layout.square_at(*position);
                    if let Some(square) = local.hover {
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(square.0))));
                    }
                    if *button == PointerButton::Primary {
                        let is_promotion =
                            matches!(local.interaction, Interaction::Promotion { .. });
                        if !is_promotion
                            && matches!(view.status, Status::Playing)
                            && view.you == Some(view.turn)
                        {
                            if let Some(square) = local.hover {
                                let own_piece = view
                                    .board
                                    .get(usize::from(square.0))
                                    .and_then(|piece| *piece)
                                    .is_some_and(|piece| Some(piece.color) == view.you);
                                if own_piece {
                                    let was_selected = matches!(
                                        local.interaction,
                                        Interaction::Selected { square: prev } if prev == square
                                    );
                                    local.interaction = Interaction::Pressed {
                                        from: square,
                                        down_at: *position,
                                        was_selected,
                                    };
                                }
                            }
                        }
                    }
                    None
                }
                PointerPhase::Move => {
                    local.hover = layout.square_at(*position);
                    if let Some(square) = local.hover {
                        local
                            .focus
                            .set_pointer_focus(Some(FocusId::new(u32::from(square.0))));
                    }
                    match local.interaction {
                        Interaction::Pressed {
                            from,
                            down_at,
                            was_selected,
                        } => {
                            if layout.exceeds_drag_threshold(down_at, *position) {
                                local.interaction = Interaction::Dragging {
                                    from,
                                    pointer: *position,
                                    over: local.hover,
                                };
                            } else {
                                local.interaction = Interaction::Pressed {
                                    from,
                                    down_at,
                                    was_selected,
                                };
                            }
                        }
                        Interaction::Dragging { from, .. } => {
                            local.interaction = Interaction::Dragging {
                                from,
                                pointer: *position,
                                over: local.hover,
                            };
                        }
                        Interaction::Idle
                        | Interaction::Selected { .. }
                        | Interaction::Promotion { .. } => {}
                    }
                    None
                }
                PointerPhase::Cancel => {
                    local.activation_keys.pointer_cancelled = true;
                    match local.interaction {
                        Interaction::Dragging { from, .. } | Interaction::Pressed { from, .. } => {
                            local.interaction = Interaction::Selected { square: from };
                        }
                        Interaction::Idle
                        | Interaction::Selected { .. }
                        | Interaction::Promotion { .. } => {
                            local.clear_interaction();
                        }
                    }
                    local.hover = None;
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
                    match local.interaction {
                        Interaction::Dragging { from, .. } => {
                            if let Some(to) = square {
                                if has_promotion_command(view, from, to) {
                                    local.promotion_observed =
                                        Some(hud::VisiblePosition::new(view));
                                    local.interaction = Interaction::Promotion {
                                        from,
                                        to,
                                        selected: PromotionChoice::Queen,
                                    };
                                    local.focus.set_current(Some(promotion_choice_focus_id(
                                        PromotionChoice::Queen,
                                    )));
                                    None
                                } else if legal_destination(view, from, to) {
                                    local.clear_interaction();
                                    Some(move_intent(from, to))
                                } else {
                                    local.interaction = Interaction::Selected { square: from };
                                    None
                                }
                            } else {
                                local.interaction = Interaction::Selected { square: from };
                                None
                            }
                        }
                        Interaction::Pressed {
                            from, was_selected, ..
                        } => {
                            if square == Some(from) {
                                if was_selected {
                                    local.clear_interaction();
                                } else {
                                    local.interaction = Interaction::Selected { square: from };
                                }
                                None
                            } else if let Some(to) = square {
                                local.interaction = Interaction::Selected { square: from };
                                click_square(view, local, Some(to))
                            } else {
                                local.clear_interaction();
                                None
                            }
                        }
                        Interaction::Idle
                        | Interaction::Selected { .. }
                        | Interaction::Promotion { .. } => click_square(view, local, square),
                    }
                }
                PointerPhase::Up => None,
            },
            InputEvent::Key { .. } | InputEvent::Focus(_) => {
                let graph = chess_board_focus_graph(layout);
                if !local.focus.current().is_some_and(|id| graph.contains(id)) {
                    local.focus.set_current(graph.first_id());
                }

                let result = match handle_navigation(&graph, &mut local.focus, input) {
                    NavigationAction::None | NavigationAction::FocusChanged(_) => None,
                    NavigationAction::Cancel => {
                        local.clear_interaction();
                        None
                    }
                    NavigationAction::Activate(focus_id) => {
                        let square = u8::try_from(focus_id.get()).ok().and_then(Square::new);
                        click_square(view, local, square)
                    }
                };
                if matches!(local.interaction, Interaction::Promotion { .. }) {
                    if let InputEvent::Key { key, pressed: true } = input {
                        local
                            .promotion_buttons
                            .suppress_activation_until_release(*key);
                    }
                }
                result
            }
        }
    }

    fn a11y(view: &View, local: &ChessLocal) -> A11yDescription {
        chess_a11y(view, local)
    }
}

fn one_cue(id: &'static str) -> AudioCues {
    let mut cues = AudioCues::new();
    cues.push(AudioCue::from_static(id));
    cues
}

#[allow(clippy::float_arithmetic)]
fn click_square(
    view: &View,
    local: &mut ChessLocal,
    square: Option<Square>,
) -> Option<Intent<Command>> {
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
        Interaction::Selected { square: from }
        | Interaction::Pressed { from, .. }
        | Interaction::Dragging { from, .. } => {
            if square == from {
                local.clear_interaction();
                return None;
            }
            if own_piece(square) {
                local.interaction = Interaction::Selected { square };
                return None;
            }
            if has_promotion_command(view, from, square) {
                local.promotion_observed = Some(hud::VisiblePosition::new(view));
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
            Some(move_intent(from, square))
        }
        Interaction::Promotion { .. } => None,
    }
}

fn move_intent(from: Square, to: Square) -> Intent<Command> {
    Intent::new(Command::Move {
        from: from.0,
        to: to.0,
        promotion: None,
    })
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

/// Fixed hit geometry shared by promotion input, focus, and drawing.
/// The chooser opens inward from its destination, stays upright in both board
/// orientations, and retains a viewport inset on short/compact screens.
struct PromotionLayout {
    panel: Rect,
    choices: [Rect; 4],
    cancel: Rect,
}

impl PromotionLayout {
    #[allow(clippy::float_arithmetic)]
    fn new(layout: BoardLayout, target: Square) -> Option<Self> {
        // Spatial tokens are common to all schemes. Input geometry is independent
        // of scheme color, while retaining the authored accessibility metrics.
        let metrics = Theme::by_kind(tabula_design::ThemeKind::Light);
        let padding = f32::from(metrics.space.md);
        let gap = f32::from(metrics.space.xxs);
        let section_gap = f32::from(metrics.space.sm);
        let minimum = metrics.density.min_target.get().max(44.0);
        let viewport = layout.viewport.size();
        let heading_height = metrics
            .text_style(TextStyleToken::TitleMd)
            .line_height()
            .get();
        let button_size = (layout.square_size() * 0.9)
            .max(minimum + f32::from(metrics.space.lg))
            .min((viewport.x - padding * 4.0 - gap * 3.0) / 4.0)
            .min(viewport.y - padding * 4.0 - heading_height - section_gap * 2.0 - minimum);
        if button_size < minimum {
            return None;
        }
        let group_width = button_size * 4.0 + gap * 3.0;
        let panel_size = Vec2::new(
            group_width + padding * 2.0,
            padding * 2.0 + heading_height + section_gap * 2.0 + button_size + minimum,
        );
        let target = layout.square_rect(target)?;
        let center = target.origin() + target.size() * 0.5;
        let y = if center.y < layout.board.origin().y + layout.board.size().y * 0.5 {
            target.origin().y + target.size().y + section_gap
        } else {
            target.origin().y - panel_size.y - section_gap
        };
        let origin = Vec2::new(center.x - panel_size.x * 0.5, y)
            .max(Vec2::splat(padding))
            .min(viewport - panel_size - Vec2::splat(padding));
        let panel = Rect::new(origin, panel_size).ok()?;
        let first = panel.origin() + Vec2::new(padding, padding + heading_height + section_gap);
        let choice = |x| {
            Rect::new(
                first + Vec2::new(x * (button_size + gap), 0.0),
                Vec2::splat(button_size),
            )
            .ok()
        };
        let choices = [choice(0.0)?, choice(1.0)?, choice(2.0)?, choice(3.0)?];
        let cancel = Rect::new(
            first + Vec2::new(0.0, button_size + section_gap),
            Vec2::new(group_width, minimum),
        )
        .ok()?;
        Some(Self {
            panel,
            choices,
            cancel,
        })
    }
}

fn promotion_choice_rect(layout: BoardLayout, target: Square, index: usize) -> Option<Rect> {
    PromotionLayout::new(layout, target)?
        .choices
        .get(index)
        .copied()
}

fn promotion_panel_rect(layout: BoardLayout, target: Square) -> Option<Rect> {
    Some(PromotionLayout::new(layout, target)?.panel)
}

fn promotion_cancel_rect(layout: BoardLayout, target: Square) -> Option<Rect> {
    Some(PromotionLayout::new(layout, target)?.cancel)
}

fn promotion_choice_enabled(
    view: &View,
    local: &ChessLocal,
    from: Square,
    to: Square,
    choice: PromotionChoice,
) -> bool {
    matches!(view.status, Status::Playing)
        && local
            .promotion_observed
            .as_ref()
            .is_some_and(|observed| *observed == hud::VisiblePosition::new(view))
        && view.you == Some(view.turn)
        && view
            .legal_moves
            .contains(&promotion_intent(from, to, choice).into_command())
}

fn promotion_buttons(
    view: &View,
    local: &ChessLocal,
    layout: BoardLayout,
) -> Vec<ActionButton<'static>> {
    let metrics = Theme::by_kind(tabula_design::ThemeKind::Light);
    let Interaction::Promotion { from, to, selected } = local.interaction else {
        return Vec::new();
    };
    let mut buttons: Vec<_> = PROMOTION_CHOICES
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, choice)| {
            let rect = promotion_choice_rect(layout, to, index)?;
            let enabled = promotion_choice_enabled(view, local, from, to, choice);
            Some(
                ActionButton::new(
                    promotion_choice_focus_id(choice),
                    rect,
                    "",
                    metrics.density.min_target,
                )
                .ok()?
                .tone(if choice == selected {
                    ButtonTone::Filled
                } else {
                    ButtonTone::Tonal
                })
                .shape(match index {
                    0 => ButtonShape::ConnectedStart,
                    3 => ButtonShape::ConnectedEnd,
                    _ => ButtonShape::ConnectedMiddle,
                })
                .enabled(enabled),
            )
        })
        .collect();
    if let Some(rect) = promotion_cancel_rect(layout, to) {
        if let Ok(button) = ActionButton::new(
            PROMOTION_CANCEL_FOCUS_ID,
            rect,
            "Cancel",
            metrics.density.min_target,
        ) {
            buttons.push(button.shape(ButtonShape::Square));
        }
    }
    buttons
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
    let layout = BoardLayout::oriented(frame.viewport(), local.flipped);
    let mut builder = RenderListBuilder::new(Camera2D::default());
    hud::draw_material(&mut builder, frame, layout)?;
    material::draw_frame(&mut builder, frame, layout)?;

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
                    theme.game_art.chess.board_light
                } else {
                    theme.game_art.chess.board_dark
                })),
                border: None,
                layer: Layer::BOARD,
                z: i16::from(square.0),
            })?;
        }
    }

    material::draw_grain(&mut builder, frame, layout)?;

    let is_promotion = matches!(local.interaction, Interaction::Promotion { .. });

    for square in 0..64_u8 {
        let square = Square::new(square).ok_or(RenderListError::InvalidGeometry)?;
        let is_selected = matches!(
            local.interaction,
            Interaction::Selected { square: selected }
                | Interaction::Pressed { from: selected, .. }
                | Interaction::Dragging { from: selected, .. }
            if selected == square
        );
        let is_last_move = local
            .last_move
            .is_some_and(|(from, to)| from == square || to == square);
        let is_legal_destination = matches!(
            local.interaction,
            Interaction::Selected { square: from }
                | Interaction::Pressed { from, .. }
                | Interaction::Dragging { from, .. }
            if legal_destination(view, from, square)
        );
        let is_focused = local.focus.is_focus_visible()
            && !is_promotion
            && local.focus.current() == Some(FocusId::new(u32::from(square.0)));
        let rect = layout
            .square_rect(square)
            .ok_or(RenderListError::InvalidGeometry)?;

        if view.in_check
            && view.board[usize::from(square.0)]
                .is_some_and(|piece| piece.color == view.turn && piece.kind == PieceKind::King)
        {
            builder.push(contrast_plate(rect, &theme, Layer::OVERLAY, 124)?)?;
            builder.push(outline(
                rect,
                theme.color.danger,
                Layer::OVERLAY,
                125,
                &theme,
            )?)?;
            if rect.size().x >= 44.0 {
                builder.push(RenderCmd::Rect {
                    rect: Rect::new(
                        rect.origin(),
                        Vec2::new(rect.size().x, (rect.size().y * 0.3).min(18.0)),
                    )?,
                    radii: Corners::uniform(0.0)?,
                    fill: Some(Paint::Solid(theme.color.danger)),
                    border: None,
                    layer: Layer::OVERLAY,
                    z: 126,
                })?;
                builder.push(RenderCmd::Text {
                    text: "CHECK".into(),
                    at: rect.origin(),
                    style: TextStyleToken::LabelSm,
                    align: Align::Start,
                    max_width: None,
                    color: theme.color.on_danger,
                    layer: Layer::OVERLAY,
                    z: 127,
                })?;
            }
        }
        if is_last_move {
            builder.push(contrast_plate(rect, &theme, Layer::OVERLAY, -1)?)?;
            builder.push(outline(
                rect,
                theme.color.last_action,
                Layer::OVERLAY,
                0,
                &theme,
            )?)?;
        }
        if is_legal_destination {
            if view.board[usize::from(square.0)].is_some() {
                builder.push(contrast_plate(rect, &theme, Layer::OVERLAY, 60)?)?;
                builder.push(outline(
                    rect,
                    theme.color.legal_target,
                    Layer::OVERLAY,
                    61,
                    &theme,
                )?)?;
            } else {
                let marker_size = Vec2::splat(layout.square_size() * 0.22);
                for (size, color, z) in [
                    (marker_size + Vec2::splat(4.0), theme.color.surface, 60),
                    (marker_size, theme.color.legal_target, 61),
                ] {
                    builder.push(RenderCmd::Rect {
                        rect: Rect::new(rect.origin() + (rect.size() - size) * 0.5, size)?,
                        radii: Corners::uniform(size.x * 0.5)?,
                        fill: Some(Paint::Solid(color)),
                        border: None,
                        layer: Layer::OVERLAY,
                        z,
                    })?;
                }
            }
        }
        if is_selected {
            builder.push(contrast_plate(rect, &theme, Layer::OVERLAY, 99)?)?;
            builder.push(outline(
                rect,
                theme.color.selected,
                Layer::OVERLAY,
                100,
                &theme,
            )?)?;
        }
        if let Interaction::Dragging {
            from,
            over: Some(over_square),
            ..
        } = local.interaction
        {
            if over_square == square {
                if legal_destination(view, from, square)
                    || has_promotion_command(view, from, square)
                {
                    builder.push(outline(
                        rect,
                        theme.color.legal_target,
                        Layer::OVERLAY,
                        120,
                        &theme,
                    )?)?;
                } else if square != from {
                    builder.push(outline(
                        rect,
                        theme.color.illegal_target,
                        Layer::OVERLAY,
                        120,
                        &theme,
                    )?)?;
                }
            }
        }
        if is_focused {
            builder.push(contrast_plate(rect, &theme, Layer::OVERLAY, 149)?)?;
            builder.push(outline(
                rect,
                theme.focus.ring_color,
                Layer::OVERLAY,
                150,
                &theme,
            )?)?;
        }
    }

    motion::draw_pieces(&mut builder, view, local, frame, layout)?;

    hud::draw(&mut builder, view, local, frame, layout)?;

    if let Interaction::Promotion { to, .. } = local.interaction {
        builder.push(RenderCmd::PushOpacity {
            opacity: tabula_presentation::Opacity::try_from(0.38)
                .map_err(|_| RenderListError::InvalidGeometry)?,
            layer: Layer::MODAL,
            z: -1,
        })?;
        builder.push(RenderCmd::Rect {
            rect: Rect::new(Vec2::ZERO, layout.viewport.size())?,
            radii: Corners::uniform(0.0)?,
            fill: Some(Paint::Solid(theme.color.hidden)),
            border: None,
            layer: Layer::MODAL,
            z: -1,
        })?;
        builder.push(RenderCmd::PopOpacity {
            layer: Layer::MODAL,
            z: -1,
        })?;
        let buttons = promotion_buttons(view, local, layout);
        if let Some(panel) = promotion_panel_rect(layout, to) {
            builder.push(RenderCmd::Rect {
                rect: panel,
                radii: Corners::uniform(theme.shape.sheet.get())?,
                fill: Some(Paint::Solid(theme.color.surface_container)),
                border: None,
                layer: Layer::MODAL,
                z: 0,
            })?;
            builder.push(RenderCmd::Text {
                text: if buttons.iter().all(|button| button.is_enabled()) {
                    "Choose promotion"
                } else {
                    "Unavailable: position changed"
                }
                .to_owned(),
                at: panel.origin() + Vec2::splat(f32::from(theme.space.md)),
                style: TextStyleToken::TitleMd,
                align: Align::Start,
                max_width: Some(
                    tabula_design::Positive::new(panel.size().x - f32::from(theme.space.md) * 2.0)
                        .map_err(|_| RenderListError::InvalidGeometry)?,
                ),
                color: theme.color.on_surface,
                layer: Layer::MODAL,
                z: 0,
            })?;
        }
        for button in buttons {
            button.draw(
                &mut builder,
                &theme,
                &local.promotion_buttons,
                &local.focus,
                Layer::MODAL,
            )?;
            if let Some(choice) = focus_id_to_promotion_choice(button.id()) {
                let rect = button.rect();
                let sprite_size = rect.size().x * 0.72;
                let sprite_rect = Rect::new(
                    rect.origin() + Vec2::new((rect.size().x - sprite_size) * 0.5, 2.0),
                    Vec2::splat(sprite_size),
                )?;
                builder.push(piece_sprite(
                    Piece {
                        color: view.turn,
                        kind: choice.piece_kind(),
                    },
                    sprite_rect,
                    &theme,
                    Layer::MODAL,
                    2,
                )?)?;
                builder.push(RenderCmd::Text { text: piece_name(choice.piece_kind()).into(),
                    at: rect.origin() + Vec2::new(rect.size().x * 0.5, rect.size().y - 24.0),
                    style: TextStyleToken::LabelMd, align: Align::Center, max_width: None,
                    color: if button.is_enabled() && matches!(local.interaction, Interaction::Promotion { selected, .. } if selected == choice) {
                        theme.color.on_primary } else { theme.color.on_surface }, layer: Layer::MODAL, z: 2 })?;
            }
        }
    }

    builder.finish()
}

#[allow(clippy::float_arithmetic)]
fn piece_sprite(
    piece: Piece,
    cell: Rect,
    theme: &Theme,
    layer: Layer,
    z: i16,
) -> Result<RenderCmd, RenderListError> {
    // Chessnut's square viewBox includes its authored transparent silhouette
    // inset. Keep that aspect ratio and use 97% of the cell in board and trays.
    let inset = cell.size() * 0.015;
    let rect = Rect::new(cell.origin() + inset, cell.size() - inset * 2.0)?;
    Ok(RenderCmd::Sprite {
        asset: assets::piece_asset(piece),
        rect,
        tint: theme.game_art.chess.piece_tint,
        rotation: 0.0,
        pivot: cell.origin() + cell.size() * 0.5,
        layer,
        z,
    })
}

#[allow(clippy::float_arithmetic)]
fn contrast_plate(
    rect: Rect,
    theme: &Theme,
    layer: Layer,
    z: i16,
) -> Result<RenderCmd, RenderListError> {
    Ok(RenderCmd::Rect {
        rect,
        radii: Corners::uniform(0.0)?,
        fill: None,
        border: Some(Border::new(
            theme.focus.ring_width.get() + 4.0,
            theme.color.surface,
        )?),
        layer,
        z,
    })
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
                format!(
                    "Your turn / {}{}",
                    color_name(view.turn),
                    if view.in_check { " / CHECK" } else { "" }
                )
            } else {
                format!(
                    "{} to move{}",
                    color_name(view.turn),
                    if view.in_check { " / CHECK" } else { "" }
                )
            }
        }
        Status::Ended { .. } => format!(
            "Game over / {}",
            result_title(view).unwrap_or_else(|| "Finished".into())
        ),
    }
}

fn result_title(view: &View) -> Option<String> {
    let Status::Ended { outcome } = &view.status else {
        return None;
    };
    Some(match outcome.kind() {
        tabula_core::OutcomeKind::Draw => "Draw".into(),
        tabula_core::OutcomeKind::Aborted { .. } => "Game cancelled".into(),
        tabula_core::OutcomeKind::Decisive => outcome
            .standings()
            .iter()
            .find(|standing| standing.rank == 0)
            .and_then(|standing| ChessColor::from_seat(standing.seat))
            .map_or_else(
                || "Game finished".into(),
                |color| format!("{} wins", color_name(color)),
            ),
    })
}

/// Derives a presentation-only live clock from the last authoritative clock
/// checkpoint and the current frame. It cannot alter rules state or timer
/// scheduling; the next authoritative input replaces this estimate.
fn clock_remaining(view: &View, frame: &FrameCtx) -> Option<[u64; 2]> {
    let clock = view.clock?;
    let elapsed = frame.now_ms().saturating_sub(clock.last_move_at.0);
    let charge = match clock.control {
        ClockControl::Fischer { .. } => elapsed,
        ClockControl::Bronstein { delay } => elapsed.saturating_sub(delay.0),
    };
    let mut remaining = clock.remaining;
    let active = match view.turn {
        ChessColor::White => 0,
        ChessColor::Black => 1,
    };
    if matches!(view.status, Status::Playing) {
        remaining[active].0 = remaining[active].0.saturating_sub(charge);
    }
    Some([remaining[0].0, remaining[1].0])
}

fn format_clock(color: &str, millis: u64) -> String {
    let seconds = millis / 1_000;
    format!("{color} {}:{:02}", seconds / 60, seconds % 60)
}

#[allow(clippy::too_many_lines)]
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
            let mut state = if piece.is_some() {
                String::from("occupied")
            } else {
                String::from("empty")
            };
            if view.in_check
                && piece
                    .is_some_and(|piece| piece.color == view.turn && piece.kind == PieceKind::King)
            {
                state.push_str(", in check");
            }
            if local
                .last_move
                .is_some_and(|(from, to)| from == square || to == square)
            {
                state.push_str(", last move");
            }
            if let Interaction::Selected { square: from }
            | Interaction::Pressed { from, .. }
            | Interaction::Dragging { from, .. } = local.interaction
            {
                if square == from {
                    state.push_str(", selected");
                }
                if legal_destination(view, from, square) {
                    state.push_str(", legal destination");
                }
            }
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
    let confirmation_active = local.confirmation.is_some();
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
                && !promotion_active
                && !confirmation_active,
        }],
    };

    if let Status::Ended { outcome } = &view.status {
        description.status.push_str(" / ");
        description.status.push_str(outcome.summary());
    }
    if let Interaction::Promotion { from, to, selected } = local.interaction {
        description.status = format!(
            "{} / choose promotion, {} selected",
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
                    state: if !promotion_choice_enabled(view, local, from, to, choice) {
                        String::from("unavailable: position changed")
                    } else if choice == selected {
                        String::from("selected")
                    } else {
                        String::from("available")
                    },
                    activates: promotion_choice_enabled(view, local, from, to, choice)
                        .then(|| ActionId(String::from(choice.action_id()))),
                })
                .collect(),
        });
        description
            .actions
            .extend(PROMOTION_CHOICES.iter().copied().map(|choice| A11yAction {
                id: ActionId(String::from(choice.action_id())),
                label: format!("Promote to {}", piece_name(choice.piece_kind())),
                enabled: promotion_choice_enabled(view, local, from, to, choice),
            }));
    }

    if matches!(local.interaction, Interaction::Promotion { .. }) {
        description.actions.push(A11yAction {
            id: ActionId(String::from("cancel-promotion")),
            label: String::from("Cancel promotion and return to the board"),
            enabled: true,
        });
    }
    if !promotion_active {
        for (id, label, command) in [
            ("resign", "Resign with confirmation", Command::Resign),
            (
                "offer-draw",
                "Offer a draw with confirmation",
                Command::OfferDraw,
            ),
            (
                "accept-draw",
                "Accept the offered draw with confirmation",
                Command::AcceptDraw,
            ),
            (
                "decline-draw",
                "Decline the offered draw",
                Command::DeclineDraw,
            ),
            (
                "claim-draw",
                "Claim an eligible draw with confirmation",
                Command::ClaimDraw,
            ),
        ] {
            description.actions.push(A11yAction {
                id: ActionId(id.into()),
                label: label.into(),
                enabled: !confirmation_active && view.actions.contains(&command),
            });
        }
        description.actions.push(A11yAction {
            id: ActionId("flip-board".into()),
            label: "Flip board orientation".into(),
            enabled: !confirmation_active,
        });
    }
    hud::describe_confirmation(&mut description, view, local);
    description
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
mod layout_pointer_fixture_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{
        canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Millis, SeatId, Viewer,
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
        rect.origin() + rect.size() * 0.5
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

    fn press_promotion_choice(view: &View, local: &mut ChessLocal, position: PointerPosition) {
        let press = InputEvent::Pointer {
            position,
            button: PointerButton::Primary,
            phase: PointerPhase::Down,
        };
        assert!(ChessPresentation::on_input(&press, view, local).is_none());
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

    fn cues_for_outcome(
        state: &crate::State,
        outcome: &Outcome<ChessRules>,
        local: &mut ChessLocal,
        frame: &FrameCtx,
        viewer: Viewer,
    ) -> AudioCues {
        let mut cues = AudioCues::new();
        for event in &outcome.events {
            if let Some(event) = ChessRules::view_event(state, event, viewer) {
                cues.extend(ChessPresentation::on_view_event(&event, local, frame));
            }
        }
        cues
    }

    fn cue_ids(cues: &AudioCues) -> Vec<&str> {
        cues.iter().map(AudioCue::id).collect()
    }

    #[test]
    fn promotion_chooser_anchors_inward_from_the_target_in_both_orientations() {
        for flipped in [false, true] {
            let layout = BoardLayout::oriented(viewport(1100.0, 850.0), flipped);
            for target in [Square(0), Square(7), Square(56), Square(63)] {
                let chooser = PromotionLayout::new(layout, target).unwrap();
                let cell = layout.square_rect(target).unwrap();
                let center = cell.origin().y + cell.size().y * 0.5;
                if center < layout.board.origin().y + layout.board.size().y * 0.5 {
                    assert!(chooser.panel.origin().y >= cell.origin().y + cell.size().y);
                } else {
                    assert!(chooser.panel.origin().y + chooser.panel.size().y <= cell.origin().y);
                }
                for rect in chooser.choices.into_iter().chain([chooser.cancel]) {
                    assert!(rect.size().cmpge(Vec2::splat(44.0)).all());
                    assert!(rect.origin().cmpge(chooser.panel.origin()).all());
                    assert!((rect.origin() + rect.size())
                        .cmple(chooser.panel.origin() + chooser.panel.size())
                        .all());
                }
            }
        }
    }

    #[test]
    fn clocked_presentation_visibly_counts_down_from_authoritative_clock_state() {
        let mut state = crate::State::initial();
        state.clock = Some(crate::ClockState {
            remaining: [Millis(60_000), Millis(120_000)],
            last_move_at: LogicalTime::ZERO,
            control: crate::ClockControl::Fischer {
                increment: Millis::ZERO,
            },
        });
        let view = view(&state);
        let scene = ChessPresentation::present(
            &view,
            &ChessLocal::default(),
            &frame_at(640.0, 640.0, 1_500),
        );

        let clock_labels = scene
            .commands()
            .iter()
            .filter_map(|command| match command {
                RenderCmd::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(clock_labels.contains(&"0:58"));
        assert!(clock_labels.contains(&"2:00"));
        assert_eq!(
            scene
                .commands()
                .iter()
                .filter(|cmd| matches!(cmd,
                    RenderCmd::Text { text, style: TextStyleToken::MonoMd, .. }
                    if text == "0:58" || text == "2:00"
                ))
                .count(),
            2
        );
    }

    #[test]
    fn terminal_clock_checkpoint_does_not_keep_counting_down() {
        let mut state = crate::State::initial();
        state.clock = Some(crate::ClockState {
            remaining: [Millis(29_000), Millis(120_000)],
            last_move_at: LogicalTime::ZERO,
            control: ClockControl::Fischer {
                increment: Millis::ZERO,
            },
        });
        let mut projected = view(&state);
        let outcome = legal_apply(&mut state, 0, 1, Command::Resign);
        assert!(
            !outcome.effects.is_empty(),
            "resignation must reach a terminal state"
        );
        projected.status = state.status.clone();
        assert_eq!(
            clock_remaining(&projected, &frame_at(640.0, 640.0, 50_000)),
            Some([29_000, 120_000])
        );
    }

    #[test]
    fn clock_hud_marks_turn_and_low_time_in_text_in_every_theme() {
        let mut state = crate::State::initial();
        state.clock = Some(crate::ClockState {
            remaining: [Millis(30_000), Millis(30_000)],
            last_move_at: LogicalTime::ZERO,
            control: ClockControl::Bronstein {
                delay: Millis(2_000),
            },
        });
        let projected = view(&state);
        for kind in [
            tabula_design::ThemeKind::Light,
            tabula_design::ThemeKind::Dark,
            tabula_design::ThemeKind::HighContrastLight,
            tabula_design::ThemeKind::HighContrastDark,
        ] {
            let theme = Theme::by_kind(kind);
            let frame = frame_with_theme(320.0, 640.0, 1_000, &theme);
            assert_eq!(clock_remaining(&projected, &frame), Some([30_000, 30_000]));
            let scene = ChessPresentation::present(&projected, &ChessLocal::default(), &frame);
            assert!(scene
                .commands()
                .iter()
                .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text == "White / turn")));
            assert!(scene.commands().iter().any(|cmd| matches!(cmd, RenderCmd::Text { text, style: TextStyleToken::MonoMd, color, .. }
                if text == "0:30 LOW" && *color == theme.color.on_danger)));
            assert!(scene
                .commands()
                .iter()
                .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text == "0:30")));
        }
    }

    #[test]
    fn clock_cards_fit_the_dock_at_compact_and_short_landscape_sizes() {
        let mut state = crate::State::initial();
        state.clock = Some(crate::ClockState {
            remaining: [Millis(60_000), Millis(120_000)],
            last_move_at: LogicalTime::ZERO,
            control: ClockControl::Fischer {
                increment: Millis::ZERO,
            },
        });
        let projected = view(&state);
        for (width, height) in [
            (320.0, 320.0),
            (390.0, 844.0),
            (640.0, 240.0),
            (768.0, 500.0),
            (1440.0, 900.0),
        ] {
            let frame = frame(width, height);
            let layout = BoardLayout::from_viewport(frame.viewport());
            let scene = ChessPresentation::present(&projected, &ChessLocal::default(), &frame);
            let cards: Vec<_> = scene
                .commands()
                .iter()
                .filter_map(|cmd| match cmd {
                    RenderCmd::Rect {
                        rect,
                        layer: Layer::HUD,
                        ..
                    } => Some(rect),
                    _ => None,
                })
                .collect();
            assert!(
                cards.len() >= 5,
                "player bars, clocks and status stay visible"
            );
            for rect in cards {
                assert!(rect.origin().cmpge(Vec2::ZERO).all());
                assert!((rect.origin() + rect.size())
                    .cmple(Vec2::new(width, height))
                    .all());
            }
            assert!(
                layout.top_player.origin().y + layout.top_player.size().y
                    <= layout.board.origin().y
            );
            assert!(
                layout.bottom_player.origin().y >= layout.board.origin().y + layout.board.size().y
            );
            assert_eq!(
                scene
                    .commands()
                    .iter()
                    .filter(|cmd| matches!(
                        cmd,
                        RenderCmd::Text {
                            style: TextStyleToken::MonoMd,
                            ..
                        }
                    ))
                    .count(),
                2
            );
        }
    }

    #[test]
    fn accepted_projected_move_emits_one_cue_without_mutating_canonical_state() {
        let mut state = crate::State::initial();
        let outcome = legal_apply(
            &mut state,
            0,
            1,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        let before_presentation = canonical_encode(&state).unwrap();
        let mut local = ChessLocal::default();

        let cues = cues_for_outcome(
            &state,
            &outcome,
            &mut local,
            &frame_at(640.0, 640.0, 100),
            Viewer::Seat(SeatId(0)),
        );

        assert_eq!(cue_ids(&cues), ["move"]);
        assert_eq!(canonical_encode(&state).unwrap(), before_presentation);
    }

    #[test]
    fn projected_capture_emits_capture_not_move() {
        let mut state = crate::State::initial();
        legal_apply(
            &mut state,
            0,
            1,
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
        );
        legal_apply(
            &mut state,
            1,
            2,
            Command::Move {
                from: 51,
                to: 35,
                promotion: None,
            },
        );
        let capture = legal_apply(
            &mut state,
            0,
            3,
            Command::Move {
                from: 28,
                to: 35,
                promotion: None,
            },
        );

        let cues = cues_for_outcome(
            &state,
            &capture,
            &mut ChessLocal::default(),
            &frame(640.0, 640.0),
            Viewer::Seat(SeatId(0)),
        );

        assert_eq!(cue_ids(&cues), ["capture"]);
    }

    #[test]
    fn terminal_projected_events_preserve_move_then_game_end_cue_order() {
        let mut state = crate::State::initial();
        for (seat, index, from, to) in [(0, 1, 13, 21), (1, 2, 52, 36), (0, 3, 14, 30)] {
            legal_apply(
                &mut state,
                seat,
                index,
                Command::Move {
                    from,
                    to,
                    promotion: None,
                },
            );
        }
        let terminal = legal_apply(
            &mut state,
            1,
            4,
            Command::Move {
                from: 59,
                to: 31,
                promotion: None,
            },
        );

        let cues = cues_for_outcome(
            &state,
            &terminal,
            &mut ChessLocal::default(),
            &frame(640.0, 640.0),
            Viewer::Seat(SeatId(0)),
        );

        assert_eq!(cue_ids(&cues), ["move", "game-end"]);
    }

    #[test]
    fn cue_identity_is_frame_independent_and_present_is_audio_free() {
        let event = crate::ViewEvent::Moved {
            seat: SeatId(0),
            from: Square(12),
            to: Square(28),
            promotion: None,
            captured: None,
        };
        let mut local = ChessLocal::default();
        let first =
            ChessPresentation::on_view_event(&event, &mut local, &frame_at(640.0, 640.0, 100));
        let later = ChessPresentation::on_view_event(
            &event,
            &mut ChessLocal::default(),
            &frame_at(640.0, 640.0, 900),
        );
        let state = crate::State::initial();
        let current_view = view(&state);

        for now_ms in 0..100 {
            let _ =
                ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, now_ms));
        }

        assert_eq!(cue_ids(&first), ["move"]);
        assert_eq!(cue_ids(&later), ["move"]);
    }

    #[test]
    fn clock_updates_are_silent() {
        let cues = ChessPresentation::on_view_event(
            &crate::ViewEvent::ClockUpdated {
                seat: SeatId(0),
                remaining: tabula_core::Millis(1_000),
            },
            &mut ChessLocal::default(),
            &frame(640.0, 640.0),
        );

        assert!(cues.is_empty());
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
            assert!(
                board.origin().y >= status.origin().y + status.size().y
                    || board.origin().y + board.size().y <= status.origin().y
                    || board.origin().x + board.size().x <= status.origin().x
                    || status.origin().x + status.size().x <= board.origin().x
            );
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
        let layout = BoardLayout::from_viewport(viewport(800.0, 500.0));
        assert_eq!(first, second);
        assert_eq!(
            first
                .commands()
                .iter()
                .filter(|command| matches!(
                    command,
                    RenderCmd::Rect {
                        rect,
                        layer: Layer::BOARD,
                        z,
                        ..
                    } if *z >= 0 && rect.size() == Vec2::splat(layout.square_size())
                ))
                .count(),
            64
        );
        for index in 0..64_u8 {
            let square = Square(index);
            let expected = layout.square_rect(square).unwrap();
            let color = if (square.file() + square.rank()) % 2 == 1 {
                frame(800.0, 500.0).theme().game_art.chess.board_light
            } else {
                frame(800.0, 500.0).theme().game_art.chess.board_dark
            };
            assert_eq!(first.commands().iter().filter(|command| matches!(command,
                RenderCmd::Rect { rect, fill: Some(Paint::Solid(actual)), layer: Layer::BOARD,
                    border: None, z, .. } if *rect == expected && *actual == color && *z == i16::from(index)
            )).count(), 1, "one correctly colored cell per square identity");
        }
        assert_eq!(
            first
                .commands()
                .iter()
                .filter(|command| matches!(
                    command,
                    RenderCmd::Sprite {
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
        let choice = promotion_choice_rect(layout, Square(60), 0).unwrap();
        let event = InputEvent::Pointer {
            position: clicked_center(choice),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        local.set_viewport(viewport(640.0, 640.0));
        press_promotion_choice(&view, &mut local, clicked_center(choice));
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
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: false,
            },
            &view,
            &mut local,
        );
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
    fn promotion_controls_retain_44_dp_targets_on_compact_viewports() {
        for (width, height) in [
            (320.0, 640.0),
            (390.0, 844.0),
            (640.0, 320.0),
            (320.0, 300.0),
            (640.0, 280.0),
        ] {
            let layout = BoardLayout::from_viewport(viewport(width, height));
            for index in 0..PROMOTION_CHOICES.len() {
                let rect = promotion_choice_rect(layout, Square(60), index).unwrap();
                assert!(rect.size().x >= 44.0 && rect.size().y >= 44.0);
                assert!(rect.origin().x >= 0.0 && rect.origin().y >= 0.0);
                assert!(rect.origin().x + rect.size().x <= width);
                assert!(rect.origin().y + rect.size().y <= height);
            }
            let panel = promotion_panel_rect(layout, Square(60)).unwrap();
            assert!(panel.origin().x >= 0.0 && panel.origin().y >= 0.0);
            assert!(panel.origin().x + panel.size().x <= width);
            assert!(panel.origin().y + panel.size().y <= height);
            let cancel = promotion_cancel_rect(layout, Square(60)).unwrap();
            assert!(cancel.size().x >= 44.0 && cancel.size().y >= 44.0);
        }
    }

    #[test]
    fn promotion_release_without_a_press_never_activates_a_choice() {
        let (view, layout, mut local) = promotion_fixture();
        let event = InputEvent::Pointer {
            position: clicked_center(promotion_choice_rect(layout, Square(60), 0).unwrap()),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        assert!(ChessPresentation::on_input(&event, &view, &mut local).is_none());
        assert!(matches!(local.interaction(), Interaction::Promotion { .. }));
    }

    #[test]
    fn promotion_window_focus_loss_blocks_activation() {
        let (view, _layout, mut local) = promotion_fixture();
        assert!(
            ChessPresentation::on_input(&InputEvent::Focus(false), &view, &mut local).is_none()
        );
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        assert!(matches!(local.interaction(), Interaction::Promotion { .. }));
    }

    #[test]
    fn promotion_pointer_cancellation_and_outside_release_preserve_the_modal() {
        for phase in [PointerPhase::Cancel, PointerPhase::Up] {
            let (view, layout, mut local) = promotion_fixture();
            let position = clicked_center(promotion_choice_rect(layout, Square(60), 0).unwrap());
            press_promotion_choice(&view, &mut local, position);
            let event = InputEvent::Pointer {
                position: PointerPosition::new(Vec2::ZERO).unwrap(),
                button: PointerButton::Primary,
                phase,
            };
            assert!(ChessPresentation::on_input(&event, &view, &mut local).is_none());
            assert!(matches!(local.interaction(), Interaction::Promotion { .. }));
            assert!(ChessPresentation::on_input(&key(Key::Escape), &view, &mut local).is_none());
            assert_eq!(local.cursor(), Square(52));
        }
    }

    #[test]
    fn promotion_choices_disable_when_the_projection_no_longer_permits_them() {
        let (mut view, layout, mut local) = promotion_fixture();
        let position = clicked_center(promotion_choice_rect(layout, Square(60), 0).unwrap());
        press_promotion_choice(&view, &mut local, position);
        view.legal_moves.clear();
        let release = InputEvent::Pointer {
            position,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        assert!(ChessPresentation::on_input(&release, &view, &mut local).is_none());
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        let description = ChessPresentation::a11y(&view, &local);
        assert!(description
            .actions
            .iter()
            .filter(|action| action.id.0.starts_with("promote-"))
            .all(|action| !action.enabled));
        let list = ChessPresentation::present(&view, &local, &frame(640.0, 640.0));
        assert!(list.commands().iter().any(|command| matches!(command,
            RenderCmd::Text { text, layer: Layer::MODAL, .. }
                if text == "Unavailable: position changed")));
    }

    #[test]
    fn keyboard_opening_press_cannot_repeat_into_a_promotion_action() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let view = view(&state);
        let before = canonical_encode(&state).unwrap();
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));
        local.focus.set_keyboard_focus(Some(FocusId::new(52)));
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        let release = InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        };
        assert!(ChessPresentation::on_input(&release, &view, &mut local).is_none());
        local.focus.set_keyboard_focus(Some(FocusId::new(60)));
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).is_none());
        assert!(matches!(local.interaction(), Interaction::Promotion { .. }));
        assert!(ChessPresentation::on_input(&release, &view, &mut local).is_none());
        let intent = ChessPresentation::on_input(&key(Key::Enter), &view, &mut local).unwrap();
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 52,
                to: 60,
                promotion: Some(PieceKind::Queen)
            }
        );
        assert_eq!(local.cursor(), Square(60));
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }

    #[test]
    fn promotion_cancel_is_available_to_pointer_and_keyboard_when_choices_are_disabled() {
        for keyboard in [false, true] {
            let (mut view, layout, mut local) = promotion_fixture();
            view.legal_moves.clear();
            if keyboard {
                assert!(ChessPresentation::on_input(&key(Key::Tab), &view, &mut local).is_none());
                assert_eq!(local.focus.current(), Some(PROMOTION_CANCEL_FOCUS_ID));
                assert!(ChessPresentation::on_input(&key(Key::Space), &view, &mut local).is_none());
            } else {
                let position = clicked_center(promotion_cancel_rect(layout, Square(60)).unwrap());
                press_promotion_choice(&view, &mut local, position);
                assert!(ChessPresentation::on_input(
                    &InputEvent::Pointer {
                        position,
                        button: PointerButton::Primary,
                        phase: PointerPhase::Up,
                    },
                    &view,
                    &mut local
                )
                .is_none());
            }
            assert_eq!(local.interaction(), Interaction::Idle);
            assert_eq!(local.cursor(), Square(52));
        }
    }

    #[test]
    fn promotion_widgets_use_semantic_themes_and_keep_reduced_motion_immediate() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let before = canonical_encode(&state).unwrap();
        let (view, layout, mut local) = promotion_fixture();
        local
            .focus
            .set_keyboard_focus(Some(promotion_choice_focus_id(PromotionChoice::Queen)));
        for kind in [
            tabula_design::ThemeKind::Light,
            tabula_design::ThemeKind::Dark,
            tabula_design::ThemeKind::HighContrastLight,
            tabula_design::ThemeKind::HighContrastDark,
        ] {
            let mut theme = Theme::by_kind(kind);
            let list = ChessPresentation::present(
                &view,
                &local,
                &frame_with_theme(640.0, 640.0, 0, &theme),
            );
            for name in ["queen", "rook", "bishop", "knight"] {
                assert!(list.commands().iter().any(|command| matches!(command,
                    RenderCmd::Text { text, style: TextStyleToken::LabelMd, layer: Layer::MODAL, .. }
                        if text == name)));
            }
            assert!(list.commands().iter().any(|command| matches!(command,
                RenderCmd::Rect { rect, fill: Some(Paint::Solid(color)), border: None, layer: Layer::MODAL, .. }
                    if *rect == promotion_choice_rect(layout, Square(60), 0).unwrap() && *color == theme.color.primary)));
            theme.motion.reduced.duration_scale = tabula_design::Percent::new(0).unwrap();
            assert_eq!(
                list,
                ChessPresentation::present(
                    &view,
                    &local,
                    &frame_with_theme(640.0, 640.0, 100, &theme)
                )
            );
        }
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }

    #[test]
    fn every_pointer_promotion_choice_emits_the_matching_command() {
        for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
            let (view, layout, mut local) = promotion_fixture();
            let event = InputEvent::Pointer {
                position: clicked_center(promotion_choice_rect(layout, Square(60), index).unwrap()),
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            };
            local.set_viewport(viewport(640.0, 640.0));
            press_promotion_choice(
                &view,
                &mut local,
                clicked_center(promotion_choice_rect(layout, Square(60), index).unwrap()),
            );
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
            position: clicked_center(promotion_choice_rect(layout, Square(60), 2).unwrap()),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        pointer_local.set_viewport(viewport(640.0, 640.0));
        press_promotion_choice(
            &view,
            &mut pointer_local,
            clicked_center(promotion_choice_rect(layout, Square(60), 2).unwrap()),
        );
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
                RenderCmd::Sprite {
                    asset,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if asset.as_str() == "pieces/white-pawn"
            )
        }));
        assert!(!intermediate.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Sprite {
                    asset,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if asset.as_str() == "pieces/white-queen"
            )
        }));

        let terminal =
            ChessPresentation::present(&current_view, &local, &frame_at(640.0, 640.0, 1280));
        let destination_position = piece_position(layout, Square(60));
        assert!(terminal.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Sprite {
                    asset,
                    pivot,
                    layer: Layer::PIECES,
                    ..
                } if asset.as_str() == "pieces/white-queen" && *pivot == destination_position
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
        for kind in [
            PieceKind::Queen,
            PieceKind::Rook,
            PieceKind::Bishop,
            PieceKind::Knight,
        ] {
            let expected = assets::piece_asset(Piece {
                color: ChessColor::Black,
                kind,
            });
            assert!(rendered.commands().iter().any(|command| matches!(command,
                RenderCmd::Sprite { asset, layer: Layer::MODAL, .. } if *asset == expected)));
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
    #[allow(clippy::too_many_lines)]
    fn active_animation_does_not_gate_drag() {
        let mut state = crate::State::initial();
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));
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
        assert_eq!(current_view.turn, ChessColor::Black);
        assert!(
            !local
                .move_animation()
                .expect("White's opening move is animating")
                .timeline
                .sample(1100)
                .done
        );

        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let pos_e7 = pointer(layout, Square(52));
        let pos_e5 = pointer(layout, Square(36));
        let intermediate_step =
            PointerPosition::new(pos_e7.get() + Vec2::new(0.0, layout.drag_threshold() * 1.5))
                .unwrap();

        // 1. Pointer Down at center(e7)
        let down_intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e7,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &current_view,
            &mut local,
        );
        assert!(down_intent.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Pressed {
                from: Square(52),
                down_at: pos_e7,
                was_selected: false,
            }
        );
        assert!(local.move_animation().is_none());

        // 2. Pointer Move beyond threshold
        let move_step_intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: intermediate_step,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &current_view,
            &mut local,
        );
        assert!(move_step_intent.is_none());
        assert!(matches!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(52),
                ..
            }
        ));

        // 3. Pointer Move to e5
        let move_dest_intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e5,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &current_view,
            &mut local,
        );
        assert!(move_dest_intent.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(52),
                pointer: pos_e5,
                over: Some(Square(36)),
            }
        );

        // 4. Pointer Up at e5 produces the legal move intent without waiting for animation to finish
        let intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e5,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &current_view,
            &mut local,
        )
        .expect("active animation does not gate drag intent");

        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 52,
                to: 36,
                promotion: None,
            }
        );
        assert_eq!(local.interaction(), Interaction::Idle);
        assert!(local.move_animation().is_none());
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
                    RenderCmd::Sprite {
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
                RenderCmd::Sprite {
                    asset,
                    pivot,
                    layer: Layer::PIECES,
                    z: IN_TRANSIT_PIECE_Z,
                    ..
                } if asset.as_str() == "pieces/white-pawn" => Some(*pivot),
                _ => None,
            });
        let focal_position = focal_position.expect("one focal pawn is rendered in transit");
        assert_ne!(focal_position, start_position);
        assert_ne!(focal_position, destination_position);
        assert!(!mid_render.commands().iter().any(|command| {
            matches!(
                command,
                RenderCmd::Sprite {
                    asset,
                    pivot,
                    layer: Layer::PIECES,
                    z: 28,
                    ..
                } if asset.as_str() == "pieces/white-pawn" && *pivot == destination_position
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
            } if text == "Black wins"
        )));
        assert!(rendered.commands().iter().any(|command| matches!(command,
            RenderCmd::Text { text, layer: Layer::HUD, .. } if text == "checkmate"
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
        assert_eq!(description.actions.len(), 2 + PROMOTION_CHOICES.len());
        assert!(description
            .actions
            .iter()
            .any(|action| action.id.0 == "cancel-promotion" && action.enabled));
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

    #[test]
    fn pointer_down_on_movable_piece_enters_pressed_state() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        let down_event = InputEvent::Pointer {
            position: down_pos,
            button: PointerButton::Primary,
            phase: PointerPhase::Down,
        };

        let before = canonical_encode(&state).unwrap();
        let intent = ChessPresentation::on_input(&down_event, &view, &mut local);
        assert!(intent.is_none());
        assert_eq!(canonical_encode(&state).unwrap(), before);
        assert_eq!(
            local.interaction(),
            Interaction::Pressed {
                from: Square(12),
                down_at: down_pos,
                was_selected: false,
            }
        );
    }

    #[test]
    fn movement_below_drag_threshold_remains_tap_candidate() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        // Move by half the threshold distance (small jitter)
        let jitter_pos =
            PointerPosition::new(down_pos.get() + Vec2::new(layout.drag_threshold() * 0.5, 0.0))
                .unwrap();
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: jitter_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        assert_eq!(
            local.interaction(),
            Interaction::Pressed {
                from: Square(12),
                down_at: down_pos,
                was_selected: false,
            }
        );

        // Releasing within the same square commits selection as a tap
        let up_event = InputEvent::Pointer {
            position: jitter_pos,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        assert!(ChessPresentation::on_input(&up_event, &view, &mut local).is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );
    }

    #[test]
    fn movement_above_drag_threshold_enters_dragging() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        let drag_pos =
            PointerPosition::new(down_pos.get() + Vec2::new(0.0, -layout.drag_threshold() * 1.5))
                .unwrap();
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: drag_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        assert_eq!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(12),
                pointer: drag_pos,
                over: layout.square_at(drag_pos),
            }
        );
        assert_eq!(local.focus().modality(), FocusModality::Pointer);
        assert!(!local.focus().is_focus_visible());
    }

    #[test]
    fn drag_source_identity_never_changes() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        for target_square in [Square(20), Square(28), Square(36), Square(63)] {
            let target_pos = pointer(layout, target_square);
            ChessPresentation::on_input(
                &InputEvent::Pointer {
                    position: target_pos,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Move,
                },
                &view,
                &mut local,
            );
            assert_eq!(
                local.interaction(),
                Interaction::Dragging {
                    from: Square(12),
                    pointer: target_pos,
                    over: Some(target_square),
                }
            );
        }
    }

    #[test]
    fn drag_outside_board_is_total_and_has_no_target() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        let outside_pos = PointerPosition::new(Vec2::new(-50.0, -50.0)).unwrap();
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: outside_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        assert_eq!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(12),
                pointer: outside_pos,
                over: None,
            }
        );

        let up_intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: outside_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local,
        );
        assert!(up_intent.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );
    }

    #[test]
    fn pointer_cancel_during_drag_emits_no_intent_and_clears_drag() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        let move_pos = pointer(layout, Square(28));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: move_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let cancel_event = InputEvent::Pointer {
            position: move_pos,
            button: PointerButton::Primary,
            phase: PointerPhase::Cancel,
        };
        let intent = ChessPresentation::on_input(&cancel_event, &view, &mut local);
        assert!(intent.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );
        assert_eq!(local.hover(), None);
    }

    #[test]
    fn invalid_drop_emits_no_intent_and_restores_source_selection() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        // Move to e5 (Square(36)), which is illegal on move 1 for pawn on e2
        let invalid_pos = pointer(layout, Square(36));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: invalid_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let up_event = InputEvent::Pointer {
            position: invalid_pos,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        let intent = ChessPresentation::on_input(&up_event, &view, &mut local);
        assert!(intent.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Selected { square: Square(12) }
        );
    }

    #[test]
    fn legal_drag_emits_exact_move_intent() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let down_pos = pointer(layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        let target_pos = pointer(layout, Square(28));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: target_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let up_event = InputEvent::Pointer {
            position: target_pos,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        };
        let intent = ChessPresentation::on_input(&up_event, &view, &mut local)
            .expect("legal drag-and-drop produces a move intent");
        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            }
        );
        assert_eq!(local.interaction(), Interaction::Idle);
    }

    #[test]
    fn real_opening_e2_to_e4_drag_drop_sequence() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let pos_e2 = pointer(layout, Square(12));
        let pos_e4 = pointer(layout, Square(28));
        let intermediate_step =
            PointerPosition::new(pos_e2.get() + Vec2::new(0.0, -layout.drag_threshold() * 1.5))
                .unwrap();

        // 1. Pointer Down at center(e2)
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        )
        .is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Pressed {
                from: Square(12),
                down_at: pos_e2,
                was_selected: false,
            }
        );

        // 2. Pointer Move beyond threshold
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: intermediate_step,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        )
        .is_none());
        assert!(matches!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(12),
                ..
            }
        ));

        // 3. Pointer Move near/inside e4
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        )
        .is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(12),
                pointer: pos_e4,
                over: Some(Square(28)),
            }
        );

        // 4. Pointer Up at e4
        let intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local,
        )
        .expect("e2->e4 drag emits the move intent");

        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            }
        );
        assert_eq!(local.interaction(), Interaction::Idle);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn drag_and_tap_and_keyboard_converge_on_the_same_move_command() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));

        let pos_e2 = pointer(layout, Square(12));
        let pos_e4 = pointer(layout, Square(28));

        // Path A: real tap-tap (Down -> Pressed -> Up)
        let mut local_a = ChessLocal::default();
        local_a.set_viewport(viewport(640.0, 640.0));

        // Tap 1: select e2
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local_a,
        )
        .is_none());
        assert_eq!(
            local_a.interaction(),
            Interaction::Pressed {
                from: Square(12),
                down_at: pos_e2,
                was_selected: false,
            }
        );
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local_a,
        )
        .is_none());
        assert_eq!(
            local_a.interaction(),
            Interaction::Selected { square: Square(12) }
        );

        // Tap 2: choose destination e4
        assert!(ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local_a,
        )
        .is_none());
        let intent_a = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local_a,
        )
        .expect("tap-tap produces a move intent");
        assert_eq!(local_a.interaction(), Interaction::Idle);

        // Path B: real drag-and-drop (Down -> Move -> Up)
        let mut local_b = ChessLocal::default();
        local_b.set_viewport(viewport(640.0, 640.0));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local_b,
        );
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local_b,
        );
        let intent_b = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local_b,
        )
        .expect("drag-and-drop produces a move intent");
        assert_eq!(local_b.interaction(), Interaction::Idle);

        // Path C: keyboard activation
        let mut local_c = ChessLocal::default();
        local_c.set_viewport(viewport(640.0, 640.0));
        local_c
            .focus_mut()
            .set_keyboard_focus(Some(FocusId::new(12)));
        assert!(ChessPresentation::on_input(&key(Key::Enter), &view, &mut local_c).is_none());
        local_c
            .focus_mut()
            .set_keyboard_focus(Some(FocusId::new(28)));
        let intent_c = ChessPresentation::on_input(&key(Key::Space), &view, &mut local_c)
            .expect("keyboard navigation produces a move intent");
        assert_eq!(local_c.interaction(), Interaction::Idle);

        assert_eq!(intent_a, intent_b);
        assert_eq!(intent_b, intent_c);
        assert_eq!(
            intent_a.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            }
        );
    }

    #[test]
    fn promotion_drag_regression() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));

        let pos_e7 = pointer(layout, Square(52));
        let pos_e8 = pointer(layout, Square(60));

        // 1. Driving the pawn with pointer drag to e8 opens the promotion chooser
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e7,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e8,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let drop_result = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e8,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local,
        );
        assert!(drop_result.is_none());
        assert_eq!(
            local.interaction(),
            Interaction::Promotion {
                from: Square(52),
                to: Square(60),
                selected: PromotionChoice::Queen,
            }
        );
        assert_eq!(local.focus().modality(), FocusModality::Pointer);
        assert!(!local.focus().is_focus_visible());

        // 2. Selecting each promotion choice produces identical commands to tap/keyboard paths
        for (index, choice) in PROMOTION_CHOICES.iter().copied().enumerate() {
            let mut choice_local = ChessLocal::default();
            choice_local.set_viewport(viewport(640.0, 640.0));

            ChessPresentation::on_input(
                &InputEvent::Pointer {
                    position: pos_e7,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down,
                },
                &view,
                &mut choice_local,
            );
            ChessPresentation::on_input(
                &InputEvent::Pointer {
                    position: pos_e8,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Move,
                },
                &view,
                &mut choice_local,
            );
            ChessPresentation::on_input(
                &InputEvent::Pointer {
                    position: pos_e8,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up,
                },
                &view,
                &mut choice_local,
            );

            let choice_pos =
                clicked_center(promotion_choice_rect(layout, Square(60), index).unwrap());
            press_promotion_choice(&view, &mut choice_local, choice_pos);
            let intent = ChessPresentation::on_input(
                &InputEvent::Pointer {
                    position: choice_pos,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up,
                },
                &view,
                &mut choice_local,
            )
            .expect("promotion choice selection emits a command");

            assert_eq!(
                intent.into_command(),
                Command::Move {
                    from: 52,
                    to: 60,
                    promotion: Some(choice.piece_kind()),
                }
            );
        }
    }

    #[test]
    fn drag_does_not_enable_keyboard_focus_visible() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let pos_e2 = pointer(layout, Square(12));
        let pos_e4 = pointer(layout, Square(28));

        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        assert_eq!(local.focus().modality(), FocusModality::Pointer);
        assert!(!local.focus().is_focus_visible());

        let mut theme = Theme::by_kind(tabula_design::ThemeKind::Light);
        theme.focus.ring_color = theme.color.danger;
        let rendered =
            ChessPresentation::present(&view, &local, &frame_with_theme(640.0, 640.0, 0, &theme));

        // No focus ring command with z=150 should exist during pointer drag
        assert!(!rendered.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                layer: Layer::OVERLAY,
                border: Some(border),
                z: 150,
                ..
            } if border.color() == theme.focus.ring_color
        )));
    }

    #[test]
    fn resize_mid_drag_remains_total() {
        let state = crate::State::initial();
        let view = view(&state);

        let initial_viewport = viewport(800.0, 500.0);
        let initial_layout = BoardLayout::from_viewport(initial_viewport);
        let mut local = ChessLocal::default();
        local.set_viewport(initial_viewport);

        let down_pos = pointer(initial_layout, Square(12));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: down_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );

        // Viewport resizes while dragging
        let resized_viewport = viewport(320.0, 640.0);
        let resized_layout = BoardLayout::from_viewport(resized_viewport);
        local.set_viewport(resized_viewport);

        let resized_dest = pointer(resized_layout, Square(28));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: resized_dest,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        assert_eq!(
            local.interaction(),
            Interaction::Dragging {
                from: Square(12),
                pointer: resized_dest,
                over: Some(Square(28)),
            }
        );

        let intent = ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: resized_dest,
                button: PointerButton::Primary,
                phase: PointerPhase::Up,
            },
            &view,
            &mut local,
        )
        .expect("drop on resized board successfully produces move intent");

        assert_eq!(
            intent.into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            }
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn rendering_proves_no_duplicate_piece_during_drag() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let pos_e2 = pointer(layout, Square(12));
        let pos_e4 = pointer(layout, Square(28));
        let mid_pos =
            PointerPosition::new((pos_e2.get() + pos_e4.get()) * 0.5).expect("valid mid pointer");

        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: mid_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        let list = ChessPresentation::present(&view, &local, &frame);

        // 1. Stationary pieces: exactly 31 stationary pieces (32 minus the 1 lifted pawn)
        let stationary_pieces = list
            .commands()
            .iter()
            .filter(|cmd| {
                matches!(
                    cmd,
                    RenderCmd::Sprite {
                        layer: Layer::PIECES,
                        z,
                        ..
                    } if *z < IN_TRANSIT_PIECE_Z
                )
            })
            .count();
        assert_eq!(stationary_pieces, 31);

        // 2. The source resting position does NOT have a stationary piece rendered
        let e2_resting_pos = piece_position(layout, Square(12));
        assert!(!list.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Sprite {
                pivot,
                layer: Layer::PIECES,
                z,
                ..
            } if *z < IN_TRANSIT_PIECE_Z && *pivot == e2_resting_pos
        )));

        // 3. Exactly one focal dragged piece exists in Layer::PIECES with z=IN_TRANSIT_PIECE_Z + 10
        let dragged_pieces: Vec<_> = list
            .commands()
            .iter()
            .filter(|cmd| {
                matches!(
                    cmd,
                    RenderCmd::Sprite {
                        layer: Layer::PIECES,
                        z,
                        ..
                    } if *z == IN_TRANSIT_PIECE_Z + 10
                )
            })
            .collect();
        assert_eq!(dragged_pieces.len(), 1);

        let expected_lifted_at = mid_pos.get();

        match dragged_pieces[0] {
            RenderCmd::Sprite { asset, pivot, .. } => {
                assert_eq!(asset.as_str(), "pieces/white-pawn");
                assert_eq!(*pivot, expected_lifted_at);
            }
            _ => panic!("expected RenderCmd::Sprite"),
        }

        // 4. Source square has selected outline cue (z=100)
        assert!(list.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                layer: Layer::OVERLAY,
                z: 100,
                border: Some(border),
                ..
            } if border.color() == frame.theme().color.selected
        )));

        // 5. Target square shows legal feedback when hovering over e4
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e4,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );
        let list_over_legal = ChessPresentation::present(&view, &local, &frame);
        assert!(list_over_legal.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                rect,
                layer: Layer::OVERLAY,
                z: 120,
                border: Some(border),
                ..
            } if *rect == layout.square_rect(Square(28)).unwrap() && border.color() == frame.theme().color.legal_target
        )));

        // 6. Target square shows illegal feedback when hovering over an illegal destination (e.g. d3: Square(19))
        let illegal_target_pos = pointer(layout, Square(19));
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: illegal_target_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );
        let list_over_illegal = ChessPresentation::present(&view, &local, &frame);
        assert!(list_over_illegal.commands().iter().any(|cmd| matches!(
            cmd,
            RenderCmd::Rect {
                rect,
                layer: Layer::OVERLAY,
                z: 120,
                border: Some(border),
                ..
            } if *rect == layout.square_rect(Square(19)).unwrap() && border.color() == frame.theme().color.illegal_target
        )));
    }

    #[test]
    fn golden_chess_drag_e2_to_e4_midflight_dark() {
        let state = crate::State::initial();
        let view = view(&state);
        let layout = BoardLayout::from_viewport(viewport(640.0, 640.0));
        let mut local = ChessLocal::default();
        local.set_viewport(viewport(640.0, 640.0));

        let pos_e2 = pointer(layout, Square(12));
        let pos_e4 = pointer(layout, Square(28));
        let mid_pos =
            PointerPosition::new((pos_e2.get() + pos_e4.get()) * 0.5).expect("valid mid pointer");

        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: pos_e2,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            },
            &view,
            &mut local,
        );
        ChessPresentation::on_input(
            &InputEvent::Pointer {
                position: mid_pos,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            },
            &view,
            &mut local,
        );

        let frame = frame_with_theme(
            640.0,
            640.0,
            0,
            &Theme::by_kind(tabula_design::ThemeKind::Dark),
        );
        let list = ChessPresentation::present(&view, &local, &frame);
        assert_render_list_snapshot!("chess_drag_e2_to_e4_midflight_dark", list);
    }

    #[test]
    #[allow(clippy::float_arithmetic, clippy::float_cmp)]
    fn classic_piece_viewports_are_upright_nearly_full_square_and_untinted_in_all_schemes() {
        for theme in [
            tabula_design::ThemeKind::Light,
            tabula_design::ThemeKind::Dark,
            tabula_design::ThemeKind::HighContrastLight,
            tabula_design::ThemeKind::HighContrastDark,
        ] {
            let theme = Theme::by_kind(theme);
            for size in [28.0, 36.0, 64.0] {
                let cell = Rect::new(Vec2::new(8.0, 12.0), Vec2::splat(size)).unwrap();
                for color in [ChessColor::White, ChessColor::Black] {
                    for kind in [
                        PieceKind::King,
                        PieceKind::Queen,
                        PieceKind::Bishop,
                        PieceKind::Knight,
                        PieceKind::Rook,
                        PieceKind::Pawn,
                    ] {
                        let piece = Piece { color, kind };
                        let RenderCmd::Sprite {
                            asset,
                            rect,
                            tint,
                            rotation,
                            pivot,
                            ..
                        } = piece_sprite(piece, cell, &theme, Layer::PIECES, 0).unwrap()
                        else {
                            panic!("piece must remain a managed sprite");
                        };
                        assert_eq!(asset, assets::piece_asset(piece));
                        assert_eq!(rect.size().x, rect.size().y);
                        assert!((rect.size().x - size * 0.97).abs() < 0.001);
                        assert_eq!(pivot, cell.origin() + cell.size() * 0.5);
                        assert_eq!(rotation, 0.0);
                        assert_eq!(
                            (tint.red(), tint.green(), tint.blue(), tint.alpha()),
                            (u8::MAX, u8::MAX, u8::MAX, u8::MAX)
                        );
                        assert!(
                            rect.origin().x > cell.origin().x && rect.origin().y > cell.origin().y
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn chess_declares_typed_asset_pack_matching_metadata() {
        let pack = ChessPresentation::asset_pack();
        assert_eq!(pack, AssetPackRef::from_static("chess", "0.3.0"));
        assert_eq!(pack.to_string(), "chess@0.3.0");
        assert_eq!(pack.pack().as_str(), "chess");
        assert_eq!(pack.version().as_str(), "0.3.0");
    }
}
