//! Renderer-neutral Tiles presentation. (doc 04 §5)
//!
//! # Everything interactive is local
//!
//! Camera pan and zoom, the rotation of the tile being previewed, the keyboard
//! cursor, hover, and the drag in progress all live in [`TilesLocal`] and none
//! of them is ever an input to `apply` (I-10). Two players looking at the same
//! board from different camera positions is not a desync, and
//! the presentation tests prove it by driving one command sequence from
//! several camera positions and comparing state hashes.
//!
//! # The camera, and why the HUD compensates for it
//!
//! `RenderList` carries exactly **one** camera and the backend applies it to
//! every draw after that draw's local transforms
//! (`logical = (local − origin) × zoom`, doc 04 §5.3). So a screen-fixed HUD
//! cannot be expressed by simply not using the camera — it has to undo it.
//! Tiles emits board geometry in **world units** (one tile is
//! [`TILE_SIZE`] units) with the real camera on the list, and wraps the HUD in
//! a `PushTransform` of the camera's inverse. The composition is exactly the
//! identity, so HUD text keeps its size at every zoom while board text scales
//! with the board — which is what both should do.
//!
//! That is the honest use of the contract rather than a workaround: it is what
//! makes Tiles the camera benchmark instead of a game that happens to draw a
//! grid. The presentation tests assert the HUD occupies the same logical
//! rectangle at every zoom level.
//!
//! # Keyboard play is mandatory (doc 04 §10.3)
//!
//! Arrows move a cursor over board squares (not the camera — a keyboard player
//! expects to move *on* the board), Tab jumps to the next legal square, Space
//! rotates the tile, Enter places or claims, Escape declines a claim. During
//! the claim step Tab cycles the permitted features, each visibly numbered.
//! Activation keys require a fresh press; blur and pointer cancellation clear
//! gestures without producing a command. The
//! camera follows the cursor when it would leave the viewport, so a
//! keyboard-only player never loses the tile they are placing.
//!
//! @ai.role game-presentation
//! @ai.domain presentation.tiles
//! @ai.invariant no-authoritative-game-state
//! @ai.invariant camera-does-not-affect-canonical-state
//! @ai.evidence tests::the_hud_keeps_its_logical_geometry_at_every_zoom
//! @ai.evidence tests::a_pointer_maps_back_to_the_square_it_was_taken_from

#![allow(clippy::doc_markdown)]
#![allow(clippy::float_arithmetic)]

use core::fmt::Write as _;

use tabula_design::{Positive, Theme};
use tabula_game_api::{A11yAction, A11yDescription, ActionId, GameRules};
use tabula_presentation::{
    ActionButton, Affine2, Align, AssetPackRef, AudioCue, AudioCues, Border, ButtonInteraction,
    ButtonShape, ButtonTone, Camera2D, Corners, FocusGraph, FocusId, FocusNode, FocusState,
    FrameCtx, GamePresentation, InputEvent, Intent, Key, Layer, NavigationAction, Paint,
    PointerButton, PointerPhase, PointerPosition, Rect, RenderCmd, RenderList, RenderListBuilder,
    RenderListError, TextStyleToken, Vec2, Viewport,
};

use crate::rules::{
    legal_placements, Command, Coord, Event, FeatureKind, PlacedTile, Rotation, Side, Terrain,
    TilesRules, TurnPhase, View,
};

/// One board square, in world units, at zoom 1.
pub const TILE_SIZE: f32 = 64.0;

/// Clamps on the camera's zoom. Below the floor a tile is a smudge; above the
/// ceiling the board is unnavigable.
pub const MIN_ZOOM: f32 = 0.25;
/// See [`MIN_ZOOM`].
pub const MAX_ZOOM: f32 = 3.0;

/// Multiplier per zoom step.
const ZOOM_STEP: f32 = 1.25;

/// How far a pointer must travel before a press becomes a pan rather than a tap.
const DRAG_THRESHOLD: f32 = 6.0;

/// HUD button height and the gap between buttons, in logical units.
const HUD_BUTTON: f32 = 56.0;
const HUD_GAP: f32 = 8.0;
const CONNECTED_GAP: f32 = 2.0;
/// Height of the status strip along the top.
const HUD_STATUS_HEIGHT: f32 = 64.0;

/// Screen-fixed camera and turn actions, grouped by their meaning.
///
/// An enum rather than a list of rectangles: adding a control that nothing
/// handles becomes a compile error in [`TilesLocal::apply_control`] rather than
/// a button that silently does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    ZoomIn,
    ZoomOut,
    Recenter,
    Rotate,
    Confirm,
    Skip,
}

impl Control {
    /// Every control, in layout order.
    pub const ALL: [Self; 6] = [
        Self::ZoomIn,
        Self::ZoomOut,
        Self::Recenter,
        Self::Rotate,
        Self::Confirm,
        Self::Skip,
    ];

    const fn label(self, phase: TurnPhase) -> &'static str {
        match self {
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::Recenter => "Center",
            Self::Rotate => "Rotate",
            Self::Confirm if matches!(phase, TurnPhase::PlaceTile) => "Place tile",
            Self::Confirm => "Claim follower",
            Self::Skip => "Pass",
        }
    }

    const fn action(self, phase: TurnPhase) -> &'static str {
        match self {
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::Recenter => "recenter",
            Self::Rotate => "rotate",
            Self::Confirm if matches!(phase, TurnPhase::PlaceTile) => "place-tile",
            Self::Confirm => "claim-follower",
            Self::Skip => "skip-follower",
        }
    }

    const fn id(self) -> FocusId {
        FocusId::new(match self {
            Self::ZoomIn => 0,
            Self::ZoomOut => 1,
            Self::Recenter => 2,
            Self::Rotate => 3,
            Self::Confirm => 4,
            Self::Skip => 5,
        })
    }

    fn enabled(self, view: &View, local: &TilesLocal) -> bool {
        match self {
            Self::ZoomIn | Self::ZoomOut | Self::Recenter => true,
            Self::Rotate => on_turn(view) && view.phase == TurnPhase::PlaceTile,
            Self::Confirm => act_on_square(view, local, local.cursor).is_some(),
            Self::Skip => on_turn(view) && view.phase == TurnPhase::PlaceMeeple,
        }
    }

    /// Where this control sits, in **logical** (screen) coordinates.
    fn rect(self, viewport: Viewport) -> Option<Rect> {
        let (index, camera) = match self {
            Self::ZoomIn => (0u16, true),
            Self::ZoomOut => (1, true),
            Self::Recenter => (2, true),
            Self::Rotate => (0, false),
            Self::Confirm => (1, false),
            Self::Skip => (2, false),
        };
        let gap = if camera { CONNECTED_GAP } else { HUD_GAP };
        let width = ((viewport.size().x - HUD_GAP * 2.0 - gap * 2.0) / 3.0).min(if camera {
            96.0
        } else {
            160.0
        });
        let top = if camera {
            HUD_STATUS_HEIGHT + HUD_GAP
        } else {
            viewport.size().y - HUD_GAP - HUD_BUTTON
        };
        if width < 44.0
            || top + HUD_BUTTON > viewport.size().y
            || (!camera && top < HUD_STATUS_HEIGHT + HUD_GAP * 2.0 + HUD_BUTTON + 32.0)
        {
            return None;
        }
        let group_width = width * 3.0 + gap * 2.0;
        let left = if camera {
            HUD_GAP
        } else {
            (viewport.size().x - group_width) * 0.5
        };
        Rect::new(
            Vec2::new(left + (width + gap) * f32::from(index), top),
            Vec2::new(width, HUD_BUTTON),
        )
        .ok()
    }

    /// The control under a logical pointer position, if any.
    fn at(viewport: Viewport, point: Vec2) -> Option<Self> {
        Self::ALL.into_iter().find(|control| {
            control
                .rect(viewport)
                .is_some_and(|rect| rect.contains(point))
        })
    }
}

/// A pointer press that has not yet been decided to be a tap or a pan.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    /// Where the press started, in logical coordinates.
    from: Vec2,
    /// The camera origin at the moment of the press, so panning is computed
    /// from the press rather than accumulated frame by frame (which drifts).
    camera_origin: Vec2,
    /// Set once the pointer has travelled past [`DRAG_THRESHOLD`]: from then on
    /// the release is a pan, never a placement.
    moved: bool,
}

/// Presentation-local state. Never sent to rules and never authoritative.
#[derive(Clone, Debug, PartialEq)]
pub struct TilesLocal {
    camera: Camera2D,
    /// The rotation the next placement would use. Local until the moment a
    /// `PlaceTile` command carries it.
    preview: Rotation,
    /// The keyboard cursor. Also the tap target Enter uses.
    cursor: Coord,
    hover: Option<Coord>,
    press: Option<Press>,
    viewport: Viewport,
    /// Set once the first real viewport arrives, so the opening view is centred
    /// on the start tile without `Default` having to guess a screen size.
    centred: bool,
    last_placed: Option<Coord>,
    /// Claim selection is an affordance from `View`, never a canonical follower.
    claim: Option<u8>,
    buttons: ButtonInteraction,
    button_focus: FocusState,
    /// Normalized key events can repeat; activation needs a fresh key press.
    activation_held: [bool; 3],
}

impl Default for TilesLocal {
    fn default() -> Self {
        Self {
            camera: Camera2D::default(),
            preview: Rotation::R0,
            cursor: Coord::ORIGIN,
            hover: None,
            press: None,
            viewport: Viewport::new(Vec2::splat(1.0)).expect("unit viewport is valid"),
            centred: false,
            last_placed: None,
            claim: None,
            buttons: ButtonInteraction::default(),
            button_focus: FocusState::default(),
            activation_held: [false; 3],
        }
    }
}

impl TilesLocal {
    #[must_use]
    pub const fn camera(&self) -> Camera2D {
        self.camera
    }

    #[must_use]
    pub const fn preview_rotation(&self) -> Rotation {
        self.preview
    }

    #[must_use]
    pub const fn cursor(&self) -> Coord {
        self.cursor
    }

    #[must_use]
    pub const fn hover(&self) -> Option<Coord> {
        self.hover
    }

