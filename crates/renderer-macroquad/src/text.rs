//! Macroquad default-font mapping for semantic text tokens.
//!
//! This first backend maps the token's validated size and line height consistently for measuring
//! and drawing. Small tokens rasterize the default pixel font at no less than 16 pixels and
//! scale back to their logical size; smaller direct rasters lose most of their dark ink. Tabular
//! styles give ASCII digits equal advances (doc 04 §7.4), centering the default-font glyphs within
//! those cells. This is a fallback layout, not a loaded mono face.
//! Font family, weight, tracking, and complex shaping still need the Phase 3 font asset path;
//! they intentionally do not leak into the presentation contract.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]

use glam::{Affine2, Vec2};
use macroquad::prelude as mq;
use tabula_design::{Color, Positive, TextStyle, TextStyleToken, Theme};
use tabula_presentation::{Align, FrameCtx, RenderCmd, RenderError, RenderList, TextMetrics};

/// Warms every glyph at its actual backend size before any frame primitive is queued.
///
/// Macroquad 0.4.16 grows its font atlas by deleting the old texture. Drawing a
/// character that triggers growth after earlier glyphs were queued can therefore
/// leave those draws with deleted unmanaged texture IDs. The renderer calls this
/// for all accepted lists before executing the frame (doc 04 §6).
pub(crate) fn prepare(list: &RenderList, frame: &FrameCtx) -> Result<(), RenderError> {
    for command in list.commands() {
        if let RenderCmd::Text {
            text: value,
            style: token,
            max_width,
            ..
        } = command
        {
            validate(value, *token, *max_width, frame)?;
            let style = frame.theme().text_style(*token);
            let size = font_size(style);
            raw_measure(value, size);
            if style.tabular_figures() {
                // TextLayout measures every digit to choose one shared advance.
                raw_measure("0123456789", size);
            }
        }
    }
    Ok(())
}

pub(crate) fn measure(
    text: &str,
    style: TextStyle,
    max_width: Option<Positive>,
) -> Result<TextMetrics, RenderError> {
    let layout = TextLayout::new(style);
    let lines = wrap_lines(text, max_width, |line| layout.width(line));
    let width = lines
        .iter()
        .map(|line| layout.width(line))
        .fold(0.0, f32::max);
    let line_count = u16::try_from(lines.len()).map_err(|_| {
        RenderError::Execution(String::from(
            "renderer-macroquad text has more than u16::MAX lines",
        ))
    })?;
    TextMetrics::new(
        Vec2::new(width, style.line_height().get() * f32::from(line_count)),
        line_count,
    )
    .map_err(|error| RenderError::Execution(error.to_string()))
}

/// Purely proves the line count fits before drawing can emit any primitive.
///
/// Without a width limit the explicit line count is exact. Bounded text conservatively reserves
/// one line per Unicode scalar (and one for each empty paragraph), capped at `u16::MAX`. Thus
/// preflight needs no font or graphics context; measuring and drawing still use exact wrapping.
pub(crate) fn validate(
    value: &str,
    _token: TextStyleToken,
    max_width: Option<Positive>,
    _frame: &tabula_presentation::FrameCtx,
) -> Result<(), RenderError> {
    let line_bound = value.split('\n').try_fold(0_u16, |count, paragraph| {
        let paragraph_bound = if max_width.is_some() {
            paragraph
                .chars()
                .take(usize::from(u16::MAX) + 1)
                .count()
                .max(1)
        } else {
            1
        };
        count.checked_add(u16::try_from(paragraph_bound).ok()?)
    });
    line_bound.map(|_| ()).ok_or_else(|| {
        RenderError::Execution(String::from(if max_width.is_some() {
            "backend bounded text exceeds u16::MAX preflight line capacity"
        } else {
            "backend text has more than u16::MAX lines"
        }))
    })
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
        return Err(RenderError::Unsupported(
            tabula_presentation::RenderCmdKind::Text,
        ));
    };
    let style = theme.text_style(token);
    let layout = TextLayout::new(style);
    let lines = wrap_lines(value, max_width, |line| layout.width(line));
    let container_width = max_width.map(Positive::get);
    for (index, line) in lines.iter().enumerate() {
        let width = layout.width(line);
        let offset = match align {
            Align::Start => 0.0,
            Align::Center => -width / 2.0,
            Align::End => -container_width.unwrap_or(width),
        };
        let baseline = style.line_height().get()
            * f32::from(u16::try_from(index + 1).map_err(|_| {
                RenderError::Execution(String::from(
                    "renderer-macroquad text has more than u16::MAX lines",
                ))
            })?);
        layout_line(
            line,
            layout.digit_advance,
            |run| layout.run_width(run),
            |run, x| {
                let point = transform.transform_point2(at + Vec2::new(offset + x, baseline));
                mq::draw_text_ex(
                    run,
                    point.x,
                    point.y,
                    mq::TextParams {
                        font_size: layout.font_size,
                        font_scale: scale * layout.logical_font_scale,
                        font_scale_aspect: 1.0,
                        font: None,
                        color: with_opacity(color, opacity),
                        rotation: 0.0,
                    },
                );
            },
        );
    }
    Ok(())
}

