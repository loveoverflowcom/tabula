//! Backend-neutral [`RenderList`] golden snapshot oracle. (Phase 2, doc 04 §6.1)
//!
//! This module provides a deterministic, reviewable text projection for [`RenderList`]
//! instances. It captures the entire [`RenderCmd`] vocabulary — geometry, styling,
//! camera, text tokens, layers, and scope stacking contexts — without requiring
//! a GPU, font engine, or pixel rasterization.
//!
//! # Usage
//!
//! ```rust,ignore
//! use tabula_presentation::GamePresentation;
//! use tabula_testkit::assert_render_list_snapshot;
//!
//! let list = ChessPresentation::present(&view, &local, &frame);
//! assert_render_list_snapshot!("chess_initial_640x640_light", list);
//! ```
//!
//! # Oracle Separation
//!
//! - **RenderList Oracle (this module)**: Verifies the exact semantic trace that a game
//!   presenter submitted to the renderer. Covers 100% of draw commands and themes.
//! - **Raster Oracle ([`renderer_headless`])**: Verifies that the CPU rasterizer executes
//!   its documented subset (solid rects, borders, scissors, transforms, opacity) faithfully.

#![forbid(unsafe_code)]
#![allow(clippy::doc_markdown)]

use std::fmt::Write;
use tabula_presentation::{
    Align, Border, Color, Corners, Layer, Paint, Positive, Rect, RenderCmd, RenderList,
    TextStyleToken, Vec2,
};

#[doc(hidden)]
pub use insta as __insta;

/// Helper trait allowing [`assert_render_list_snapshot!`] to accept either value or reference.
pub trait AsRenderList {
    fn as_render_list(&self) -> &RenderList;
}

impl AsRenderList for RenderList {
    fn as_render_list(&self) -> &RenderList {
        self
    }
}

impl AsRenderList for &RenderList {
    fn as_render_list(&self) -> &RenderList {
        self
    }
}

impl AsRenderList for &mut RenderList {
    fn as_render_list(&self) -> &RenderList {
        self
    }
}

/// Formats a [`RenderList`] into a deterministic, reviewable snapshot representation.
///
/// Every visually meaningful property of each command is captured in sequence order.
/// Floats are formatted with fixed 3-decimal precision and normalized (-0.0 -> 0.0).
///
/// @ai.role visual-oracle
/// @ai.domain presentation.oracle
/// @ai.pure true
/// @ai.invariant deterministic-snapshot-formatting
/// @ai.evidence tests::same_render_list_formatted_twice_is_byte_identical
/// @ai.evidence tests::rect_origin_change_changes_snapshot
#[must_use]
pub fn render_list_snapshot(list: &RenderList) -> String {
    let mut out = String::new();
    let camera = list.camera();
    let _ = writeln!(
        out,
        "camera origin=({}, {}) zoom={}",
        format_scalar(camera.origin().x),
        format_scalar(camera.origin().y),
        format_scalar(camera.zoom()),
    );
    let _ = writeln!(out, "commands (total {}):", list.commands().len());

    for (index, cmd) in list.commands().iter().enumerate() {
        let _ = write!(out, "  {index}: ");
        format_command(&mut out, cmd);
        out.push('\n');
    }
    out
}

/// Asserts that a [`RenderList`] matches its committed insta snapshot.
#[macro_export]
macro_rules! assert_render_list_snapshot {
    ($name:expr, $list:expr $(,)?) => {
        $crate::presentation::__insta::assert_snapshot!(
            $name,
            &$crate::presentation::render_list_snapshot(
                $crate::presentation::AsRenderList::as_render_list(&$list)
            )
        )
    };
    ($list:expr $(,)?) => {
        $crate::presentation::__insta::assert_snapshot!(
            &$crate::presentation::render_list_snapshot(
                $crate::presentation::AsRenderList::as_render_list(&$list)
            )
        )
    };
}

