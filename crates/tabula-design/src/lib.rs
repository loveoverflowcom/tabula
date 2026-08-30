//! Semantic, renderer-neutral design tokens. (doc 04 §7–§8)
//!
//! `tokens.toml` is validated and deterministically resolved into the generated
//! themes in [`generated`]. Presentation consumes these semantic roles, never
//! palette literals, font handles, or arbitrary visual constants.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// An sRGB colour used by semantic tokens and render commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
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

    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }
    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }
    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }
    #[must_use]
    pub const fn alpha(self) -> u8 {
        self.alpha
    }
}

/// A finite, non-negative logical visual measurement established by token validation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonNegative(f32);

impl NonNegative {
    /// Constructs a bounded logical measurement.
    pub fn new(value: f32) -> Result<Self, TokenValueError> {
        (value.is_finite() && value >= 0.0)
            .then_some(Self(value))
            .ok_or(TokenValueError::NonNegative)
    }
    /// Used only by generated themes after `xtask` has validated the source.
    #[must_use]
    pub(crate) const fn generated(value: f32) -> Self {
        assert!(value >= 0.0);
        Self(value)
    }
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// A finite, strictly positive logical visual measurement established by token validation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Positive(f32);

impl Positive {
    /// Constructs a positive logical measurement.
    pub fn new(value: f32) -> Result<Self, TokenValueError> {
        (value.is_finite() && value > 0.0)
            .then_some(Self(value))
            .ok_or(TokenValueError::Positive)
    }
    /// Used only by generated themes after `xtask` has validated the source.
    #[must_use]
    pub(crate) const fn generated(value: f32) -> Self {
        assert!(value > 0.0);
        Self(value)
    }
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// An inclusive percentage, used for state layers and reduced-motion duration scaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Percent(u8);

impl Percent {
    /// Constructs a percentage in `0..=100`.
    pub fn new(value: u8) -> Result<Self, TokenValueError> {
        (value <= 100)
            .then_some(Self(value))
            .ok_or(TokenValueError::Percent)
    }
    /// Used only by generated themes after `xtask` has validated the source.
    #[must_use]
    pub(crate) const fn generated(value: u8) -> Self {
        assert!(value <= 100);
        Self(value)
    }
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A token-value constructor rejected an invalid value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenValueError {
    NonNegative,
    Positive,
    Percent,
}

impl core::fmt::Display for TokenValueError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::NonNegative => "token value must be finite and non-negative",
            Self::Positive => "token value must be finite and positive",
            Self::Percent => "token percentage must be in 0..=100",
        })
    }
}

impl std::error::Error for TokenValueError {}

/// The four supported accessibility-aware colour schemes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeKind {
    Light,
    Dark,
    HighContrastLight,
    HighContrastDark,
}

