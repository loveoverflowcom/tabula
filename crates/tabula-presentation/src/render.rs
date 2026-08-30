use std::cmp::Ordering;

use glam::{Affine2, Vec2};
use smallvec::SmallVec;
use tabula_design::Color;

/// A screen-space rectangle. Construction rejects non-finite and negative sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub origin: Vec2,
    pub size: Vec2,
}

impl Rect {
    pub fn new(origin: Vec2, size: Vec2) -> Result<Self, RenderListError> {
        if !origin.is_finite() || !size.is_finite() || size.x < 0.0 || size.y < 0.0 {
            return Err(RenderListError::InvalidGeometry);
        }
        Ok(Self { origin, size })
    }
    #[must_use]
    #[allow(clippy::float_arithmetic)]
    pub fn contains(self, point: Vec2) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x <= self.origin.x + self.size.x
            && point.y <= self.origin.y + self.size.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}
impl Corners {
    #[must_use]
    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// Semantic draw layers; higher values appear above lower values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Layer(pub u8);
impl Layer {
    pub const BOARD: Self = Self(0);
    pub const PIECES: Self = Self(10);
    pub const OVERLAY: Self = Self(20);
    pub const HUD: Self = Self(30);
    pub const MODAL: Self = Self(40);
    pub const TOAST: Self = Self(50);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2D {
    pub center: Vec2,
    pub zoom: f32,
}
impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

/// An opacity proof barrier. Values are always in the inclusive 0..=1 range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(f32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpacityError {
    NonFinite,
    OutOfRange,
}
impl TryFrom<f32> for Opacity {
    type Error = OpacityError;
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            Err(OpacityError::NonFinite)
        } else if !(0.0..=1.0).contains(&value) {
            Err(OpacityError::OutOfRange)
        } else {
            Ok(Self(value))
        }
    }
}
impl Opacity {
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

#[allow(clippy::doc_markdown)]
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    LinearGradient {
        from: Vec2,
        to: Vec2,
        stops: SmallVec<[(f32, Color); 4]>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyleToken {
    Display,
    Headline,
    Title,
    Body,
    Label,
    Mono,
}

/// The intentionally small backend-neutral rendering vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderCmd {
    Sprite {
        asset: String,
        rect: Rect,
        src: Option<Rect>,
        tint: Color,
        rotation: f32,
        pivot: Vec2,
        layer: Layer,
        z: i16,
    },
    Rect {
        rect: Rect,
        radii: Corners,
        fill: Option<Paint>,
        border: Option<Border>,
        layer: Layer,
        z: i16,
    },
    Text {
        text: String,
        at: Vec2,
        style: TextStyleToken,
        align: Align,
        max_width: Option<f32>,
        color: Color,
        layer: Layer,
        z: i16,
    },
    Path {
        points: SmallVec<[Vec2; 8]>,
        stroke: Border,
        closed: bool,
        fill: Option<Paint>,
        layer: Layer,
        z: i16,
    },
    PushClip {
        rect: Rect,
    },
    PopClip,
    PushTransform {
        matrix: Affine2,
    },
    PopTransform,
    PushOpacity {
        opacity: Opacity,
    },
    PopOpacity,
}

impl RenderCmd {
    fn draw_key(&self) -> Option<(Layer, i16)> {
        match self {
            Self::Sprite { layer, z, .. }
            | Self::Rect { layer, z, .. }
            | Self::Text { layer, z, .. }
            | Self::Path { layer, z, .. } => Some((*layer, *z)),
            _ => None,
        }
    }
}

/// Immutable, validated command list consumed by every renderer.
///
/// @ai.role renderer-contract
/// @ai.domain presentation.render
/// @ai.invariant balanced-render-state
/// @ai.law deterministic-layer-order
/// @ai.evidence tests::builder_orders_draws_within_render_scopes
#[allow(clippy::doc_markdown)]
#[derive(Clone, Debug, PartialEq)]
pub struct RenderList {
    pub commands: Vec<RenderCmd>,
    pub camera: Camera2D,
}

/// Fallible builder that validates geometry and balances state stacks before exposing a list.
#[derive(Debug, Default)]
pub struct RenderListBuilder {
    camera: Camera2D,
    commands: Vec<RenderCmd>,
    clip_depth: usize,
    transform_depth: usize,
    opacity_depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderListError {
    InvalidGeometry,
    InvalidTransform,
    InvalidTextWidth,
    UnbalancedClip,
    UnbalancedTransform,
    UnbalancedOpacity,
}

impl RenderListBuilder {
    #[must_use]
    pub fn new(camera: Camera2D) -> Self {
        Self {
            camera,
            ..Self::default()
        }
    }
    pub fn push(&mut self, command: RenderCmd) -> Result<(), RenderListError> {
        Self::validate(&command)?;
        match command {
            RenderCmd::PushClip { .. } => self.clip_depth += 1,
            RenderCmd::PopClip => {
                self.clip_depth = self
                    .clip_depth
                    .checked_sub(1)
                    .ok_or(RenderListError::UnbalancedClip)?;
            }
            RenderCmd::PushTransform { .. } => self.transform_depth += 1,
            RenderCmd::PopTransform => {
                self.transform_depth = self
                    .transform_depth
                    .checked_sub(1)
                    .ok_or(RenderListError::UnbalancedTransform)?;
            }
            RenderCmd::PushOpacity { .. } => self.opacity_depth += 1,
            RenderCmd::PopOpacity => {
                self.opacity_depth = self
                    .opacity_depth
                    .checked_sub(1)
                    .ok_or(RenderListError::UnbalancedOpacity)?;
            }
            _ => {}
        }
        self.commands.push(command);
        Ok(())
    }
    pub fn finish(mut self) -> Result<RenderList, RenderListError> {
        if self.clip_depth != 0 {
            return Err(RenderListError::UnbalancedClip);
        }
        if self.transform_depth != 0 {
            return Err(RenderListError::UnbalancedTransform);
        }
        if self.opacity_depth != 0 {
            return Err(RenderListError::UnbalancedOpacity);
        }
        // State operations delimit scopes. Sorting only inside a scope preserves clipping,
        // transform, and opacity semantics while still giving deterministic layer/z order.
        let mut start = 0;
        for index in 0..=self.commands.len() {
            if index == self.commands.len() || self.commands[index].draw_key().is_none() {
                self.commands[start..index].sort_by(|a, b| match (a.draw_key(), b.draw_key()) {
                    (Some(left), Some(right)) => left.cmp(&right),
                    _ => Ordering::Equal,
                });
                start = index + 1;
            }
        }
        Ok(RenderList {
            commands: self.commands,
            camera: self.camera,
        })
    }
    fn validate(command: &RenderCmd) -> Result<(), RenderListError> {
        let finite = |v: Vec2| v.is_finite();
        match command {
            RenderCmd::Sprite {
                rect,
                src,
                rotation,
                pivot,
                ..
            } => {
                Self::valid_rect(*rect)?;
                if let Some(src) = src {
                    Self::valid_rect(*src)?;
                }
                if !rotation.is_finite() || !finite(*pivot) {
                    return Err(RenderListError::InvalidGeometry);
                }
            }
            RenderCmd::Rect {
                rect,
                radii,
                border,
                ..
            } => {
                Self::valid_rect(*rect)?;
                if ![
                    radii.top_left,
                    radii.top_right,
                    radii.bottom_right,
                    radii.bottom_left,
                ]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
                    || border.is_some_and(|border| !border.width.is_finite() || border.width < 0.0)
                {
                    return Err(RenderListError::InvalidGeometry);
                }
            }
            RenderCmd::Text { at, max_width, .. } => {
                if !finite(*at) || max_width.is_some_and(|width| !width.is_finite() || width <= 0.0)
                {
                    return Err(RenderListError::InvalidTextWidth);
                }
            }
            RenderCmd::Path { points, stroke, .. } => {
                if points.iter().any(|point| !finite(*point))
                    || !stroke.width.is_finite()
                    || stroke.width < 0.0
                {
                    return Err(RenderListError::InvalidGeometry);
                }
            }
            RenderCmd::PushClip { rect } => Self::valid_rect(*rect)?,
            RenderCmd::PushTransform { matrix } => {
                if !matrix.matrix2.x_axis.is_finite()
                    || !matrix.matrix2.y_axis.is_finite()
                    || !finite(matrix.translation)
                {
                    return Err(RenderListError::InvalidTransform);
                }
            }
            RenderCmd::PushOpacity { .. }
            | RenderCmd::PopClip
            | RenderCmd::PopTransform
            | RenderCmd::PopOpacity => {}
        }
        Ok(())
    }
    fn valid_rect(rect: Rect) -> Result<(), RenderListError> {
        if !rect.origin.is_finite()
            || !rect.size.is_finite()
            || rect.size.x < 0.0
            || rect.size.y < 0.0
        {
            Err(RenderListError::InvalidGeometry)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(layer: Layer, z: i16) -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
            radii: Corners::uniform(0.0),
            fill: None,
            border: None,
            layer,
            z,
        }
    }
    #[test]
    fn builder_orders_draws_within_render_scopes() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(rect(Layer::HUD, 1)).unwrap();
        builder.push(rect(Layer::BOARD, 2)).unwrap();
        builder
            .push(RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
            })
            .unwrap();
        builder.push(rect(Layer::PIECES, 0)).unwrap();
        builder.push(RenderCmd::PopOpacity).unwrap();
        let list = builder.finish().unwrap();
        assert!(matches!(
            list.commands[0],
            RenderCmd::Rect {
                layer: Layer::BOARD,
                ..
            }
        ));
        assert!(matches!(
            list.commands[1],
            RenderCmd::Rect {
                layer: Layer::HUD,
                ..
            }
        ));
        assert!(matches!(list.commands[2], RenderCmd::PushOpacity { .. }));
    }
    #[test]
    fn builder_rejects_unbalanced_state() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        assert_eq!(
            builder.push(RenderCmd::PopClip),
            Err(RenderListError::UnbalancedClip)
        );
    }
    #[test]
    fn opacity_is_a_proof_barrier() {
        assert_eq!(Opacity::try_from(1.01), Err(OpacityError::OutOfRange));
        assert_eq!(Opacity::try_from(f32::NAN), Err(OpacityError::NonFinite));
    }
}