#[allow(clippy::too_many_lines)]
fn format_command(out: &mut String, cmd: &RenderCmd) {
    match cmd {
        RenderCmd::Sprite {
            asset,
            rect,
            src,
            tint,
            rotation,
            pivot,
            layer,
            z,
        } => {
            let _ = write!(
                out,
                "sprite asset=\"{}\" rect={} src={} tint={} rotation={} pivot={} layer={} z={}",
                asset.escape_debug(),
                format_rect(*rect),
                format_opt_rect(*src),
                format_color(*tint),
                format_scalar(*rotation),
                format_vec2(*pivot),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::Rect {
            rect,
            radii,
            fill,
            border,
            layer,
            z,
        } => {
            let _ = write!(
                out,
                "rect {} radii={} fill={} border={} layer={} z={}",
                format_rect(*rect),
                format_corners(*radii),
                format_opt_paint(fill.as_ref()),
                format_opt_border(*border),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::Text {
            text,
            at,
            style,
            align,
            max_width,
            color,
            layer,
            z,
        } => {
            let _ = write!(
                out,
                "text \"{}\" at={} style={} align={} max_width={} color={} layer={} z={}",
                text.escape_debug(),
                format_vec2(*at),
                format_text_style(*style),
                format_align(*align),
                format_opt_positive(*max_width),
                format_color(*color),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::Path {
            points,
            stroke,
            closed,
            fill,
            layer,
            z,
        } => {
            let pts = points
                .iter()
                .map(|p| format_vec2(*p))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(
                out,
                "path points=[{}] stroke={} closed={} fill={} layer={} z={}",
                pts,
                format_border(*stroke),
                closed,
                format_opt_paint(fill.as_ref()),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::PushClip { rect, layer, z } => {
            let _ = write!(
                out,
                "push-clip rect={} layer={} z={}",
                format_rect(*rect),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::PopClip { layer, z } => {
            let _ = write!(out, "pop-clip layer={} z={}", format_layer(*layer), z);
        }
        RenderCmd::PushTransform { matrix, layer, z } => {
            let _ = write!(
                out,
                "push-transform matrix=[{}, {}, {}, {}, {}, {}] layer={} z={}",
                format_scalar(matrix.matrix2.x_axis.x),
                format_scalar(matrix.matrix2.x_axis.y),
                format_scalar(matrix.matrix2.y_axis.x),
                format_scalar(matrix.matrix2.y_axis.y),
                format_scalar(matrix.translation.x),
                format_scalar(matrix.translation.y),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::PopTransform { layer, z } => {
            let _ = write!(out, "pop-transform layer={} z={}", format_layer(*layer), z);
        }
        RenderCmd::PushOpacity { opacity, layer, z } => {
            let _ = write!(
                out,
                "push-opacity opacity={} layer={} z={}",
                format_scalar(opacity.get()),
                format_layer(*layer),
                z
            );
        }
        RenderCmd::PopOpacity { layer, z } => {
            let _ = write!(out, "pop-opacity layer={} z={}", format_layer(*layer), z);
        }
    }
}

#[allow(clippy::float_arithmetic)]
fn normalize_float(val: f32) -> f32 {
    if val == 0.0 || val == -0.0 {
        0.0
    } else {
        val
    }
}

fn format_scalar(value: f32) -> String {
    let normalized = normalize_float(value);
    format!("{normalized:.3}")
}

fn format_vec2(v: Vec2) -> String {
    format!("({}, {})", format_scalar(v.x), format_scalar(v.y))
}

fn format_rect(r: Rect) -> String {
    format!(
        "[origin=({}, {}) size=({}, {})]",
        format_scalar(r.origin().x),
        format_scalar(r.origin().y),
        format_scalar(r.size().x),
        format_scalar(r.size().y)
    )
}

fn format_opt_rect(r: Option<Rect>) -> String {
    r.map_or_else(|| "none".to_string(), format_rect)
}

#[allow(clippy::float_arithmetic)]
fn format_corners(c: Corners) -> String {
    if (c.top_left() - c.top_right()).abs() < 1e-6
        && (c.top_right() - c.bottom_right()).abs() < 1e-6
        && (c.bottom_right() - c.bottom_left()).abs() < 1e-6
    {
        format!("uniform({})", format_scalar(c.top_left()))
    } else {
        format!(
            "(tl={}, tr={}, br={}, bl={})",
            format_scalar(c.top_left()),
            format_scalar(c.top_right()),
            format_scalar(c.bottom_right()),
            format_scalar(c.bottom_left())
        )
    }
}

fn format_color(c: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        c.red(),
        c.green(),
        c.blue(),
        c.alpha()
    )
}

fn format_border(b: Border) -> String {
    format!(
        "[width={} color={}]",
        format_scalar(b.width()),
        format_color(b.color())
    )
}

fn format_opt_border(b: Option<Border>) -> String {
    b.map_or_else(|| "none".to_string(), format_border)
}

fn format_paint(p: &Paint) -> String {
    match p {
        Paint::Solid(c) => format!("solid({})", format_color(*c)),
        Paint::LinearGradient(g) => {
            let stops = g
                .stops()
                .iter()
                .map(|s| format!("{}@{}", format_color(s.color()), format_scalar(s.offset())))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "gradient(from={} to={} stops=[{}])",
                format_vec2(g.from()),
                format_vec2(g.to()),
                stops
            )
        }
    }
}

fn format_opt_paint(p: Option<&Paint>) -> String {
    p.map_or_else(|| "none".to_string(), format_paint)
}

const fn format_text_style(s: TextStyleToken) -> &'static str {
    match s {
        TextStyleToken::DisplayLg => "DisplayLg",
        TextStyleToken::DisplayMd => "DisplayMd",
        TextStyleToken::DisplaySm => "DisplaySm",
        TextStyleToken::HeadlineLg => "HeadlineLg",
        TextStyleToken::HeadlineMd => "HeadlineMd",
        TextStyleToken::HeadlineSm => "HeadlineSm",
        TextStyleToken::TitleLg => "TitleLg",
        TextStyleToken::TitleMd => "TitleMd",
        TextStyleToken::TitleSm => "TitleSm",
        TextStyleToken::BodyLg => "BodyLg",
        TextStyleToken::BodyMd => "BodyMd",
        TextStyleToken::BodySm => "BodySm",
        TextStyleToken::LabelLg => "LabelLg",
        TextStyleToken::LabelMd => "LabelMd",
        TextStyleToken::LabelSm => "LabelSm",
        TextStyleToken::MonoMd => "MonoMd",
        TextStyleToken::MonoSm => "MonoSm",
    }
}

const fn format_align(a: Align) -> &'static str {
    match a {
        Align::Start => "Start",
        Align::Center => "Center",
        Align::End => "End",
    }
}

fn format_opt_positive(p: Option<Positive>) -> String {
    p.map_or_else(|| "none".to_string(), |pos| format_scalar(pos.get()))
}

fn format_layer(l: Layer) -> String {
    match l {
        Layer::BOARD => "board(0)".to_string(),
        Layer::PIECES => "pieces(10)".to_string(),
        Layer::OVERLAY => "overlay(20)".to_string(),
        Layer::HUD => "hud(30)".to_string(),
        Layer::MODAL => "modal(40)".to_string(),
        Layer::TOAST => "toast(50)".to_string(),
        other => format!("layer({})", other.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_presentation::{
        Affine2, Camera2D, GradientStop, LinearGradient, Opacity, RenderListBuilder, Vec2,
    };

    fn make_list(camera: Camera2D, commands: impl IntoIterator<Item = RenderCmd>) -> RenderList {
        let mut builder = RenderListBuilder::new(camera);
        for cmd in commands {
            builder.push(cmd).expect("valid test command");
        }
        builder.finish().expect("valid test list")
    }

    fn sample_rect(origin: Vec2, size: Vec2, color: Color, layer: Layer, z: i16) -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(origin, size).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(color)),
            border: None,
            layer,
            z,
        }
    }

    #[test]
    fn same_render_list_formatted_twice_is_byte_identical() {
        let list = make_list(
            Camera2D::default(),
            [
                sample_rect(
                    Vec2::ZERO,
                    Vec2::splat(10.0),
                    Color::rgb(255, 0, 0),
                    Layer::BOARD,
                    0,
                ),
                RenderCmd::Text {
                    text: "Hello".to_string(),
                    at: Vec2::new(5.0, 5.0),
                    style: TextStyleToken::BodyMd,
                    align: Align::Center,
                    max_width: None,
                    color: Color::rgb(0, 0, 0),
                    layer: Layer::HUD,
                    z: 1,
                },
            ],
        );
        let first = render_list_snapshot(&list);
        let second = render_list_snapshot(&list);
        assert_eq!(first, second);
    }

    #[test]
    fn rect_origin_change_changes_snapshot() {
        let list_a = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::BOARD,
                0,
            )],
        );
        let list_b = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::new(1.0, 0.0),
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::BOARD,
                0,
            )],
        );
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
    }

    #[test]
    fn color_change_changes_snapshot() {
        let list_a = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::BOARD,
                0,
            )],
        );
        let list_b = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(0, 255, 0),
                Layer::BOARD,
                0,
            )],
        );
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
    }

    #[test]
    fn layer_and_z_change_changes_snapshot() {
        let list_a = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::BOARD,
                0,
            )],
        );
        let list_b = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::PIECES,
                0,
            )],
        );
        let list_c = make_list(
            Camera2D::default(),
            [sample_rect(
                Vec2::ZERO,
                Vec2::splat(10.0),
                Color::rgb(255, 0, 0),
                Layer::BOARD,
                5,
            )],
        );
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_c));
    }

    #[test]
    fn text_content_and_style_change_changes_snapshot() {
        let list_a = make_list(
            Camera2D::default(),
            [RenderCmd::Text {
                text: "PieceA".to_string(),
                at: Vec2::ZERO,
                style: TextStyleToken::DisplaySm,
                align: Align::Start,
                max_width: None,
                color: Color::rgb(0, 0, 0),
                layer: Layer::PIECES,
                z: 0,
            }],
        );
        let list_b = make_list(
            Camera2D::default(),
            [RenderCmd::Text {
                text: "PieceB".to_string(),
                at: Vec2::ZERO,
                style: TextStyleToken::DisplaySm,
                align: Align::Start,
                max_width: None,
                color: Color::rgb(0, 0, 0),
                layer: Layer::PIECES,
                z: 0,
            }],
        );
        let list_c = make_list(
            Camera2D::default(),
            [RenderCmd::Text {
                text: "PieceA".to_string(),
                at: Vec2::ZERO,
                style: TextStyleToken::BodyMd,
                align: Align::Start,
                max_width: None,
                color: Color::rgb(0, 0, 0),
                layer: Layer::PIECES,
                z: 0,
            }],
        );
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_c));
    }

    #[test]
    fn camera_zoom_and_origin_change_changes_snapshot() {
        let list_a = make_list(Camera2D::new(Vec2::ZERO, 1.0).unwrap(), []);
        let list_b = make_list(Camera2D::new(Vec2::ZERO, 2.0).unwrap(), []);
        let list_c = make_list(Camera2D::new(Vec2::new(10.0, 0.0), 1.0).unwrap(), []);

        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_c));
    }

    #[test]
    fn scope_order_change_changes_snapshot() {
        let list_a = make_list(
            Camera2D::default(),
            [
                RenderCmd::PushOpacity {
                    opacity: Opacity::try_from(0.5).unwrap(),
                    layer: Layer::BOARD,
                    z: 0,
                },
                sample_rect(
                    Vec2::ZERO,
                    Vec2::ONE,
                    Color::rgb(255, 0, 0),
                    Layer::BOARD,
                    0,
                ),
                RenderCmd::PopOpacity {
                    layer: Layer::BOARD,
                    z: 0,
                },
            ],
        );
        let list_b = make_list(
            Camera2D::default(),
            [
                RenderCmd::PushTransform {
                    matrix: Affine2::IDENTITY,
                    layer: Layer::BOARD,
                    z: 0,
                },
                sample_rect(
                    Vec2::ZERO,
                    Vec2::ONE,
                    Color::rgb(255, 0, 0),
                    Layer::BOARD,
                    0,
                ),
                RenderCmd::PopTransform {
                    layer: Layer::BOARD,
                    z: 0,
                },
            ],
        );
        assert_ne!(render_list_snapshot(&list_a), render_list_snapshot(&list_b));
    }

    #[test]
    fn negative_zero_is_normalized_to_positive_zero() {
        let list = make_list(
            Camera2D::new(Vec2::new(-0.0, 0.0), 1.0).unwrap(),
            [sample_rect(
                Vec2::new(-0.0, 0.0),
                Vec2::splat(1.0),
                Color::rgb(0, 0, 0),
                Layer::BOARD,
                0,
            )],
        );
        let snapshot = render_list_snapshot(&list);
        assert!(!snapshot.contains("-0.000"));
        assert!(snapshot.contains("0.000"));
    }

    #[test]
    fn sprite_and_path_and_gradient_are_fully_formatted() {
        let gradient = LinearGradient::new(
            Vec2::ZERO,
            Vec2::ONE,
            [
                GradientStop::new(0.0, Color::rgb(0, 0, 0)).unwrap(),
                GradientStop::new(1.0, Color::rgb(255, 255, 255)).unwrap(),
            ],
        )
        .unwrap();

        let list = make_list(
            Camera2D::default(),
            [
                RenderCmd::Sprite {
                    asset: "pawn_white".to_string(),
                    rect: Rect::new(Vec2::ZERO, Vec2::splat(10.0)).unwrap(),
                    src: Some(Rect::new(Vec2::ZERO, Vec2::splat(64.0)).unwrap()),
                    tint: Color::rgba(255, 255, 255, 200),
                    rotation: 0.5,
                    pivot: Vec2::new(5.0, 5.0),
                    layer: Layer::PIECES,
                    z: 2,
                },
                RenderCmd::Path {
                    points: [Vec2::ZERO, Vec2::new(10.0, 10.0), Vec2::new(20.0, 0.0)]
                        .into_iter()
                        .collect(),
                    stroke: Border::new(2.0, Color::rgb(255, 0, 0)).unwrap(),
                    closed: true,
                    fill: Some(Paint::LinearGradient(gradient)),
                    layer: Layer::OVERLAY,
                    z: 10,
                },
            ],
        );

        let snapshot = render_list_snapshot(&list);
        assert!(snapshot.contains("sprite asset=\"pawn_white\""));
        assert!(
            snapshot.contains("path points=[(0.000, 0.000), (10.000, 10.000), (20.000, 0.000)]")
        );
        assert!(snapshot.contains("gradient(from=(0.000, 0.000) to=(1.000, 1.000)"));
    }
}
