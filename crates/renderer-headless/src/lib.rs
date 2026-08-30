//! GPU-free implementation of the presentation renderer contract. (doc 04 §6.1)
//!
//! The recorder preserves the complete validated [`RenderList`] contract. The deliberately small
//! CPU rasterizer is a golden-image oracle for the documented subset: solid square rectangles,
//! borders, logical scissors, local transforms, camera, and inherited primitive opacity. It
//! rejects every other stable draw feature explicitly rather than silently omitting pixels.

#![forbid(unsafe_code)]

use glam::{Affine2, Vec2};
use tabula_design::{Color, Theme};
use tabula_presentation::{
    Camera2D, Corners, Dpi, FrameCtx, InputEvent, Paint, Rect, RenderCmd, RenderCmdKind,
    RenderError, RenderList, Renderer, TextMetrics, TextStyleToken, Viewport,
};

#[derive(Debug, Default)]
pub struct HeadlessRenderer {
    submitted: Vec<RenderList>,
    input: Vec<InputEvent>,
}

impl HeadlessRenderer {
    #[must_use]
    pub fn submitted(&self) -> &[RenderList] {
        &self.submitted
    }

    /// Test-only backend input queue; input normalization belongs to the interaction contract.
    pub fn queue_input(&mut self, event: InputEvent) {
        self.input.push(event);
    }

    /// Rasterizes the documented CPU subset into the device-pixel target derived from `frame`.
    ///
    /// The `RenderList` remains in logical units. Device dimensions are
    /// `ceil(frame.viewport * frame.dpi)`, and every unsupported stable draw feature returns an
    /// error before this method reports an image.
    #[allow(clippy::float_arithmetic)]
    pub fn rasterize(
        &self,
        list: &RenderList,
        frame: FrameCtx,
        background: Color,
    ) -> Result<RasterImage, RasterError> {
        let (width, height) = device_dimensions(&frame)?;
        let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(RasterError::ImageTooLarge)?;
        pixmap.fill(to_skia(background));

        let mut state = RasterState::default();
        let mut scopes = Vec::new();
        for command in list.commands() {
            match command {
                RenderCmd::PushClip { rect, .. } => {
                    scopes.push(ScopeFrame::new(ScopeKind::Clip, state));
                    state.clip = state.clip.intersect(*rect);
                }
                RenderCmd::PushTransform { matrix, .. } => {
                    scopes.push(ScopeFrame::new(ScopeKind::Transform, state));
                    state.transform *= *matrix;
                }
                RenderCmd::PushOpacity { opacity, .. } => {
                    scopes.push(ScopeFrame::new(ScopeKind::Opacity, state));
                    state.opacity *= opacity.get();
                }
                RenderCmd::PopClip { .. } => state = pop_scope(&mut scopes, ScopeKind::Clip),
                RenderCmd::PopTransform { .. } => {
                    state = pop_scope(&mut scopes, ScopeKind::Transform);
                }
                RenderCmd::PopOpacity { .. } => state = pop_scope(&mut scopes, ScopeKind::Opacity),
                RenderCmd::Rect {
                    rect,
                    radii,
                    fill,
                    border,
                    ..
                } => {
                    if has_rounded_corners(*radii) {
                        return Err(RasterError::UnsupportedCommand(RenderCmdKind::RoundedRect));
                    }
                    if matches!(fill, Some(Paint::LinearGradient(_))) {
                        return Err(RasterError::UnsupportedCommand(
                            RenderCmdKind::LinearGradient,
                        ));
                    }
                    let mask = state.clip.mask(width, height, frame.dpi());
                    let transform = device_transform(list.camera(), state.transform, frame.dpi());
                    if let Some(Paint::Solid(color)) = fill {
                        fill_rect(
                            &mut pixmap,
                            *rect,
                            apply_opacity(*color, state.opacity),
                            transform,
                            Some(&mask),
                        );
                    }
                    if let Some(border) = border {
                        stroke_rect(
                            &mut pixmap,
                            *rect,
                            border.width(),
                            apply_opacity(border.color(), state.opacity),
                            transform,
                            Some(&mask),
                        );
                    }
                }
                RenderCmd::Sprite { .. } => {
                    return Err(RasterError::UnsupportedCommand(RenderCmdKind::Sprite));
                }
                RenderCmd::Text { .. } => {
                    return Err(RasterError::UnsupportedCommand(RenderCmdKind::Text));
                }
                RenderCmd::Path { .. } => {
                    return Err(RasterError::UnsupportedCommand(RenderCmdKind::Path));
                }
            }
        }

        Ok(RasterImage {
            width,
            height,
            rgba: pixmap.data().to_vec(),
        })
    }
}

