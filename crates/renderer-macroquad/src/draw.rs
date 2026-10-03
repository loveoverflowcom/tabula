#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]

use glam::{Affine2, Vec2};
use macroquad::{models::Vertex, prelude as mq};
use tabula_design::Color;
use tabula_presentation::{
    Border, Corners, LinearGradient, Paint, Rect, RenderCmd, RenderCmdKind, RenderError,
};

use crate::assets::ResolvedSprite;
use crate::state::{logical_transform, Clip, DrawState};
use crate::text;

pub(crate) fn execute(
    command: &RenderCmd,
    state: DrawState,
    camera: tabula_presentation::Camera2D,
    frame: &tabula_presentation::FrameCtx,
    sprite: Option<&ResolvedSprite<mq::Texture2D>>,
) -> Result<(), RenderError> {
    configure_clip_viewport(state.clip, frame)?;
    let transform = logical_transform(camera, state.transform);
    match command {
        RenderCmd::Rect {
            rect,
            radii,
            fill,
            border,
            ..
        } => {
            if let Some(paint) = fill {
                draw_rect(*rect, *radii, paint, state.opacity.get(), transform)?;
            }
            if let Some(border) = border {
                let mut outline = rounded_outline(*rect, *radii);
                outline.push(outline[0]);
                stroke_outline(&outline, *border, state.opacity.get(), transform);
            }
        }
        RenderCmd::Text {
            text: value,
            at,
            style,
            align,
            max_width,
            color,
            ..
        } => text::draw(
            value,
            *at,
            *style,
            *align,
            *max_width,
            *color,
            state.opacity.get(),
            transform,
            &frame.theme(),
            frame.dpi().get(),
        )?,
        RenderCmd::Path {
            points,
            stroke,
            closed,
            fill,
            ..
        } => draw_path(
            points,
            *stroke,
            *closed,
            fill.as_ref(),
            state.opacity.get(),
            transform,
        )?,
        RenderCmd::Sprite {
            rect,
            tint,
            rotation,
            pivot,
            ..
        } => {
            let sprite = sprite.ok_or_else(|| {
                RenderError::Execution(String::from(
                    "renderer-macroquad sprite was not prepared before execution",
                ))
            })?;
            draw_sprite(
                sprite,
                *rect,
                *tint,
                *rotation,
                *pivot,
                state.opacity.get(),
                transform,
            )?;
        }
        RenderCmd::PushClip { .. }
        | RenderCmd::PopClip { .. }
        | RenderCmd::PushTransform { .. }
        | RenderCmd::PopTransform { .. }
        | RenderCmd::PushOpacity { .. }
        | RenderCmd::PopOpacity { .. } => {
            return Err(RenderError::Execution(String::from(
                "renderer-macroquad received a scope command as a primitive",
            )));
        }
    }
    Ok(())
}

/// Checks one primitive and its effective state without calling Macroquad.
pub(crate) fn validate(
    command: &RenderCmd,
    state: DrawState,
    camera: tabula_presentation::Camera2D,
    frame: &tabula_presentation::FrameCtx,
) -> Result<(), RenderError> {
    validate_clip(state.clip, frame)?;
    let transform = logical_transform(camera, state.transform);
    if !transform.matrix2.x_axis.is_finite()
        || !transform.matrix2.y_axis.is_finite()
        || !transform.translation.is_finite()
    {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad effective transform is not finite",
        )));
    }

    match command {
        RenderCmd::Sprite {
            rect,
            rotation,
            pivot,
            ..
        } => sprite_quad(*rect, *rotation, *pivot, transform).map(|_| ()),
        RenderCmd::Text { .. } if !text::supports_transform(transform) => {
            Err(RenderError::Unsupported(RenderCmdKind::Text))
        }
        RenderCmd::Rect { rect, radii, .. } => {
            validate_transformed_points(&rounded_outline(*rect, *radii), transform)
        }
        RenderCmd::Path {
            points,
            fill: Some(_),
            ..
        } if !is_convex(points) => Err(RenderError::Unsupported(RenderCmdKind::Path)),
        RenderCmd::Path { points, fill, .. } => {
            if fill.is_some() && points.len() > usize::from(u16::MAX) {
                return Err(RenderError::Execution(String::from(
                    "renderer-macroquad polygon exceeds mesh index capacity",
                )));
            }
            validate_transformed_points(points, transform)
        }
        RenderCmd::Text {
            text: value,
            style,
            max_width,
            ..
        } => text::validate(value, *style, *max_width, frame),
        RenderCmd::PushClip { .. } | RenderCmd::PopClip { .. } => Ok(()),
        RenderCmd::PushTransform { .. }
        | RenderCmd::PopTransform { .. }
        | RenderCmd::PushOpacity { .. }
        | RenderCmd::PopOpacity { .. } => Err(RenderError::Execution(String::from(
            "renderer-macroquad received a scope command as a primitive",
        ))),
    }
}