    /// The shell calls this every frame with the measured viewport.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if self.viewport != viewport {
            // A press uses the geometry at Down. A resized toolbar cannot
            // inherit it and activate a different action on Up.
            self.press = None;
            self.hover = None;
            self.buttons = ButtonInteraction::default();
            self.button_focus.set_current(None);
        }
        self.viewport = viewport;
        if !self.centred {
            self.centred = true;
            self.recenter();
        }
    }

    /// Put the cursor's square in the middle of the available map.
    pub fn recenter(&mut self) {
        self.look_at(self.cursor);
    }

    fn look_at(&mut self, coord: Coord) {
        let centre = world_centre(coord);
        let (minimum, maximum) = board_screen_bounds(self.viewport);
        let focus = (minimum + maximum) * 0.5 / self.camera.zoom();
        self.set_origin(centre - focus);
    }

    fn set_origin(&mut self, origin: Vec2) {
        if let Ok(camera) = Camera2D::new(origin, self.camera.zoom()) {
            self.camera = camera;
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        // Keep the viewed map point stationary; the viewport centre can be
        // inside the HUD on short screens.
        let (minimum, maximum) = board_screen_bounds(self.viewport);
        let screen_focus = (minimum + maximum) * 0.5;
        let focus = self.local_to_world(screen_focus);
        if let Ok(camera) = Camera2D::new(self.camera.origin(), clamped) {
            self.camera = camera;
            let half = screen_focus / clamped;
            self.set_origin(focus - half);
        }
    }

    /// World point under a logical (screen) point.
    #[must_use]
    pub fn local_to_world(&self, point: Vec2) -> Vec2 {
        point / self.camera.zoom() + self.camera.origin()
    }

    /// The board square under a logical (screen) point, if it is inside the
    /// playable coordinate space.
    #[must_use]
    pub fn coord_at(&self, position: PointerPosition) -> Option<Coord> {
        let world = self.local_to_world(position.get());
        let x = (world.x / TILE_SIZE).floor();
        let y = (world.y / TILE_SIZE).floor();
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Coord::new(clamp_to_i16(x), clamp_to_i16(y)).ok()
    }

    /// Move the cursor and follow it with the camera if it would leave the view.
    fn move_cursor(&mut self, side: Side) {
        if let Some(next) = self.cursor.neighbour(side) {
            self.cursor = next;
            self.keep_cursor_visible();
        }
    }

    fn keep_cursor_visible(&mut self) {
        let rect = world_rect(self.cursor);
        let zoom = self.camera.zoom();
        let origin = self.camera.origin();
        let (minimum, maximum) = board_screen_bounds(self.viewport);
        let minimum = minimum / zoom;
        let maximum = maximum / zoom;
        let mut next = origin;
        if TILE_SIZE > maximum.x - minimum.x {
            next.x = world_centre(self.cursor).x - (minimum.x + maximum.x) * 0.5;
        } else if rect.origin().x < origin.x + minimum.x {
            next.x = rect.origin().x - minimum.x;
        } else if rect.origin().x + TILE_SIZE > origin.x + maximum.x {
            next.x = rect.origin().x + TILE_SIZE - maximum.x;
        }
        if TILE_SIZE > maximum.y - minimum.y {
            next.y = world_centre(self.cursor).y - (minimum.y + maximum.y) * 0.5;
        } else if rect.origin().y < origin.y + minimum.y {
            next.y = rect.origin().y - minimum.y;
        } else if rect.origin().y + TILE_SIZE > origin.y + maximum.y {
            next.y = rect.origin().y + TILE_SIZE - maximum.y;
        }
        if next != origin {
            self.set_origin(next);
        }
    }

    /// Map the shared button's activation to game meaning or local camera work.
    fn apply_control(&mut self, control: Control, view: &View) -> Option<Intent<Command>> {
        if !control.enabled(view, self) {
            return None;
        }
        match control {
            Control::ZoomIn => {
                self.set_zoom(self.camera.zoom() * ZOOM_STEP);
                None
            }
            Control::ZoomOut => {
                self.set_zoom(self.camera.zoom() / ZOOM_STEP);
                None
            }
            Control::Recenter => {
                self.recenter();
                None
            }
            Control::Rotate => {
                self.preview = self.preview.next();
                None
            }
            Control::Confirm => act_on_square(view, self, self.cursor),
            Control::Skip => Some(Intent::new(Command::SkipMeeple)),
        }
    }
}

fn clamp_to_i16(value: f32) -> i16 {
    // `as` on a float that is out of range saturates in Rust, and the clamp
    // keeps it inside `Coord`'s own bound anyway.
    #[allow(clippy::cast_possible_truncation)]
    let clamped = value.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
    clamped
}

/// The world rectangle a board square occupies.
#[must_use]
pub fn world_rect(coord: Coord) -> Rect {
    Rect::new(
        Vec2::new(
            f32::from(coord.x()) * TILE_SIZE,
            f32::from(coord.y()) * TILE_SIZE,
        ),
        Vec2::splat(TILE_SIZE),
    )
    .expect("a bounded coordinate produces finite geometry")
}

fn world_centre(coord: Coord) -> Vec2 {
    let rect = world_rect(coord);
    rect.origin() + rect.size() * 0.5
}

/// Where a follower sitting on `segment` is drawn, in world units.
///
/// The mean of the segment's edge midpoints, so a road running north–south sits
/// in the middle of the tile while a city cap sits against its own edge and two
/// segments of one tile never overlap. A monastery has no edges and sits in the
/// centre.
#[must_use]
pub fn segment_centre(coord: Coord, tile: PlacedTile, index: u8) -> Vec2 {
    let centre = world_centre(coord);
    let mut sum = Vec2::ZERO;
    let mut count = 0.0f32;
    for side in tile.segment_edges(index) {
        sum += edge_midpoint(coord, side);
        count += 1.0;
    }
    if count == 0.0 {
        return centre;
    }
    // Pulled a third of the way from the tile centre toward the segment's own
    // edges: far enough apart to distinguish two segments, near enough that a
    // follower reads as belonging to this tile.
    centre + (sum / count - centre) * 0.55
}

fn edge_midpoint(coord: Coord, side: Side) -> Vec2 {
    let rect = world_rect(coord);
    let centre = rect.origin() + rect.size() * 0.5;
    let half = TILE_SIZE * 0.5;
    match side {
        Side::North => Vec2::new(centre.x, centre.y - half),
        Side::East => Vec2::new(centre.x + half, centre.y),
        Side::South => Vec2::new(centre.x, centre.y + half),
        Side::West => Vec2::new(centre.x - half, centre.y),
    }
}

/// The Tiles presenter.
#[derive(Debug, Default)]
pub struct TilesPresentation;

impl GamePresentation for TilesPresentation {
    type Rules = TilesRules;
    type Local = TilesLocal;

    fn asset_pack() -> AssetPackRef {
        AssetPackRef::from_static("tiles", "0.1.0")
    }

    fn present(view: &View, local: &TilesLocal, frame: &FrameCtx) -> RenderList {
        build(view, local, frame).unwrap_or_else(|_| {
            RenderListBuilder::new(Camera2D::default())
                .finish()
                .expect("the empty render list is valid")
        })
    }

    fn on_view_event(
        event: &<TilesRules as GameRules>::ViewEvent,
        local: &mut TilesLocal,
        _frame: &FrameCtx,
    ) -> AudioCues {
        let mut cues = AudioCues::new();
        // A release cannot reuse an affordance from the previous projection.
        local.press = None;
        local.buttons = ButtonInteraction::default();
        match event {
            Event::TilePlaced { at, .. } => {
                local.last_placed = Some(*at);
                local.cursor = *at;
                local.claim = None;
                local.press = None;
                // The next tile gets a fresh orientation rather than inheriting
                // the last one, which is what a player at a table does.
                local.preview = Rotation::R0;
                cues.push(AudioCue::from_static("tile-place"));
            }
            Event::MeeplePlaced { .. } => cues.push(AudioCue::from_static("token-drop")),
            Event::FeatureScored { .. } | Event::FinalScored { .. } => {
                cues.push(AudioCue::from_static("score-update"));
            }
            Event::TileDiscarded { .. } => cues.push(AudioCue::from_static("tile-discard")),
            Event::Ended { .. } => cues.push(AudioCue::from_static("game-end")),
            Event::TileDrawn { .. }
            | Event::MeepleSkipped { .. }
            | Event::TurnAutoResolved { .. }
            | Event::Paused
            | Event::Resumed => {}
        }
        cues
    }

    fn on_input(
        input: &InputEvent,
        view: &View,
        local: &mut TilesLocal,
    ) -> Option<Intent<Command>> {
        match input {
            InputEvent::Pointer {
                position,
                button,
                phase,
            } => on_pointer(*position, *button, *phase, view, local),
            InputEvent::Key { key, pressed } => {
                let activation = match key {
                    Key::Enter => Some(0),
                    Key::Space => Some(1),
                    Key::Escape => Some(2),
                    _ => None,
                };
                if !local.button_focus.is_window_focused() {
                    if !pressed {
                        if let Some(index) = activation {
                            local.activation_held[index] = false;
                        }
                    }
                    return None;
                }
                if let Some(index) = activation {
                    let repeated = local.activation_held[index];
                    local.activation_held[index] = *pressed;
                    if repeated || !pressed {
                        return None;
                    }
                }
                if *pressed {
                    local.press = None;
                    local.buttons = ButtonInteraction::default();
                }
                (*pressed && local.button_focus.is_window_focused())
                    .then(|| on_key(*key, view, local))
                    .flatten()
            }
            InputEvent::Focus(focused) => {
                // Losing focus mid-drag must not leave a phantom pan armed.
                local.press = None;
                local.hover = None;
                local.button_focus.set_window_focused(*focused);
                local.buttons = ButtonInteraction::default();
                if !focused {
                    // The OS may deliver the release to another window.
                    local.activation_held = [false; 3];
                }
                None
            }
        }
    }