/// One unscaled fallback layout, shared by wrapping, measuring, and drawing.
struct TextLayout {
    font_size: u16,
    logical_font_scale: f32,
    digit_advance: Option<f32>,
}

impl TextLayout {
    fn new(style: TextStyle) -> Self {
        let (font_size, logical_font_scale) = font_raster(style);
        let digit_advance = style.tabular_figures().then(|| {
            tabular_digit_advance(|digit| raw_measure(digit, font_size).width * logical_font_scale)
        });
        Self {
            font_size,
            logical_font_scale,
            digit_advance,
        }
    }

    fn width(&self, line: &str) -> f32 {
        layout_line(
            line,
            self.digit_advance,
            |run| self.run_width(run),
            |_, _| {},
        )
    }

    fn run_width(&self, value: &str) -> f32 {
        raw_measure(value, self.font_size).width * self.logical_font_scale
    }
}

fn tabular_digit_advance(measure: impl Fn(&str) -> f32) -> f32 {
    ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
        .into_iter()
        .map(measure)
        .fold(0.0, f32::max)
}

/// Emits proportional non-digit runs and centered tabular digits at logical x offsets.
/// The returned advance is also the width used for alignment and width-limited wrapping.
fn layout_line(
    text: &str,
    digit_advance: Option<f32>,
    measure: impl Fn(&str) -> f32,
    mut emit: impl FnMut(&str, f32),
) -> f32 {
    let Some(digit_advance) = digit_advance else {
        emit(text, 0.0);
        return measure(text);
    };
    let mut x = 0.0;
    let mut run_start = 0;
    for (index, character) in text.char_indices() {
        if !character.is_ascii_digit() {
            continue;
        }
        if run_start < index {
            let run = &text[run_start..index];
            emit(run, x);
            x += measure(run);
        }
        let end = index + character.len_utf8();
        let digit = &text[index..end];
        emit(digit, x + (digit_advance - measure(digit)) / 2.0);
        x += digit_advance;
        run_start = end;
    }
    if run_start < text.len() {
        let run = &text[run_start..];
        emit(run, x);
        x += measure(run);
    }
    x
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
    font_raster(style).0
}

/// One size mapping for prepare, measure, wrap, tabular advances, and draw. `ProggyClean` has
/// complete pixel-font coverage at 16 pixels; preserve token size with a logical scale instead
/// of asking fontdue for a sparse 11/12-pixel raster.
fn font_raster(style: TextStyle) -> (u16, f32) {
    let raster_size = style.size().get().round().clamp(16.0, f32::from(u16::MAX)) as u16;
    (raster_size, style.size().get() / f32::from(raster_size))
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

pub(crate) fn supports_transform(transform: Affine2) -> bool {
    uniform_positive_scale(transform).is_some()
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
        for (index, character) in paragraph.char_indices() {
            let end = index + character.len_utf8();
            if measure(&paragraph[start..end]) <= max_width.get() {
                continue;
            }
            if start < index {
                lines.push(&paragraph[start..index]);
                start = index;
            }
            // An oversized scalar still makes progress, but cannot absorb the next scalar.
            if measure(&paragraph[start..end]) > max_width.get() {
                lines.push(&paragraph[start..end]);
                start = end;
            }
        }
        if start < paragraph.len() || paragraph.is_empty() {
            lines.push(&paragraph[start..]);
        }
    }
    lines
}

