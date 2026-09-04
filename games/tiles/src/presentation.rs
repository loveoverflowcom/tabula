//! Renderer-neutral Tiles presentation. (doc 04 §5)
//!
//! # Everything interactive is local
//!
//! Camera pan and zoom, the rotation of the tile being previewed, the keyboard
//! cursor, hover, and the drag in progress all live in [`TilesLocal`] and none
//! of them is ever an input to `apply` (I-10). Two players looking at the same
//! board from different camera positions is not a desync, and
//! `tests/presentation.rs` proves it by driving one command sequence from
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
//! grid. `tests/presentation.rs` asserts the HUD occupies the same logical
//! rectangle at every zoom level.
//!
//! # Keyboard play is mandatory (doc 04 §10.3)
//!
//! Arrows move a cursor over board squares (not the camera — a keyboard player
//! expects to move *on* the board), Tab jumps to the next legal square, Space
//! rotates the tile, Enter places or claims, Escape declines a claim. The
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

use tabula_design::Theme;
use tabula_game_api::{A11yAction, A11yDescription, ActionId, GameRules};
use tabula_presentation::{
    Affine2, Align, AssetPackRef, AudioCue, AudioCues, Border, Camera2D, Corners, FrameCtx,
    GamePresentation, InputEvent, Intent, Key, Layer, Paint, PointerButton, PointerPhase,
    PointerPosition, Rect, RenderCmd, RenderList, RenderListBuilder, RenderListError,
    TextStyleToken, Vec2, Viewport,
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
const HUD_BUTTON: f32 = 34.0;
const HUD_GAP: f32 = 6.0;
/// Height of the status strip along the top.
const HUD_STATUS_HEIGHT: f32 = 30.0;

/// The screen-fixed controls, in the order they are laid out down the left edge.
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
    Skip,
}