    fn a11y(view: &View, local: &TilesLocal) -> A11yDescription {
        describe(view, local)
    }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn on_pointer(
    position: PointerPosition,
    button: PointerButton,
    phase: PointerPhase,
    view: &View,
    local: &mut TilesLocal,
) -> Option<Intent<Command>> {
    if !local.button_focus.is_window_focused() {
        return None;
    }
    let point = position.get();
    let buttons = hud_buttons(view, local, local.viewport, Positive::new(44.0).ok()?).ok()?;
    let graph = FocusGraph::new(
        buttons
            .iter()
            .map(|button| FocusNode::new(button.id(), button.rect()))
            .collect(),
    )
    .ok()?;
    let action = local.buttons.on_input(
        &InputEvent::Pointer {
            position,
            button,
            phase,
        },
        &buttons,
        &graph,
        &mut local.button_focus,
    );
    if let NavigationAction::Activate(id) = action {
        local.press = None;
        let control = Control::ALL
            .into_iter()
            .find(|control| control.id() == id)?;
        return local.apply_control(control, view);
    }
    match phase {
        PointerPhase::Down => {
            local.press = None;
            if button == PointerButton::Secondary {
                // Right-click rotates: the fastest possible rotate control, and
                // it needs no screen real estate.
                local.buttons = ButtonInteraction::default();
                return local.apply_control(Control::Rotate, view);
            }
            if button != PointerButton::Primary || hud_contains(view, local.viewport, point) {
                local.hover = None;
                return None;
            }
            local.press = Some(Press {
                from: point,
                camera_origin: local.camera.origin(),
                moved: false,
            });
            local.hover = local.coord_at(position);
            None
        }
        PointerPhase::Move => {
            local.hover = (!hud_contains(view, local.viewport, point))
                .then(|| local.coord_at(position))
                .flatten();
            let press = local.press.as_mut()?;
            let travelled = (point - press.from).length();
            if travelled >= DRAG_THRESHOLD {
                press.moved = true;
            }
            if press.moved {
                let from = press.from;
                let camera_origin = press.camera_origin;
                let delta = (point - from) / local.camera.zoom();
                local.set_origin(camera_origin - delta);
            }
            None
        }
        PointerPhase::Cancel => {
            local.press = None;
            local.hover = None;
            None
        }
        PointerPhase::Up => {
            let press = local.press.take()?;
            if button != PointerButton::Primary {
                return None;
            }
            // A pan is not a tap: releasing after dragging the board must not
            // also place a tile.
            if press.moved
                || (point - press.from).length() >= DRAG_THRESHOLD
                || hud_contains(view, local.viewport, point)
            {
                return None;
            }
            let coord = local.coord_at(position)?;
            local.cursor = coord;
            if view.phase == TurnPhase::PlaceMeeple && view.last_placed == Some(coord) {
                let tile = view.board.get(coord)?;
                local.claim =
                    nearest_claimable_segment(view, coord, tile, local.local_to_world(point));
            }
            act_on_square(view, local, coord)
        }
    }
}

/// What tapping a board square means, which depends only on the phase.
fn act_on_square(view: &View, local: &TilesLocal, coord: Coord) -> Option<Intent<Command>> {
    if !on_turn(view) {
        return None;
    }
    match view.phase {
        TurnPhase::PlaceTile => {
            let kind = view.drawn?;
            let tile = PlacedTile::new(kind, local.preview);
            crate::rules::is_legal_placement(&view.board, coord, tile).then(|| {
                Intent::new(Command::PlaceTile {
                    at: coord,
                    rotation: local.preview,
                })
            })
        }
        TurnPhase::PlaceMeeple => {
            // A claim is only ever on the tile just placed, so a tap anywhere
            // else in this phase is not a claim.
            let last = view.last_placed?;
            if last != coord {
                return None;
            }
            let segment = selected_claim(view, local)?;
            Some(Intent::new(Command::PlaceMeeple { segment }))
        }
    }
}

/// Of the segments the seat may claim on `coord`, the one nearest the tap.
///
/// Tapping a tile with several claimable features has to pick one; "nearest the
/// tap" is the only choice a player can predict.
fn nearest_claimable_segment(
    view: &View,
    coord: Coord,
    tile: PlacedTile,
    reference: Vec2,
) -> Option<u8> {
    view.meeple_slots.iter().copied().min_by(|left, right| {
        let a = segment_centre(coord, tile, *left).distance_squared(reference);
        let b = segment_centre(coord, tile, *right).distance_squared(reference);
        a.partial_cmp(&b)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(left.cmp(right))
    })
}

fn on_turn(view: &View) -> bool {
    view.you == Some(view.turn) && view.status == crate::rules::Status::Playing && !view.paused
}

fn selected_claim(view: &View, local: &TilesLocal) -> Option<u8> {
    local
        .claim
        .filter(|segment| view.meeple_slots.contains(segment))
        .or_else(|| view.meeple_slots.first().copied())
}

fn on_key(key: Key, view: &View, local: &mut TilesLocal) -> Option<Intent<Command>> {
    match key {
        Key::ArrowUp => {
            local.move_cursor(Side::North);
            None
        }
        Key::ArrowDown => {
            local.move_cursor(Side::South);
            None
        }
        Key::ArrowLeft => {
            local.move_cursor(Side::West);
            None
        }
        Key::ArrowRight => {
            local.move_cursor(Side::East);
            None
        }
        Key::Space => local.apply_control(Control::Rotate, view),
        Key::Tab => {
            if view.phase == TurnPhase::PlaceMeeple {
                let next = selected_claim(view, local)
                    .and_then(|current| view.meeple_slots.iter().position(|slot| *slot == current))
                    .and_then(|index| view.meeple_slots.get(index + 1))
                    .or_else(|| view.meeple_slots.first());
                local.claim = next.copied();
                if let Some(last) = view.last_placed {
                    local.cursor = last;
                    local.keep_cursor_visible();
                }
                return None;
            }
            // Jump to the next square where the tile would actually fit at the
            // current rotation, so a keyboard player is never hunting.
            if let Some(next) = next_legal_square(view, local) {
                local.cursor = next;
                local.keep_cursor_visible();
            }
            None
        }
        Key::Escape => local.apply_control(Control::Skip, view),
        Key::Enter => {
            let cursor = local.cursor;
            act_on_square(view, local, cursor)
        }
    }
}

/// The next square after the cursor, in canonical order, where the previewed
/// tile is legal — wrapping round.
fn next_legal_square(view: &View, local: &TilesLocal) -> Option<Coord> {
    let kind = view.drawn?;
    let squares: Vec<Coord> = legal_placements(&view.board, kind)
        .into_iter()
        .filter(|(_, rotations)| rotations.contains(&local.preview))
        .map(|(coord, _)| coord)
        .collect();
    if squares.is_empty() {
        // Nothing fits at this rotation; offer any legal square instead so Tab
        // is never a dead key.
        return legal_placements(&view.board, kind)
            .into_iter()
            .map(|(coord, _)| coord)
            .find(|coord| *coord > local.cursor)
            .or_else(|| {
                legal_placements(&view.board, kind)
                    .into_iter()
                    .map(|(coord, _)| coord)
                    .next()
            });
    }
    squares
        .iter()
        .copied()
        .find(|coord| *coord > local.cursor)
        .or_else(|| squares.first().copied())
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn build(view: &View, local: &TilesLocal, frame: &FrameCtx) -> Result<RenderList, RenderListError> {
    let theme = frame.theme();
    let viewport = frame.viewport();
    let mut builder = RenderListBuilder::new(local.camera);

    draw_targets(&mut builder, view, local, &theme)?;
    draw_board(&mut builder, view, &theme)?;
    draw_followers(&mut builder, view, &theme)?;
    draw_overlays(&mut builder, view, local, &theme)?;
    draw_hud(&mut builder, view, local, viewport, &theme)?;

    builder.finish()
}

/// Legal squares for the tile in hand, at the previewed rotation.
fn draw_targets(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &TilesLocal,
    theme: &Theme,
) -> Result<(), RenderListError> {
    let Some(kind) = view.drawn else {
        return Ok(());
    };
    if view.phase != TurnPhase::PlaceTile {
        return Ok(());
    }
    for (coord, rotations) in legal_placements(&view.board, kind) {
        let fits_now = rotations.contains(&local.preview);
        let rect = world_rect(coord);
        let colour = if fits_now {
            theme.color.legal_target
        } else {
            theme.color.turn_waiting
        };
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(2.0)?,
            fill: None,
            border: Some(Border::new(if fits_now { 2.0 } else { 1.0 }, colour)?),
            layer: Layer::BOARD,
            z: 0,
        })?;
        builder.push(RenderCmd::Text {
            text: if fits_now { "Place" } else { "Rotate" }.to_owned(),
            at: rect.origin() + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5 - 8.0),
            style: TextStyleToken::LabelSm,
            align: Align::Center,
            max_width: Positive::new(TILE_SIZE - 8.0).ok(),
            color: theme.color.on_surface_variant,
            layer: Layer::BOARD,
            z: 1,
        })?;
    }
    Ok(())
}

fn draw_board(
    builder: &mut RenderListBuilder,
    view: &View,
    theme: &Theme,
) -> Result<(), RenderListError> {
    for (coord, tile) in view.board.iter() {
        let rect = world_rect(coord);
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(1.0)?,
            fill: Some(Paint::Solid(theme.color.surface_container)),
            border: Some(Border::new(1.0, theme.color.outline)?),
            layer: Layer::PIECES,
            z: 0,
        })?;

        // Edge terrain, so adjacency is readable at a glance: a city edge is a
        // solid block against the tile border, a road edge a bar reaching the
        // middle, a field edge nothing at all.
        for side in Side::ALL {
            match tile.terrain(side) {
                Terrain::City => builder.push(city_edge(coord, side, theme)?)?,
                Terrain::Road => builder.push(road_edge(coord, side, theme)?)?,
                Terrain::Field => {}
            }
        }

        for index in 0..tile.segment_count() {
            let Some(def) = tile.segment(index) else {
                continue;
            };
            if def.kind == FeatureKind::Monastery {
                let centre = world_centre(coord);
                let size = Vec2::splat(TILE_SIZE * 0.28);
                builder.push(RenderCmd::Rect {
                    rect: Rect::new(centre - size * 0.5, size)?,
                    radii: Corners::uniform(2.0)?,
                    fill: Some(Paint::Solid(theme.color.on_surface_variant)),
                    border: None,
                    layer: Layer::PIECES,
                    z: 2,
                })?;
            }
            if def.pennant {
                let centre = world_centre(coord);
                let size = Vec2::splat(TILE_SIZE * 0.16);
                builder.push(RenderCmd::Rect {
                    rect: Rect::new(centre - size * 0.5 + Vec2::splat(TILE_SIZE * 0.2), size)?,
                    radii: Corners::uniform(size.x * 0.5)?,
                    fill: Some(Paint::Solid(theme.color.success)),
                    border: None,
                    layer: Layer::PIECES,
                    z: 3,
                })?;
            }
        }
    }
    Ok(())
}

fn city_edge(coord: Coord, side: Side, theme: &Theme) -> Result<RenderCmd, RenderListError> {
    let rect = world_rect(coord);
    let band = TILE_SIZE * 0.22;
    let geometry = match side {
        Side::North => Rect::new(rect.origin(), Vec2::new(TILE_SIZE, band)),
        Side::East => Rect::new(
            rect.origin() + Vec2::new(TILE_SIZE - band, 0.0),
            Vec2::new(band, TILE_SIZE),
        ),
        Side::South => Rect::new(
            rect.origin() + Vec2::new(0.0, TILE_SIZE - band),
            Vec2::new(TILE_SIZE, band),
        ),
        Side::West => Rect::new(rect.origin(), Vec2::new(band, TILE_SIZE)),
    }?;
    Ok(RenderCmd::Rect {
        rect: geometry,
        radii: Corners::uniform(0.0)?,
        fill: Some(Paint::Solid(theme.color.primary)),
        border: None,
        layer: Layer::PIECES,
        z: 1,
    })
}

fn road_edge(coord: Coord, side: Side, theme: &Theme) -> Result<RenderCmd, RenderListError> {
    let centre = world_centre(coord);
    let width = TILE_SIZE * 0.1;
    let midpoint = edge_midpoint(coord, side);
    let (bar_origin, bar_size) = if matches!(side, Side::North | Side::South) {
        let top = centre.y.min(midpoint.y);
        (
            Vec2::new(centre.x - width * 0.5, top),
            Vec2::new(width, (midpoint.y - centre.y).abs()),
        )
    } else {
        let left = centre.x.min(midpoint.x);
        (
            Vec2::new(left, centre.y - width * 0.5),
            Vec2::new((midpoint.x - centre.x).abs(), width),
        )
    };
    Ok(RenderCmd::Rect {
        rect: Rect::new(bar_origin, bar_size)?,
        radii: Corners::uniform(0.0)?,
        fill: Some(Paint::Solid(theme.color.on_surface_variant)),
        border: None,
        layer: Layer::PIECES,
        z: 1,
    })
}

fn draw_followers(
    builder: &mut RenderListBuilder,
    view: &View,
    theme: &Theme,
) -> Result<(), RenderListError> {
    for (segment, seat) in &view.followers {
        let Some(tile) = view.board.get(segment.coord()) else {
            continue;
        };
        let centre = segment_centre(segment.coord(), tile, segment.index());
        let size = Vec2::splat(TILE_SIZE * 0.24);
        builder.push(RenderCmd::Rect {
            rect: Rect::new(centre - size * 0.5, size)?,
            radii: Corners::uniform(size.x * 0.5)?,
            fill: Some(Paint::Solid(seat_colour(view, *seat, theme))),
            border: Some(Border::new(1.5, theme.color.on_surface)?),
            layer: Layer::OVERLAY,
            z: 10,
        })?;
    }
    Ok(())
}

