//! Bounded accepted-move choreography over public projection endpoints (doc 04 §9, I-10).
//! Sampling is absolute-time only. One move, one optional rook and one victim replace
//! the previous composition; there is no queue and no authority or input delay.

#![allow(clippy::float_arithmetic)]

use tabula_presentation::{is_stale_on_arrival, MotionMode, MotionTimeline, Opacity};

use super::{
    piece_sprite, BoardLayout, ChessColor, ChessLocal, Corners, FrameCtx, Layer, Paint, Piece,
    PieceKind, Rect, RenderCmd, RenderListBuilder, RenderListError, Square, Theme, Vec2, View,
    IN_TRANSIT_PIECE_Z,
};

/// Ephemeral accepted move. Its timeline never affects rules or command formation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChessMoveAnimation {
    pub from: Square,
    pub to: Square,
    /// Preserves the prior pawn's color during accepted promotion playback.
    pub color: ChessColor,
    pub promotion: Option<PieceKind>,
    pub timeline: MotionTimeline,
    captured: Option<Piece>,
    capture_timeline: MotionTimeline,
    flipped: bool,
    composition: Option<Composition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position {
    board: [Option<Piece>; 64],
    turn: ChessColor,
    castling: crate::CastlingRights,
    en_passant: Option<Square>,
    fullmove: u16,
    halfmove: u16,
}

