use glam::{Affine2, Vec2};
use smallvec::SmallVec;
use tabula_design::Color;

/// A screen-space rectangle. Construction rejects non-finite and negative sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    origin: Vec2,
    size: Vec2,
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

    #[must_use]
    pub const fn origin(self) -> Vec2 {
        self.origin
    }

    #[must_use]
    pub const fn size(self) -> Vec2 {
        self.size
    }
}

/// Validated corner radii for a rounded rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corners {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

impl Corners {
    pub fn new(
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    ) -> Result<Self, RenderListError> {
        let radii = [top_left, top_right, bottom_right, bottom_left];
        if radii
            .iter()
            .any(|radius| !radius.is_finite() || *radius < 0.0)
        {
            return Err(RenderListError::InvalidGeometry);
        }
        Ok(Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        })
    }

    pub fn uniform(radius: f32) -> Result<Self, RenderListError> {
        Self::new(radius, radius, radius, radius)
    }

    #[must_use]
    pub const fn top_left(self) -> f32 {
        self.top_left
    }

    #[must_use]
    pub const fn top_right(self) -> f32 {
        self.top_right
    }

    #[must_use]
    pub const fn bottom_right(self) -> f32 {
        self.bottom_right
    }

    #[must_use]
    pub const fn bottom_left(self) -> f32 {
        self.bottom_left
    }
}

/// Validated border width and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    width: f32,
    color: Color,
}

impl Border {
    pub fn new(width: f32, color: Color) -> Result<Self, RenderListError> {
        if !width.is_finite() || width < 0.0 {
            return Err(RenderListError::InvalidGeometry);
        }
        Ok(Self { width, color })
    }

    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }
}

/// A validated colour stop in a linear gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    offset: f32,
    color: Color,
}

impl GradientStop {
    pub fn new(offset: f32, color: Color) -> Result<Self, RenderListError> {
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(RenderListError::InvalidGradient);
        }
        Ok(Self { offset, color })
    }

    #[must_use]
    pub const fn offset(self) -> f32 {
        self.offset
    }

    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }
}

/// A finite, ordered linear gradient with at least two stops.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    from: Vec2,
    to: Vec2,
    stops: SmallVec<[GradientStop; 4]>,
}

impl LinearGradient {
    pub fn new(
        from: Vec2,
        to: Vec2,
        stops: impl IntoIterator<Item = GradientStop>,
    ) -> Result<Self, RenderListError> {
        if !from.is_finite() || !to.is_finite() {
            return Err(RenderListError::InvalidGradient);
        }
        let stops = stops.into_iter().collect::<SmallVec<[GradientStop; 4]>>();
        if stops.len() < 2 || stops.windows(2).any(|pair| pair[0].offset > pair[1].offset) {
            return Err(RenderListError::InvalidGradient);
        }
        Ok(Self { from, to, stops })
    }

    #[must_use]
    pub const fn from(&self) -> Vec2 {
        self.from
    }

    #[must_use]
    pub const fn to(&self) -> Vec2 {
        self.to
    }

    #[must_use]
    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }
}

#[allow(clippy::doc_markdown)]
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    LinearGradient(LinearGradient),
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

/// A finite camera with strictly positive zoom. It belongs to client-local presentation state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2D {
    center: Vec2,
    zoom: f32,
}

impl Camera2D {
    pub fn new(center: Vec2, zoom: f32) -> Result<Self, RenderListError> {
        if !center.is_finite() || !zoom.is_finite() || zoom <= 0.0 {
            return Err(RenderListError::InvalidCamera);
        }
        Ok(Self { center, zoom })
    }

    #[must_use]
    pub const fn center(self) -> Vec2 {
        self.center
    }

    #[must_use]
    pub const fn zoom(self) -> f32 {
        self.zoom
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new(Vec2::ZERO, 1.0).expect("default camera is valid")
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

/// The intentionally small backend-neutral rendering vocabulary from doc 04 §5.2.
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
        layer: Layer,
        z: i16,
    },
    PopClip {
        layer: Layer,
        z: i16,
    },
    PushTransform {
        matrix: Affine2,
        layer: Layer,
        z: i16,
    },
    PopTransform {
        layer: Layer,
        z: i16,
    },
    PushOpacity {
        opacity: Opacity,
        layer: Layer,
        z: i16,
    },
    PopOpacity {
        layer: Layer,
        z: i16,
    },
}