#[cfg(test)]
// The independent oracle uses exactly representable integer and half-cell widths.
#[allow(clippy::float_cmp)]
mod tests {
    use tabula_design::ThemeKind;
    use tabula_presentation::{
        Camera2D, Dpi, FrameCtx, Layer, RenderCmd, RenderListBuilder, Viewport,
    };

    use super::*;

    #[test]
    fn small_font_rasters_keep_token_size_and_share_the_preparation_size() {
        for kind in [
            ThemeKind::Light,
            ThemeKind::Dark,
            ThemeKind::HighContrastLight,
            ThemeKind::HighContrastDark,
        ] {
            let theme = Theme::by_kind(kind);
            for (token, expected_raster, expected_scale) in [
                (TextStyleToken::LabelSm, 16, 11.0 / 16.0),
                (TextStyleToken::LabelMd, 16, 12.0 / 16.0),
                (TextStyleToken::BodyMd, 16, 14.0 / 16.0),
                (TextStyleToken::TitleMd, 16, 1.0),
                (TextStyleToken::TitleLg, 22, 1.0),
            ] {
                let style = theme.text_style(token);
                let (raster, scale) = font_raster(style);
                assert_eq!(raster, expected_raster);
                assert_eq!(scale, expected_scale);
                assert_eq!(
                    font_size(style),
                    raster,
                    "glyph preparation warms the exact draw raster"
                );
                assert_eq!(
                    f32::from(raster) * scale,
                    style.size().get(),
                    "physical raster changes preserve logical token size"
                );
            }
        }
    }

    #[test]
    fn scaled_raster_measurement_keeps_tabular_alignment_and_wrapping_in_logical_units() {
        let logical_scale = 12.0 / 16.0;
        let measure = |run: &str| proportional_width(run) * logical_scale;
        let advance = tabular_digit_advance(measure);
        assert_eq!(advance, 7.5);
        let mut runs = Vec::new();
        assert_eq!(
            layout_line("11:88", Some(advance), measure, |run, at| runs
                .push((run.to_owned(), at))),
            33.0
        );
        assert_eq!(
            runs[0].1, 2.625,
            "digits center inside scaled logical cells"
        );
        assert_eq!(
            wrap_lines("1111", Some(Positive::new(15.0).unwrap()), |line| {
                layout_line(line, Some(advance), measure, |_, _| {})
            }),
            ["11", "11"]
        );
    }