fn draw_overlays(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &TilesLocal,
    theme: &Theme,
) -> Result<(), RenderListError> {
    if let Some(at) = view.last_placed {
        builder.push(RenderCmd::Rect {
            rect: world_rect(at),
            radii: Corners::uniform(2.0)?,
            fill: None,
            border: Some(Border::new(3.0, theme.color.last_action)?),
            layer: Layer::OVERLAY,
            z: 0,
        })?;
    }

    // The ghost of the tile about to be placed, at the cursor: a real preview of
    // the tile in hand, bordered by whether it would be accepted there.
    //
    // Skipped on an occupied square. An opaque ghost over a placed tile hides
    // the board the player is reading, and the square can never be a target
    // anyway — the cursor ring below is enough to say where they are.
    if view.phase == TurnPhase::PlaceTile {
        if let Some(kind) = view.drawn {
            let target = local.hover.unwrap_or(local.cursor);
            if !view.board.contains(target) {
                let tile = PlacedTile::new(kind, local.preview);
                let legal = crate::rules::is_legal_placement(&view.board, target, tile);
                builder.push(RenderCmd::Rect {
                    rect: world_rect(target),
                    radii: Corners::uniform(2.0)?,
                    // The same fill a placed tile has, so the preview reads as
                    // the tile itself rather than as a coloured hole.
                    fill: Some(Paint::Solid(theme.color.surface_container)),
                    border: Some(Border::new(
                        3.0,
                        if legal {
                            theme.color.legal_target
                        } else {
                            theme.color.illegal_target
                        },
                    )?),
                    layer: Layer::OVERLAY,
                    z: 5,
                })?;
                // Its edges too, so the rotation is visible before it commits.
                for side in Side::ALL {
                    match tile.terrain(side) {
                        Terrain::City => builder.push(city_edge(target, side, theme)?)?,
                        Terrain::Road => builder.push(road_edge(target, side, theme)?)?,
                        Terrain::Field => {}
                    }
                }
            }
        }
    }

    // Claim slots on the tile just placed.
    if view.phase == TurnPhase::PlaceMeeple {
        if let Some(at) = view.last_placed {
            if let Some(tile) = view.board.get(at) {
                for index in &view.meeple_slots {
                    let centre = segment_centre(at, tile, *index);
                    let size = Vec2::splat(TILE_SIZE * 0.3);
                    builder.push(RenderCmd::Rect {
                        rect: Rect::new(centre - size * 0.5, size)?,
                        radii: Corners::uniform(size.x * 0.5)?,
                        fill: None,
                        border: Some(Border::new(
                            if selected_claim(view, local) == Some(*index) {
                                3.0
                            } else {
                                2.0
                            },
                            theme.color.legal_target,
                        )?),
                        layer: Layer::OVERLAY,
                        z: 8,
                    })?;
                    builder.push(RenderCmd::Text {
                        text: (index + 1).to_string(),
                        at: centre - Vec2::new(0.0, 8.0),
                        style: TextStyleToken::LabelSm,
                        align: Align::Center,
                        max_width: Positive::new(size.x).ok(),
                        color: theme.color.on_surface,
                        layer: Layer::OVERLAY,
                        z: 9,
                    })?;
                }
            }
        }
    }

    draw_cursor(builder, local, theme)
}

fn draw_cursor(
    builder: &mut RenderListBuilder,
    local: &TilesLocal,
    theme: &Theme,
) -> Result<(), RenderListError> {
    // The cursor, always visible so keyboard play has a focus indicator.
    builder.push(RenderCmd::Rect {
        rect: world_rect(local.cursor),
        radii: Corners::uniform(2.0)?,
        fill: None,
        border: Some(Border::new(
            theme.focus.ring_width.get(),
            theme.focus.ring_color,
        )?),
        layer: Layer::OVERLAY,
        z: 20,
    })?;
    let (minimum, maximum) = board_screen_bounds(local.viewport);
    if TILE_SIZE * local.camera.zoom() > (maximum - minimum).min_element() {
        // When the square is larger than an available axis, its perimeter
        // may sit beneath the HUD. Retain a small, unfilled focus marker at
        // the centred cursor, with a stable logical size at every zoom.
        let size = Vec2::splat(16.0 / local.camera.zoom());
        builder.push(RenderCmd::Rect {
            rect: Rect::new(world_centre(local.cursor) - size * 0.5, size)?,
            radii: Corners::uniform(2.0 / local.camera.zoom())?,
            fill: None,
            border: Some(Border::new(
                theme.focus.ring_width.get() / local.camera.zoom(),
                theme.focus.ring_color,
            )?),
            layer: Layer::OVERLAY,
            z: 21,
        })?;
    }
    Ok(())
}

/// The screen-fixed HUD.
///
/// Wrapped in the camera's inverse so the backend's `camera × local`
/// composition is the identity: the HUD keeps its logical position and its text
/// keeps its size at every zoom. See the module docs.
fn draw_hud(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &TilesLocal,
    viewport: Viewport,
    theme: &Theme,
) -> Result<(), RenderListError> {
    let inverse = camera_inverse(local.camera);
    builder.push(RenderCmd::PushTransform {
        matrix: inverse,
        layer: Layer::HUD,
        z: 0,
    })?;

    let status = Rect::new(Vec2::ZERO, Vec2::new(viewport.size().x, HUD_STATUS_HEIGHT))?;
    builder.push(RenderCmd::Rect {
        rect: status,
        radii: Corners::uniform(0.0)?,
        fill: Some(Paint::Solid(theme.color.surface_container_high)),
        border: None,
        layer: Layer::HUD,
        z: 0,
    })?;
    builder.push(RenderCmd::Text {
        text: status_line(view),
        at: Vec2::new(HUD_GAP, 8.0),
        style: TextStyleToken::TitleMd,
        align: Align::Start,
        max_width: Positive::new(viewport.size().x - HUD_GAP * 2.0).ok(),
        color: theme.color.on_surface,
        layer: Layer::HUD,
        z: 1,
    })?;
    builder.push(RenderCmd::Text {
        text: selection_line(view, local),
        at: Vec2::new(HUD_GAP, 38.0),
        style: TextStyleToken::LabelMd,
        align: Align::Start,
        max_width: Positive::new(viewport.size().x - HUD_GAP * 2.0).ok(),
        color: theme.color.on_surface_variant,
        layer: Layer::HUD,
        z: 1,
    })?;

    draw_scores(builder, view, viewport, theme)?;

    if let Some(rect) = turn_toolbar_rect(viewport) {
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(theme.shape.card.get())?,
            fill: Some(Paint::Solid(theme.color.surface_container)),
            border: None,
            layer: Layer::HUD,
            z: 0,
        })?;
        if viewport.size().y >= 400.0 {
            builder.push(RenderCmd::Text {
                text: action_hint(view, local),
                at: rect.origin() + Vec2::new(HUD_GAP, HUD_GAP),
                style: TextStyleToken::LabelSm,
                align: Align::Start,
                max_width: Positive::new(rect.size().x - HUD_GAP * 2.0).ok(),
                color: theme.color.on_surface_variant,
                layer: Layer::HUD,
                z: 1,
            })?;
        }
    }
    for button in hud_buttons(view, local, viewport, theme.density.min_target)? {
        button.draw(
            builder,
            theme,
            &local.buttons,
            &local.button_focus,
            Layer::HUD,
        )?;
    }

    builder.push(RenderCmd::PopTransform {
        layer: Layer::HUD,
        z: 0,
    })?;
    Ok(())
}

fn draw_scores(
    builder: &mut RenderListBuilder,
    view: &View,
    viewport: Viewport,
    theme: &Theme,
) -> Result<(), RenderListError> {
    // A contained score list on wide boards; a compact row leaves mobile maps
    // their width. Seat labels carry meaning independently of marker colours.
    let compact = viewport.size().x < 600.0 || viewport.size().y < 400.0;
    if let Some(rect) = scores_rect(view, viewport) {
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(theme.shape.card.get())?,
            fill: Some(Paint::Solid(theme.color.surface_container)),
            border: None,
            layer: Layer::HUD,
            z: 0,
        })?;
    }
    for (index, seat) in view.seats.iter().enumerate() {
        let Some(scores) = scores_rect(view, viewport) else {
            break;
        };
        let step = u16::try_from(index).map_or(0.0, f32::from);
        let y = scores.origin().y + HUD_GAP + if compact { 0.0 } else { 40.0 * step };
        let count = u16::try_from(view.seats.len())
            .map_or(1.0, f32::from)
            .max(1.0);
        let x = if compact {
            HUD_GAP + viewport.size().x / count * step
        } else {
            scores.origin().x + HUD_GAP
        };
        let active =
            *seat == view.turn && view.status == crate::rules::Status::Playing && !view.paused;
        let colour = if active {
            theme.color.turn_active
        } else {
            theme.color.on_surface_variant
        };
        builder.push(RenderCmd::Text {
            text: if compact {
                format!(
                    "S{}: {}",
                    seat.0,
                    view.scores.get(seat).copied().unwrap_or(0)
                )
            } else {
                format!(
                    "Seat {}: {} points",
                    seat.0,
                    view.scores.get(seat).copied().unwrap_or(0)
                )
            },
            at: Vec2::new(x, y),
            style: TextStyleToken::LabelSm,
            align: Align::Start,
            max_width: Positive::new(if compact {
                viewport.size().x / count - HUD_GAP
            } else {
                scores.size().x - HUD_GAP * 2.0
            })
            .ok(),
            color: colour,
            layer: Layer::HUD,
            z: 1,
        })?;
        if !compact {
            builder.push(RenderCmd::Rect {
                rect: Rect::new(Vec2::new(x, y + 20.0), Vec2::splat(10.0))?,
                radii: Corners::uniform(5.0)?,
                fill: Some(Paint::Solid(seat_colour(view, *seat, theme))),
                border: None,
                layer: Layer::HUD,
                z: 1,
            })?;
        }
        builder.push(RenderCmd::Text {
            text: format!(
                "{} followers{}",
                view.meeples_in_hand.get(seat).copied().unwrap_or(0),
                if active && !compact { ". On turn" } else { "" }
            ),
            at: Vec2::new(x + if compact { 0.0 } else { 16.0 }, y + 18.0),
            style: TextStyleToken::LabelSm,
            align: Align::Start,
            max_width: Positive::new(if compact {
                viewport.size().x / count - HUD_GAP
            } else {
                scores.size().x - 32.0
            })
            .ok(),
            color: theme.color.on_surface_variant,
            layer: Layer::HUD,
            z: 1,
        })?;
    }

    Ok(())
}

fn hud_buttons<'a>(
    view: &View,
    local: &TilesLocal,
    viewport: Viewport,
    min_target: Positive,
) -> Result<Vec<ActionButton<'a>>, RenderListError> {
    Control::ALL
        .into_iter()
        .filter_map(|control| control.rect(viewport).map(|rect| (control, rect)))
        .map(|(control, rect)| {
            let mut button =
                ActionButton::new(control.id(), rect, control.label(view.phase), min_target)?
                    .enabled(control.enabled(view, local));
            button = match control {
                Control::ZoomIn => button.with_icon("+").shape(ButtonShape::ConnectedStart),
                Control::ZoomOut => button.with_icon("-").shape(ButtonShape::ConnectedMiddle),
                Control::Recenter => button.with_icon("@").shape(ButtonShape::ConnectedEnd),
                Control::Rotate => button.with_icon("R"),
                Control::Confirm => button.tone(ButtonTone::Filled),
                Control::Skip => button,
            };
            Ok(button)
        })
        .collect()
}

fn turn_toolbar_rect(viewport: Viewport) -> Option<Rect> {
    let first = Control::Rotate.rect(viewport)?;
    let last = Control::Skip.rect(viewport)?;
    let heading = if viewport.size().y < 400.0 {
        HUD_GAP
    } else {
        32.0
    };
    Rect::new(
        first.origin() - Vec2::new(HUD_GAP, heading),
        Vec2::new(
            last.origin().x + last.size().x - first.origin().x + HUD_GAP * 2.0,
            HUD_BUTTON + heading + HUD_GAP,
        ),
    )
    .ok()
}

