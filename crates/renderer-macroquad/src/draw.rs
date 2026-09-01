#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]

use glam::{Affine2, Vec2};
use macroquad::{models::Vertex, prelude as mq};
use tabula_design::Color;
use tabula_presentation::{Border, Corners, LinearGradient, Paint, Rect, RenderCmd, RenderError};

use crate::state::{logical_transform, Clip, DrawState};
use crate::text;

pub(crate) fn execute(
    command: &RenderCmd,
    state: DrawState,
    camera: tabula_presentation::Camera2D,
    frame: &tabula_presentation::FrameCtx,
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
        RenderCmd::Sprite { asset, .. } => {
            return Err(RenderError(format!(
                "renderer-macroquad cannot render sprite {asset:?}: asset loading is deferred to Phase 3"
            )));
        }
        RenderCmd::PushClip { .. }
        | RenderCmd::PopClip { .. }
        | RenderCmd::PushTransform { .. }
        | RenderCmd::PopTransform { .. }
        | RenderCmd::PushOpacity { .. }
        | RenderCmd::PopOpacity { .. } => {
            return Err(RenderError(String::from(
                "renderer-macroquad received a scope command as a primitive",
            )));
        }
    }
    Ok(())
}

fn configure_clip_viewport(
    clip: Clip,
    frame: &tabula_presentation::FrameCtx,
) -> Result<(), RenderError> {
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
    let dpi = frame.dpi().get();
    let mut camera = mq::Camera2D::from_display_rect(mq::Rect::new(
        rect.origin().x,
        rect.origin().y,
        rect.size().x,
        rect.size().y,
    ));
    camera.viewport = Some((
        device_coordinate(rect.origin().x * dpi)?,
        device_coordinate(rect.origin().y * dpi)?,
        device_coordinate(rect.size().x * dpi)?,
        device_coordinate(rect.size().y * dpi)?,
    ));
    mq::set_camera(&camera);
    Ok(())
}

#[allow(clippy::cast_possible_truncation, clippy::float_arithmetic)]
fn device_coordinate(value: f32) -> Result<i32, RenderError> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
        return Err(RenderError(String::from(
            "renderer-macroquad scissor exceeds supported device coordinates",
        )));
    }
    Ok(rounded as i32)
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
            return Err(RenderError(String::from(
                "renderer-macroquad supports filled convex paths only",
            )));
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
        return Err(RenderError(String::from(
            "renderer-macroquad received invalid convex fill geometry",
        )));
    }
    let mut vertices = Vec::with_capacity(points.len());
    for (point, color) in points.iter().zip(colors) {
        vertices.push(vertex(transform.transform_point2(*point), *color));
    }
    let mut indices = Vec::with_capacity((points.len() - 2) * 3);
    for index in 1..(points.len() - 1) {
        let first = u16::try_from(0).expect("zero fits in a mesh index");
        let second = u16::try_from(index).map_err(|_| {
            RenderError(String::from(
                "renderer-macroquad polygon exceeds mesh index capacity",
            ))
        })?;
        let third = u16::try_from(index + 1).map_err(|_| {
            RenderError(String::from(
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
        let start = transform.transform_point2(pair[0]);
        let end = transform.transform_point2(pair[1]);
        let delta = end - start;
        let length = delta.length();
        if length == 0.0 {
            continue;
        }
        let normal = Vec2::new(-delta.y, delta.x) * (border.width() / (2.0 * length));
        let color = apply_opacity(border.color(), opacity);
        let vertices = [
            vertex(start + normal, color),
            vertex(start - normal, color),
            vertex(end - normal, color),
            vertex(end + normal, color),
        ];
        mq::draw_mesh(&mq::Mesh {
            vertices: vertices.to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: None,
        });
    }
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
    let mut sign = None;
    for window in points.windows(3) {
        let cross = (window[1] - window[0]).perp_dot(window[2] - window[1]);
        if cross == 0.0 {
            continue;
        }
        let current = cross.is_sign_positive();
        if sign.is_some_and(|previous| previous != current) {
            return false;
        }
        sign = Some(current);
    }
    true
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