fn validate_transformed_points(points: &[Vec2], transform: Affine2) -> Result<(), RenderError> {
    if points
        .iter()
        .any(|point| !transform.transform_point2(*point).is_finite())
    {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad transformed geometry is not finite",
        )));
    }
    Ok(())
}

fn configure_clip_viewport(
    clip: Clip,
    frame: &tabula_presentation::FrameCtx,
) -> Result<(), RenderError> {
    validate_clip(clip, frame)?;
    let Clip::Rect(rect) = clip else {
        if matches!(clip, Clip::Unbounded) {
            mq::set_default_camera();
        } else {
            let mut empty = mq::Camera2D::from_display_rect(mq::Rect::new(0.0, 0.0, 1.0, 1.0));
            empty.viewport = Some((0, 0, 0, 0));
            mq::set_camera(&empty);
        }
        return Ok(());
    };
    let viewport = clip_device_viewport(rect, frame)?;
    let mut camera = mq::Camera2D::from_display_rect(mq::Rect::new(
        rect.origin().x,
        rect.origin().y,
        rect.size().x,
        rect.size().y,
    ));
    camera.viewport = Some(viewport);
    mq::set_camera(&camera);
    Ok(())
}

fn validate_clip(clip: Clip, frame: &tabula_presentation::FrameCtx) -> Result<(), RenderError> {
    let Clip::Rect(rect) = clip else {
        return Ok(());
    };
    clip_device_viewport(rect, frame).map(|_| ())
}

/// The contract's logical scissor has a top-left origin, while the GPU viewport has a bottom-left
/// origin. Keep its logical camera rect intact, and invert only the physical viewport's Y origin.
/// Both preflight and execution use this exact conversion, including off-screen rectangles.
fn clip_device_viewport(
    rect: Rect,
    frame: &tabula_presentation::FrameCtx,
) -> Result<(i32, i32, i32, i32), RenderError> {
    let dpi = frame.dpi().get();
    let bottom_origin = frame.viewport().size().y - (rect.origin().y + rect.size().y);
    Ok((
        device_coordinate(rect.origin().x * dpi)?,
        device_coordinate(bottom_origin * dpi)?,
        device_coordinate(rect.size().x * dpi)?,
        device_coordinate(rect.size().y * dpi)?,
    ))
}

#[allow(clippy::cast_possible_truncation, clippy::float_arithmetic)]
fn device_coordinate(value: f32) -> Result<i32, RenderError> {
    let rounded = value.round();
    if !rounded.is_finite()
        || f64::from(rounded) < f64::from(i32::MIN)
        || f64::from(rounded) > f64::from(i32::MAX)
    {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad scissor exceeds supported device coordinates",
        )));
    }
    Ok(rounded as i32)
}

