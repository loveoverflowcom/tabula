//! Pure interpretation of the validated render-state stack.

#![allow(clippy::doc_markdown)]

use glam::{Affine2, Vec2};
use tabula_presentation::{Camera2D, Opacity, Rect, RenderCmd, RenderList};

/// Effective state for one primitive in a flattened [`RenderList`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrawState {
    pub(crate) transform: Affine2,
    pub(crate) clip: Clip,
    pub(crate) opacity: Opacity,
}

impl Default for DrawState {
    fn default() -> Self {
        Self {
            transform: Affine2::IDENTITY,
            clip: Clip::Unbounded,
            opacity: Opacity::try_from(1.0).expect("one is a valid opacity"),
        }
    }
}

/// A logical viewport scissor, intentionally independent of camera and transforms. (doc 04 §5.1.1)
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Clip {
    Unbounded,
    Rect(Rect),
    Empty,
}

impl Clip {
    fn intersect(self, rect: Rect) -> Self {
        if rect.size().x <= 0.0 || rect.size().y <= 0.0 {
            return Self::Empty;
        }
        match self {
            Self::Unbounded => Self::Rect(rect),
            Self::Rect(current) => intersect_rects(current, rect).map_or(Self::Empty, Self::Rect),
            Self::Empty => Self::Empty,
        }
    }
}

/// Visits draw primitives in command-stream order with their fully composed backend state.
///
/// `RenderList` construction has already proven scope balance and nesting, so this function only
/// interprets state; it neither sorts nor mutates the command stream.
///
/// @ai.role renderer-state-interpreter
/// @ai.domain presentation.backend
/// @ai.pure true
/// @ai.invariant logical-clips-ignore-local-transforms
/// @ai.invariant nested-opacity-multiplies
/// @ai.law pop-restores-parent-state
/// @ai.evidence state::tests::nested_scopes_compose_and_pop_restores_the_exact_parent_state
/// @ai.evidence state::tests::camera_maps_after_local_transforms_while_clips_remain_logical
pub(crate) fn visit_draws(list: &RenderList, mut visit: impl FnMut(&RenderCmd, DrawState)) {
    let mut state = DrawState::default();
    let mut stack = Vec::new();

    for command in list.commands() {
        match command {
            RenderCmd::PushClip { rect, .. } => {
                stack.push(state);
                state.clip = state.clip.intersect(*rect);
            }
            RenderCmd::PushTransform { matrix, .. } => {
                stack.push(state);
                state.transform *= *matrix;
            }
            RenderCmd::PushOpacity { opacity, .. } => {
                stack.push(state);
                state.opacity = multiply_opacity(state.opacity, *opacity);
            }
            RenderCmd::PopClip { .. }
            | RenderCmd::PopTransform { .. }
            | RenderCmd::PopOpacity { .. } => {
                state = stack
                    .pop()
                    .expect("RenderList construction guarantees balanced scopes");
            }
            RenderCmd::Sprite { .. }
            | RenderCmd::Rect { .. }
            | RenderCmd::Text { .. }
            | RenderCmd::Path { .. } => visit(command, state),
        }
    }

    debug_assert!(stack.is_empty(), "RenderList construction balances scopes");
}

/// Maps local geometry into logical coordinates using the contract's transform order.
#[must_use]
#[allow(clippy::float_arithmetic)]
pub(crate) fn logical_transform(camera: Camera2D, local: Affine2) -> Affine2 {
    Affine2::from_scale_angle_translation(
        Vec2::splat(camera.zoom()),
        0.0,
        -camera.origin() * camera.zoom(),
    ) * local
}

#[allow(clippy::float_arithmetic)]
fn multiply_opacity(parent: Opacity, child: Opacity) -> Opacity {
    Opacity::try_from(parent.get() * child.get()).expect("the product of valid opacities is valid")
}