fn scores_rect(view: &View, viewport: Viewport) -> Option<Rect> {
    if viewport.size().x < 600.0 || viewport.size().y < 400.0 {
        if viewport.size().y < 320.0 {
            return None;
        }
        return Rect::new(
            Vec2::new(0.0, HUD_STATUS_HEIGHT + HUD_BUTTON + HUD_GAP * 2.0),
            Vec2::new(viewport.size().x, 48.0),
        )
        .ok();
    }
    let count = u16::try_from(view.seats.len()).ok().map(f32::from)?;
    let height = count * 40.0 + HUD_GAP * 2.0;
    let top = HUD_STATUS_HEIGHT + HUD_GAP;
    if top + height > viewport.size().y - HUD_BUTTON - 48.0 {
        return None;
    }
    Rect::new(
        Vec2::new(viewport.size().x - 192.0 - HUD_GAP, top),
        Vec2::new(192.0, height),
    )
    .ok()
}

fn board_screen_bounds(viewport: Viewport) -> (Vec2, Vec2) {
    let row_scores =
        (viewport.size().x < 600.0 || viewport.size().y < 400.0) && viewport.size().y >= 320.0;
    let top = HUD_STATUS_HEIGHT + HUD_BUTTON + HUD_GAP * 2.0 + if row_scores { 48.0 } else { 0.0 };
    let bottom = viewport.size().y
        - HUD_BUTTON
        - if viewport.size().y < 400.0 {
            HUD_GAP * 2.0
        } else {
            40.0
        };
    let right = if viewport.size().x >= 600.0 && viewport.size().y >= 400.0 {
        viewport.size().x - 200.0
    } else {
        viewport.size().x
    };
    (
        Vec2::new(0.0, top),
        Vec2::new(right.max(1.0), bottom.max(top + 1.0)),
    )
}

fn hud_contains(view: &View, viewport: Viewport, point: Vec2) -> bool {
    point.y < HUD_STATUS_HEIGHT
        || Control::at(viewport, point).is_some()
        || turn_toolbar_rect(viewport).is_some_and(|rect| rect.contains(point))
        || scores_rect(view, viewport).is_some_and(|rect| rect.contains(point))
}

fn selection_line(view: &View, local: &TilesLocal) -> String {
    match view.phase {
        TurnPhase::PlaceTile => format!(
            "{} / {:?} / Cursor {},{}",
            view.drawn.map_or("No tile", |kind| kind.def().name),
            local.preview,
            local.cursor.x(),
            local.cursor.y()
        ),
        TurnPhase::PlaceMeeple => selected_claim(view, local).map_or_else(
            || "No claimable feature. Pass to finish the turn.".to_owned(),
            |segment| format!("Feature {} / Tab changes feature.", segment + 1),
        ),
    }
}

fn action_hint(view: &View, local: &TilesLocal) -> String {
    if view.status != crate::rules::Status::Playing {
        return "Match finished. Camera controls remain available.".to_owned();
    }
    if view.paused {
        return "Paused. Placement and claims are unavailable.".to_owned();
    }
    if view.you != Some(view.turn) {
        return "Waiting for the active seat. Camera controls remain available.".to_owned();
    }
    match view.phase {
        TurnPhase::PlaceTile if !Control::Confirm.enabled(view, local) => {
            "Choose a valid square (Tab). Space rotates.".to_owned()
        }
        TurnPhase::PlaceTile => "Enter places the tile. Space rotates.".to_owned(),
        TurnPhase::PlaceMeeple if view.meeple_slots.is_empty() => {
            "No claimable feature. Pass (Esc) finishes the turn.".to_owned()
        }
        TurnPhase::PlaceMeeple => "Tab chooses. Enter claims. Esc passes.".to_owned(),
    }
}

/// The affine that undoes the camera, so a HUD scope composes to the identity.
///
/// The camera is a uniform positive scale plus a translation, so its inverse
/// always exists and is itself a uniform positive scale plus a translation —
/// which matters because the Macroquad backend only draws text under a
/// uniform-positive-scale transform.
fn camera_inverse(camera: Camera2D) -> Affine2 {
    Affine2::from_scale_angle_translation(
        Vec2::splat(camera.zoom()),
        0.0,
        -camera.origin() * camera.zoom(),
    )
    .inverse()
}

fn seat_colour(view: &View, seat: tabula_core::SeatId, theme: &Theme) -> tabula_design::Color {
    let index = view
        .seats
        .iter()
        .position(|candidate| *candidate == seat)
        .unwrap_or(0);
    theme.color.seat_marker[index % theme.color.seat_marker.len()]
}

fn status_line(view: &View) -> String {
    match view.status {
        crate::rules::Status::Ended => "Match over.".to_owned(),
        crate::rules::Status::Aborted => "Match cancelled.".to_owned(),
        crate::rules::Status::Playing if view.paused => "Paused.".to_owned(),
        crate::rules::Status::Playing => {
            let step = match view.phase {
                TurnPhase::PlaceTile => "Place tile",
                TurnPhase::PlaceMeeple => "Claim or pass",
            };
            format!(
                "Seat {}: {step}. {} tiles left.",
                view.turn.0, view.bag_remaining
            )
        }
    }
}

