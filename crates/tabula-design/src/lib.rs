//! Semantic, renderer-neutral design tokens. (doc 04 §7–§8)
//!
//! Values originate in `tokens.toml`; generated schemes live in [`generated`].
//! Presentation code consumes semantic roles, never palette literals.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// An sRGB colour used by semantic tokens and render commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: u8::MAX,
        }
    }

    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// The four supported accessibility-aware colour schemes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeKind {
    Light,
    Dark,
    HighContrastLight,
    HighContrastDark,
}

/// The complete resolved semantic theme. It is data, not a renderer handle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub kind: ThemeKind,
    pub color: ColorTokens,
    pub shape: ShapeTokens,
    pub space: SpaceTokens,
    pub elevation: ElevationTokens,
    pub motion: MotionTokens,
    pub state: StateLayerTokens,
    pub density: Density,
    pub focus: FocusTokens,
}

impl Theme {
    #[must_use]
    pub const fn by_kind(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Light => generated::LIGHT,
            ThemeKind::Dark => generated::DARK,
            ThemeKind::HighContrastLight => generated::HIGH_CONTRAST_LIGHT,
            ThemeKind::HighContrastDark => generated::HIGH_CONTRAST_DARK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorTokens {
    pub surface: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub outline: Color,
    pub primary: Color,
    pub on_primary: Color,
    pub success: Color,
    pub on_success: Color,
    pub danger: Color,
    pub on_danger: Color,
    pub turn_active: Color,
    pub turn_waiting: Color,
    pub legal_target: Color,
    pub illegal_target: Color,
    pub selected: Color,
    pub last_action: Color,
    pub threat: Color,
    pub hidden: Color,
    pub team: [Color; 8],
    pub seat_marker: [Color; 8],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeTokens {
    pub card: f32,
    pub board: f32,
    pub button: f32,
    pub chip: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceTokens {
    pub xs: u16,
    pub sm: u16,
    pub md: u16,
    pub lg: u16,
    pub xl: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationTokens {
    pub low: u8,
    pub medium: u8,
    pub high: u8,
}

/// Semantic timings; presenters select a named transition rather than milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotionTokens {
    pub piece_move_ms: u16,
    pub card_deal_ms: u16,
    pub invalid_ms: u16,
    pub phase_change_ms: u16,
    pub win_ms: u16,
    pub lose_ms: u16,
    pub reduced_duration_scale_percent: u8,
}

/// State-layer opacities, represented as percentages to avoid invalid values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateLayerTokens {
    pub hover: u8,
    pub focus: u8,
    pub press: u8,
    pub drag: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Density {
    pub scale: f32,
    pub min_target: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FocusTokens {
    pub ring_width: f32,
    pub ring_color: Color,
}

pub mod generated;

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::float_arithmetic)]
    fn ratio(a: Color, b: Color) -> f32 {
        fn channel(value: u8) -> f32 {
            let linear = f32::from(value) / 255.0;
            if linear <= 0.04045 {
                linear / 12.92
            } else {
                ((linear + 0.055) / 1.055).powf(2.4)
            }
        }
        fn luminance(color: Color) -> f32 {
            0.2126 * channel(color.red)
                + 0.7152 * channel(color.green)
                + 0.0722 * channel(color.blue)
        }
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn semantic_text_pairs_meet_wcag_aa() {
        for theme in [
            ThemeKind::Light,
            ThemeKind::Dark,
            ThemeKind::HighContrastLight,
            ThemeKind::HighContrastDark,
        ] {
            let color = Theme::by_kind(theme).color;
            assert!(ratio(color.on_surface, color.surface) >= 4.5, "{theme:?}");
            assert!(ratio(color.on_primary, color.primary) >= 4.5, "{theme:?}");
            assert!(ratio(color.on_danger, color.danger) >= 4.5, "{theme:?}");
        }
    }
}