#[allow(clippy::float_arithmetic)]
fn intersect_rects(first: Rect, second: Rect) -> Option<Rect> {
    let start = first.origin().max(second.origin());
    let end = (first.origin() + first.size()).min(second.origin() + second.size());
    let size = (end - start).max(Vec2::ZERO);
    (size.x > 0.0 && size.y > 0.0)
        .then(|| Rect::new(start, size).expect("intersection of validated rectangles is valid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_design::{Theme, ThemeKind};
    use tabula_presentation::{Corners, Layer, Paint, RenderListBuilder};

    fn rectangle() -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Theme::by_kind(ThemeKind::Light).color.primary)),
            border: None,
            layer: Layer::BOARD,
            z: 0,
        }
    }

    fn states(list: &RenderList) -> Vec<DrawState> {
        let mut states = Vec::new();
        visit_draws(list, |_, state| states.push(state));
        states
    }

    #[test]
    fn nested_scopes_compose_and_pop_restores_the_exact_parent_state() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_translation(Vec2::new(3.0, 2.0)),
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
        builder
            .push(RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopOpacity {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopOpacity {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();

        let states = states(&builder.finish().unwrap());
        assert_eq!(states[0].opacity, Opacity::try_from(0.25).unwrap());
        assert_eq!(states[1].opacity, Opacity::try_from(0.5).unwrap());
        assert_eq!(
            states[2].transform,
            Affine2::from_translation(Vec2::new(3.0, 2.0))
        );
        assert_eq!(states[3], DrawState::default());
    }

    #[test]
    fn camera_maps_after_local_transforms_while_clips_remain_logical() {
        let camera = Camera2D::new(Vec2::new(2.0, 1.0), 2.0).unwrap();
        let mut builder = RenderListBuilder::new(camera);
        let clip = Rect::new(Vec2::new(4.0, 3.0), Vec2::new(2.0, 2.0)).unwrap();
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_translation(Vec2::new(4.0, 0.0)),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushClip {
                rect: clip,
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopClip {
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

        let state = states(&builder.finish().unwrap())[0];
        assert_eq!(state.clip, Clip::Rect(clip));
        assert_eq!(
            logical_transform(camera, state.transform).transform_point2(Vec2::ZERO),
            Vec2::new(4.0, -2.0)
        );
    }

    #[test]
    fn nested_clips_intersect_and_restore_the_previous_scissor() {
        let outer = Rect::new(Vec2::ZERO, Vec2::splat(8.0)).unwrap();
        let inner = Rect::new(Vec2::new(2.0, 2.0), Vec2::splat(4.0)).unwrap();
        let mut builder = RenderListBuilder::new(Camera2D::default());
        for command in [
            RenderCmd::PushClip {
                rect: outer,
                layer: Layer::BOARD,
                z: 0,
            },
            RenderCmd::PushClip {
                rect: inner,
                layer: Layer::BOARD,
                z: 0,
            },
        ] {
            builder.push(command).unwrap();
        }
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rectangle()).unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();

        let states = states(&builder.finish().unwrap());
        assert_eq!(states[0].clip, Clip::Rect(inner));
        assert_eq!(states[1].clip, Clip::Rect(outer));
    }

    #[test]
    fn direct_zero_area_clips_canonicalize_to_empty() {
        let zero_width = Rect::new(Vec2::ZERO, Vec2::new(0.0, 2.0)).unwrap();
        let zero_height = Rect::new(Vec2::ZERO, Vec2::new(2.0, 0.0)).unwrap();
        assert_eq!(Clip::Unbounded.intersect(zero_width), Clip::Empty);
        assert_eq!(Clip::Unbounded.intersect(zero_height), Clip::Empty);
    }

    #[test]
    fn nested_clip_collapse_stays_empty() {
        let outer = Rect::new(Vec2::ZERO, Vec2::splat(4.0)).unwrap();
        let zero = Rect::new(Vec2::new(2.0, 2.0), Vec2::new(0.0, 1.0)).unwrap();
        assert_eq!(Clip::Rect(outer).intersect(zero), Clip::Empty);
    }
}
