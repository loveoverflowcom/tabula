//! Chess-only board depth and quiet material. (doc 04 §5, issue #85)
//!
//! The frame and grain are decorative commands beneath every semantic marker.
//! No board image encodes squares, coordinates, state or input geometry.
#![allow(clippy::float_arithmetic)]

use super::{
    assets, BoardLayout, Corners, FrameCtx, Layer, Paint, Rect, RenderCmd, RenderListBuilder,
    RenderListError, Vec2,
};
use tabula_presentation::{Border, GradientStop, LinearGradient, Opacity};

pub(super) fn draw_frame(
    builder: &mut RenderListBuilder,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    if layout.board.size().x < 32.0 {
        return Ok(());
    }
    let theme = frame.theme();
    let art = theme.game_art.chess;
    let envelope = layout.table;
    let border = (layout.board.origin().x - envelope.origin().x).max(0.0);
    let depth = (border * 0.42).min(6.0);
    let face = Rect::new(envelope.origin(), envelope.size() - Vec2::new(0.0, depth))?;
    let radius = (border * 0.65).min(10.0);

    // The contact shadow and lower rim are contained in the already reserved
    // table slot. They never steal space from player bars, actions or host UI.
    builder.push(RenderCmd::PushOpacity {
        opacity: Opacity::try_from(0.32).map_err(|_| RenderListError::InvalidGeometry)?,
        layer: Layer::BOARD,
        z: -9,
    })?;
    builder.push(RenderCmd::Rect {
        rect: envelope,
        radii: Corners::uniform(radius)?,
        fill: Some(Paint::Solid(theme.color.hidden)),
        border: None,
        layer: Layer::BOARD,
        z: 0,
    })?;
    builder.push(RenderCmd::PopOpacity {
        layer: Layer::BOARD,
        z: -9,
    })?;
    builder.push(RenderCmd::Rect {
        rect: Rect::new(
            envelope.origin() + Vec2::new(0.0, depth),
            envelope.size() - Vec2::new(0.0, depth),
        )?,
        radii: Corners::uniform(radius)?,
        fill: Some(Paint::Solid(art.deep)),
        border: None,
        layer: Layer::BOARD,
        z: -8,
    })?;
    builder.push(RenderCmd::Rect {
        rect: face,
        radii: Corners::uniform(radius)?,
        fill: Some(Paint::LinearGradient(LinearGradient::new(
            face.origin(),
            face.origin() + face.size(),
            [
                GradientStop::new(0.0, art.board_dark)?,
                GradientStop::new(0.45, art.deep)?,
                GradientStop::new(1.0, art.board_dark)?,
            ],
        )?)),
        border: Some(Border::new(1.0, art.brass)?),
        layer: Layer::BOARD,
        z: -7,
    })?;

    let inset = (border * 0.35).min(4.0);
    builder.push(RenderCmd::PushOpacity {
        opacity: Opacity::try_from(0.32).map_err(|_| RenderListError::InvalidGeometry)?,
        layer: Layer::BOARD,
        z: -6,
    })?;
    builder.push(RenderCmd::Rect {
        rect: Rect::new(
            face.origin() + Vec2::splat(inset),
            face.size() - Vec2::splat(inset * 2.0),
        )?,
        radii: Corners::uniform((radius - inset).max(0.0))?,
        fill: None,
        border: Some(Border::new(1.0, art.brass)?),
        layer: Layer::BOARD,
        z: 0,
    })?;
    builder.push(RenderCmd::PopOpacity {
        layer: Layer::BOARD,
        z: -6,
    })?;
    builder.push(RenderCmd::Rect {
        rect: layout.board,
        radii: Corners::uniform(0.0)?,
        fill: None,
        border: Some(Border::new(2.0, art.deep)?),
        layer: Layer::BOARD,
        z: -5,
    })?;
    Ok(())
}

pub(super) fn draw_grain(
    builder: &mut RenderListBuilder,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    // High-contrast boards keep the exact solid cells; ornamental texture is
    // unnecessary there. The source alpha is only .035 in standard schemes.
    if matches!(
        frame.theme().kind,
        tabula_design::ThemeKind::HighContrastLight | tabula_design::ThemeKind::HighContrastDark
    ) || layout.square_size() < 12.0
    {
        return Ok(());
    }
    for index in 0..64_u8 {
        let square = crate::Square::new(index).ok_or(RenderListError::InvalidGeometry)?;
        let rect = layout
            .square_rect(square)
            .ok_or(RenderListError::InvalidGeometry)?;
        builder.push(RenderCmd::Sprite {
            asset: assets::grain_asset(),
            rect,
            tint: frame.theme().game_art.chess.piece_tint,
            rotation: 0.0,
            pivot: rect.origin() + rect.size() * 0.5,
            layer: Layer::BOARD,
            z: 64 + i16::from(index),
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use tabula_design::{Theme, ThemeKind};
    use tabula_presentation::{Camera2D, Dpi, Viewport};

    #[test]
    fn decorative_frame_stays_in_its_reserved_slot_and_grain_is_below_markers() {
        for (width, height) in [
            (1440.0, 960.0),
            (1100.0, 850.0),
            (390.0, 844.0),
            (320.0, 640.0),
            (844.0, 390.0),
        ] {
            for flipped in [false, true] {
                let viewport = Viewport::new(Vec2::new(width, height)).unwrap();
                let frame = FrameCtx::new(
                    viewport,
                    Dpi::new(1.0).unwrap(),
                    0,
                    Theme::by_kind(ThemeKind::Dark),
                );
                let layout = BoardLayout::oriented(viewport, flipped);
                let mut builder = RenderListBuilder::new(Camera2D::default());
                draw_frame(&mut builder, &frame, layout).unwrap();
                draw_grain(&mut builder, &frame, layout).unwrap();
                let list = builder.finish().unwrap();
                let mut grains = 0;
                for cmd in list.commands() {
                    match cmd {
                        RenderCmd::Rect { rect, layer, .. } => {
                            assert_eq!(*layer, Layer::BOARD);
                            assert!(layout.table.contains(rect.origin()));
                            assert!(layout.table.contains(rect.origin() + rect.size()));
                        }
                        RenderCmd::Sprite {
                            asset,
                            rect,
                            rotation,
                            layer,
                            ..
                        } => {
                            assert_eq!(*asset, assets::grain_asset());
                            assert_eq!(*rotation, 0.0);
                            assert_eq!(*layer, Layer::BOARD);
                            assert!(layout.board.contains(rect.origin()));
                            let corner = rect.origin() + rect.size();
                            let board_corner = layout.board.origin() + layout.board.size();
                            assert!(
                                corner.cmple(board_corner + Vec2::splat(0.001)).all(),
                                "{width}x{height}: grain corner {corner:?}, board {board_corner:?}"
                            );
                            grains += 1;
                        }
                        _ => {}
                    }
                }
                assert_eq!(grains, 64);
            }
        }
    }

    #[test]
    fn high_contrast_removes_only_decorative_grain() {
        for kind in [ThemeKind::HighContrastLight, ThemeKind::HighContrastDark] {
            let viewport = Viewport::new(Vec2::new(390.0, 844.0)).unwrap();
            let frame = FrameCtx::new(viewport, Dpi::new(1.0).unwrap(), 0, Theme::by_kind(kind));
            let layout = BoardLayout::from_viewport(viewport);
            let mut builder = RenderListBuilder::new(Camera2D::default());
            draw_grain(&mut builder, &frame, layout).unwrap();
            assert!(builder.finish().unwrap().commands().is_empty());
        }
    }
}