impl RenderCmd {
    fn key(&self) -> (Layer, i16) {
        match self {
            Self::Sprite { layer, z, .. }
            | Self::Rect { layer, z, .. }
            | Self::Text { layer, z, .. }
            | Self::Path { layer, z, .. }
            | Self::PushClip { layer, z, .. }
            | Self::PopClip { layer, z }
            | Self::PushTransform { layer, z, .. }
            | Self::PopTransform { layer, z }
            | Self::PushOpacity { layer, z, .. }
            | Self::PopOpacity { layer, z } => (*layer, *z),
        }
    }

    fn scope_kind(&self) -> Option<ScopeKind> {
        match self {
            Self::PushClip { .. } | Self::PopClip { .. } => Some(ScopeKind::Clip),
            Self::PushTransform { .. } | Self::PopTransform { .. } => Some(ScopeKind::Transform),
            Self::PushOpacity { .. } | Self::PopOpacity { .. } => Some(ScopeKind::Opacity),
            _ => None,
        }
    }

    fn is_scope_open(&self) -> bool {
        matches!(
            self,
            Self::PushClip { .. } | Self::PushTransform { .. } | Self::PushOpacity { .. }
        )
    }
}

/// Immutable, validated command list consumed by every renderer.
///
/// Commands are a flattened backend stream. The builder retains scopes as an internal tree until
/// `finish`: sibling draws and scope groups are stably ordered by `(layer, z)`, while each scope
/// remains contiguous. A child draw cannot escape its group's ordering position.
///
/// ```compile_fail
/// use tabula_presentation::{Camera2D, RenderList};
/// let _ = RenderList { commands: vec![], camera: Camera2D::default() };
/// ```
///
/// @ai.role renderer-contract
/// @ai.domain presentation.render
/// @ai.invariant balanced-render-state
/// @ai.law deterministic-hierarchical-layer-order
/// @ai.evidence render::tests::scope_groups_preserve_layer_order_without_constraining_children
#[allow(clippy::doc_markdown)]
#[derive(Clone, Debug, PartialEq)]
pub struct RenderList {
    commands: Vec<RenderCmd>,
    camera: Camera2D,
}

impl RenderList {
    #[must_use]
    pub fn commands(&self) -> &[RenderCmd] {
        &self.commands
    }

    #[must_use]
    pub const fn camera(&self) -> Camera2D {
        self.camera
    }
}

/// Fallible builder that validates geometry and turns state scopes into ordered tree groups.
#[derive(Debug, Default)]
pub struct RenderListBuilder {
    camera: Camera2D,
    root: Vec<RenderNode>,
    scopes: Vec<OpenScope>,
}

#[derive(Debug)]
enum RenderNode {
    Draw(RenderCmd),
    Scope {
        opening: RenderCmd,
        closing: RenderCmd,
        children: Vec<RenderNode>,
    },
}

impl RenderNode {
    fn key(&self) -> (Layer, i16) {
        match self {
            Self::Draw(command)
            | Self::Scope {
                opening: command, ..
            } => command.key(),
        }
    }

    fn flatten(self, destination: &mut Vec<RenderCmd>) {
        match self {
            Self::Draw(command) => destination.push(command),
            Self::Scope {
                opening,
                closing,
                mut children,
            } => {
                destination.push(opening);
                Self::flatten_children(&mut children, destination);
                destination.push(closing);
            }
        }
    }

    fn flatten_children(children: &mut Vec<RenderNode>, destination: &mut Vec<RenderCmd>) {
        children.sort_by_key(Self::key);
        for child in std::mem::take(children) {
            child.flatten(destination);
        }
    }
}