    #[test]
    fn bounded_and_unbounded_text_pass_preflight_without_a_graphics_context() {
        let frame = FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        );
        for (style, value) in [
            (TextStyleToken::MonoMd, ""),
            (TextStyleToken::MonoMd, "11:11\n88:88"),
            (TextStyleToken::MonoSm, "LOW 00:08"),
            (TextStyleToken::TitleSm, "Board"),
            (TextStyleToken::LabelLg, "Zoom in"),
        ] {
            for max_width in [None, Some(Positive::new(1.0).unwrap())] {
                let mut builder = RenderListBuilder::new(Camera2D::default());
                builder
                    .push(RenderCmd::Text {
                        text: String::from(value),
                        at: Vec2::ZERO,
                        style,
                        align: Align::Start,
                        max_width,
                        color: frame.theme().color.on_surface,
                        layer: Layer::HUD,
                        z: 0,
                    })
                    .unwrap();
                assert_eq!(
                    crate::MacroquadRenderer::preflight(&builder.finish().unwrap(), &frame),
                    Ok(())
                );
            }
        }
    }

    #[test]
    fn bounded_preflight_capacity_counts_unicode_scalars_and_empty_paragraphs() {
        let frame = FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        );
        let maximum = "🙂".repeat(usize::from(u16::MAX));
        let width = Some(Positive::new(1.0).unwrap());
        assert_eq!(
            validate(&maximum, TextStyleToken::MonoMd, width, &frame),
            Ok(())
        );
        for oversized in [
            format!("{maximum}\n"),
            "🙂".repeat(usize::from(u16::MAX) + 1),
        ] {
            assert_eq!(
                validate(&oversized, TextStyleToken::MonoMd, width, &frame),
                Err(RenderError::Execution(String::from(
                    "backend bounded text exceeds u16::MAX preflight line capacity"
                )))
            );
        }
        assert_eq!(
            validate(&maximum, TextStyleToken::MonoMd, None, &frame),
            Ok(())
        );
    }

    /// Deliberately proportional oracle: narrow 1, wide 8, and a kerned non-digit run.
    fn proportional_width(value: &str) -> f32 {
        if value == "AV" {
            return 9.0;
        }
        value
            .chars()
            .map(|character| match character {
                '1' => 3.0,
                '8' => 10.0,
                ':' => 4.0,
                'é' | '中' | '🙂' => 7.0,
                _ => 5.0,
            })
            .sum()
    }

    #[test]
    fn tabular_clock_figures_keep_the_same_width_across_every_digit() {
        let advance = tabular_digit_advance(proportional_width);
        assert_eq!(advance, 10.0);
        for left in '0'..='9' {
            for right in '0'..='9' {
                let clock = format!("{left}{right}:{right}{left}");
                assert_eq!(
                    layout_line(&clock, Some(advance), proportional_width, |_, _| {}),
                    44.0,
                    "four digit cells and the colon stay fixed for {clock}"
                );
            }
        }
        assert_ne!(proportional_width("11:11"), proportional_width("88:88"));
    }

    #[test]
    fn tabular_glyphs_are_centered_without_changing_non_digit_runs() {
        let mut runs = Vec::new();
        let width = layout_line("AV1é中8🙂", Some(10.0), proportional_width, |run, x| {
            runs.push((run.to_owned(), x));
        });
        assert_eq!(width, 50.0);
        assert_eq!(
            runs,
            [
                (String::from("AV"), 0.0),
                (String::from("1"), 12.5),
                (String::from("é中"), 19.0),
                (String::from("8"), 33.0),
                (String::from("🙂"), 43.0),
            ]
        );
        assert_eq!(
            layout_line("", Some(10.0), proportional_width, |_, _| {
                panic!("empty tabular text must not emit a run")
            }),
            0.0
        );
    }

    #[test]
    fn non_tabular_text_retains_one_proportional_run() {
        let mut runs = Vec::new();
        assert_eq!(
            layout_line("AV11", None, proportional_width, |run, x| {
                runs.push((run.to_owned(), x));
            }),
            16.0
        );
        assert_eq!(runs, [(String::from("AV11"), 0.0)]);
    }

    #[test]
    fn tabular_wrapping_uses_digit_cells_instead_of_narrow_glyph_widths() {
        for (width, expected) in [
            (20.0, vec!["11", "11"]),
            (10.0, vec!["1", "1", "1", "1"]),
            (9.0, vec!["1", "1", "1", "1"]),
        ] {
            assert_eq!(
                wrap_lines("1111", Some(Positive::new(width).unwrap()), |line| {
                    layout_line(line, Some(10.0), proportional_width, |_, _| {})
                }),
                expected
            );
        }
    }

    #[test]
    fn wrapping_keeps_oversized_unicode_scalars_separate_from_following_text() {
        let lines = wrap_lines("a中b🙂c", Some(Positive::new(2.0).unwrap()), |value| {
            value
                .chars()
                .map(|character| match character {
                    '中' => 3.0,
                    '🙂' => 4.0,
                    _ => 1.0,
                })
                .sum()
        });
        assert_eq!(lines, ["a", "中", "b", "🙂", "c"]);
    }

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