fn describe(view: &View, local: &TilesLocal) -> A11yDescription {
    let mut description = A11yDescription {
        status: status_line(view),
        regions: Vec::new(),
        actions: Vec::new(),
    };
    let _ = write!(
        description.status,
        "  Cursor at {},{}.",
        local.cursor.x(),
        local.cursor.y()
    );
    let _ = write!(
        description.status,
        " {} {}",
        selection_line(view, local),
        action_hint(view, local)
    );
    for seat in &view.seats {
        let _ = write!(
            description.status,
            " Seat {}: {} points, {} followers.",
            seat.0,
            view.scores.get(seat).copied().unwrap_or(0),
            view.meeples_in_hand.get(seat).copied().unwrap_or(0)
        );
    }

    let can_place = on_turn(view)
        && view.phase == TurnPhase::PlaceTile
        && view.drawn.is_some_and(|kind| {
            crate::rules::is_legal_placement(
                &view.board,
                local.cursor,
                PlacedTile::new(kind, local.preview),
            )
        });

    description.actions.push(A11yAction {
        id: ActionId("claim-follower".to_owned()),
        label: selected_claim(view, local).map_or_else(
            || "Claim a follower (no claimable feature)".to_owned(),
            |segment| {
                format!(
                    "Claim follower feature {} (Enter; Tab changes feature)",
                    segment + 1
                )
            },
        ),
        enabled: on_turn(view)
            && view.phase == TurnPhase::PlaceMeeple
            && local.cursor == view.last_placed.unwrap_or(Coord::ORIGIN)
            && selected_claim(view, local).is_some(),
    });
    description.actions.push(A11yAction {
        id: ActionId("place-tile".to_owned()),
        label: format!(
            "Place the tile at {},{} (rotation {:?})",
            local.cursor.x(),
            local.cursor.y(),
            local.preview
        ),
        enabled: can_place,
    });
    description.actions.push(A11yAction {
        id: ActionId("skip-follower".to_owned()),
        label: "Pass on placing a follower".to_owned(),
        enabled: Control::Skip.enabled(view, local),
    });
    for control in Control::ALL {
        if matches!(control, Control::Skip | Control::Confirm) {
            continue;
        }
        description.actions.push(A11yAction {
            id: ActionId(control.action(view.phase).to_owned()),
            label: control.label(view.phase).to_owned(),
            enabled: control.enabled(view, local),
        });
    }
    description
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{
        DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster,
        UserId, Viewer,
    };
    use tabula_design::ThemeKind;
    use tabula_game_api::{Budget, Ctx, Input};
    use tabula_presentation::Dpi;

    fn roster(count: u8) -> SeatRoster {
        SeatRoster::new(
            (0..count)
                .map(|index| SeatEntry {
                    seat: SeatId(index),
                    occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                    team: None,
                })
                .collect(),
        )
        .expect("fixture seats are unique")
    }

    fn frame(now_ms: u64) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::new(800.0, 600.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            now_ms,
            Theme::by_kind(ThemeKind::Light),
        )
    }

    fn opening() -> (crate::rules::State, View) {
        opening_with_seed(21)
    }

    fn opening_with_seed(byte: u8) -> (crate::rules::State, View) {
        let seed = MatchSeed::from_bytes([byte; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let state = TilesRules::create(
            &crate::rules::Config {
                turn_deadline_ms: 0,
            },
            &roster(3),
            &mut ctx,
        )
        .expect("valid setup")
        .state;
        let view = TilesRules::project(&state, Viewer::Seat(SeatId(0)));
        (state, view)
    }

    fn local_for(frame: &FrameCtx) -> TilesLocal {
        let mut local = TilesLocal::default();
        local.set_viewport(frame.viewport());
        local
    }

    fn click(point: Vec2, phase: PointerPhase) -> InputEvent {
        InputEvent::Pointer {
            position: PointerPosition::new(point).expect("finite pointer"),
            button: PointerButton::Primary,
            phase,
        }
    }

    fn tap(point: Vec2, view: &View, local: &mut TilesLocal) -> Option<Intent<Command>> {
        assert!(
            TilesPresentation::on_input(&click(point, PointerPhase::Down), view, local).is_none()
        );
        TilesPresentation::on_input(&click(point, PointerPhase::Up), view, local)
    }

    fn key_tap(key: Key, view: &View, local: &mut TilesLocal) -> Option<Intent<Command>> {
        TilesPresentation::on_input(
            &InputEvent::Key {
                key,
                pressed: false,
            },
            view,
            local,
        );
        TilesPresentation::on_input(&InputEvent::Key { key, pressed: true }, view, local)
    }

    fn claim_step_with_seed(byte: u8) -> (crate::rules::State, View) {
        let (mut state, _) = opening_with_seed(byte);
        let (at, rotation) = crate::rules::first_legal_placement(
            state.board(),
            state.drawn().expect("opening tile"),
        )
        .expect("opening has a placement");
        let seed = MatchSeed::from_bytes([byte; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(1));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(1),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let seat = state.turn();
        TilesRules::apply(
            &mut state,
            Input::Player {
                seat,
                command: Command::PlaceTile { at, rotation },
            },
            &mut ctx,
        )
        .expect("legal placement");
        let view = TilesRules::project(&state, Viewer::Seat(seat));
        assert_eq!(view.phase, TurnPhase::PlaceMeeple);
        assert!(
            !view.meeple_slots.is_empty(),
            "claim fixture must exercise the claim branch"
        );
        (state, view)
    }

    #[test]
    fn the_opening_view_is_centred_on_the_start_tile() {
        let frame = frame(0);
        let local = local_for(&frame);
        // Wide HUD reserves 200dp for scores; the map spans y136..504.
        let centre = local.local_to_world(Vec2::new(300.0, 320.0));
        assert!((centre - world_centre(Coord::ORIGIN)).length() < 0.001);
    }

    /// Pointer mapping must be the exact inverse of the drawing transform, or
    /// a player's tap lands on a different square from the one they saw.
    #[test]
    fn a_pointer_maps_back_to_the_square_it_was_taken_from() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        for zoom in [MIN_ZOOM, 0.5, 1.0, 2.0, MAX_ZOOM] {
            local.set_zoom(zoom);
            for x in -3..=3 {
                for y in -3..=3 {
                    let coord = Coord::new(x, y).unwrap();
                    let world = world_centre(coord);
                    let screen = (world - local.camera.origin()) * local.camera.zoom();
                    let position = PointerPosition::new(screen).unwrap();
                    assert_eq!(
                        local.coord_at(position),
                        Some(coord),
                        "zoom {zoom} lost the square at {x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    fn zoom_is_clamped_at_both_ends() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        for _ in 0..50 {
            local.set_zoom(local.camera.zoom() * ZOOM_STEP);
        }
        assert!((local.camera.zoom() - MAX_ZOOM).abs() < 0.001);
        for _ in 0..100 {
            local.set_zoom(local.camera.zoom() / ZOOM_STEP);
        }
        assert!((local.camera.zoom() - MIN_ZOOM).abs() < 0.001);
    }

    /// The HUD is screen-fixed: its logical geometry must not move with the
    /// camera. Checked through the same composition the backend performs.
    #[test]
    fn the_hud_keeps_its_logical_geometry_at_every_zoom() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();

        let mut seen: Vec<Vec<Vec2>> = Vec::new();
        for zoom in [MIN_ZOOM, 0.6, 1.0, 1.8, MAX_ZOOM] {
            local.set_zoom(zoom);
            let list = TilesPresentation::present(&view, &local, &frame);
            let camera = list.camera();
            let to_logical = Affine2::from_scale_angle_translation(
                Vec2::splat(camera.zoom()),
                0.0,
                -camera.origin() * camera.zoom(),
            );

            // Walk the flattened stream, tracking the HUD transform scope the
            // way a backend does.
            let mut transform = Affine2::IDENTITY;
            let mut positions = Vec::new();
            for command in list.commands() {
                match command {
                    RenderCmd::PushTransform { matrix, .. } => transform = *matrix,
                    RenderCmd::PopTransform { .. } => transform = Affine2::IDENTITY,
                    RenderCmd::Rect {
                        rect,
                        layer: Layer::HUD,
                        ..
                    } => {
                        let combined = to_logical * transform;
                        positions.push(combined.transform_point2(rect.origin()));
                    }
                    _ => {}
                }
            }
            assert!(!positions.is_empty(), "the HUD drew nothing at zoom {zoom}");
            seen.push(positions);
        }

        let first = &seen[0];
        for (index, positions) in seen.iter().enumerate().skip(1) {
            assert_eq!(
                positions.len(),
                first.len(),
                "the HUD changed shape at zoom step {index}"
            );
            for (a, b) in first.iter().zip(positions) {
                assert!(
                    (*a - *b).length() < 0.01,
                    "a HUD rect moved from {a:?} to {b:?} when the camera zoomed"
                );
            }
        }
    }

    #[test]
    fn a_tap_on_a_legal_square_asks_to_place_and_an_illegal_one_does_not() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        let kind = view.drawn.expect("a tile is in hand");

        let legal = legal_placements(&view.board, kind);
        let (coord, rotations) = legal.first().cloned().expect("something is playable");
        local.preview = rotations[0];

        let screen = (world_centre(coord) - local.camera.origin()) * local.camera.zoom();
        let intent = tap(screen, &view, &mut local);
        assert_eq!(
            intent.map(Intent::into_command),
            Some(Command::PlaceTile {
                at: coord,
                rotation: rotations[0]
            })
        );

        // The origin square is occupied, so tapping it asks for nothing.
        let occupied = (world_centre(Coord::ORIGIN) - local.camera.origin()) * local.camera.zoom();
        assert!(tap(occupied, &view, &mut local).is_none());
    }

    /// Dragging the board pans it and must **not** also place a tile on release
    /// — the single most annoying bug a pannable board can have.
    #[test]
    fn dragging_pans_the_camera_and_does_not_place_on_release() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        let kind = view.drawn.expect("a tile is in hand");
        let legal = legal_placements(&view.board, kind);
        let (coord, rotations) = legal.first().cloned().expect("something is playable");
        local.preview = rotations[0];

        let start = (world_centre(coord) - local.camera.origin()) * local.camera.zoom();
        let before = local.camera.origin();

        assert!(
            TilesPresentation::on_input(&click(start, PointerPhase::Down), &view, &mut local)
                .is_none()
        );
        assert!(TilesPresentation::on_input(
            &click(start + Vec2::new(80.0, -40.0), PointerPhase::Move),
            &view,
            &mut local
        )
        .is_none());
        assert_ne!(local.camera.origin(), before, "the drag did not pan");

        let intent = TilesPresentation::on_input(
            &click(start + Vec2::new(80.0, -40.0), PointerPhase::Up),
            &view,
            &mut local,
        );
        assert!(
            intent.is_none(),
            "releasing after a pan must not place a tile"
        );
    }

    /// A press that never moves far enough is a tap, not a pan.
    #[test]
    fn a_press_below_the_drag_threshold_is_still_a_tap() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        let kind = view.drawn.expect("a tile is in hand");
        let legal = legal_placements(&view.board, kind);
        let (coord, rotations) = legal.first().cloned().expect("something is playable");
        local.preview = rotations[0];

        let start = (world_centre(coord) - local.camera.origin()) * local.camera.zoom();
        TilesPresentation::on_input(&click(start, PointerPhase::Down), &view, &mut local);
        TilesPresentation::on_input(
            &click(start + Vec2::splat(1.0), PointerPhase::Move),
            &view,
            &mut local,
        );
        let intent = TilesPresentation::on_input(
            &click(start + Vec2::splat(1.0), PointerPhase::Up),
            &view,
            &mut local,
        );
        assert!(intent.is_some(), "a 1.4px wobble is a tap");
    }

    #[test]
    fn space_and_the_rotate_control_both_turn_the_preview() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        assert_eq!(local.preview_rotation(), Rotation::R0);

        TilesPresentation::on_input(
            &InputEvent::Key {
                key: Key::Space,
                pressed: true,
            },
            &view,
            &mut local,
        );
        assert_eq!(local.preview_rotation(), Rotation::R90);

        let rect = Control::Rotate
            .rect(frame.viewport())
            .expect("the rotate control fits");
        tap(rect.origin() + rect.size() * 0.5, &view, &mut local);
        assert_eq!(local.preview_rotation(), Rotation::R180);

        // A key release must not rotate again.
        TilesPresentation::on_input(
            &InputEvent::Key {
                key: Key::Space,
                pressed: false,
            },
            &view,
            &mut local,
        );
        assert_eq!(local.preview_rotation(), Rotation::R180);
    }

    #[test]
    fn the_zoom_and_recenter_controls_change_only_the_camera() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        for control in [Control::ZoomIn, Control::ZoomOut, Control::Recenter] {
            let rect = control.rect(frame.viewport()).expect("the control fits");
            let intent = tap(rect.origin() + rect.size() * 0.5, &view, &mut local);
            assert!(
                intent.is_none(),
                "{control:?} must not produce a game command"
            );
        }
    }

    #[test]
    fn arrows_move_the_cursor_and_tab_finds_a_legal_square() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        let key = |key| InputEvent::Key { key, pressed: true };

        assert_eq!(local.cursor(), Coord::ORIGIN);
        TilesPresentation::on_input(&key(Key::ArrowRight), &view, &mut local);
        assert_eq!(local.cursor(), Coord::new(1, 0).unwrap());
        TilesPresentation::on_input(&key(Key::ArrowUp), &view, &mut local);
        assert_eq!(local.cursor(), Coord::new(1, -1).unwrap());

        let kind = view.drawn.expect("a tile is in hand");
        TilesPresentation::on_input(&key(Key::Tab), &view, &mut local);
        let squares: Vec<Coord> = legal_placements(&view.board, kind)
            .into_iter()
            .map(|(coord, _)| coord)
            .collect();
        assert!(
            squares.contains(&local.cursor()),
            "Tab must land on a square the tile can actually go"
        );
    }

    /// Enter commits at the cursor, which is what makes the game completable
    /// without a pointer (doc 04 §10.3).
    #[test]
    fn the_whole_placement_step_is_reachable_from_the_keyboard() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        let key = |key| InputEvent::Key { key, pressed: true };

        TilesPresentation::on_input(&key(Key::Tab), &view, &mut local);
        // Rotate until the tile fits where Tab put the cursor.
        let mut intent = None;
        for _ in 0..4 {
            intent = key_tap(Key::Enter, &view, &mut local);
            if intent.is_some() {
                break;
            }
            key_tap(Key::Space, &view, &mut local);
        }
        assert!(
            matches!(
                intent.map(Intent::into_command),
                Some(Command::PlaceTile { .. })
            ),
            "Enter at a Tab-selected square must place the tile at some rotation"
        );
    }

    #[test]
    fn escape_passes_on_a_follower_only_in_the_claim_step() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (state, view) = opening();
        let key = |key| InputEvent::Key { key, pressed: true };

        assert!(
            TilesPresentation::on_input(&key(Key::Escape), &view, &mut local).is_none(),
            "there is nothing to pass on during the placement step"
        );

        // Place a tile to reach the claim step.
        let mut state = state;
        let kind = state.drawn().expect("a tile is in hand");
        let (at, rotation) = crate::rules::first_legal_placement(state.board(), kind)
            .expect("something is playable");
        let seed = MatchSeed::from_bytes([21u8; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(1));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(1),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let seat = state.turn();
        TilesRules::apply(
            &mut state,
            Input::Player {
                seat,
                command: Command::PlaceTile { at, rotation },
            },
            &mut ctx,
        )
        .expect("legal");
        let claim_view = TilesRules::project(&state, Viewer::Seat(state.turn()));
        assert_eq!(claim_view.phase, TurnPhase::PlaceMeeple);

        assert_eq!(
            key_tap(Key::Escape, &claim_view, &mut local).map(Intent::into_command),
            Some(Command::SkipMeeple)
        );

        // And tapping a claim slot claims it.
        if let Some(slot) = claim_view.meeple_slots.first().copied() {
            let tile = claim_view.board.get(at).expect("the tile is on the board");
            let centre = segment_centre(at, tile, slot);
            let screen = (centre - local.camera.origin()) * local.camera.zoom();
            let intent = tap(screen, &claim_view, &mut local);
            assert!(matches!(
                intent.map(Intent::into_command),
                Some(Command::PlaceMeeple { .. })
            ));
        }
    }

    #[test]
    fn losing_focus_disarms_a_drag_in_progress() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        TilesPresentation::on_input(
            &click(Vec2::splat(400.0), PointerPhase::Down),
            &view,
            &mut local,
        );
        assert!(local.press.is_some());
        TilesPresentation::on_input(&InputEvent::Focus(false), &view, &mut local);
        assert!(local.press.is_none());
        assert!(local.hover.is_none());
    }

    #[test]
    fn a_view_event_updates_only_local_state_and_emits_a_cue() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        local.preview = Rotation::R270;
        let at = Coord::new(1, 0).unwrap();
        let cues = TilesPresentation::on_view_event(
            &Event::TilePlaced {
                seat: SeatId(0),
                at,
                kind: crate::rules::TileKind::new(0).unwrap(),
                rotation: Rotation::R90,
            },
            &mut local,
            &frame,
        );
        assert!(!cues.is_empty());
        assert_eq!(local.last_placed, Some(at));
        assert_eq!(
            local.preview_rotation(),
            Rotation::R0,
            "a fresh tile starts unrotated"
        );
    }

    #[test]
    fn the_a11y_description_names_the_cursor_and_gates_its_actions() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (_, view) = opening();
        local.cursor = Coord::new(4, 4).unwrap();

        let description = TilesPresentation::a11y(&view, &local);
        assert!(description.status.contains("4,4"));
        let place = description
            .actions
            .iter()
            .find(|action| action.id == ActionId("place-tile".to_owned()))
            .expect("the place action is described");
        assert!(
            !place.enabled,
            "a square that touches nothing must be described as unavailable, not omitted"
        );
        assert!(description
            .actions
            .iter()
            .any(|action| action.id == ActionId("skip-follower".to_owned())));
    }

    /// **I-10, as a property.** The camera changes pixels and nothing else.
    ///
    /// The same *logical* interaction — "tap the first legal square" — is
    /// driven from five different camera positions and zoom levels. Each one
    /// produces different screen coordinates, so each takes a genuinely
    /// different path through `coord_at`; the resulting canonical state must be
    /// byte-identical. This is the test doc 08 §4.5 asks for.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn identical_play_from_different_cameras_produces_identical_state() {
        let seed = MatchSeed::from_bytes([33u8; 32]);
        let cameras = [
            (Vec2::ZERO, 1.0),
            (Vec2::new(-500.0, -500.0), 1.0),
            (Vec2::new(120.0, -80.0), MIN_ZOOM),
            (Vec2::new(-40.0, 900.0), 2.0),
            (Vec2::new(0.0, 0.0), MAX_ZOOM),
        ];

        let mut hashes = Vec::new();
        for (origin, zoom) in cameras {
            let frame = frame(0);
            let mut local = local_for(&frame);
            local.set_zoom(zoom);
            local.set_origin(origin);

            let mut rng = DetRng::for_input(&seed, InputIndex(0));
            let mut ctx = Ctx {
                now: LogicalTime::ZERO,
                index: InputIndex(0),
                rng: &mut rng,
                budget: Budget::default(),
            };
            let mut state = TilesRules::create(
                &crate::rules::Config {
                    turn_deadline_ms: 0,
                },
                &roster(3),
                &mut ctx,
            )
            .expect("valid setup")
            .state;

            let mut index = 1u64;
            for _ in 0..24 {
                if state.status() != crate::rules::Status::Playing {
                    break;
                }
                let seat = state.turn();
                let view = TilesRules::project(&state, Viewer::Seat(seat));

                // Pick the target in *world* terms so every camera is asked to
                // do the same thing, then express it in that camera's screen
                // coordinates and let `on_input` map it back.
                let intent = match view.phase {
                    TurnPhase::PlaceTile => {
                        let kind = view.drawn.expect("a tile is in hand");
                        let Some((at, rotation)) =
                            crate::rules::first_legal_placement(&view.board, kind)
                        else {
                            break;
                        };
                        local.preview = rotation;
                        // Follow the square with the camera, exactly as a player
                        // would, so the tap is inside the viewport at every zoom.
                        local.cursor = at;
                        local.recenter();
                        let screen =
                            (world_centre(at) - local.camera.origin()) * local.camera.zoom();
                        tap(screen, &view, &mut local)
                    }
                    TurnPhase::PlaceMeeple => key_tap(Key::Escape, &view, &mut local),
                };
                let Some(intent) = intent else {
                    break;
                };

                let mut rng = DetRng::for_input(&seed, InputIndex(index));
                let mut ctx = Ctx {
                    now: LogicalTime(index),
                    index: InputIndex(index),
                    rng: &mut rng,
                    budget: Budget::default(),
                };
                TilesRules::apply(
                    &mut state,
                    Input::Player {
                        seat,
                        command: intent.into_command(),
                    },
                    &mut ctx,
                )
                .expect("the presenter only ever asks for a legal command");
                index += 1;
            }

            assert!(
                index > 12,
                "the run from camera {origin:?}@{zoom} stopped after {} inputs; \
                 too short to be evidence",
                index - 1
            );
            hashes.push((origin, zoom, TilesRules::state_hash(&state).0));
        }

        let (_, _, expected) = hashes[0];
        for (origin, zoom, hash) in &hashes {
            assert_eq!(
                *hash, expected,
                "the camera at {origin:?} zoom {zoom} changed the canonical state"
            );
        }
    }

    /// The negative control for the test above: the camera really did differ,
    /// so the equality it asserts is not the trivial one.
    #[test]
    fn different_cameras_really_do_produce_different_render_lists() {
        let frame = frame(0);
        let (_, view) = opening();
        let mut a = local_for(&frame);
        let mut b = local_for(&frame);
        b.set_zoom(2.0);
        b.set_origin(Vec2::new(-300.0, 40.0));

        let list_a = TilesPresentation::present(&view, &a, &frame);
        let list_b = TilesPresentation::present(&view, &b, &frame);
        assert_ne!(list_a.camera(), list_b.camera());
        assert_ne!(
            tabula_testkit::presentation::render_list_snapshot(&list_a),
            tabula_testkit::presentation::render_list_snapshot(&list_b)
        );

        // And a pan alone is enough.
        a.set_origin(Vec2::new(7.0, 9.0));
        assert_ne!(
            TilesPresentation::present(&view, &a, &frame).camera(),
            list_a.camera()
        );
    }

    #[test]
    fn golden_tiles_opening_800x600_light() {
        let frame = frame(0);
        let local = local_for(&frame);
        let (_, view) = opening();
        let list = TilesPresentation::present(&view, &local, &frame);
        tabula_testkit::assert_render_list_snapshot!("tiles_opening_800x600_light", list);
    }

    #[test]
    fn golden_tiles_opening_zoomed_out_dark() {
        let frame = FrameCtx::new(
            Viewport::new(Vec2::new(800.0, 600.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            0,
            Theme::by_kind(ThemeKind::Dark),
        );
        let mut local = local_for(&frame);
        local.set_zoom(MIN_ZOOM);
        let (_, view) = opening();
        let list = TilesPresentation::present(&view, &local, &frame);
        tabula_testkit::assert_render_list_snapshot!("tiles_opening_zoomed_out_dark", list);
    }

    #[test]
    fn golden_tiles_claim_step_light() {
        let frame = frame(0);
        let mut local = local_for(&frame);
        let (state, _) = opening();

        let mut state = state;
        let kind = state.drawn().expect("a tile is in hand");
        let (at, rotation) = crate::rules::first_legal_placement(state.board(), kind)
            .expect("something is playable");
        let seed = MatchSeed::from_bytes([21u8; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(1));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(1),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let seat = state.turn();
        TilesRules::apply(
            &mut state,
            Input::Player {
                seat,
                command: Command::PlaceTile { at, rotation },
            },
            &mut ctx,
        )
        .expect("legal");

        local.cursor = at;
        let view = TilesRules::project(&state, Viewer::Seat(state.turn()));
        assert_eq!(view.phase, TurnPhase::PlaceMeeple);
        let list = TilesPresentation::present(&view, &local, &frame);
        tabula_testkit::assert_render_list_snapshot!("tiles_claim_step_light", list);
    }

    #[test]
    fn the_asset_pack_matches_the_manifest() {
        assert_eq!(
            TilesPresentation::asset_pack(),
            AssetPackRef::from_static("tiles", "0.1.0")
        );
    }

    #[test]
    fn every_hud_action_has_a_labeled_44_dp_target_and_compact_primary_reach() {
        let (_, view) = opening();
        for size in [
            Vec2::new(320.0, 568.0),
            Vec2::new(390.0, 844.0),
            Vec2::new(800.0, 600.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            let viewport = Viewport::new(size).unwrap();
            let mut local = TilesLocal::default();
            local.set_viewport(viewport);
            let buttons =
                hud_buttons(&view, &local, viewport, Positive::new(44.0).unwrap()).unwrap();
            assert_eq!(buttons.len(), Control::ALL.len());
            for control in Control::ALL {
                let rect = control
                    .rect(viewport)
                    .expect("control must fit a supported viewport");
                assert!(rect.size().min_element() >= 44.0, "{control:?} at {size:?}");
                assert!(rect.origin().min_element() >= 0.0);
                assert!((rect.origin() + rect.size()).cmple(size).all());
                assert_eq!(
                    Control::at(viewport, rect.origin() + Vec2::splat(1.0)),
                    Some(control)
                );
            }
            if size.x < 600.0 {
                assert!(Control::Confirm.rect(viewport).unwrap().origin().y >= size.y * 2.0 / 3.0);
            }
            for kind in [
                ThemeKind::Light,
                ThemeKind::Dark,
                ThemeKind::HighContrastLight,
                ThemeKind::HighContrastDark,
            ] {
                let frame =
                    FrameCtx::new(viewport, Dpi::new(1.0).unwrap(), 0, Theme::by_kind(kind));
                let list = build(&view, &local, &frame).expect("all theme layouts are valid");
                for label in [
                    "Zoom in",
                    "Zoom out",
                    "Center",
                    "Rotate",
                    "Place tile",
                    "Pass",
                ] {
                    assert!(list.commands().iter().any(|command| matches!(command,
                        RenderCmd::Text { text, layer: Layer::HUD, .. } if text == label)));
                }
                // No ornamental action borders; the principal action uses the
                // same semantic colour in all four generated schemes.
                let (at, rotation) =
                    crate::rules::first_legal_placement(&view.board, view.drawn.unwrap()).unwrap();
                local.cursor = at;
                local.preview = rotation;
                let rect = Control::Confirm.rect(viewport).unwrap();
                let list = build(&view, &local, &frame).unwrap();
                assert!(list.commands().iter().any(|command| matches!(command,
                    RenderCmd::Rect { rect: drawn, fill: Some(Paint::Solid(color)), border: None, layer: Layer::HUD, .. }
                    if *drawn == rect && *color == frame.theme().color.primary)));
            }
        }
    }

    #[test]
    fn cancelled_blurred_resized_and_repeated_releases_do_not_activate() {
        let (_, view) = opening();
        for interruption in 0..4 {
            let mut local = local_for(&frame(0));
            let point = Control::Rotate.rect(local.viewport).unwrap().origin() + Vec2::splat(1.0);
            TilesPresentation::on_input(&click(point, PointerPhase::Down), &view, &mut local);
            match interruption {
                0 => {
                    TilesPresentation::on_input(
                        &click(point, PointerPhase::Cancel),
                        &view,
                        &mut local,
                    );
                }
                1 => {
                    TilesPresentation::on_input(&InputEvent::Focus(false), &view, &mut local);
                    TilesPresentation::on_input(&InputEvent::Focus(true), &view, &mut local);
                }
                2 => local.set_viewport(Viewport::new(Vec2::new(390.0, 844.0)).unwrap()),
                _ => {
                    TilesPresentation::on_view_event(&Event::Paused, &mut local, &frame(1));
                }
            }
            assert!(TilesPresentation::on_input(
                &click(point, PointerPhase::Up),
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(
                local.preview,
                Rotation::R0,
                "interruption {interruption} activated a stale button"
            );
            let rect = Control::Rotate.rect(local.viewport).unwrap();
            let point = rect.origin() + Vec2::splat(1.0);
            tap(point, &view, &mut local);
            assert_eq!(local.preview, Rotation::R90, "fresh press still works");
            TilesPresentation::on_input(&click(point, PointerPhase::Up), &view, &mut local);
            assert_eq!(
                local.preview,
                Rotation::R90,
                "duplicate Up must not activate"
            );
        }
        let mut local = local_for(&frame(0));
        let (at, rotation) =
            crate::rules::first_legal_placement(&view.board, view.drawn.unwrap()).unwrap();
        local.cursor = at;
        local.preview = rotation;
        let point = (world_centre(local.cursor) - local.camera.origin()) * local.camera.zoom();
        assert!(
            tap(point, &view, &mut local).is_some(),
            "the board target is reachable"
        );
        assert!(
            TilesPresentation::on_input(&click(point, PointerPhase::Up), &view, &mut local)
                .is_none()
        );
        TilesPresentation::on_input(&click(point, PointerPhase::Down), &view, &mut local);
        TilesPresentation::on_input(&click(point, PointerPhase::Cancel), &view, &mut local);
        assert!(
            TilesPresentation::on_input(&click(point, PointerPhase::Up), &view, &mut local)
                .is_none()
        );
        TilesPresentation::on_input(&click(point, PointerPhase::Down), &view, &mut local);
        assert!(
            TilesPresentation::on_input(
                &click(point + Vec2::new(20.0, 0.0), PointerPhase::Up),
                &view,
                &mut local
            )
            .is_none(),
            "missing Move events cannot turn a drag into a placement"
        );
    }

    #[test]
    fn activation_key_repeats_are_suppressed_and_blur_does_not_latch_a_key() {
        let (_, view) = opening();
        let mut local = local_for(&frame(0));
        let down = |key| InputEvent::Key { key, pressed: true };
        TilesPresentation::on_input(&down(Key::Space), &view, &mut local);
        TilesPresentation::on_input(&down(Key::Space), &view, &mut local);
        assert_eq!(local.preview, Rotation::R90);
        TilesPresentation::on_input(&InputEvent::Focus(false), &view, &mut local);
        TilesPresentation::on_input(&down(Key::Space), &view, &mut local);
        assert_eq!(local.preview, Rotation::R90, "unfocused input is ignored");
        TilesPresentation::on_input(&InputEvent::Focus(true), &view, &mut local);
        TilesPresentation::on_input(&down(Key::Space), &view, &mut local);
        assert_eq!(
            local.preview,
            Rotation::R180,
            "keyup may have been delivered outside the app"
        );
        key_tap(Key::Space, &view, &mut local);
        local.cursor = next_legal_square(&view, &local).unwrap();
        // The square may require another rotation; each fresh press is explicit.
        for _ in 0..4 {
            if act_on_square(&view, &local, local.cursor).is_some() {
                break;
            }
            key_tap(Key::Space, &view, &mut local);
        }
        assert!(
            act_on_square(&view, &local, local.cursor).is_some(),
            "a legal square has at least one of four rotations"
        );
        assert!(TilesPresentation::on_input(&down(Key::Enter), &view, &mut local).is_some());
        assert!(TilesPresentation::on_input(&down(Key::Enter), &view, &mut local).is_none());
        assert!(key_tap(Key::Enter, &view, &mut local).is_some());
    }

    #[test]
    fn gameplay_actions_are_gated_for_paused_terminal_and_other_viewers() {
        let (_, original) = opening();
        for case in 0..4 {
            let mut view = original.clone();
            match case {
                0 => view.paused = true,
                1 => view.you = None,
                2 => view.you = Some(SeatId(1)),
                _ => view.status = crate::rules::Status::Ended,
            }
            let before = view.clone();
            let mut local = local_for(&frame(0));
            local.cursor = next_legal_square(&view, &local).unwrap();
            assert!(key_tap(Key::Enter, &view, &mut local).is_none());
            key_tap(Key::Space, &view, &mut local);
            assert_eq!(local.preview, Rotation::R0);
            for control in [Control::Rotate, Control::Confirm, Control::Skip] {
                let rect = control.rect(local.viewport).unwrap();
                assert!(tap(rect.origin() + rect.size() * 0.5, &view, &mut local).is_none());
            }
            let description = describe(&view, &local);
            for action in &description.actions {
                if ["place-tile", "claim-follower", "rotate", "skip-follower"]
                    .contains(&action.id.0.as_str())
                {
                    assert!(
                        !action.enabled,
                        "{} exposed an unavailable action",
                        action.id.0
                    );
                }
            }
            let rect = Control::ZoomIn.rect(local.viewport).unwrap();
            tap(rect.origin() + rect.size() * 0.5, &view, &mut local);
            assert!(
                local.camera.zoom() > 1.0,
                "camera is still usable in case {case}"
            );
            assert_eq!(view, before, "local UI never writes the projection");
        }
    }

    #[test]
    fn every_reachable_claim_is_selectable_by_keyboard_and_by_its_pointer_position() {
        let (byte, (state, view)) = (0..64u8)
            .map(|byte| (byte, claim_step_with_seed(byte)))
            .find(|(_, (_, view))| {
                if view.meeple_slots.len() < 2 {
                    return false;
                }
                let at = view.last_placed.unwrap();
                let tile = view.board.get(at).unwrap();
                view.meeple_slots.iter().enumerate().all(|(index, slot)| {
                    view.meeple_slots.iter().skip(index + 1).all(|other| {
                        segment_centre(at, tile, *slot)
                            .distance_squared(segment_centre(at, tile, *other))
                            > 1.0
                    })
                })
            })
            .expect("a real multi-feature claim must be reached");
        let at = view.last_placed.unwrap();
        let tile = view.board.get(at).unwrap();
        let mut local = local_for(&frame(0));
        local.cursor = at;
        let first = selected_claim(&view, &local).unwrap();
        let mut visited = Vec::new();
        for _ in 0..view.meeple_slots.len() {
            let segment = selected_claim(&view, &local).unwrap();
            visited.push(segment);
            let command = key_tap(Key::Enter, &view, &mut local)
                .expect("selected claim is active")
                .into_command();
            assert_eq!(command, Command::PlaceMeeple { segment });
            let mut state = state.clone();
            let seed = MatchSeed::from_bytes([byte; 32]);
            let mut rng = DetRng::for_input(&seed, InputIndex(2));
            let mut ctx = Ctx {
                now: LogicalTime::ZERO,
                index: InputIndex(2),
                rng: &mut rng,
                budget: Budget::default(),
            };
            TilesRules::apply(
                &mut state,
                Input::Player {
                    seat: view.turn,
                    command,
                },
                &mut ctx,
            )
            .expect("advertised claim must be accepted");
            key_tap(Key::Tab, &view, &mut local);
        }
        assert_eq!(visited, view.meeple_slots);
        assert_eq!(
            selected_claim(&view, &local),
            Some(first),
            "Tab wraps claim selection"
        );
        for slot in &view.meeple_slots {
            let point =
                (segment_centre(at, tile, *slot) - local.camera.origin()) * local.camera.zoom();
            assert_eq!(
                tap(point, &view, &mut local).map(Intent::into_command),
                Some(Command::PlaceMeeple { segment: *slot })
            );
        }
        let confirm = Control::Confirm.rect(local.viewport).unwrap();
        let description = describe(&view, &local);
        assert!(description
            .actions
            .iter()
            .any(|action| action.id.0 == "claim-follower" && action.enabled));
        assert!(matches!(
            tap(confirm.origin() + confirm.size() * 0.5, &view, &mut local)
                .map(Intent::into_command),
            Some(Command::PlaceMeeple { .. })
        ));
        assert_eq!(
            key_tap(Key::Escape, &view, &mut local).map(Intent::into_command),
            Some(Command::SkipMeeple)
        );
    }

    #[test]
    fn short_and_compact_results_keep_every_score_and_follower_label() {
        let (_, original) = opening();
        for count in 2..=5usize {
            let mut view = original.clone();
            view.status = crate::rules::Status::Ended;
            view.seats = [0, 64, 128, 192, 255]
                .into_iter()
                .take(count)
                .map(SeatId)
                .collect();
            for (index, seat) in view.seats.iter().enumerate() {
                view.scores
                    .insert(*seat, 100 + i64::try_from(index).unwrap());
                view.meeples_in_hand.insert(*seat, 7);
            }
            for size in [Vec2::new(320.0, 568.0), Vec2::new(640.0, 320.0)] {
                let viewport = Viewport::new(size).unwrap();
                let frame = FrameCtx::new(
                    viewport,
                    Dpi::new(1.0).unwrap(),
                    0,
                    Theme::by_kind(ThemeKind::Light),
                );
                let mut local = local_for(&frame);
                local.cursor = Coord::ORIGIN;
                let scores = scores_rect(&view, viewport)
                    .expect("supported short layouts must retain scores");
                assert!((scores.size().y - 48.0).abs() < f32::EPSILON);
                let list = build(&view, &local, &frame).unwrap();
                let description = describe(&view, &local);
                for seat in &view.seats {
                    let score = view.scores[seat];
                    assert!(description
                        .status
                        .contains(&format!("Seat {}: {score} points, 7 followers.", seat.0)));
                    let text = format!("S{}: {score}", seat.0);
                    assert!(list.commands().iter().any(|command| matches!(command,
                        RenderCmd::Text { text: drawn, at, max_width: Some(width), layer: Layer::HUD, .. }
                        if *drawn == text && width.get() >= 56.0 && scores.contains(*at))));
                }
                let followers = list.commands().iter().filter(|command| matches!(command,
                    RenderCmd::Text { text, at, max_width: Some(width), layer: Layer::HUD, .. }
                    if text == "7 followers" && width.get() >= 56.0
                        && at.y + frame.theme().text_style(TextStyleToken::LabelSm).line_height().get() <= scores.origin().y + scores.size().y)).count();
                assert_eq!(
                    followers, count,
                    "every seat's counters need a bounded second line"
                );
            }
        }
    }

    #[test]
    fn maximum_zoom_keeps_the_short_map_cursor_and_focus_marker_visible() {
        let viewport = Viewport::new(Vec2::new(640.0, 320.0)).unwrap();
        let frame = FrameCtx::new(
            viewport,
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        );
        let (_, view) = opening();
        let mut local = local_for(&frame);
        local.set_zoom(MAX_ZOOM);
        let centre = (world_centre(local.cursor) - local.camera.origin()) * local.camera.zoom();
        assert!(
            (centre - Vec2::new(320.0, 216.0)).length() < 0.001,
            "zoom must preserve the map centre, outside the HUD"
        );
        for key in [
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowLeft,
            Key::ArrowDown,
            Key::Tab,
        ] {
            key_tap(key, &view, &mut local);
            let centre = (world_centre(local.cursor) - local.camera.origin()) * local.camera.zoom();
            assert!(
                (centre.y - 216.0).abs() < 0.001,
                "an oversized tile must be centred in the available axis"
            );
            assert!(centre.x > 0.0 && centre.x < 640.0);
            assert!(!hud_contains(&view, viewport, centre));
            let list = build(&view, &local, &frame).unwrap();
            let marker = list
                .commands()
                .iter()
                .find_map(|command| match command {
                    RenderCmd::Rect {
                        rect,
                        layer: Layer::OVERLAY,
                        z: 21,
                        border: Some(border),
                        ..
                    } if border.color() == frame.theme().focus.ring_color => Some(*rect),
                    _ => None,
                })
                .expect("the oversized cursor needs a visible centre marker");
            let from = (marker.origin() - local.camera.origin()) * local.camera.zoom();
            let to = from + marker.size() * local.camera.zoom();
            assert!(from.x >= 0.0 && to.x <= 640.0 && from.y >= 184.0 && to.y <= 248.0);
        }
    }
}