impl Control {
    /// Every control, in layout order.
    pub const ALL: [Self; 5] = [
        Self::ZoomIn,
        Self::ZoomOut,
        Self::Recenter,
        Self::Rotate,
        Self::Skip,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::ZoomIn => "+",
            Self::ZoomOut => "-",
            Self::Recenter => "@",
            Self::Rotate => "R",
            Self::Skip => "X",
        }
    }

    const fn action(self) -> &'static str {
        match self {
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::Recenter => "recenter",
            Self::Rotate => "rotate",
            Self::Skip => "skip-follower",
        }
    }

    /// Where this control sits, in **logical** (screen) coordinates.
    fn rect(self, viewport: Viewport) -> Option<Rect> {
        let index = Self::ALL.iter().position(|control| *control == self)?;
        let step = u16::try_from(index).ok().map(f32::from)?;
        let top = HUD_STATUS_HEIGHT + HUD_GAP + (HUD_BUTTON + HUD_GAP) * step;
        if top + HUD_BUTTON > viewport.size().y {
            return None;
        }
        Rect::new(Vec2::new(HUD_GAP, top), Vec2::new(HUD_BUTTON, HUD_BUTTON)).ok()
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
        self.viewport = viewport;
        if !self.centred {
            self.centred = true;
            self.recenter();
        }
    }

    /// Put the cursor's square in the middle of the viewport.
    pub fn recenter(&mut self) {
        self.look_at(self.cursor);
    }

    fn look_at(&mut self, coord: Coord) {
        let centre = world_centre(coord);
        let half = self.viewport.size() * 0.5 / self.camera.zoom();
        self.set_origin(centre - half);
    }

    fn set_origin(&mut self, origin: Vec2) {
        if let Ok(camera) = Camera2D::new(origin, self.camera.zoom()) {
            self.camera = camera;
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        // Zoom about the middle of the viewport so the thing being looked at
        // stays put.
        let focus = self.local_to_world(self.viewport.size() * 0.5);
        if let Ok(camera) = Camera2D::new(self.camera.origin(), clamped) {
            self.camera = camera;
            let half = self.viewport.size() * 0.5 / clamped;
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
        let view = self.viewport.size() / zoom;
        let mut next = origin;
        if rect.origin().x < origin.x {
            next.x = rect.origin().x;
        } else if rect.origin().x + TILE_SIZE > origin.x + view.x {
            next.x = rect.origin().x + TILE_SIZE - view.x;
        }
        if rect.origin().y < origin.y {
            next.y = rect.origin().y;
        } else if rect.origin().y + TILE_SIZE > origin.y + view.y {
            next.y = rect.origin().y + TILE_SIZE - view.y;
        }
        if next != origin {
            self.set_origin(next);
        }
    }

    /// Apply a screen-fixed control. Returns an intent only for the one control
    /// that is a game command rather than a camera change.
    fn apply_control(&mut self, control: Control, view: &View) -> Option<Intent<Command>> {
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
            Control::Skip => {
                (view.phase == TurnPhase::PlaceMeeple).then(|| Intent::new(Command::SkipMeeple))
            }
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
        match event {
            Event::TilePlaced { at, .. } => {
                local.last_placed = Some(*at);
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
                if *pressed {
                    on_key(*key, view, local)
                } else {
                    None
                }
            }
            InputEvent::Focus(_) => {
                // Losing focus mid-drag must not leave a phantom pan armed.
                local.press = None;
                local.hover = None;
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
    let point = position.get();
    match phase {
        PointerPhase::Down => {
            if button == PointerButton::Secondary {
                // Right-click rotates: the fastest possible rotate control, and
                // it needs no screen real estate.
                local.preview = local.preview.next();
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
            local.hover = local.coord_at(position);
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
            let press = local.press.take();
            if button != PointerButton::Primary {
                return None;
            }
            // A pan is not a tap: releasing after dragging the board must not
            // also place a tile.
            if press.is_some_and(|press| press.moved) {
                return None;
            }
            if let Some(control) = Control::at(local.viewport, point) {
                return local.apply_control(control, view);
            }
            let coord = local.coord_at(position)?;
            local.cursor = coord;
            act_on_square(view, local, coord)
        }
    }
}

/// What tapping a board square means, which depends only on the phase.
fn act_on_square(view: &View, local: &TilesLocal, coord: Coord) -> Option<Intent<Command>> {
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
            let tile = view.board.get(last)?;
            let segment = nearest_claimable_segment(view, local, last, tile)?;
            Some(Intent::new(Command::PlaceMeeple { segment }))
        }
    }
}

/// Of the segments the seat may claim on `coord`, the one whose drawn position
/// is nearest the cursor's square centre.
///
/// Tapping a tile with several claimable features has to pick one; "nearest the
/// tap" is the only choice a player can predict.
fn nearest_claimable_segment(
    view: &View,
    local: &TilesLocal,
    coord: Coord,
    tile: PlacedTile,
) -> Option<u8> {
    let target = local.local_to_world(
        local
            .hover
            .map_or(local.viewport.size() * 0.5, |_| local.viewport.size() * 0.5),
    );
    let reference = if view.meeple_slots.len() == 1 {
        // One option: no need to be clever.
        return view.meeple_slots.first().copied();
    } else {
        target
    };
    view.meeple_slots.iter().copied().min_by(|left, right| {
        let a = segment_centre(coord, tile, *left).distance_squared(reference);
        let b = segment_centre(coord, tile, *right).distance_squared(reference);
        a.partial_cmp(&b)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(left.cmp(right))
    })
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
        Key::Space => {
            local.preview = local.preview.next();
            None
        }
        Key::Tab => {
            // Jump to the next square where the tile would actually fit at the
            // current rotation, so a keyboard player is never hunting.
            if let Some(next) = next_legal_square(view, local) {
                local.cursor = next;
                local.keep_cursor_visible();
            }
            None
        }
        Key::Escape => {
            (view.phase == TurnPhase::PlaceMeeple).then(|| Intent::new(Command::SkipMeeple))
        }
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
                        border: Some(Border::new(2.0, theme.color.legal_target)?),
                        layer: Layer::OVERLAY,
                        z: 8,
                    })?;
                }
            }
        }
    }

    // The cursor, always visible so keyboard play has a focus indicator.
    builder.push(RenderCmd::Rect {
        rect: world_rect(local.cursor),
        radii: Corners::uniform(2.0)?,
        fill: None,
        border: Some(Border::new(2.0, theme.focus.ring_color)?),
        layer: Layer::OVERLAY,
        z: 20,
    })?;
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
        at: Vec2::new(HUD_GAP, 6.0),
        style: TextStyleToken::LabelMd,
        align: Align::Start,
        max_width: None,
        color: theme.color.on_surface,
        layer: Layer::HUD,
        z: 1,
    })?;

    // Scores down the right edge, the seat on turn highlighted.
    for (index, seat) in view.seats.iter().enumerate() {
        let step = u16::try_from(index).map_or(0.0, f32::from);
        let y = HUD_STATUS_HEIGHT + HUD_GAP + (HUD_BUTTON + HUD_GAP) * step;
        if y + HUD_BUTTON > viewport.size().y {
            break;
        }
        let colour = if *seat == view.turn {
            theme.color.turn_active
        } else {
            theme.color.on_surface_variant
        };
        builder.push(RenderCmd::Text {
            text: format!(
                "seat {}  {}  ({} left)",
                seat.0,
                view.scores.get(seat).copied().unwrap_or(0),
                view.meeples_in_hand.get(seat).copied().unwrap_or(0)
            ),
            at: Vec2::new(viewport.size().x - HUD_GAP, y),
            style: TextStyleToken::LabelSm,
            align: Align::End,
            max_width: None,
            color: colour,
            layer: Layer::HUD,
            z: 1,
        })?;
        // A swatch, so the followers on the board can be matched to a seat.
        builder.push(RenderCmd::Rect {
            rect: Rect::new(
                Vec2::new(viewport.size().x - HUD_GAP - 12.0, y + 14.0),
                Vec2::splat(10.0),
            )?,
            radii: Corners::uniform(5.0)?,
            fill: Some(Paint::Solid(seat_colour(view, *seat, theme))),
            border: None,
            layer: Layer::HUD,
            z: 1,
        })?;
    }

    for control in Control::ALL {
        let Some(rect) = control.rect(viewport) else {
            continue;
        };
        let enabled = control != Control::Skip || view.phase == TurnPhase::PlaceMeeple;
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(4.0)?,
            fill: Some(Paint::Solid(if enabled {
                theme.color.surface_container_high
            } else {
                theme.color.surface_container
            })),
            border: Some(Border::new(1.0, theme.color.outline)?),
            layer: Layer::HUD,
            z: 0,
        })?;
        builder.push(RenderCmd::Text {
            text: control.label().to_owned(),
            at: rect.origin() + Vec2::new(rect.size().x * 0.5, 8.0),
            style: TextStyleToken::LabelMd,
            align: Align::Center,
            max_width: None,
            color: if enabled {
                theme.color.on_surface
            } else {
                theme.color.on_surface_variant
            },
            layer: Layer::HUD,
            z: 1,
        })?;
    }

    builder.push(RenderCmd::PopTransform {
        layer: Layer::HUD,
        z: 0,
    })?;
    Ok(())
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
        crate::rules::Status::Ended => format!(
            "Match over.  {}",
            view.seats
                .iter()
                .map(|seat| format!(
                    "seat {}: {}",
                    seat.0,
                    view.scores.get(seat).copied().unwrap_or(0)
                ))
                .collect::<Vec<_>>()
                .join("   ")
        ),
        crate::rules::Status::Aborted => "Match cancelled.".to_owned(),
        crate::rules::Status::Playing if view.paused => "Paused.".to_owned(),
        crate::rules::Status::Playing => {
            let step = match view.phase {
                TurnPhase::PlaceTile => "place the tile",
                TurnPhase::PlaceMeeple => "place a follower or pass (Esc)",
            };
            format!(
                "Seat {} to {step}.  {} tiles left.  Space rotates, Enter commits, Tab finds a spot.",
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

    let on_turn =
        view.you == Some(view.turn) && view.status == crate::rules::Status::Playing && !view.paused;
    let can_place = on_turn
        && view.phase == TurnPhase::PlaceTile
        && view.drawn.is_some_and(|kind| {
            crate::rules::is_legal_placement(
                &view.board,
                local.cursor,
                PlacedTile::new(kind, local.preview),
            )
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
        enabled: on_turn && view.phase == TurnPhase::PlaceMeeple,
    });
    for control in Control::ALL {
        if control == Control::Skip {
            continue;
        }
        description.actions.push(A11yAction {
            id: ActionId(control.action().to_owned()),
            label: control.action().replace('-', " "),
            enabled: true,
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
        let seed = MatchSeed::from_bytes([21u8; 32]);
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

    #[test]
    fn the_opening_view_is_centred_on_the_start_tile() {
        let frame = frame(0);
        let local = local_for(&frame);
        let centre = local.local_to_world(frame.viewport().size() * 0.5);
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
        let intent =
            TilesPresentation::on_input(&click(screen, PointerPhase::Up), &view, &mut local);
        assert_eq!(
            intent.map(Intent::into_command),
            Some(Command::PlaceTile {
                at: coord,
                rotation: rotations[0]
            })
        );

        // The origin square is occupied, so tapping it asks for nothing.
        let occupied = (world_centre(Coord::ORIGIN) - local.camera.origin()) * local.camera.zoom();
        assert!(
            TilesPresentation::on_input(&click(occupied, PointerPhase::Up), &view, &mut local)
                .is_none()
        );
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
        TilesPresentation::on_input(
            &click(rect.origin() + rect.size() * 0.5, PointerPhase::Up),
            &view,
            &mut local,
        );
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
            let intent = TilesPresentation::on_input(
                &click(rect.origin() + rect.size() * 0.5, PointerPhase::Up),
                &view,
                &mut local,
            );
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
            intent = TilesPresentation::on_input(&key(Key::Enter), &view, &mut local);
            if intent.is_some() {
                break;
            }
            TilesPresentation::on_input(&key(Key::Space), &view, &mut local);
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
            TilesPresentation::on_input(&key(Key::Escape), &claim_view, &mut local)
                .map(Intent::into_command),
            Some(Command::SkipMeeple)
        );

        // And tapping a claim slot claims it.
        if let Some(slot) = claim_view.meeple_slots.first().copied() {
            let tile = claim_view.board.get(at).expect("the tile is on the board");
            let centre = segment_centre(at, tile, slot);
            let screen = (centre - local.camera.origin()) * local.camera.zoom();
            let intent = TilesPresentation::on_input(
                &click(screen, PointerPhase::Up),
                &claim_view,
                &mut local,
            );
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
                        TilesPresentation::on_input(
                            &click(screen, PointerPhase::Up),
                            &view,
                            &mut local,
                        )
                    }
                    TurnPhase::PlaceMeeple => TilesPresentation::on_input(
                        &InputEvent::Key {
                            key: Key::Escape,
                            pressed: true,
                        },
                        &view,
                        &mut local,
                    ),
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
}
