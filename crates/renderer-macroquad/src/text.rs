//! Macroquad default-font mapping for semantic text tokens.
//!
//! This first backend maps the token's validated size and line height consistently for measuring
//! and drawing. Font family, weight, tracking, and complex shaping need the Phase 3 font asset
//! path; they intentionally do not leak into the presentation contract.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]

use glam::{Affine2, Vec2};
use macroquad::prelude as mq;
use tabula_design::{Color, Positive, TextStyle, TextStyleToken, Theme};
use tabula_presentation::{Align, RenderError, TextMetrics};

pub(crate) fn measure(
    text: &str,
    style: TextStyle,
    max_width: Option<Positive>,
) -> Result<TextMetrics, RenderError> {
    let lines = wrap_lines(text, max_width, |line| {
        raw_measure(line, font_size(style)).width
    });
    let width = lines
        .iter()
        .map(|line| raw_measure(line, font_size(style)).width)
        .fold(0.0, f32::max);
    let line_count = u16::try_from(lines.len()).map_err(|_| {
        RenderError(String::from(
            "renderer-macroquad text has more than u16::MAX lines",
        ))
    })?;
    TextMetrics::new(
        Vec2::new(width, style.line_height().get() * f32::from(line_count)),
        line_count,
    )
    .map_err(|error| RenderError(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    value: &str,
    at: Vec2,
    token: TextStyleToken,
    align: Align,
    max_width: Option<Positive>,
    color: Color,
    opacity: f32,
    transform: Affine2,
    theme: &Theme,
    _dpi: f32,
) -> Result<(), RenderError> {
    let Some(scale) = uniform_positive_scale(transform) else {
        return Err(RenderError(String::from(
            "renderer-macroquad cannot draw transformed text without a text-shaping backend",
        )));
    };
    let style = theme.text_style(token);
    let lines = wrap_lines(value, max_width, |line| {
        raw_measure(line, font_size(style)).width
    });
    let container_width = max_width.map(Positive::get);
    for (index, line) in lines.iter().enumerate() {
        let dimensions = raw_measure(line, font_size(style));
        let width = dimensions.width;
        let offset = match align {
            Align::Start => 0.0,
            Align::Center => -width / 2.0,
            Align::End => -container_width.unwrap_or(width),
        };
        let point = transform.transform_point2(
            at + Vec2::new(
                offset,
                style.line_height().get()
                    * f32::from(u16::try_from(index + 1).expect("line count fits u16")),
            ),
        );
        mq::draw_text_ex(
            line,
            point.x,
            point.y,
            mq::TextParams {
                font_size: font_size(style),
                font_scale: scale,
                font_scale_aspect: 1.0,
                font: None,
                color: with_opacity(color, opacity),
                rotation: 0.0,
            },
        );
    }
    Ok(())
}

fn raw_measure(text: &str, font_size: u16) -> mq::TextDimensions {
    mq::measure_text(text, None, font_size, 1.0)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn font_size(style: TextStyle) -> u16 {
    style.size().get().round().clamp(1.0, f32::from(u16::MAX)) as u16
}

fn uniform_positive_scale(transform: Affine2) -> Option<f32> {
    let scale = transform.matrix2.x_axis.x;
    (transform.matrix2.x_axis.y == 0.0
        && transform.matrix2.y_axis.x == 0.0
        && (scale - transform.matrix2.y_axis.y).abs() <= f32::EPSILON
        && scale.is_finite()
        && scale > 0.0)
        .then_some(scale)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn with_opacity(color: Color, opacity: f32) -> mq::Color {
    mq::Color::from_rgba(
        color.red(),
        color.green(),
        color.blue(),
        (f32::from(color.alpha()) * opacity).round() as u8,
    )
}

fn wrap_lines(text: &str, max_width: Option<Positive>, measure: impl Fn(&str) -> f32) -> Vec<&str> {
    let Some(max_width) = max_width else {
        return text.split('\n').collect();
    };
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut start = 0;
        let mut last_fitting = 0;
        for (index, character) in paragraph.char_indices() {
            let end = index + character.len_utf8();
            if measure(&paragraph[start..end]) <= max_width.get() {
                last_fitting = end;
                continue;
            }
            if last_fitting == start {
                last_fitting = end;
            }
            lines.push(&paragraph[start..last_fitting]);
            start = last_fitting;
            last_fitting = 0;
        }
        if start < paragraph.len() || paragraph.is_empty() {
            lines.push(&paragraph[start..]);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_limited_text_wraps_with_a_measurement_oracle() {
        let width = Positive::new(2.0).unwrap();
        let lines = wrap_lines("abcd", Some(width), |value| {
            f32::from(u16::try_from(value.chars().count()).expect("test input is short"))
        });
        assert_eq!(lines, ["ab", "cd"]);
    }

    #[test]
    fn explicit_newlines_are_preserved_without_a_width_limit() {
        assert_eq!(
            wrap_lines("first\nsecond", None, |_| 0.0),
            ["first", "second"]
        );
    }

    #[test]
    fn wrapping_never_slices_a_valid_unicode_scalar() {
        for text in ["Tiếng Việt", "こんにちは", "🙂🙂🙂", "aé中🙂"] {
            let unbounded = wrap_lines(text, None, |_| 0.0);
            assert_eq!(unbounded.concat(), text);

            let bounded = wrap_lines(text, Some(Positive::new(2.0).unwrap()), |value| {
                f32::from(u16::try_from(value.chars().count()).expect("test input is short"))
            });
            assert_eq!(bounded.concat(), text);
        }
    }

    #[test]
    fn text_transform_requires_positive_uniform_scale() {
        assert_eq!(
            uniform_positive_scale(Affine2::from_scale(Vec2::splat(2.0))),
            Some(2.0)
        );
        assert_eq!(
            uniform_positive_scale(Affine2::from_scale(Vec2::new(2.0, 3.0))),
            None
        );
        assert_eq!(uniform_positive_scale(Affine2::from_angle(0.5)), None);
        assert_eq!(
            uniform_positive_scale(Affine2::from_scale(Vec2::new(-1.0, 1.0))),
            None
        );
    }
}
