//! Pure Macroquad support validation for a complete [`RenderList`].
//!
//! Structural validity belongs to `tabula-presentation`; this module checks the
//! smaller set of commands and effective states that this backend can execute.
//! It deliberately visits the already-interpreted state stream instead of
//! maintaining a second scope stack.

use tabula_presentation::{FrameCtx, RenderError, RenderList};

use crate::{draw, state};

/// Rejects unsupported or non-executable commands before any primitive is drawn.
pub(crate) fn preflight(list: &RenderList, frame: &FrameCtx) -> Result<(), RenderError> {
    let mut result = Ok(());
    state::visit_draws(list, |command, draw_state| {
        if result.is_ok() {
            result = draw::validate(command, draw_state, list.camera(), frame);
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use glam::{Affine2, Vec2};
    use tabula_design::{Color, TextStyleToken, Theme, ThemeKind};
    use tabula_presentation::{
        Border, Camera2D, Corners, Dpi, Layer, Opacity, Paint, Rect, RenderCmd, RenderCmdKind,
        RenderError, RenderList, RenderListBuilder, Viewport,
    };

    use super::preflight;

    fn color() -> Color {
        Theme::by_kind(ThemeKind::Light).color.primary
    }

    fn frame() -> tabula_presentation::FrameCtx {
        tabula_presentation::FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            0,
            Theme::by_kind(ThemeKind::Light),
        )
    }

    fn rect() -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(Vec2::new(10.0, 10.0), Vec2::splat(100.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(color())),
            border: None,
            layer: Layer::BOARD,
            z: 0,
        }
    }

    fn text() -> RenderCmd {
        RenderCmd::Text {
            text: String::from("supported"),
            at: Vec2::new(10.0, 20.0),
            style: TextStyleToken::BodyMd,
            align: tabula_presentation::Align::Start,
            max_width: None,
            color: color(),
            layer: Layer::HUD,
            z: 0,
        }
    }

    fn list_with(command: RenderCmd) -> RenderList {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(command).unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn ordinary_rect_and_text_lists_pass_macroquad_preflight() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(rect()).unwrap();
        builder.push(text()).unwrap();
        let list = builder.finish().unwrap();

        assert_eq!(preflight(&list, &frame()), Ok(()));
    }

    #[test]
    fn oversized_wrapped_text_is_rejected_before_execution() {
        let mut command = text();
        if let RenderCmd::Text { text: value, .. } = &mut command {
            *value = "x\n".repeat(usize::from(u16::MAX) + 1);
        }
        assert_eq!(
            preflight(&list_with(command), &frame()),
            Err(RenderError::Execution(String::from(
                "backend text has more than u16::MAX lines",
            )))
        );
    }

    #[test]
    fn supported_nested_scopes_pass_with_their_effective_state() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::splat(320.0)).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_scale(Vec2::splat(2.0)),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rect()).unwrap();
        builder.push(text()).unwrap();
        builder
            .push(RenderCmd::PopOpacity {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();

        assert_eq!(preflight(&builder.finish().unwrap(), &frame()), Ok(()));
    }

    #[test]
    fn sprite_is_rejected_before_execution() {
        let list = list_with(RenderCmd::Sprite {
            asset: String::from("deferred/asset"),
            rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
            src: None,
            tint: color(),
            rotation: 0.0,
            pivot: Vec2::ZERO,
            layer: Layer::PIECES,
            z: 0,
        });

        assert_eq!(
            preflight(&list, &frame()),
            Err(RenderError::Unsupported(RenderCmdKind::Sprite))
        );
    }

    #[test]
    fn transformed_text_is_rejected_before_execution() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_scale(Vec2::new(2.0, 1.0)),
                layer: Layer::HUD,
                z: 0,
            })
            .unwrap();
        builder.push(text()).unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::HUD,
                z: 0,
            })
            .unwrap();

        assert_eq!(
            preflight(&builder.finish().unwrap(), &frame()),
            Err(RenderError::Unsupported(RenderCmdKind::Text))
        );
    }

    #[test]
    fn concave_filled_path_is_rejected_before_execution() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(0.0, 3.0),
        ]
        .into_iter()
        .collect();
        let list = list_with(RenderCmd::Path {
            points,
            stroke: Border::new(1.0, color()).unwrap(),
            closed: true,
            fill: Some(Paint::Solid(color())),
            layer: Layer::BOARD,
            z: 0,
        });

        assert_eq!(
            preflight(&list, &frame()),
            Err(RenderError::Unsupported(RenderCmdKind::Path))
        );
    }
}