impl Renderer for HeadlessRenderer {
    fn begin_frame(&mut self, viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> FrameCtx {
        FrameCtx::new(viewport, dpi, now_ms, theme)
    }

    fn submit(&mut self, list: &RenderList) -> Result<(), RenderError> {
        self.submitted.push(list.clone());
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), RenderError> {
        Ok(())
    }

    #[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
    fn measure_text(
        &self,
        text: &str,
        _style: TextStyleToken,
        max_width: Option<f32>,
    ) -> TextMetrics {
        let natural = text.chars().count() as f32 * 8.0;
        let width = max_width.map_or(natural, |max| natural.min(max));
        TextMetrics {
            size: glam::vec2(width, 16.0),
            line_count: 1,
        }
    }

    fn drain_input(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.input)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterError {
    ImageTooLarge,
    UnsupportedCommand(RenderCmdKind),
}

impl RasterImage {
    #[must_use]
    pub fn checksum(&self) -> u64 {
        self.rgba.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let offset = usize::try_from(y.checked_mul(self.width)?.checked_add(x)?)
            .ok()?
            .checked_mul(4)?;
        let rgba = self.rgba.get(offset..offset.checked_add(4)?)?;
        Some([rgba[0], rgba[1], rgba[2], rgba[3]])
    }
}

#[derive(Clone, Copy)]
struct RasterState {
    clip: Clip,
    transform: Affine2,
    opacity: f32,
}

impl Default for RasterState {
    fn default() -> Self {
        Self {
            clip: Clip::Unbounded,
            transform: Affine2::IDENTITY,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Copy)]
struct ScopeFrame {
    kind: ScopeKind,
    previous: RasterState,
}

impl ScopeFrame {
    const fn new(kind: ScopeKind, previous: RasterState) -> Self {
        Self { kind, previous }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeKind {
    Clip,
    Transform,
    Opacity,
}

fn pop_scope(scopes: &mut Vec<ScopeFrame>, expected: ScopeKind) -> RasterState {
    let scope = scopes
        .pop()
        .expect("RenderList validation guarantees a matching scope pop");
    debug_assert_eq!(
        scope.kind, expected,
        "RenderList validation guarantees scope nesting"
    );
    scope.previous
}

#[derive(Clone, Copy)]
enum Clip {
    Unbounded,
    Rect(Rect),
    Empty,
}

impl Clip {
    fn intersect(self, rect: Rect) -> Self {
        match self {
            Self::Unbounded => Self::Rect(rect),
            Self::Rect(existing) => intersect(existing, rect).map_or(Self::Empty, Self::Rect),
            Self::Empty => Self::Empty,
        }
    }

    fn mask(self, width: u32, height: u32, dpi: Dpi) -> tiny_skia::Mask {
        let mut mask =
            tiny_skia::Mask::new(width, height).expect("derived raster dimensions are positive");
        match self {
            Self::Unbounded => mask.data_mut().fill(u8::MAX),
            Self::Rect(rect) => {
                let Some(rect) = to_skia_rect(rect) else {
                    return mask;
                };
                let path = tiny_skia::PathBuilder::from_rect(rect);
                mask.fill_path(
                    &path,
                    tiny_skia::FillRule::Winding,
                    false,
                    tiny_skia::Transform::from_scale(dpi.get(), dpi.get()),
                );
            }
            Self::Empty => {}
        }
        mask
    }
}

#[allow(clippy::float_arithmetic)]
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let min = a.origin().max(b.origin());
    let max = (a.origin() + a.size()).min(b.origin() + b.size());
    Rect::new(min, (max - min).max(Vec2::ZERO))
        .ok()
        .filter(|intersection| intersection.size().x > 0.0 && intersection.size().y > 0.0)
}

#[allow(clippy::float_arithmetic)]
fn device_dimensions(frame: &FrameCtx) -> Result<(u32, u32), RasterError> {
    let size = frame.viewport().size() * frame.dpi().get();
    let width = rounded_device_dimension(size.x)?;
    let height = rounded_device_dimension(size.y)?;
    Ok((width, height))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn rounded_device_dimension(value: f32) -> Result<u32, RasterError> {
    let value = value.ceil();
    if !value.is_finite() || value <= 0.0 || value > u32::MAX as f32 {
        return Err(RasterError::ImageTooLarge);
    }
    Ok(value as u32)
}

fn has_rounded_corners(corners: Corners) -> bool {
    corners.top_left() != 0.0
        || corners.top_right() != 0.0
        || corners.bottom_right() != 0.0
        || corners.bottom_left() != 0.0
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn apply_opacity(color: Color, opacity: f32) -> Color {
    Color::rgba(
        color.red(),
        color.green(),
        color.blue(),
        (f32::from(color.alpha()) * opacity).round() as u8,
    )
}

#[allow(clippy::float_arithmetic)]
fn device_transform(camera: Camera2D, local_transform: Affine2, dpi: Dpi) -> Affine2 {
    let camera_transform = Affine2::from_scale_angle_translation(
        Vec2::splat(camera.zoom()),
        0.0,
        -camera.origin() * camera.zoom(),
    );
    Affine2::from_scale(Vec2::splat(dpi.get())) * camera_transform * local_transform
}

fn to_skia(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.red(), color.green(), color.blue(), color.alpha())
}

fn to_skia_transform(transform: Affine2) -> tiny_skia::Transform {
    tiny_skia::Transform::from_row(
        transform.matrix2.x_axis.x,
        transform.matrix2.x_axis.y,
        transform.matrix2.y_axis.x,
        transform.matrix2.y_axis.y,
        transform.translation.x,
        transform.translation.y,
    )
}

fn to_skia_rect(rect: Rect) -> Option<tiny_skia::Rect> {
    tiny_skia::Rect::from_xywh(
        rect.origin().x,
        rect.origin().y,
        rect.size().x,
        rect.size().y,
    )
}

fn fill_rect(
    pixmap: &mut tiny_skia::Pixmap,
    rect: Rect,
    color: Color,
    transform: Affine2,
    mask: Option<&tiny_skia::Mask>,
) {
    let Some(rect) = to_skia_rect(rect) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(to_skia(color));
    pixmap.fill_rect(rect, &paint, to_skia_transform(transform), mask);
}

#[allow(clippy::float_arithmetic)]
fn stroke_rect(
    pixmap: &mut tiny_skia::Pixmap,
    rect: Rect,
    width: f32,
    color: Color,
    transform: Affine2,
    mask: Option<&tiny_skia::Mask>,
) {
    let horizontal = width.min(rect.size().y / 2.0);
    let vertical = width.min(rect.size().x / 2.0);
    for (origin, size) in [
        (rect.origin(), glam::vec2(rect.size().x, horizontal)),
        (
            glam::vec2(
                rect.origin().x,
                rect.origin().y + rect.size().y - horizontal,
            ),
            glam::vec2(rect.size().x, horizontal),
        ),
        (rect.origin(), glam::vec2(vertical, rect.size().y)),
        (
            glam::vec2(rect.origin().x + rect.size().x - vertical, rect.origin().y),
            glam::vec2(vertical, rect.size().y),
        ),
    ] {
        fill_rect(
            pixmap,
            Rect::new(origin, size).expect("derived border is valid"),
            color,
            transform,
            mask,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_design::ThemeKind;
    use tabula_presentation::{
        Align, Border, GradientStop, LinearGradient, Opacity, RenderListBuilder,
    };

    fn frame(width: f32, height: f32) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::new(width, height)).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        )
    }

    fn square_rect(origin: Vec2, size: Vec2, color: Color) -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(origin, size).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(color)),
            border: None,
            layer: tabula_presentation::Layer::BOARD,
            z: 0,
        }
    }

    fn list(camera: Camera2D, commands: impl IntoIterator<Item = RenderCmd>) -> RenderList {
        let mut builder = RenderListBuilder::new(camera);
        for command in commands {
            builder.push(command).unwrap();
        }
        builder.finish().unwrap()
    }

    fn pixel(image: &RasterImage, x: u32, y: u32) -> [u8; 4] {
        image.pixel(x, y).unwrap()
    }

    #[test]
    fn recorder_consumes_the_backend_neutral_contract() {
        let list = list(
            Camera2D::default(),
            [square_rect(
                Vec2::ZERO,
                Vec2::splat(8.0),
                Color::rgb(255, 255, 255),
            )],
        );
        let mut renderer = HeadlessRenderer::default();
        renderer.submit(&list).unwrap();
        assert_eq!(renderer.submitted(), &[list]);
    }

    #[test]
    fn default_camera_transform_and_opacity_are_identity() {
        let baseline = list(
            Camera2D::default(),
            [square_rect(
                Vec2::ONE,
                Vec2::splat(2.0),
                Color::rgb(255, 0, 0),
            )],
        );
        let scoped = list(
            Camera2D::default(),
            [
                RenderCmd::PushTransform {
                    matrix: Affine2::IDENTITY,
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
                RenderCmd::PushOpacity {
                    opacity: Opacity::try_from(1.0).unwrap(),
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
                square_rect(Vec2::ONE, Vec2::splat(2.0), Color::rgb(255, 0, 0)),
                RenderCmd::PopOpacity {
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
                RenderCmd::PopTransform {
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
            ],
        );
        let renderer = HeadlessRenderer::default();
        assert_eq!(
            renderer.rasterize(&baseline, frame(4.0, 4.0), Color::rgb(0, 0, 0)),
            renderer.rasterize(&scoped, frame(4.0, 4.0), Color::rgb(0, 0, 0))
        );
    }

    #[test]
    fn dpi_scales_the_device_target_but_not_logical_command_coordinates() {
        let list = list(
            Camera2D::default(),
            [square_rect(Vec2::ZERO, Vec2::ONE, Color::rgb(255, 0, 0))],
        );
        let frame = FrameCtx::new(
            Viewport::new(Vec2::splat(2.0)).unwrap(),
            Dpi::new(2.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        );
        let image = HeadlessRenderer::default()
            .rasterize(&list, frame, Color::rgb(0, 0, 0))
            .unwrap();
        assert_eq!((image.width, image.height), (4, 4));
        assert!(pixel(&image, 1, 1)[0] > 0);
        assert_eq!(pixel(&image, 2, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn camera_origin_zoom_and_local_transform_change_raster_positions() {
        let renderer = HeadlessRenderer::default();
        let origin = list(
            Camera2D::new(Vec2::new(1.0, 0.0), 1.0).unwrap(),
            [square_rect(Vec2::ONE, Vec2::ONE, Color::rgb(255, 0, 0))],
        );
        assert!(
            pixel(
                &renderer
                    .rasterize(&origin, frame(4.0, 4.0), Color::rgb(0, 0, 0))
                    .unwrap(),
                0,
                1
            )[0] > 0
        );

        let zoom = list(
            Camera2D::new(Vec2::ZERO, 2.0).unwrap(),
            [square_rect(Vec2::ONE, Vec2::ONE, Color::rgb(255, 0, 0))],
        );
        assert!(
            pixel(
                &renderer
                    .rasterize(&zoom, frame(4.0, 4.0), Color::rgb(0, 0, 0))
                    .unwrap(),
                2,
                2
            )[0] > 0
        );

        let composed = list(
            Camera2D::new(Vec2::new(1.0, 0.0), 2.0).unwrap(),
            [
                RenderCmd::PushTransform {
                    matrix: Affine2::from_translation(Vec2::new(2.0, 0.0)),
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
                square_rect(Vec2::ZERO, Vec2::ONE, Color::rgb(255, 0, 0)),
                RenderCmd::PopTransform {
                    layer: tabula_presentation::Layer::BOARD,
                    z: 0,
                },
            ],
        );
        assert!(
            pixel(
                &renderer
                    .rasterize(&composed, frame(4.0, 4.0), Color::rgb(0, 0, 0))
                    .unwrap(),
                2,
                0
            )[0] > 0
        );
    }

    #[test]
    fn clip_and_transform_order_use_the_same_logical_viewport_scissor() {
        let clip = RenderCmd::PushClip {
            rect: Rect::new(Vec2::new(1.0, 0.0), Vec2::new(2.0, 4.0)).unwrap(),
            layer: tabula_presentation::Layer::BOARD,
            z: 0,
        };
        let transform = RenderCmd::PushTransform {
            matrix: Affine2::from_translation(Vec2::new(1.0, 0.0)),
            layer: tabula_presentation::Layer::BOARD,
            z: 0,
        };
        let draw = square_rect(Vec2::ZERO, Vec2::new(2.0, 4.0), Color::rgb(255, 0, 0));
        let close_clip = RenderCmd::PopClip {
            layer: tabula_presentation::Layer::BOARD,
            z: 0,
        };
        let close_transform = RenderCmd::PopTransform {
            layer: tabula_presentation::Layer::BOARD,
            z: 0,
        };
        let clip_then_transform = list(
            Camera2D::default(),
            [
                clip.clone(),
                transform.clone(),
                draw.clone(),
                close_transform.clone(),
                close_clip.clone(),
            ],
        );
        let transform_then_clip = list(
            Camera2D::default(),
            [transform, clip, draw, close_clip, close_transform],
        );
        let renderer = HeadlessRenderer::default();
        let first = renderer
            .rasterize(&clip_then_transform, frame(4.0, 4.0), Color::rgb(0, 0, 0))
            .unwrap();
        let second = renderer
            .rasterize(&transform_then_clip, frame(4.0, 4.0), Color::rgb(0, 0, 0))
            .unwrap();
        assert_eq!(first, second);
        assert!(pixel(&first, 1, 1)[0] > 0);
        assert_eq!(pixel(&first, 0, 1), [0, 0, 0, 255]);
        assert_eq!(pixel(&first, 3, 1), [0, 0, 0, 255]);
    }

    #[test]
    fn nested_clips_and_transforms_keep_a_logical_scissor() {
        let commands = [
            RenderCmd::PushTransform {
                matrix: Affine2::from_translation(Vec2::new(1.0, 0.0)),
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PushClip {
                rect: Rect::new(Vec2::new(1.0, 0.0), Vec2::new(3.0, 4.0)).unwrap(),
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PushTransform {
                matrix: Affine2::from_translation(Vec2::new(1.0, 0.0)),
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PushClip {
                rect: Rect::new(Vec2::new(2.0, 0.0), Vec2::new(1.0, 4.0)).unwrap(),
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            square_rect(Vec2::ZERO, Vec2::new(2.0, 4.0), Color::rgb(255, 0, 0)),
            RenderCmd::PopClip {
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PopTransform {
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PopClip {
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            RenderCmd::PopTransform {
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
        ];
        let image = HeadlessRenderer::default()
            .rasterize(
                &list(Camera2D::default(), commands),
                frame(4.0, 4.0),
                Color::rgb(0, 0, 0),
            )
            .unwrap();
        assert!(pixel(&image, 2, 1)[0] > 0);
        assert_eq!(pixel(&image, 1, 1), [0, 0, 0, 255]);
        assert_eq!(pixel(&image, 3, 1), [0, 0, 0, 255]);
    }

    #[test]
    fn inherited_opacity_is_per_primitive_not_true_group_compositing() {
        let commands = [
            RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
            square_rect(Vec2::ZERO, Vec2::new(3.0, 2.0), Color::rgb(255, 0, 0)),
            square_rect(
                Vec2::new(1.0, 0.0),
                Vec2::new(3.0, 2.0),
                Color::rgb(255, 0, 0),
            ),
            RenderCmd::PopOpacity {
                layer: tabula_presentation::Layer::BOARD,
                z: 0,
            },
        ];
        let image = HeadlessRenderer::default()
            .rasterize(
                &list(Camera2D::default(), commands),
                frame(4.0, 2.0),
                Color::rgb(0, 0, 0),
            )
            .unwrap();
        assert!(pixel(&image, 1, 0)[0] > pixel(&image, 0, 0)[0]);
    }

    #[test]
    fn unsupported_stable_draw_features_fail_loudly() {
        let black = Color::rgb(0, 0, 0);
        let renderer = HeadlessRenderer::default();
        let cases = [
            (
                list(
                    Camera2D::default(),
                    [RenderCmd::Sprite {
                        asset: String::from("piece"),
                        rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                        src: None,
                        tint: black,
                        rotation: 0.0,
                        pivot: Vec2::ZERO,
                        layer: tabula_presentation::Layer::BOARD,
                        z: 0,
                    }],
                ),
                RenderCmdKind::Sprite,
            ),
            (
                list(
                    Camera2D::default(),
                    [RenderCmd::Text {
                        text: String::from("text"),
                        at: Vec2::ZERO,
                        style: TextStyleToken::BodyMd,
                        align: Align::Start,
                        max_width: None,
                        color: black,
                        layer: tabula_presentation::Layer::BOARD,
                        z: 0,
                    }],
                ),
                RenderCmdKind::Text,
            ),
            (
                list(
                    Camera2D::default(),
                    [RenderCmd::Path {
                        points: [Vec2::ZERO, Vec2::ONE].into_iter().collect(),
                        stroke: Border::new(1.0, black).unwrap(),
                        closed: false,
                        fill: None,
                        layer: tabula_presentation::Layer::BOARD,
                        z: 0,
                    }],
                ),
                RenderCmdKind::Path,
            ),
            (
                list(
                    Camera2D::default(),
                    [RenderCmd::Rect {
                        rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                        radii: Corners::uniform(1.0).unwrap(),
                        fill: Some(Paint::Solid(black)),
                        border: None,
                        layer: tabula_presentation::Layer::BOARD,
                        z: 0,
                    }],
                ),
                RenderCmdKind::RoundedRect,
            ),
            (
                list(
                    Camera2D::default(),
                    [RenderCmd::Rect {
                        rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                        radii: Corners::uniform(0.0).unwrap(),
                        fill: Some(Paint::LinearGradient(
                            LinearGradient::new(
                                Vec2::ZERO,
                                Vec2::ONE,
                                [
                                    GradientStop::new(0.0, black).unwrap(),
                                    GradientStop::new(1.0, black).unwrap(),
                                ],
                            )
                            .unwrap(),
                        )),
                        border: None,
                        layer: tabula_presentation::Layer::BOARD,
                        z: 0,
                    }],
                ),
                RenderCmdKind::LinearGradient,
            ),
        ];
        for (list, kind) in cases {
            assert_eq!(
                renderer.rasterize(&list, frame(4.0, 4.0), black),
                Err(RasterError::UnsupportedCommand(kind))
            );
        }
    }
}