/// The complete resolved semantic theme. It is cheap data, not a renderer handle. (I-10)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub kind: ThemeKind,
    pub color: ColorTokens,
    pub type_: TypographyTokens,
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
    /// Resolves the intentionally closed presentation text-style vocabulary.
    #[must_use]
    pub const fn text_style(self, token: TextStyleToken) -> TextStyle {
        match token {
            TextStyleToken::DisplayLg => self.type_.display.lg,
            TextStyleToken::DisplayMd => self.type_.display.md,
            TextStyleToken::DisplaySm => self.type_.display.sm,
            TextStyleToken::HeadlineLg => self.type_.headline.lg,
            TextStyleToken::HeadlineMd => self.type_.headline.md,
            TextStyleToken::HeadlineSm => self.type_.headline.sm,
            TextStyleToken::TitleLg => self.type_.title.lg,
            TextStyleToken::TitleMd => self.type_.title.md,
            TextStyleToken::TitleSm => self.type_.title.sm,
            TextStyleToken::BodyLg => self.type_.body.lg,
            TextStyleToken::BodyMd => self.type_.body.md,
            TextStyleToken::BodySm => self.type_.body.sm,
            TextStyleToken::LabelLg => self.type_.label.lg,
            TextStyleToken::LabelMd => self.type_.label.md,
            TextStyleToken::LabelSm => self.type_.label.sm,
            TextStyleToken::MonoMd => self.type_.mono.md,
            TextStyleToken::MonoSm => self.type_.mono.sm,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Renderer-neutral font-family roles, not font files or handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFamilyRole {
    Display,
    Text,
    Mono,
}

/// One resolved semantic text style in logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamilyRole,
    pub size: Positive,
    pub line_height: Positive,
    pub weight: u16,
    pub letter_spacing: f32,
    pub tabular_figures: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSizes {
    pub lg: TextStyle,
    pub md: TextStyle,
    pub sm: TextStyle,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonoTextSizes {
    pub md: TextStyle,
    pub sm: TextStyle,
}
/// Typography tokens chosen by semantic role and size. (doc 04 §7.4)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypographyTokens {
    pub display: TextSizes,
    pub headline: TextSizes,
    pub title: TextSizes,
    pub body: TextSizes,
    pub label: TextSizes,
    pub mono: MonoTextSizes,
}

/// The stable semantic text vocabulary accepted by normal presentation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyleToken {
    DisplayLg,
    DisplayMd,
    DisplaySm,
    HeadlineLg,
    HeadlineMd,
    HeadlineSm,
    TitleLg,
    TitleMd,
    TitleSm,
    BodyLg,
    BodyMd,
    BodySm,
    LabelLg,
    LabelMd,
    LabelSm,
    MonoMd,
    MonoSm,
}

/// Reference and semantic shape roles; values are logical radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeTokens {
    pub none: NonNegative,
    pub xs: NonNegative,
    pub sm: NonNegative,
    pub md: NonNegative,
    pub lg: NonNegative,
    pub xl: NonNegative,
    pub full: NonNegative,
    pub card: NonNegative,
    pub board: NonNegative,
    pub token: NonNegative,
    pub sheet: NonNegative,
    pub button: NonNegative,
    pub chip: NonNegative,
}

/// Complete named logical spacing scale; consumers never index an arbitrary array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpaceTokens {
    pub none: u16,
    pub xxs: u16,
    pub xs: u16,
    pub sm: u16,
    pub md: u16,
    pub lg: u16,
    pub xl: u16,
    pub xxl: u16,
    pub xxxl: u16,
    pub xxxxl: u16,
    pub xxxxxl: u16,
    pub xxxxxxl: u16,
}

/// Abstract elevation levels. Each renderer maps them to appropriate shadows or assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElevationTokens {
    pub low: u8,
    pub medium: u8,
    pub high: u8,
}

/// Motion data only; animation execution belongs to the later presentation runtime. (I-10)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionTokens {
    pub instant: MotionDuration,
    pub short: MotionDuration,
    pub medium: MotionDuration,
    pub long: MotionDuration,
    pub xlong: MotionDuration,
    pub stagger: MotionDuration,
    pub spring_snappy: Spring,
    pub spring_standard: Spring,
    pub spring_weighty: Spring,
    pub spring_bouncy: Spring,
    pub reduced: ReducedMotion,
    pub piece_move: MotionProfile,
    pub card_deal: MotionProfile,
    pub card_play: MotionProfile,
    pub tile_place: MotionProfile,
    pub token_drop: MotionProfile,
    pub reveal: MotionProfile,
    pub phase_change: MotionProfile,
    pub turn_change: MotionProfile,
    pub vote: MotionProfile,
    pub score_update: MotionProfile,
    pub win: MotionProfile,
    pub lose: MotionProfile,
    pub invalid: MotionProfile,
    pub enter: MotionProfile,
    pub exit: MotionProfile,
    pub drag_lift: MotionProfile,
    pub drag_drop: MotionProfile,
}

/// A non-negative duration in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionDuration(u16);
impl MotionDuration {
    #[must_use]
    pub const fn milliseconds(self) -> u16 {
        self.0
    }
    #[must_use]
    pub(crate) const fn generated(value: u16) -> Self {
        Self(value)
    }
}