#[derive(Debug)]
struct OpenScope {
    opening: RenderCmd,
    children: Vec<RenderNode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeKind {
    Clip,
    Transform,
    Opacity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderListError {
    InvalidGeometry,
    InvalidGradient,
    InvalidTransform,
    InvalidTextWidth,
    UnbalancedClip,
    UnbalancedTransform,
    UnbalancedOpacity,
    ScopeLayerMismatch,
    InvalidCamera,
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
        if command.is_scope_open() {
            self.scopes.push(OpenScope {
                opening: command,
                children: Vec::new(),
            });
        } else if let Some(kind) = command.scope_kind() {
            self.close_scope(kind, command)?;
        } else {
            self.push_node(RenderNode::Draw(command));
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<RenderList, RenderListError> {
        if let Some(scope) = self.scopes.last() {
            return Err(Self::unbalanced(
                scope.opening.scope_kind().expect("scope opening"),
            ));
        }
        let mut commands = Vec::new();
        RenderNode::flatten_children(&mut self.root, &mut commands);
        Ok(RenderList {
            commands,
            camera: self.camera,
        })
    }

    fn push_node(&mut self, node: RenderNode) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.children.push(node);
        } else {
            self.root.push(node);
        }
    }

    fn close_scope(&mut self, kind: ScopeKind, closing: RenderCmd) -> Result<(), RenderListError> {
        let Some(scope) = self.scopes.last() else {
            return Err(Self::unbalanced(kind));
        };
        if scope.opening.scope_kind() != Some(kind) {
            return Err(Self::unbalanced(kind));
        }
        if scope.opening.key() != closing.key() {
            return Err(RenderListError::ScopeLayerMismatch);
        }
        let scope = self.scopes.pop().expect("scope checked above");
        self.push_node(RenderNode::Scope {
            opening: scope.opening,
            closing,
            children: scope.children,
        });
        Ok(())
    }

    fn unbalanced(kind: ScopeKind) -> RenderListError {
        match kind {
            ScopeKind::Clip => RenderListError::UnbalancedClip,
            ScopeKind::Transform => RenderListError::UnbalancedTransform,
            ScopeKind::Opacity => RenderListError::UnbalancedOpacity,
        }
    }

    fn validate(command: &RenderCmd) -> Result<(), RenderListError> {
        let finite = |value: Vec2| value.is_finite();
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
            RenderCmd::Rect { rect, .. } | RenderCmd::PushClip { rect, .. } => {
                Self::valid_rect(*rect)?;
            }
            RenderCmd::Text { at, max_width, .. } => {
                if !finite(*at) || max_width.is_some_and(|width| !width.is_finite() || width <= 0.0)
                {
                    return Err(RenderListError::InvalidTextWidth);
                }
            }
            RenderCmd::Path {
                points,
                closed,
                fill,
                ..
            } => {
                let minimum_points = if *closed || fill.is_some() { 3 } else { 2 };
                if points.len() < minimum_points || points.iter().any(|point| !finite(*point)) {
                    return Err(RenderListError::InvalidGeometry);
                }
            }
            RenderCmd::PushTransform { matrix, .. } => {
                if !matrix.matrix2.x_axis.is_finite()
                    || !matrix.matrix2.y_axis.is_finite()
                    || !finite(matrix.translation)
                {
                    return Err(RenderListError::InvalidTransform);
                }
            }
            RenderCmd::PushOpacity { .. }
            | RenderCmd::PopClip { .. }
            | RenderCmd::PopTransform { .. }
            | RenderCmd::PopOpacity { .. } => {}
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
            radii: Corners::uniform(0.0).unwrap(),
            fill: None,
            border: None,
            layer,
            z,
        }
    }

    fn push_opacity(layer: Layer, z: i16) -> RenderCmd {
        RenderCmd::PushOpacity {
            opacity: Opacity::try_from(0.5).unwrap(),
            layer,
            z,
        }
    }

    fn pop_opacity(layer: Layer, z: i16) -> RenderCmd {
        RenderCmd::PopOpacity { layer, z }
    }

    #[test]
    fn scope_groups_preserve_layer_order_without_constraining_children() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(rect(Layer::HUD, 0)).unwrap();
        builder.push(push_opacity(Layer::BOARD, 0)).unwrap();
        builder.push(rect(Layer::PIECES, 1)).unwrap();
        builder.push(rect(Layer::BOARD, 0)).unwrap();
        builder.push(pop_opacity(Layer::BOARD, 0)).unwrap();
        let list = builder.finish().unwrap();