impl Position {
    fn new(view: &View) -> Self {
        Self {
            board: view.board,
            turn: view.turn,
            castling: view.castling,
            en_passant: view.en_passant,
            fullmove: view.fullmove_number,
            halfmove: view.halfmove_clock,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Travel {
    piece: Piece,
    from: Square,
    to: Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Victim {
    piece: Piece,
    square: Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Composition {
    mover: Travel,
    rook: Option<Travel>,
    victim: Option<Victim>,
    endpoint: Position,
}

impl ChessMoveAnimation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start(
        from: Square,
        to: Square,
        color: ChessColor,
        promotion: Option<PieceKind>,
        captured: Option<Piece>,
        started_at_ms: u64,
        local: &ChessLocal,
        frame: &FrameCtx,
    ) -> Option<Self> {
        if local.reduced_motion
            || !local.focus.is_window_focused()
            || is_stale_on_arrival(started_at_ms, frame.now_ms())
            || Square::new(from.0).is_none()
            || Square::new(to.0).is_none()
            || from == to
        {
            return None;
        }
        let theme = frame.theme();
        Some(Self {
            from,
            to,
            color,
            promotion,
            timeline: MotionTimeline::from_profile(
                started_at_ms,
                theme.motion.piece_move,
                &theme,
                MotionMode::Full,
            ),
            captured,
            // Capture is secondary informative feedback, reusing the authored 160 ms
            // turn-change profile rather than introducing a private duration source.
            capture_timeline: MotionTimeline::from_profile(
                started_at_ms,
                theme.motion.turn_change,
                &theme,
                MotionMode::Full,
            ),
            flipped: local.flipped,
            composition: None,
        })
    }

    /// Pins all animated pieces to the accepted projection, including the real victim square.
    /// A mismatched event/transition is rendered statically rather than inventing a ghost.
    pub(super) fn with_projection(mut self, previous: &View, current: &View) -> Option<Self> {
        let mover = previous
            .board
            .get(usize::from(self.from.0))
            .copied()
            .flatten()?;
        if mover.color != self.color || previous.turn != self.color {
            return None;
        }
        let prior_destination = previous
            .board
            .get(usize::from(self.to.0))
            .copied()
            .flatten();
        let victim = match (self.captured, prior_destination) {
            (Some(piece), Some(prior)) if piece == prior && piece.color != self.color => {
                Some(Victim {
                    piece,
                    square: self.to,
                })
            }
            (Some(piece), None)
                if mover.kind == PieceKind::Pawn
                    && self.from.file().abs_diff(self.to.file()) == 1
                    && previous.en_passant == Some(self.to)
                    && piece.kind == PieceKind::Pawn
                    && piece.color != self.color =>
            {
                let square = Square::new(self.from.rank() * 8 + self.to.file())?;
                if previous.board[usize::from(square.0)] != Some(piece) {
                    return None;
                }
                Some(Victim { piece, square })
            }
            (None, None) => None,
            _ => return None,
        };
        let rook = castle_rook(mover, self.from, self.to);
        let mut expected = previous.board;
        expected[usize::from(self.from.0)] = None;
        if let Some(victim) = victim {
            expected[usize::from(victim.square.0)] = None;
        }
        if let Some(rook) = rook {
            if expected[usize::from(rook.from.0)] != Some(rook.piece)
                || expected[usize::from(rook.to.0)].is_some()
            {
                return None;
            }
            expected[usize::from(rook.from.0)] = None;
            expected[usize::from(rook.to.0)] = Some(rook.piece);
        }
        let endpoint_piece = if let Some(kind) = self.promotion {
            if mover.kind != PieceKind::Pawn
                || !matches!(
                    kind,
                    PieceKind::Queen | PieceKind::Rook | PieceKind::Bishop | PieceKind::Knight
                )
            {
                return None;
            }
            Piece {
                color: self.color,
                kind,
            }
        } else {
            mover
        };
        expected[usize::from(self.to.0)] = Some(endpoint_piece);
        if current.board != expected || current.turn != self.color.other() {
            return None;
        }
        self.composition = Some(Composition {
            mover: Travel {
                piece: mover,
                from: self.from,
                to: self.to,
            },
            rook,
            victim,
            endpoint: Position::new(current),
        });
        Some(self)
    }

    fn composition(self, view: &View) -> Option<Composition> {
        if let Some(composition) = self.composition {
            return (composition.endpoint == Position::new(view)).then_some(composition);
        }
        // Legacy event-only callers cannot locate an en-passant victim. Keep the
        // mover/castle/promotion compatible, but never guess a capture location.
        if view.board.get(usize::from(self.from.0))?.is_some() || view.turn != self.color.other() {
            return None;
        }
        let endpoint_piece = view.board.get(usize::from(self.to.0)).copied().flatten()?;
        if endpoint_piece.color != self.color
            || self
                .promotion
                .is_some_and(|kind| kind != endpoint_piece.kind)
        {
            return None;
        }
        let mover = if self.promotion.is_some() {
            Piece {
                color: self.color,
                kind: PieceKind::Pawn,
            }
        } else {
            endpoint_piece
        };
        let rook = castle_rook(mover, self.from, self.to);
        if rook.is_some_and(|rook| {
            view.board[usize::from(rook.from.0)].is_some()
                || view.board[usize::from(rook.to.0)] != Some(rook.piece)
        }) {
            return None;
        }
        Some(Composition {
            mover: Travel {
                piece: mover,
                from: self.from,
                to: self.to,
            },
            rook,
            victim: None,
            endpoint: Position::new(view),
        })
    }
}

fn castle_rook(piece: Piece, from: Square, to: Square) -> Option<Travel> {
    if piece.kind != PieceKind::King
        || from.file() != 4
        || from.rank() != to.rank()
        || from.rank()
            != if piece.color == ChessColor::White {
                0
            } else {
                7
            }
    {
        return None;
    }
    let (from_file, to_file) = match to.file() {
        6 => (7, 5),
        2 => (0, 3),
        _ => return None,
    };
    Some(Travel {
        piece: Piece {
            color: piece.color,
            kind: PieceKind::Rook,
        },
        from: Square(from.rank() * 8 + from_file),
        to: Square(to.rank() * 8 + to_file),
    })
}

pub(super) fn draw_pieces(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &ChessLocal,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    let active = local.move_animation.and_then(|animation| {
        (!local.reduced_motion
            && local.focus.is_window_focused()
            && animation.flipped == local.flipped
            && !animation.timeline.sample(frame.now_ms()).done)
            .then(|| {
                animation
                    .composition(view)
                    .map(|composition| (animation, composition))
            })
            .flatten()
    });
    let dragged = match local.interaction {
        super::Interaction::Dragging { from, .. } => Some(from),
        _ => None,
    };
    for (index, piece) in view.board.iter().enumerate() {
        let Some(piece) = *piece else { continue };
        let square =
            Square::new(u8::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?)
                .ok_or(RenderListError::InvalidGeometry)?;
        if dragged == Some(square)
            || active.is_some_and(|(_, composition)| {
                composition.mover.to == square
                    || composition.rook.is_some_and(|rook| rook.to == square)
            })
        {
            continue;
        }
        let cell = layout
            .square_rect(square)
            .ok_or(RenderListError::InvalidGeometry)?;
        let selected = matches!(local.interaction,
            super::Interaction::Selected { square: selected }
                | super::Interaction::Pressed { from: selected, .. } if selected == square);
        let lift = if local.reduced_motion {
            0.0
        } else if selected {
            cell.size().y.min(48.0) * 0.0625
        } else if local.hover == Some(square) && Some(piece.color) == view.you {
            cell.size().y.min(48.0) * 0.0417
        } else {
            0.0
        };
        draw_piece(
            builder,
            piece,
            cell,
            lift,
            1.0,
            &theme,
            i16::try_from(index).map_err(|_| RenderListError::InvalidGeometry)?,
        )?;
    }
    if let Some((animation, composition)) = active {
        draw_composition(builder, animation, composition, frame, layout)?;
    }
    if let super::Interaction::Dragging { from, pointer, .. } = local.interaction {
        if let Some(Some(piece)) = view.board.get(usize::from(from.0)) {
            let cell = Rect::new(
                pointer.get() - Vec2::splat(layout.square_size() * 0.5),
                Vec2::splat(layout.square_size()),
            )?;
            draw_piece(
                builder,
                *piece,
                cell,
                0.0,
                1.04,
                &theme,
                IN_TRANSIT_PIECE_Z + 10,
            )?;
        }
    }
    Ok(())
}

fn draw_composition(
    builder: &mut RenderListBuilder,
    animation: ChessMoveAnimation,
    composition: Composition,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    if let Some(victim) = composition.victim {
        let sample = animation.capture_timeline.sample(frame.now_ms());
        if !sample.done {
            let factor = sample.factor.clamp(0.0, 1.0);
            let z = IN_TRANSIT_PIECE_Z - 2;
            builder.push(RenderCmd::PushOpacity {
                opacity: opacity(1.0 - factor)?,
                layer: Layer::PIECES,
                z,
            })?;
            draw_piece(
                builder,
                victim.piece,
                layout
                    .square_rect(victim.square)
                    .ok_or(RenderListError::InvalidGeometry)?,
                0.0,
                1.0 - factor * 0.28,
                &theme,
                z,
            )?;
            builder.push(RenderCmd::PopOpacity {
                layer: Layer::PIECES,
                z,
            })?;
        }
    }
    for (travel, z) in [
        (Some(composition.mover), IN_TRANSIT_PIECE_Z),
        (composition.rook, IN_TRANSIT_PIECE_Z + 1),
    ] {
        let Some(travel) = travel else { continue };
        let from = layout
            .square_rect(travel.from)
            .ok_or(RenderListError::InvalidGeometry)?;
        let to = layout
            .square_rect(travel.to)
            .ok_or(RenderListError::InvalidGeometry)?;
        let factor = animation
            .timeline
            .sample(frame.now_ms())
            .factor
            .clamp(0.0, 1.0);
        let cell = Rect::new(
            tabula_presentation::lerp_vec2(from.origin(), to.origin(), factor),
            from.size(),
        )?;
        // A low lift/arc resolves exactly to zero, with a 0.94→1 landing scale.
        let lift = 4.0 * factor * (1.0 - factor) * from.size().y.min(80.0) * 0.15;
        let scale = 0.94 + factor * 0.06;
        draw_piece(builder, travel.piece, cell, lift, scale, &theme, z)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_piece(
    builder: &mut RenderListBuilder,
    piece: Piece,
    cell: Rect,
    lift: f32,
    scale: f32,
    theme: &Theme,
    z: i16,
) -> Result<(), RenderListError> {
    // Concentric low-opacity capsules are renderer-neutral soft contact shadows.
    // They remain grounded while the upright sprite lifts, and never cover markers.
    for (spread, alpha) in [(1.28, 0.07), (1.0, 0.12)] {
        let size = Vec2::new(
            cell.size().x * 0.52 * spread + lift * 0.3,
            cell.size().y * 0.07 * spread,
        );
        let rect = Rect::new(
            cell.origin() + Vec2::new((cell.size().x - size.x) * 0.5, cell.size().y * 0.82),
            size,
        )?;
        builder.push(RenderCmd::PushOpacity {
            opacity: opacity(alpha)?,
            layer: Layer::PIECES,
            z,
        })?;
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(size.y * 0.5)?,
            fill: Some(Paint::Solid(theme.game_art.chess.deep)),
            border: None,
            layer: Layer::PIECES,
            z,
        })?;
        builder.push(RenderCmd::PopOpacity {
            layer: Layer::PIECES,
            z,
        })?;
    }
    let size = cell.size() * scale;
    let sprite_cell = Rect::new(
        cell.origin() + (cell.size() - size) * 0.5 - Vec2::new(0.0, lift),
        size,
    )?;
    builder.push(piece_sprite(piece, sprite_cell, theme, Layer::PIECES, z)?)
}

fn opacity(value: f32) -> Result<Opacity, RenderListError> {
    Opacity::try_from(value).map_err(|_| RenderListError::InvalidGeometry)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::presentation::ChessPresentation;
    use crate::{ChessRules, Command, State};
    use tabula_core::{DetRng, InputIndex, LogicalTime, MatchSeed, SeatId, Viewer};
    use tabula_game_api::{Budget, Ctx, GameRules, Input};
    use tabula_presentation::{Dpi, GamePresentation, InputEvent, Key, Viewport};

    fn frame(now: u64) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).unwrap(),
            Dpi::new(1.0).unwrap(),
            now,
            Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    fn accept(
        state: &mut State,
        local: &mut ChessLocal,
        from: u8,
        to: u8,
        promotion: Option<PieceKind>,
        now: u64,
    ) -> View {
        let previous = ChessRules::project(state, Viewer::Seat(SeatId(0)));
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(1));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(1),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let outcome = ChessRules::apply(
            state,
            Input::Player {
                seat: previous.turn.seat(),
                command: Command::Move {
                    from,
                    to,
                    promotion,
                },
            },
            &mut ctx,
        )
        .unwrap();
        let current = ChessRules::project(state, Viewer::Seat(SeatId(0)));
        for event in outcome.events {
            if let Some(event) = ChessRules::view_event(state, &event, Viewer::Seat(SeatId(0))) {
                ChessPresentation::on_view_event_with_projection(
                    &event,
                    Some(&previous),
                    &current,
                    local,
                    &frame(now),
                );
            }
        }
        current
    }

    fn piece_count(list: &tabula_presentation::RenderList) -> usize {
        list.commands()
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
            .count()
    }

    fn pivot_at(list: &tabula_presentation::RenderList, z: i16) -> Option<Vec2> {
        list.commands().iter().find_map(|cmd| match cmd {
            RenderCmd::Sprite {
                pivot,
                layer: Layer::PIECES,
                z: actual,
                rotation,
                ..
            } if *actual == z => {
                assert_eq!(*rotation, 0.0, "orientation never rotates glyphs");
                Some(*pivot)
            }
            _ => None,
        })
    }

    #[test]
    fn accepted_move_has_low_arc_landing_scale_and_grounded_soft_shadow() {
        let mut state = State::initial();
        let mut local = ChessLocal::default();
        let current = accept(&mut state, &mut local, 12, 28, None, 1000);
        let now = frame(1100);
        let layout = BoardLayout::oriented(now.viewport(), false);
        let animation = local.move_animation.unwrap();
        let factor = animation
            .timeline
            .sample(now.now_ms())
            .factor
            .clamp(0.0, 1.0);
        let from = layout.square_rect(Square(12)).unwrap();
        let to = layout.square_rect(Square(28)).unwrap();
        let expected_linear =
            tabula_presentation::lerp_vec2(from.origin(), to.origin(), factor) + from.size() * 0.5;
        let rendered = ChessPresentation::present(&current, &local, &now);
        let actual = pivot_at(&rendered, IN_TRANSIT_PIECE_Z).unwrap();
        assert_eq!(actual.x, expected_linear.x);
        assert!(
            actual.y < expected_linear.y,
            "the moving piece lifts above its path"
        );
        assert_eq!(piece_count(&rendered), 32);
        assert!(rendered.commands().iter().any(|cmd| matches!(cmd,
            RenderCmd::Rect { fill: Some(Paint::Solid(color)), layer: Layer::PIECES, .. }
                if *color == now.theme().game_art.chess.deep)));
        let moving_width = rendered
            .commands()
            .iter()
            .find_map(|cmd| match cmd {
                RenderCmd::Sprite {
                    rect,
                    z: IN_TRANSIT_PIECE_Z,
                    layer: Layer::PIECES,
                    ..
                } => Some(rect.size().x),
                _ => None,
            })
            .unwrap();
        // The licensed sprite occupies 97% of its animated cell; landing
        // still scales the cell from 0.94 to 1.0.
        assert!((moving_width - from.size().x * 0.97 * (0.94 + factor * 0.06)).abs() < 0.001);
        assert!(pivot_at(
            &ChessPresentation::present(&current, &local, &frame(1280)),
            IN_TRANSIT_PIECE_Z
        )
        .is_none());
    }

    #[test]
    fn accepted_capture_fades_at_actual_square_including_both_en_passant_colors() {
        for (fen, from, to, victim_square) in [
            ("4k3/8/3p4/4P3/8/8/8/4K3 w - - 0 1", 36, 43, 43),
            ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", 36, 43, 35),
            ("4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1", 28, 19, 27),
        ] {
            for flipped in [false, true] {
                let mut state = State::from_fen(fen).unwrap();
                let mut local = ChessLocal {
                    flipped,
                    ..ChessLocal::default()
                };
                let current = accept(&mut state, &mut local, from, to, None, 1000);
                let layout = BoardLayout::oriented(frame(1000).viewport(), flipped);
                let capture = ChessPresentation::present(&current, &local, &frame(1050));
                let rect = layout.square_rect(Square(victim_square)).unwrap();
                assert_eq!(
                    pivot_at(&capture, IN_TRANSIT_PIECE_Z - 2),
                    Some(rect.origin() + rect.size() * 0.5)
                );
                assert_eq!(
                    piece_count(&capture),
                    4,
                    "3 current pieces and exactly one fading victim"
                );
                assert!(capture.commands().iter().any(|cmd| matches!(cmd,
                    RenderCmd::PushOpacity { opacity, z, .. } if *z == IN_TRANSIT_PIECE_Z - 2 && opacity.get() > 0.0 && opacity.get() < 1.0)));
                let after_fade = ChessPresentation::present(&current, &local, &frame(1160));
                assert!(pivot_at(&after_fade, IN_TRANSIT_PIECE_Z - 2).is_none());
                assert_eq!(piece_count(&after_fade), 3);
            }
        }
    }

    #[test]
    fn accepted_castling_has_two_synchronized_upright_travels_and_no_endpoint_duplicates() {
        for (fen, from, to, rook_to) in [
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", 4, 6, 5_u8),
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", 4, 2, 3_u8),
            ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", 60, 62, 61_u8),
            ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", 60, 58, 59_u8),
        ] {
            for flipped in [false, true] {
                let mut state = State::from_fen(fen).unwrap();
                let mut local = ChessLocal {
                    flipped,
                    ..ChessLocal::default()
                };
                let current = accept(&mut state, &mut local, from, to, None, 1000);
                let mid = ChessPresentation::present(&current, &local, &frame(1100));
                assert_eq!(piece_count(&mid), 6);
                assert!(pivot_at(&mid, IN_TRANSIT_PIECE_Z).is_some());
                assert!(pivot_at(&mid, IN_TRANSIT_PIECE_Z + 1).is_some());
                assert!(pivot_at(&mid, i16::from(to)).is_none());
                assert!(pivot_at(&mid, i16::from(rook_to)).is_none());
                let end = ChessPresentation::present(&current, &local, &frame(1280));
                assert!(pivot_at(&end, IN_TRANSIT_PIECE_Z).is_none());
                assert!(pivot_at(&end, IN_TRANSIT_PIECE_Z + 1).is_none());
                assert!(pivot_at(&end, i16::from(to)).is_some());
                assert!(pivot_at(&end, i16::from(rook_to)).is_some());
            }
        }
    }

    #[test]
    fn projection_pin_new_move_and_reduced_motion_discard_old_composition() {
        let mut state = State::initial();
        let mut local = ChessLocal::default();
        let first = accept(&mut state, &mut local, 12, 28, None, 1000);
        let mut mismatch = first.clone();
        mismatch.fullmove_number += 1;
        assert!(pivot_at(
            &ChessPresentation::present(&mismatch, &local, &frame(1100)),
            IN_TRANSIT_PIECE_Z
        )
        .is_none());
        let second = accept(&mut state, &mut local, 52, 36, None, 1050);
        assert_eq!(local.move_animation.unwrap().from, Square(52));
        assert_eq!(
            piece_count(&ChessPresentation::present(&second, &local, &frame(1100))),
            32
        );
        assert!(pivot_at(
            &ChessPresentation::present(&first, &local, &frame(1100)),
            IN_TRANSIT_PIECE_Z
        )
        .is_none());
        local.flipped = true;
        assert!(pivot_at(
            &ChessPresentation::present(&second, &local, &frame(1100)),
            IN_TRANSIT_PIECE_Z
        )
        .is_none());
        local.set_reduced_motion(true);
        assert!(local.move_animation.is_none());
        accept(&mut state, &mut local, 6, 21, None, 1100);
        assert!(local.move_animation.is_none());
    }

    #[test]
    fn input_blur_rejection_and_reset_snap_without_changing_the_projection() {
        for input in [
            InputEvent::Key {
                key: Key::ArrowRight,
                pressed: true,
            },
            InputEvent::Focus(false),
        ] {
            let mut state = State::initial();
            let mut local = ChessLocal::default();
            let current = accept(&mut state, &mut local, 12, 28, None, 1000);
            let expected = current.board;
            assert!(local.move_animation.is_some());
            ChessPresentation::on_input(&input, &current, &mut local);
            assert!(local.move_animation.is_none());
            assert_eq!(current.board, expected);
        }
        let mut state = State::initial();
        let mut local = ChessLocal::default();
        let current = accept(&mut state, &mut local, 12, 28, None, 1000);
        ChessPresentation::on_command_rejected(&mut local);
        assert!(local.move_animation.is_none());
        assert_eq!(
            piece_count(&ChessPresentation::present(&current, &local, &frame(1100))),
            32
        );
        local = ChessLocal::default();
        assert!(local.move_animation.is_none());
    }

    #[test]
    fn timestamped_motion_accepts_threshold_and_snaps_only_later_arrivals() {
        let local = ChessLocal::default();
        assert!(ChessMoveAnimation::start(
            Square(12),
            Square(28),
            ChessColor::White,
            None,
            None,
            1000,
            &local,
            &frame(1600)
        )
        .is_some());
        assert!(ChessMoveAnimation::start(
            Square(12),
            Square(28),
            ChessColor::White,
            None,
            None,
            1000,
            &local,
            &frame(1601)
        )
        .is_none());
        assert!(ChessMoveAnimation::start(
            Square(255),
            Square(28),
            ChessColor::White,
            None,
            None,
            1000,
            &local,
            &frame(1000)
        )
        .is_none());
    }

    #[test]
    fn accepted_promotion_uses_prior_pawn_then_each_upright_endpoint_choice() {
        for (fen, from, to, color_prefix) in [
            ("k7/4P3/8/8/8/8/8/4K3 w - - 0 1", 52, 60, "white"),
            ("4k3/8/8/8/8/8/4p3/K7 b - - 0 1", 12, 4, "black"),
        ] {
            for kind in [
                PieceKind::Queen,
                PieceKind::Rook,
                PieceKind::Bishop,
                PieceKind::Knight,
            ] {
                for flipped in [false, true] {
                    let mut state = State::from_fen(fen).unwrap();
                    let mut local = ChessLocal {
                        flipped,
                        ..ChessLocal::default()
                    };
                    let current = accept(&mut state, &mut local, from, to, Some(kind), 1000);
                    let mid = ChessPresentation::present(&current, &local, &frame(1100));
                    let prior_asset = format!("pieces/{color_prefix}-pawn");
                    assert!(mid.commands().iter().any(|cmd| matches!(cmd,
                        RenderCmd::Sprite { asset, z: IN_TRANSIT_PIECE_Z, rotation, layer: Layer::PIECES, .. }
                            if asset.as_str() == prior_asset && *rotation == 0.0)));
                    assert_eq!(piece_count(&mid), 3);
                    let direct = ChessPresentation::present(&current, &local, &frame(1280));
                    for timestamp in [1000, 1023, 1070, 1110, 1240, 1279] {
                        let _ = ChessPresentation::present(&current, &local, &frame(timestamp));
                    }
                    assert_eq!(
                        direct,
                        ChessPresentation::present(&current, &local, &frame(1280))
                    );
                    let endpoint_asset = super::super::assets::piece_asset(Piece {
                        color: current.turn.other(),
                        kind,
                    });
                    assert!(direct.commands().iter().any(|cmd| matches!(cmd,
                        RenderCmd::Sprite { asset, z, rotation, layer: Layer::PIECES, .. }
                            if *z == i16::from(to) && *asset == endpoint_asset && *rotation == 0.0)));
                    assert_eq!(piece_count(&direct), 3);
                }
            }
        }
    }

    #[test]
    fn legacy_event_only_capture_does_not_invent_a_victim_square() {
        let mut state = State::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        let mut local = ChessLocal::default();
        let current = accept(&mut state, &mut local, 36, 43, None, 1000);
        ChessPresentation::on_view_event(
            &crate::ViewEvent::Moved {
                seat: SeatId(0),
                from: Square(36),
                to: Square(43),
                promotion: None,
                captured: Some(Piece {
                    color: ChessColor::Black,
                    kind: PieceKind::Pawn,
                }),
            },
            &mut local,
            &frame(1000),
        );
        let list = ChessPresentation::present(&current, &local, &frame(1050));
        assert!(pivot_at(&list, IN_TRANSIT_PIECE_Z).is_some());
        assert!(pivot_at(&list, IN_TRANSIT_PIECE_Z - 2).is_none());
        assert_eq!(piece_count(&list), 3);
    }
}