/// Validated finite spring parameters. All components are strictly positive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: Positive,
    pub damping: Positive,
    pub mass: Positive,
}

/// A semantic reference to one generated spring family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringKind {
    Snappy,
    Standard,
    Weighty,
    Bouncy,
}
/// Whether a profile communicates game state or is ambient/decorative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionCategory {
    Informative,
    Ambient,
}
/// A compact, resolved semantic motion request. It is not an animation timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionProfile {
    pub duration: MotionDuration,
    pub spring: SpringKind,
    pub stagger: MotionDuration,
    pub category: MotionCategory,
}

/// A first-class policy for reducing motion without erasing informative changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReducedMotion {
    pub duration_scale: Percent,
    pub prefer_fade: bool,
    pub disable_ambient: bool,
    pub keep_informative: bool,
}

/// State-layer opacities. Percent prevents an invalid opacity entering a resolved theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateLayerTokens {
    pub hover: Percent,
    pub focus: Percent,
    pub press: Percent,
    pub drag: Percent,
    pub disabled_content: Percent,
    pub disabled_container: Percent,
}

/// Logical density and accessibility target metrics, never physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Density {
    pub scale: Positive,
    pub min_target: Positive,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusTokens {
    pub ring_width: Positive,
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
            0.2126 * channel(color.red())
                + 0.7152 * channel(color.green())
                + 0.0722 * channel(color.blue())
        }
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn named_semantic_accessibility_pairs_meet_their_thresholds() {
        for kind in [
            ThemeKind::Light,
            ThemeKind::Dark,
            ThemeKind::HighContrastLight,
            ThemeKind::HighContrastDark,
        ] {
            let theme = Theme::by_kind(kind);
            let c = theme.color;
            for (name, foreground, background, minimum) in [
                ("on-surface/surface", c.on_surface, c.surface, 4.5),
                ("on-primary/primary", c.on_primary, c.primary, 4.5),
                ("on-success/success", c.on_success, c.success, 4.5),
                ("on-danger/danger", c.on_danger, c.danger, 4.5),
                ("legal-target/surface", c.legal_target, c.surface, 3.0),
                ("selected/surface", c.selected, c.surface, 3.0),
                ("threat/surface", c.threat, c.surface, 3.0),
                ("turn-active/surface", c.turn_active, c.surface, 3.0),
                ("focus/surface", theme.focus.ring_color, c.surface, 3.0),
            ] {
                assert!(ratio(foreground, background) >= minimum, "{kind:?}: {name}");
            }
        }
    }

    #[test]
    fn high_contrast_strengthens_critical_surface_pairs() {
        for (standard, high_contrast) in [
            (ThemeKind::Light, ThemeKind::HighContrastLight),
            (ThemeKind::Dark, ThemeKind::HighContrastDark),
        ] {
            let normal = Theme::by_kind(standard);
            let hc = Theme::by_kind(high_contrast);
            assert!(
                ratio(hc.color.on_surface, hc.color.surface)
                    >= ratio(normal.color.on_surface, normal.color.surface)
            );
            assert!(
                ratio(hc.focus.ring_color, hc.color.surface)
                    >= ratio(normal.focus.ring_color, normal.color.surface)
            );
        }
    }

    #[test]
    fn mono_styles_require_tabular_figures() {
        let theme = Theme::by_kind(ThemeKind::Light);
        assert!(theme.text_style(TextStyleToken::MonoMd).tabular_figures);
        assert!(theme.text_style(TextStyleToken::MonoSm).tabular_figures);
    }

    #[test]
    fn bounded_token_values_reject_invalid_boundaries() {
        assert_eq!(Percent::new(101), Err(TokenValueError::Percent));
        assert_eq!(NonNegative::new(-0.1), Err(TokenValueError::NonNegative));
        assert_eq!(Positive::new(0.0), Err(TokenValueError::Positive));
        assert_eq!(Positive::new(f32::NAN), Err(TokenValueError::Positive));
    }
}