/// Builds the textured destination quad in logical coordinates. Rotation is around an absolute
/// local pivot, before inherited affine scopes and then camera mapping. (doc 04 §5.1.1)
fn sprite_quad(
    rect: Rect,
    rotation: f32,
    pivot: Vec2,
    transform: Affine2,
) -> Result<[Vec2; 4], RenderError> {
    let rotation = Affine2::from_angle(rotation);
    let points = [
        rect.origin(),
        rect.origin() + Vec2::new(rect.size().x, 0.0),
        rect.origin() + rect.size(),
        rect.origin() + Vec2::new(0.0, rect.size().y),
    ]
    .map(|point| transform.transform_point2(rotation.transform_vector2(point - pivot) + pivot));
    if points.iter().any(|point| !point.is_finite()) {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad transformed sprite geometry is not finite",
        )));
    }
    Ok(points)
}

/// Normalized atlas coordinates preserve the resource's source-pixel region, independent of
/// destination size, camera, density, or rotation.
fn sprite_uvs(
    source: tabula_assets::AssetPixelRegion,
    width: u16,
    height: u16,
) -> Result<[Vec2; 4], RenderError> {
    if width == 0
        || height == 0
        || source.x() + source.width() > u32::from(width)
        || source.y() + source.height() > u32::from(height)
    {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad sprite source region exceeds its ready texture",
        )));
    }
    let start = Vec2::new(
        source.x() as f32 / f32::from(width),
        source.y() as f32 / f32::from(height),
    );
    let end = Vec2::new(
        (source.x() + source.width()) as f32 / f32::from(width),
        (source.y() + source.height()) as f32 / f32::from(height),
    );
    Ok([
        start,
        Vec2::new(end.x, start.y),
        end,
        Vec2::new(start.x, end.y),
    ])
}