        assert!(matches!(list.commands()[0], RenderCmd::PushOpacity { .. }));
        assert!(matches!(
            list.commands()[1],
            RenderCmd::Rect {
                layer: Layer::BOARD,
                ..
            }
        ));
        assert!(matches!(
            list.commands()[2],
            RenderCmd::Rect {
                layer: Layer::PIECES,
                ..
            }
        ));
        assert!(matches!(list.commands()[3], RenderCmd::PopOpacity { .. }));
        assert!(matches!(
            list.commands()[4],
            RenderCmd::Rect {
                layer: Layer::HUD,
                ..
            }
        ));
    }

    #[test]
    fn stable_equal_keys_keep_insertion_order() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(rect(Layer::BOARD, 0)).unwrap();
        builder
            .push(RenderCmd::Text {
                text: String::from("second"),
                at: Vec2::ZERO,
                style: TextStyleToken::Body,
                align: Align::Start,
                max_width: None,
                color: Color::rgb(0, 0, 0),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        let list = builder.finish().unwrap();
        assert!(matches!(list.commands()[0], RenderCmd::Rect { .. }));
        assert!(matches!(&list.commands()[1], RenderCmd::Text { text, .. } if text == "second"));
    }

    #[test]
    fn nested_scope_groups_sort_siblings_but_keep_each_scope_contiguous() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(push_opacity(Layer::OVERLAY, 0)).unwrap();
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                layer: Layer::PIECES,
                z: 0,
            })
            .unwrap();
        builder.push(rect(Layer::HUD, 0)).unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::PIECES,
                z: 0,
            })
            .unwrap();
        builder.push(rect(Layer::BOARD, 0)).unwrap();
        builder.push(pop_opacity(Layer::OVERLAY, 0)).unwrap();
        let list = builder.finish().unwrap();

        assert!(matches!(list.commands()[0], RenderCmd::PushOpacity { .. }));
        assert!(matches!(
            list.commands()[1],
            RenderCmd::Rect {
                layer: Layer::BOARD,
                ..
            }
        ));
        assert!(matches!(list.commands()[2], RenderCmd::PushClip { .. }));
        assert!(matches!(
            list.commands()[3],
            RenderCmd::Rect {
                layer: Layer::HUD,
                ..
            }
        ));
        assert!(matches!(list.commands()[4], RenderCmd::PopClip { .. }));
        assert!(matches!(list.commands()[5], RenderCmd::PopOpacity { .. }));
    }

    #[test]
    fn builder_rejects_unbalanced_or_mismatched_scopes() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        assert_eq!(
            builder.push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0
            }),
            Err(RenderListError::UnbalancedClip)
        );
        builder.push(push_opacity(Layer::BOARD, 0)).unwrap();
        assert_eq!(
            builder.push(RenderCmd::PopOpacity {
                layer: Layer::HUD,
                z: 0
            }),
            Err(RenderListError::ScopeLayerMismatch)
        );
        assert_eq!(builder.finish(), Err(RenderListError::UnbalancedOpacity));
    }

    #[test]
    fn nested_clips_transforms_and_opacity_are_balanced() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::IDENTITY,
                layer: Layer::PIECES,
                z: 0,
            })
            .unwrap();
        builder.push(push_opacity(Layer::OVERLAY, 0)).unwrap();
        builder.push(rect(Layer::HUD, 0)).unwrap();
        builder.push(pop_opacity(Layer::OVERLAY, 0)).unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::PIECES,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        assert_eq!(builder.finish().unwrap().commands().len(), 7);
    }

    #[test]
    fn proof_barriers_reject_invalid_numerical_values() {
        assert_eq!(
            Rect::new(Vec2::ZERO, Vec2::new(-1.0, 1.0)),
            Err(RenderListError::InvalidGeometry)
        );
        assert_eq!(
            Corners::uniform(f32::NAN),
            Err(RenderListError::InvalidGeometry)
        );
        assert_eq!(
            Border::new(-1.0, Color::rgb(0, 0, 0)),
            Err(RenderListError::InvalidGeometry)
        );
        assert_eq!(
            GradientStop::new(1.1, Color::rgb(0, 0, 0)),
            Err(RenderListError::InvalidGradient)
        );
        assert_eq!(Opacity::try_from(1.01), Err(OpacityError::OutOfRange));
        assert_eq!(Opacity::try_from(f32::NAN), Err(OpacityError::NonFinite));
        assert_eq!(
            Camera2D::new(Vec2::ZERO, 0.0),
            Err(RenderListError::InvalidCamera)
        );
    }

    #[test]
    fn gradient_requires_finite_endpoints_and_ordered_stops() {
        let black = Color::rgb(0, 0, 0);
        let one = GradientStop::new(1.0, black).unwrap();
        let zero = GradientStop::new(0.0, black).unwrap();
        assert_eq!(
            LinearGradient::new(Vec2::ZERO, Vec2::ONE, [one, zero]),
            Err(RenderListError::InvalidGradient)
        );
        assert_eq!(
            LinearGradient::new(Vec2::splat(f32::INFINITY), Vec2::ONE, [zero, one]),
            Err(RenderListError::InvalidGradient)
        );
    }

    #[test]
    fn path_requires_enough_finite_points_for_its_geometry() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        assert_eq!(
            builder.push(RenderCmd::Path {
                points: smallvec::smallvec![Vec2::ZERO],
                stroke: Border::new(1.0, Color::rgb(0, 0, 0)).unwrap(),
                closed: false,
                fill: None,
                layer: Layer::BOARD,
                z: 0,
            }),
            Err(RenderListError::InvalidGeometry)
        );
    }
}