pub(crate) fn validate_sprite_source(
    source: tabula_assets::AssetPixelRegion,
    width: u16,
    height: u16,
) -> Result<(), RenderError> {
    sprite_uvs(source, width, height).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn draw_sprite(
    sprite: &ResolvedSprite<mq::Texture2D>,
    rect: Rect,
    tint: Color,
    rotation: f32,
    pivot: Vec2,
    opacity: f32,
    transform: Affine2,
) -> Result<(), RenderError> {
    let points = sprite_quad(rect, rotation, pivot, transform)?;
    let uvs = sprite_uvs(sprite.source(), sprite.width(), sprite.height())?;
    let vertices = sprite_vertices(points, uvs, tint, opacity);
    mq::draw_mesh(&mq::Mesh {
        vertices: vertices.to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(sprite.texture().clone()),
    });
    Ok(())
}

fn sprite_vertices(points: [Vec2; 4], uvs: [Vec2; 4], tint: Color, opacity: f32) -> [Vertex; 4] {
    let tint = apply_opacity(tint, opacity);
    core::array::from_fn(|index| {
        let mut vertex = vertex(points[index], tint);
        vertex.uv = mq::Vec2::new(uvs[index].x, uvs[index].y);
        vertex
    })
}

fn draw_rect(
    rect: Rect,
    corners: Corners,
    paint: &Paint,
    opacity: f32,
    transform: Affine2,
) -> Result<(), RenderError> {
    let points = rounded_outline(rect, corners);
    let colors = match paint {
        Paint::Solid(color) => vec![apply_opacity(*color, opacity); points.len()],
        Paint::LinearGradient(gradient) => points
            .iter()
            .map(|point| gradient_color(gradient, *point, opacity))
            .collect(),
    };
    fill_convex(&points, &colors, transform)
}

fn draw_path(
    points: &[Vec2],
    stroke: Border,
    closed: bool,
    fill: Option<&Paint>,
    opacity: f32,
    transform: Affine2,
) -> Result<(), RenderError> {
    if let Some(paint) = fill {
        if !is_convex(points) {
            return Err(RenderError::Unsupported(RenderCmdKind::Path));
        }
        let colors = match paint {
            Paint::Solid(color) => vec![apply_opacity(*color, opacity); points.len()],
            Paint::LinearGradient(gradient) => points
                .iter()
                .map(|point| gradient_color(gradient, *point, opacity))
                .collect(),
        };
        fill_convex(points, &colors, transform)?;
    }

    let mut outline = points.to_vec();
    if closed {
        outline.push(points[0]);
    }
    stroke_outline(&outline, stroke, opacity, transform);
    Ok(())
}

fn fill_convex(points: &[Vec2], colors: &[Color], transform: Affine2) -> Result<(), RenderError> {
    if points.len() != colors.len() || points.len() < 3 {
        return Err(RenderError::Execution(String::from(
            "renderer-macroquad received invalid convex fill geometry",
        )));
    }
    let mut vertices = Vec::with_capacity(points.len());
    for (point, color) in points.iter().zip(colors) {
        vertices.push(vertex(transform.transform_point2(*point), *color));
    }
    let mut indices = Vec::with_capacity((points.len() - 2) * 3);
    for index in 1..(points.len() - 1) {
        let first = 0_u16;
        let second = u16::try_from(index).map_err(|_| {
            RenderError::Execution(String::from(
                "renderer-macroquad polygon exceeds mesh index capacity",
            ))
        })?;
        let third = u16::try_from(index + 1).map_err(|_| {
            RenderError::Execution(String::from(
                "renderer-macroquad polygon exceeds mesh index capacity",
            ))
        })?;
        indices.extend([first, second, third]);
    }
    mq::draw_mesh(&mq::Mesh {
        vertices,
        indices,
        texture: None,
    });
    Ok(())
}

fn stroke_outline(points: &[Vec2], border: Border, opacity: f32, transform: Affine2) {
    for pair in points.windows(2) {
        let Some(quad) = stroke_quad(pair[0], pair[1], border.width()) else {
            continue;
        };
        let color = apply_opacity(border.color(), opacity);
        let vertices = quad.map(|point| vertex(transform.transform_point2(point), color));
        mq::draw_mesh(&mq::Mesh {
            vertices: vertices.to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: None,
        });
    }
}

fn stroke_quad(start: Vec2, end: Vec2, width: f32) -> Option<[Vec2; 4]> {
    let delta = end - start;
    let length = delta.length();
    if length == 0.0 {
        return None;
    }
    let normal = Vec2::new(-delta.y, delta.x) * (width / (2.0 * length));
    Some([start + normal, start - normal, end - normal, end + normal])
}

fn vertex(position: Vec2, color: Color) -> Vertex {
    Vertex {
        position: mq::Vec3::new(position.x, position.y, 0.0),
        uv: mq::Vec2::ZERO,
        color: [color.red(), color.green(), color.blue(), color.alpha()],
        normal: mq::Vec4::new(0.0, 0.0, 1.0, 0.0),
    }
}

#[allow(clippy::float_arithmetic)]
fn rounded_outline(rect: Rect, corners: Corners) -> Vec<Vec2> {
    let maximum = rect.size() / 2.0;
    let radii = [
        corners.top_left().min(maximum.x).min(maximum.y),
        corners.top_right().min(maximum.x).min(maximum.y),
        corners.bottom_right().min(maximum.x).min(maximum.y),
        corners.bottom_left().min(maximum.x).min(maximum.y),
    ];
    if radii.iter().all(|radius| *radius == 0.0) {
        return vec![
            rect.origin(),
            rect.origin() + Vec2::new(rect.size().x, 0.0),
            rect.origin() + rect.size(),
            rect.origin() + Vec2::new(0.0, rect.size().y),
        ];
    }

    let centres = [
        rect.origin() + Vec2::new(radii[0], radii[0]),
        rect.origin() + Vec2::new(rect.size().x - radii[1], radii[1]),
        rect.origin() + Vec2::new(rect.size().x - radii[2], rect.size().y - radii[2]),
        rect.origin() + Vec2::new(radii[3], rect.size().y - radii[3]),
    ];
    let starts = [
        core::f32::consts::PI,
        core::f32::consts::FRAC_PI_2 * 3.0,
        0.0,
        core::f32::consts::FRAC_PI_2,
    ];
    let mut points = Vec::with_capacity(24);
    for index in 0..4 {
        let segments = if radii[index] == 0.0 { 1 } else { 6 };
        for segment in 0..=segments {
            let angle =
                starts[index] + core::f32::consts::FRAC_PI_2 * segment as f32 / segments as f32;
            points.push(centres[index] + Vec2::new(angle.cos(), angle.sin()) * radii[index]);
        }
    }
    points
}

fn is_convex(points: &[Vec2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut sign = None;
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        let third = points[(index + 2) % points.len()];
        let cross = (second - first).perp_dot(third - second);
        if cross == 0.0 {
            continue;
        }
        let current = cross.is_sign_positive();
        if sign.is_some_and(|previous| previous != current) {
            return false;
        }
        sign = Some(current);
    }
    sign.is_some()
}

#[allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn gradient_color(gradient: &LinearGradient, point: Vec2, opacity: f32) -> Color {
    let direction = gradient.to() - gradient.from();
    let denominator = direction.length_squared();
    let offset = if denominator == 0.0 {
        0.0
    } else {
        ((point - gradient.from()).dot(direction) / denominator).clamp(0.0, 1.0)
    };
    let stops = gradient.stops();
    let pair = stops
        .windows(2)
        .find(|pair| offset <= pair[1].offset())
        .unwrap_or_else(|| &stops[stops.len() - 2..]);
    let span = pair[1].offset() - pair[0].offset();
    let ratio = if span == 0.0 {
        1.0
    } else {
        (offset - pair[0].offset()) / span
    };
    let blend = |start: u8, end: u8| f32::from(start) + (f32::from(end) - f32::from(start)) * ratio;
    apply_opacity(
        Color::rgba(
            blend(pair[0].color().red(), pair[1].color().red()).round() as u8,
            blend(pair[0].color().green(), pair[1].color().green()).round() as u8,
            blend(pair[0].color().blue(), pair[1].color().blue()).round() as u8,
            blend(pair[0].color().alpha(), pair[1].color().alpha()).round() as u8,
        ),
        opacity,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_frame(size: Vec2, dpi: f32) -> tabula_presentation::FrameCtx {
        tabula_presentation::FrameCtx::new(
            tabula_presentation::Viewport::new(size).unwrap(),
            tabula_presentation::Dpi::new(dpi).unwrap(),
            0,
            tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    #[test]
    fn logical_top_origin_clip_maps_to_gpu_bottom_origin_at_each_dpi() {
        // Browser regression fixture: top155..305 must use GPU415..565 in a720 logical frame.
        let rect = Rect::new(Vec2::new(40.0, 155.0), Vec2::new(175.0, 150.0)).unwrap();
        for (dpi, expected) in [(1.0, (40, 415, 175, 150)), (2.0, (80, 830, 350, 300))] {
            let frame = clip_frame(Vec2::new(960.0, 720.0), dpi);
            assert_eq!(clip_device_viewport(rect, &frame), Ok(expected));
            assert_eq!(validate_clip(Clip::Rect(rect), &frame), Ok(()));
        }
        let frame = clip_frame(Vec2::new(80.0, 100.0), 1.0);
        let full = Rect::new(Vec2::ZERO, frame.viewport().size()).unwrap();
        assert_eq!(clip_device_viewport(full, &frame), Ok((0, 0, 80, 100)));
    }

    #[test]
    fn offscreen_clips_keep_their_geometry_and_convert_negative_gpu_origins() {
        for (origin, expected) in [
            (Vec2::new(-15.0, -20.0), (-15, 80, 30, 40)),
            (Vec2::new(75.0, 90.0), (75, -30, 30, 40)),
            (Vec2::new(-15.0, 110.0), (-15, -50, 30, 40)),
        ] {
            let rect = Rect::new(origin, Vec2::new(30.0, 40.0)).unwrap();
            for (dpi, expected) in [
                (1.0, expected),
                (
                    2.0,
                    (
                        expected.0 * 2,
                        expected.1 * 2,
                        expected.2 * 2,
                        expected.3 * 2,
                    ),
                ),
            ] {
                let frame = clip_frame(Vec2::new(80.0, 100.0), dpi);
                assert_eq!(clip_device_viewport(rect, &frame), Ok(expected));
                assert_eq!(validate_clip(Clip::Rect(rect), &frame), Ok(()));
            }
        }
    }

    #[test]
    fn clip_preflight_rejects_the_same_unsupported_device_bounds_as_execution_conversion() {
        let rect = Rect::new(Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0)).unwrap();
        for frame in [
            clip_frame(Vec2::new(80.0, f32::MAX), 1.0),
            clip_frame(Vec2::new(80.0, 100.0), f32::MAX),
        ] {
            let error = clip_device_viewport(rect, &frame).unwrap_err();
            assert_eq!(validate_clip(Clip::Rect(rect), &frame), Err(error));
        }
        let maximum_rounded_up = i32::MAX as f32;
        let maximum_supported = f32::from_bits(maximum_rounded_up.to_bits() - 1);
        assert_eq!(device_coordinate(maximum_supported), Ok(2_147_483_520));
        assert!(
            device_coordinate(maximum_rounded_up).is_err(),
            "f32 rounds i32::MAX upward and cannot be accepted as an i32 coordinate"
        );
        assert_eq!(device_coordinate(i32::MIN as f32), Ok(i32::MIN));
        assert!(device_coordinate(f32::from_bits((i32::MIN as f32).to_bits() + 1)).is_err());
    }

    fn assert_points_close(actual: [Vec2; 4], expected: [Vec2; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs().max_element() < 0.000_01,
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn sprite_rotates_about_its_local_pivot_before_affine_scope_and_camera() {
        let rect = Rect::new(Vec2::new(10.0, 20.0), Vec2::new(4.0, 2.0)).unwrap();
        let local = Affine2::from_cols(
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 3.0),
            Vec2::new(7.0, 11.0),
        );
        let camera = tabula_presentation::Camera2D::new(Vec2::new(3.0, 5.0), 2.0).unwrap();
        // Quarter-turn about rect's top-left maps its corners to (10,20), (10,24),
        // (8,24), (8,20). The independent integer affine/camera oracle maps those below.
        let actual = sprite_quad(
            rect,
            core::f32::consts::FRAC_PI_2,
            rect.origin(),
            logical_transform(camera, local),
        )
        .unwrap();
        assert_points_close(
            actual,
            [
                Vec2::new(88.0, 152.0),
                Vec2::new(96.0, 176.0),
                Vec2::new(88.0, 172.0),
                Vec2::new(80.0, 148.0),
            ],
        );
    }

    #[test]
    fn sprite_affine_geometry_supports_reflection_shear_and_singular_scopes() {
        let rect = Rect::new(Vec2::ZERO, Vec2::new(4.0, 2.0)).unwrap();
        for (transform, expected) in [
            (
                Affine2::from_cols(
                    Vec2::new(-2.0, 0.0),
                    Vec2::new(1.0, 3.0),
                    Vec2::new(5.0, 7.0),
                ),
                [
                    Vec2::new(5.0, 7.0),
                    Vec2::new(-3.0, 7.0),
                    Vec2::new(-1.0, 13.0),
                    Vec2::new(7.0, 13.0),
                ],
            ),
            (
                Affine2::from_scale(Vec2::new(0.0, 2.0)),
                [
                    Vec2::ZERO,
                    Vec2::ZERO,
                    Vec2::new(0.0, 4.0),
                    Vec2::new(0.0, 4.0),
                ],
            ),
        ] {
            assert_points_close(
                sprite_quad(rect, 0.0, Vec2::ZERO, transform).unwrap(),
                expected,
            );
        }
    }

    #[test]
    fn sprite_rejects_transformed_overflow_even_with_finite_local_geometry() {
        let rect = Rect::new(Vec2::splat(f32::MAX / 4.0), Vec2::ONE).unwrap();
        assert_eq!(
            sprite_quad(rect, 0.0, Vec2::ZERO, Affine2::from_scale(Vec2::splat(8.0))),
            Err(RenderError::Execution(String::from(
                "renderer-macroquad transformed sprite geometry is not finite"
            )))
        );
        let rect = Rect::new(Vec2::splat(f32::MAX / 4.0), Vec2::ONE).unwrap();
        assert!(sprite_quad(
            rect,
            core::f32::consts::PI,
            Vec2::splat(-f32::MAX),
            Affine2::IDENTITY
        )
        .is_err());
    }

    #[test]
    fn sprite_atlas_uvs_keep_exact_pixel_region_and_reject_out_of_bounds() {
        let region = tabula_assets::AssetPixelRegion::new(16, 8, 32, 16).unwrap();
        assert_points_close(
            sprite_uvs(region, 64, 32).unwrap(),
            [
                Vec2::new(0.25, 0.25),
                Vec2::new(0.75, 0.25),
                Vec2::new(0.75, 0.75),
                Vec2::new(0.25, 0.75),
            ],
        );
        let whole = tabula_assets::AssetPixelRegion::new(0, 0, 64, 32).unwrap();
        assert_points_close(
            sprite_uvs(whole, 64, 32).unwrap(),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        );
        for (width, height) in [(47, 32), (64, 23), (0, 32), (64, 0)] {
            assert!(sprite_uvs(region, width, height).is_err());
        }
    }

    #[test]
    fn sprite_vertices_preserve_uv_order_and_multiply_tint_alpha() {
        let points = [
            Vec2::new(1.0, 2.0),
            Vec2::new(3.0, 2.0),
            Vec2::new(3.0, 4.0),
            Vec2::new(1.0, 4.0),
        ];
        let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
        let color = tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light)
            .color
            .primary;
        let tint = Color::rgba(color.red(), color.green(), color.blue(), 128);
        let vertices = sprite_vertices(points, uvs, tint, 0.25);
        for index in 0..4 {
            assert_eq!(
                vertices[index].position,
                mq::Vec3::new(points[index].x, points[index].y, 0.0)
            );
            assert_eq!(
                vertices[index].uv,
                mq::Vec2::new(uvs[index].x, uvs[index].y)
            );
            assert_eq!(
                vertices[index].color,
                [color.red(), color.green(), color.blue(), 32]
            );
        }
    }

    #[test]
    fn stroke_width_is_transformed_with_its_local_geometry() {
        let quad = stroke_quad(Vec2::ZERO, Vec2::new(4.0, 0.0), 2.0).unwrap();
        let transform = Affine2::from_scale(Vec2::new(3.0, 2.0));
        let transformed = quad.map(|point| transform.transform_point2(point));
        let height = transformed
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max)
            - transformed
                .iter()
                .map(|point| point.y)
                .fold(f32::INFINITY, f32::min);

        assert!((height - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn convexity_includes_the_wraparound_vertices() {
        let concave_at_the_wraparound = [
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        for rotation in 0..concave_at_the_wraparound.len() {
            let mut rotated = concave_at_the_wraparound.to_vec();
            rotated.rotate_left(rotation);
            assert!(!is_convex(&rotated));
        }
    }

    #[test]
    fn convexity_is_invariant_under_vertex_rotation() {
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        for rotation in 0..square.len() {
            let mut rotated = square.to_vec();
            rotated.rotate_left(rotation);
            assert!(is_convex(&rotated));
        }
    }
}
