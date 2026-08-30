//! Deterministic semantic-token validation and generation. (doc 04 §7–§8)
//!
//! Boundary map: authored TOML -> validated typed source -> resolved semantic
//! model -> Rust/CSS/JSON artifacts. The core transformation is pure; file I/O
//! and rustfmt live only at the outer command boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const SCHEMES: [Scheme; 4] = [
    Scheme {
        name: "light",
        constant: "LIGHT",
        kind: "ThemeKind::Light",
    },
    Scheme {
        name: "dark",
        constant: "DARK",
        kind: "ThemeKind::Dark",
    },
    Scheme {
        name: "hc-light",
        constant: "HIGH_CONTRAST_LIGHT",
        kind: "ThemeKind::HighContrastLight",
    },
    Scheme {
        name: "hc-dark",
        constant: "HIGH_CONTRAST_DARK",
        kind: "ThemeKind::HighContrastDark",
    },
];
const COLOR_KEYS: [&str; 20] = [
    "surface",
    "container",
    "container-high",
    "on-surface",
    "on-surface-variant",
    "outline",
    "primary",
    "on-primary",
    "success",
    "on-success",
    "danger",
    "on-danger",
    "turn-active",
    "turn-waiting",
    "legal-target",
    "illegal-target",
    "selected",
    "last-action",
    "threat",
    "hidden",
];
static FORMAT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct Scheme {
    name: &'static str,
    constant: &'static str,
    kind: &'static str,
}

/// Structured failures at the authored-token trust boundary.
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("workspace root cannot be derived")]
    Root,
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("writing {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("tokens.toml: {0}")]
    Toml(String),
    #[error("meta.schemes must be exactly {expected:?}, got {actual:?}")]
    SchemeMetadata {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("scheme declarations must be exactly {expected:?}, got {actual:?}")]
    SchemeSet {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("scheme `{scheme}` has invalid hexadecimal colour `{value}`")]
    Color { scheme: String, value: String },
    #[error("token `{key}` must be a finite fraction in 0..=1")]
    Fraction { key: &'static str },
    #[error("token `{key}` must be finite and non-negative")]
    NonNegative { key: &'static str },
    #[error("token `{key}` must be finite and positive")]
    Positive { key: &'static str },
    #[error("token `{key}` must be at least {minimum}")]
    Minimum { key: &'static str, minimum: f32 },
    #[error("token `{key}` has invalid duration ordering")]
    DurationOrder { key: &'static str },
    #[error("rustfmt failed while formatting generated Rust")]
    Rustfmt,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenSource {
    meta: MetaSource,
    #[serde(rename = "ref")]
    reference: ReferenceSource,
    sys: SystemSource,
    schemes: BTreeMap<String, SchemeSource>,
    /// The component tier is deliberately data-shaped for future additive tokens.
    /// It is JSON-only until a reusable component has a documented deviation.
    comp: BTreeMap<String, toml::Value>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MetaSource {
    identity: String,
    schemes: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReferenceSource {
    palette: BTreeMap<String, String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SystemSource {
    color: EmptySource,
    space: SpaceSource,
    shape: ShapeSource,
    #[serde(rename = "type")]
    typography: TypographySource,
    state: StateSource,
    motion: MotionSource,
    density: DensitySource,
    elevation: ElevationSource,
    focus: FocusSource,
}
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct EmptySource {}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SchemeSource {
    surface: String,
    container: String,
    #[serde(rename = "container-high")]
    container_high: String,
    #[serde(rename = "on-surface")]
    on_surface: String,
    #[serde(rename = "on-surface-variant")]
    on_surface_variant: String,
    outline: String,
    primary: String,
    #[serde(rename = "on-primary")]
    on_primary: String,
    success: String,
    #[serde(rename = "on-success")]
    on_success: String,
    danger: String,
    #[serde(rename = "on-danger")]
    on_danger: String,
    #[serde(rename = "turn-active")]
    turn_active: String,
    #[serde(rename = "turn-waiting")]
    turn_waiting: String,
    #[serde(rename = "legal-target")]
    legal_target: String,
    #[serde(rename = "illegal-target")]
    illegal_target: String,
    selected: String,
    #[serde(rename = "last-action")]
    last_action: String,
    threat: String,
    hidden: String,
    team: [String; 8],
    #[serde(rename = "seat-marker")]
    seat_marker: [String; 8],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpaceSource {
    none: u16,
    xxs: u16,
    xs: u16,
    sm: u16,
    md: u16,
    lg: u16,
    xl: u16,
    xxl: u16,
    xxxl: u16,
    xxxxl: u16,
    xxxxxl: u16,
    xxxxxxl: u16,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ShapeSource {
    none: f32,
    xs: f32,
    sm: f32,
    md: f32,
    lg: f32,
    xl: f32,
    full: f32,
    card: f32,
    board: f32,
    token: f32,
    sheet: f32,
    button: f32,
    chip: f32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TypographySource {
    family: FontStacksSource,
    display: TextSizesSource,
    headline: TextSizesSource,
    title: TextSizesSource,
    body: TextSizesSource,
    label: TextSizesSource,
    mono: MonoTextSizesSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FontStacksSource {
    display: String,
    text: String,
    mono: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TextSizesSource {
    lg: TextStyleSource,
    md: TextStyleSource,
    sm: TextStyleSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MonoTextSizesSource {
    md: TextStyleSource,
    sm: TextStyleSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TextStyleSource {
    family: FontFamilySource,
    size: f32,
    #[serde(rename = "line-height")]
    line_height: f32,
    weight: u16,
    #[serde(rename = "letter-spacing")]
    letter_spacing: f32,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum FontFamilySource {
    Display,
    Text,
    Mono,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StateSource {
    hover: f64,
    focus: f64,
    press: f64,
    drag: f64,
    #[serde(rename = "disabled-content")]
    disabled_content: f64,
    #[serde(rename = "disabled-container")]
    disabled_container: f64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MotionSource {
    instant: u16,
    short: u16,
    medium: u16,
    long: u16,
    xlong: u16,
    stagger: u16,
    spring: SpringsSource,
    reduced: ReducedSource,
    profile: MotionProfilesSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpringsSource {
    snappy: SpringSource,
    standard: SpringSource,
    weighty: SpringSource,
    bouncy: SpringSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpringSource {
    stiffness: f32,
    damping: f32,
    mass: f32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReducedSource {
    #[serde(rename = "duration-scale")]
    duration_scale: f64,
    #[serde(rename = "prefer-fade")]
    prefer_fade: bool,
    #[serde(rename = "disable-ambient")]
    disable_ambient: bool,
    #[serde(rename = "keep-informative")]
    keep_informative: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MotionProfilesSource {
    #[serde(rename = "piece-move")]
    piece_move: MotionProfileSource,
    #[serde(rename = "card-deal")]
    card_deal: MotionProfileSource,
    #[serde(rename = "card-play")]
    card_play: MotionProfileSource,
    #[serde(rename = "tile-place")]
    tile_place: MotionProfileSource,
    #[serde(rename = "token-drop")]
    token_drop: MotionProfileSource,
    reveal: MotionProfileSource,
    #[serde(rename = "phase-change")]
    phase_change: MotionProfileSource,
    #[serde(rename = "turn-change")]
    turn_change: MotionProfileSource,
    vote: MotionProfileSource,
    #[serde(rename = "score-update")]
    score_update: MotionProfileSource,
    win: MotionProfileSource,
    lose: MotionProfileSource,
    invalid: MotionProfileSource,
    enter: MotionProfileSource,
    exit: MotionProfileSource,
    #[serde(rename = "drag-lift")]
    drag_lift: MotionProfileSource,
    #[serde(rename = "drag-drop")]
    drag_drop: MotionProfileSource,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MotionProfileSource {
    duration: DurationName,
    spring: SpringName,
    #[serde(default)]
    stagger: Option<DurationName>,
    category: MotionCategorySource,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DurationName {
    Instant,
    Short,
    Medium,
    Long,
    Xlong,
    Stagger,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum SpringName {
    Snappy,
    Standard,
    Weighty,
    Bouncy,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum MotionCategorySource {
    Informative,
    Ambient,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DensitySource {
    scale: f32,
    #[serde(rename = "min-target")]
    min_target: f32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ElevationSource {
    low: u8,
    medium: u8,
    high: u8,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FocusSource {
    #[serde(rename = "ring-width")]
    ring_width: f32,
    #[serde(rename = "ring-color")]
    ring_color: String,
}

pub fn run() -> Result<(), TokenError> {
    let root = root()?;
    for (relative, content) in render(&root)? {
        write_if_changed(&root.join(relative), &content)?;
    }
    println!("gen-tokens: emitted validated Rust, CSS, and JSON semantic contracts");
    Ok(())
}

pub fn check_current() -> Result<bool, TokenError> {
    let root = root()?;
    let mut current = true;
    for (relative, rendered) in render(&root)? {
        let path = root.join(relative);
        let existing = std::fs::read_to_string(&path).map_err(|source| TokenError::Read {
            path: path.clone(),
            source,
        })?;
        if existing != rendered {
            eprintln!(
                "generated token output is stale: {} (run `cargo xtask gen-tokens`)",
                path.display()
            );
            current = false;
        }
    }
    Ok(current)
}

fn root() -> Result<PathBuf, TokenError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or(TokenError::Root)
}
fn render(root: &Path) -> Result<Vec<(&'static str, String)>, TokenError> {
    let path = root.join("tokens.toml");
    let text = std::fs::read_to_string(&path).map_err(|source| TokenError::Read {
        path: path.clone(),
        source,
    })?;
    render_source(root, &text)
}
fn render_source(root: &Path, text: &str) -> Result<Vec<(&'static str, String)>, TokenError> {
    let source: TokenSource =
        toml::from_str(text).map_err(|error| TokenError::Toml(error.to_string()))?;
    validate(&source)?;
    Ok(vec![
        (
            "crates/tabula-design/src/generated.rs",
            format_rust(root, &rust(&source))?,
        ),
        ("apps/web/style/tokens.css", css(&source)),
        ("docs/ui/tokens.json", json(&source)),
    ])
}

fn validate(source: &TokenSource) -> Result<(), TokenError> {
    let expected = SCHEMES
        .iter()
        .map(|scheme| scheme.name.to_owned())
        .collect::<Vec<_>>();
    if source.meta.schemes != expected {
        return Err(TokenError::SchemeMetadata {
            expected: expected.clone(),
            actual: source.meta.schemes.clone(),
        });
    }
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    let actual_set = source.schemes.keys().cloned().collect::<BTreeSet<_>>();
    if actual_set != expected_set {
        return Err(TokenError::SchemeSet {
            expected: expected_set.into_iter().collect(),
            actual: actual_set.into_iter().collect(),
        });
    }
    for scheme in &SCHEMES {
        validate_scheme(scheme.name, &source.schemes[scheme.name])?;
    }
    for (key, value) in [
        ("sys.state.hover", source.sys.state.hover),
        ("sys.state.focus", source.sys.state.focus),
        ("sys.state.press", source.sys.state.press),
        ("sys.state.drag", source.sys.state.drag),
        (
            "sys.state.disabled-content",
            source.sys.state.disabled_content,
        ),
        (
            "sys.state.disabled-container",
            source.sys.state.disabled_container,
        ),
        (
            "sys.motion.reduced.duration-scale",
            source.sys.motion.reduced.duration_scale,
        ),
    ] {
        fraction(key, value)?;
    }
    for (key, value) in shape_values(&source.sys.shape) {
        non_negative(key, value)?;
    }
    validate_type(&source.sys.typography.display)?;
    validate_type(&source.sys.typography.headline)?;
    validate_type(&source.sys.typography.title)?;
    validate_type(&source.sys.typography.body)?;
    validate_type(&source.sys.typography.label)?;
    validate_text("sys.type.mono.md", &source.sys.typography.mono.md)?;
    validate_text("sys.type.mono.sm", &source.sys.typography.mono.sm)?;
    let motion = &source.sys.motion;
    if !(motion.instant <= motion.short
        && motion.short <= motion.medium
        && motion.medium <= motion.long
        && motion.long <= motion.xlong)
    {
        return Err(TokenError::DurationOrder { key: "sys.motion" });
    }
    for (key, spring) in [
        ("sys.motion.spring.snappy", &motion.spring.snappy),
        ("sys.motion.spring.standard", &motion.spring.standard),
        ("sys.motion.spring.weighty", &motion.spring.weighty),
        ("sys.motion.spring.bouncy", &motion.spring.bouncy),
    ] {
        positive(key, spring.stiffness)?;
        positive(key, spring.damping)?;
        positive(key, spring.mass)?;
    }
    positive("sys.density.scale", source.sys.density.scale)?;
    positive("sys.density.min-target", source.sys.density.min_target)?;
    if source.sys.density.min_target < 44.0 {
        return Err(TokenError::Minimum {
            key: "sys.density.min-target",
            minimum: 44.0,
        });
    }
    positive("sys.focus.ring-width", source.sys.focus.ring_width)?;
    if !COLOR_KEYS.contains(&source.sys.focus.ring_color.as_str()) {
        return Err(TokenError::Color {
            scheme: "sys.focus.ring-color".into(),
            value: source.sys.focus.ring_color.clone(),
        });
    }
    Ok(())
}
fn validate_scheme(name: &str, scheme: &SchemeSource) -> Result<(), TokenError> {
    for color in scheme_colors(scheme) {
        parse_hex(name, color)?;
    }
    for color in scheme.team.iter().chain(&scheme.seat_marker) {
        parse_hex(name, color)?;
    }
    Ok(())
}
fn validate_type(styles: &TextSizesSource) -> Result<(), TokenError> {
    validate_text("sys.type", &styles.lg)?;
    validate_text("sys.type", &styles.md)?;
    validate_text("sys.type", &styles.sm)
}
fn validate_text(key: &'static str, style: &TextStyleSource) -> Result<(), TokenError> {
    positive(key, style.size)?;
    positive(key, style.line_height)?;
    if !(style.letter_spacing.is_finite() && style.weight > 0) {
        return Err(TokenError::Positive { key });
    }
    Ok(())
}
fn fraction(key: &'static str, value: f64) -> Result<(), TokenError> {
    (value.is_finite() && (0.0..=1.0).contains(&value))
        .then_some(())
        .ok_or(TokenError::Fraction { key })
}
fn non_negative(key: &'static str, value: f32) -> Result<(), TokenError> {
    (value.is_finite() && value >= 0.0)
        .then_some(())
        .ok_or(TokenError::NonNegative { key })
}
fn positive(key: &'static str, value: f32) -> Result<(), TokenError> {
    (value.is_finite() && value > 0.0)
        .then_some(())
        .ok_or(TokenError::Positive { key })
}

#[allow(clippy::write_with_newline)]
fn rust(source: &TokenSource) -> String {
    let sys = &source.sys;
    let mut output = String::from(
        "// @generated by `cargo xtask gen-tokens`; do not edit.\nuse crate::{\n    Color, ColorTokens, Density, ElevationTokens, FocusTokens, FontFamilyRole,\n    MonoTextSizes, MotionCategory, MotionDuration, MotionProfile, MotionTokens,\n    NonNegative, Percent, Positive, ReducedMotion, ShapeTokens, SpaceTokens, Spring,\n    SpringKind, StateLayerTokens, TextSizes, TextStyle, Theme, ThemeKind, TypographyTokens,\n};\n\n",
    );
    output.push_str(
        "const fn duration(value: u16) -> MotionDuration { MotionDuration::generated(value) }\n",
    );
    output.push_str("const fn positive(value: f32) -> Positive { Positive::generated(value) }\n");
    output.push_str(
        "const fn non_negative(value: f32) -> NonNegative { NonNegative::generated(value) }\n",
    );
    output.push_str("const fn percent(value: u8) -> Percent { Percent::generated(value) }\n\n");
    write!(
        output,
        "const TYPE: TypographyTokens = {};\n",
        typography_rust(&sys.typography)
    )
    .expect("String write");
    write!(
        output,
        "const SHAPE: ShapeTokens = {};\n",
        shape_rust(&sys.shape)
    )
    .expect("String write");
    write!(
        output,
        "const SPACE: SpaceTokens = {};\n",
        space_rust(&sys.space)
    )
    .expect("String write");
    write!(
        output,
        "const ELEVATION: ElevationTokens = ElevationTokens {{ low: {}, medium: {}, high: {} }};\n",
        sys.elevation.low, sys.elevation.medium, sys.elevation.high
    )
    .expect("String write");
    write!(
        output,
        "const MOTION: MotionTokens = {};\n",
        motion_rust(&sys.motion)
    )
    .expect("String write");
    write!(
        output,
        "const STATE: StateLayerTokens = {};\n",
        state_rust(&sys.state)
    )
    .expect("String write");
    write!(
        output,
        "const DENSITY: Density = Density {{ scale: positive({}), min_target: positive({}) }};\n",
        float(sys.density.scale),
        float(sys.density.min_target)
    )
    .expect("String write");
    output.push_str("const fn theme(kind: ThemeKind, color: ColorTokens) -> Theme { Theme { kind, color, type_: TYPE, shape: SHAPE, space: SPACE, elevation: ELEVATION, motion: MOTION, state: STATE, density: DENSITY, focus: FocusTokens { ring_width: positive(");
    write!(
        output,
        "{}), ring_color: color.{} }} }} }}\n",
        float(sys.focus.ring_width),
        field(&sys.focus.ring_color)
    )
    .expect("String write");
    for scheme in SCHEMES {
        writeln!(
            output,
            "pub const {}: Theme = theme({}, {});",
            scheme.constant,
            scheme.kind,
            color_tokens(&source.schemes[scheme.name])
        )
        .expect("String write");
    }
    output
}
fn typography_rust(type_: &TypographySource) -> String {
    format!("TypographyTokens {{ display: {}, headline: {}, title: {}, body: {}, label: {}, mono: {} }}", text_sizes_rust(&type_.display), text_sizes_rust(&type_.headline), text_sizes_rust(&type_.title), text_sizes_rust(&type_.body), text_sizes_rust(&type_.label), mono_sizes_rust(&type_.mono))
}
fn text_sizes_rust(value: &TextSizesSource) -> String {
    format!(
        "TextSizes {{ lg: {}, md: {}, sm: {} }}",
        text_style_rust(&value.lg),
        text_style_rust(&value.md),
        text_style_rust(&value.sm)
    )
}
fn mono_sizes_rust(value: &MonoTextSizesSource) -> String {
    format!(
        "MonoTextSizes {{ md: {}, sm: {} }}",
        text_style_rust(&value.md),
        text_style_rust(&value.sm)
    )
}
fn text_style_rust(value: &TextStyleSource) -> String {
    let family = match value.family {
        FontFamilySource::Display => "FontFamilyRole::Display",
        FontFamilySource::Text => "FontFamilyRole::Text",
        FontFamilySource::Mono => "FontFamilyRole::Mono",
    };
    format!("TextStyle {{ family: {family}, size: positive({}), line_height: positive({}), weight: {}, letter_spacing: {}, tabular_figures: {} }}", float(value.size), float(value.line_height), value.weight, float(value.letter_spacing), matches!(value.family, FontFamilySource::Mono))
}
fn shape_rust(shape: &ShapeSource) -> String {
    format!("ShapeTokens {{ none: non_negative({}), xs: non_negative({}), sm: non_negative({}), md: non_negative({}), lg: non_negative({}), xl: non_negative({}), full: non_negative({}), card: non_negative({}), board: non_negative({}), token: non_negative({}), sheet: non_negative({}), button: non_negative({}), chip: non_negative({}) }}", float(shape.none), float(shape.xs), float(shape.sm), float(shape.md), float(shape.lg), float(shape.xl), float(shape.full), float(shape.card), float(shape.board), float(shape.token), float(shape.sheet), float(shape.button), float(shape.chip))
}
fn space_rust(space: &SpaceSource) -> String {
    format!("SpaceTokens {{ none: {}, xxs: {}, xs: {}, sm: {}, md: {}, lg: {}, xl: {}, xxl: {}, xxxl: {}, xxxxl: {}, xxxxxl: {}, xxxxxxl: {} }}", space.none, space.xxs, space.xs, space.sm, space.md, space.lg, space.xl, space.xxl, space.xxxl, space.xxxxl, space.xxxxxl, space.xxxxxxl)
}
fn state_rust(state: &StateSource) -> String {
    format!("StateLayerTokens {{ hover: percent({}), focus: percent({}), press: percent({}), drag: percent({}), disabled_content: percent({}), disabled_container: percent({}) }}", percent(state.hover), percent(state.focus), percent(state.press), percent(state.drag), percent(state.disabled_content), percent(state.disabled_container))
}
fn motion_rust(motion: &MotionSource) -> String {
    format!("MotionTokens {{ instant: duration({}), short: duration({}), medium: duration({}), long: duration({}), xlong: duration({}), stagger: duration({}), spring_snappy: {}, spring_standard: {}, spring_weighty: {}, spring_bouncy: {}, reduced: ReducedMotion {{ duration_scale: percent({}), prefer_fade: {}, disable_ambient: {}, keep_informative: {} }}, piece_move: {}, card_deal: {}, card_play: {}, tile_place: {}, token_drop: {}, reveal: {}, phase_change: {}, turn_change: {}, vote: {}, score_update: {}, win: {}, lose: {}, invalid: {}, enter: {}, exit: {}, drag_lift: {}, drag_drop: {} }}", motion.instant, motion.short, motion.medium, motion.long, motion.xlong, motion.stagger, spring_rust(&motion.spring.snappy), spring_rust(&motion.spring.standard), spring_rust(&motion.spring.weighty), spring_rust(&motion.spring.bouncy), percent(motion.reduced.duration_scale), motion.reduced.prefer_fade, motion.reduced.disable_ambient, motion.reduced.keep_informative, profile_rust(&motion.profile.piece_move, motion), profile_rust(&motion.profile.card_deal, motion), profile_rust(&motion.profile.card_play, motion), profile_rust(&motion.profile.tile_place, motion), profile_rust(&motion.profile.token_drop, motion), profile_rust(&motion.profile.reveal, motion), profile_rust(&motion.profile.phase_change, motion), profile_rust(&motion.profile.turn_change, motion), profile_rust(&motion.profile.vote, motion), profile_rust(&motion.profile.score_update, motion), profile_rust(&motion.profile.win, motion), profile_rust(&motion.profile.lose, motion), profile_rust(&motion.profile.invalid, motion), profile_rust(&motion.profile.enter, motion), profile_rust(&motion.profile.exit, motion), profile_rust(&motion.profile.drag_lift, motion), profile_rust(&motion.profile.drag_drop, motion))
}
fn spring_rust(spring: &SpringSource) -> String {
    format!(
        "Spring {{ stiffness: positive({}), damping: positive({}), mass: positive({}) }}",
        float(spring.stiffness),
        float(spring.damping),
        float(spring.mass)
    )
}
fn profile_rust(profile: &MotionProfileSource, motion: &MotionSource) -> String {
    let spring = match profile.spring {
        SpringName::Snappy => "SpringKind::Snappy",
        SpringName::Standard => "SpringKind::Standard",
        SpringName::Weighty => "SpringKind::Weighty",
        SpringName::Bouncy => "SpringKind::Bouncy",
    };
    let category = match profile.category {
        MotionCategorySource::Informative => "MotionCategory::Informative",
        MotionCategorySource::Ambient => "MotionCategory::Ambient",
    };
    format!("MotionProfile {{ duration: duration({}), spring: {spring}, stagger: duration({}), category: {category} }}", duration(profile.duration, motion), profile.stagger.map_or(0, |name| duration(name, motion)))
}
fn duration(name: DurationName, motion: &MotionSource) -> u16 {
    match name {
        DurationName::Instant => motion.instant,
        DurationName::Short => motion.short,
        DurationName::Medium => motion.medium,
        DurationName::Long => motion.long,
        DurationName::Xlong => motion.xlong,
        DurationName::Stagger => motion.stagger,
    }
}
fn color_tokens(scheme: &SchemeSource) -> String {
    format!("ColorTokens {{ surface: {}, surface_container: {}, surface_container_high: {}, on_surface: {}, on_surface_variant: {}, outline: {}, primary: {}, on_primary: {}, success: {}, on_success: {}, danger: {}, on_danger: {}, turn_active: {}, turn_waiting: {}, legal_target: {}, illegal_target: {}, selected: {}, last_action: {}, threat: {}, hidden: {}, team: [{}], seat_marker: [{}] }}", color_rust(&scheme.surface), color_rust(&scheme.container), color_rust(&scheme.container_high), color_rust(&scheme.on_surface), color_rust(&scheme.on_surface_variant), color_rust(&scheme.outline), color_rust(&scheme.primary), color_rust(&scheme.on_primary), color_rust(&scheme.success), color_rust(&scheme.on_success), color_rust(&scheme.danger), color_rust(&scheme.on_danger), color_rust(&scheme.turn_active), color_rust(&scheme.turn_waiting), color_rust(&scheme.legal_target), color_rust(&scheme.illegal_target), color_rust(&scheme.selected), color_rust(&scheme.last_action), color_rust(&scheme.threat), color_rust(&scheme.hidden), scheme.team.iter().map(|value| color_rust(value)).collect::<Vec<_>>().join(", "), scheme.seat_marker.iter().map(|value| color_rust(value)).collect::<Vec<_>>().join(", "))
}

fn css(source: &TokenSource) -> String {
    let mut output = String::from(
        "/* @generated by `cargo xtask gen-tokens` from tokens.toml; do not edit. */\n",
    );
    for (index, scheme) in SCHEMES.iter().enumerate() {
        let selector = match scheme.name {
            "light" => ":root",
            "dark" => ":root[data-theme=\"dark\"]",
            "hc-light" => ":root[data-theme=\"hc-light\"]",
            _ => ":root[data-theme=\"hc-dark\"]",
        };
        writeln!(output, "{selector} {{").expect("String write");
        css_colors(&mut output, &source.schemes[scheme.name]);
        css_system(&mut output, &source.sys);
        output.push_str("}\n");
        if index + 1 != SCHEMES.len() {
            output.push('\n');
        }
    }
    output
}
fn css_colors(output: &mut String, scheme: &SchemeSource) {
    for (key, value) in COLOR_KEYS.into_iter().zip(scheme_colors(scheme)) {
        writeln!(output, "  --sys-color-{key}: {value};").expect("String write");
    }
    for (index, value) in scheme.team.iter().enumerate() {
        writeln!(output, "  --sys-color-team-{}: {value};", index + 1).expect("String write");
    }
    for (index, value) in scheme.seat_marker.iter().enumerate() {
        writeln!(output, "  --sys-color-seat-marker-{}: {value};", index + 1)
            .expect("String write");
    }
}
fn css_system(output: &mut String, sys: &SystemSource) {
    let space = &sys.space;
    for (name, value) in [
        ("none", space.none),
        ("xxs", space.xxs),
        ("xs", space.xs),
        ("sm", space.sm),
        ("md", space.md),
        ("lg", space.lg),
        ("xl", space.xl),
        ("xxl", space.xxl),
        ("xxxl", space.xxxl),
        ("xxxxl", space.xxxxl),
        ("xxxxxl", space.xxxxxl),
        ("xxxxxxl", space.xxxxxxl),
    ] {
        writeln!(output, "  --sys-space-{name}: {value}px;").expect("String write");
    }
    for (name, value) in shape_values(&sys.shape) {
        writeln!(output, "  --sys-shape-{name}: {}px;", float(value)).expect("String write");
    }
    for (name, stack) in [
        ("display", &sys.typography.family.display),
        ("text", &sys.typography.family.text),
        ("mono", &sys.typography.family.mono),
    ] {
        writeln!(output, "  --sys-type-family-{name}: {stack};").expect("String write");
    }
    css_text_styles(output, "display", &sys.typography.display);
    css_text_styles(output, "headline", &sys.typography.headline);
    css_text_styles(output, "title", &sys.typography.title);
    css_text_styles(output, "body", &sys.typography.body);
    css_text_styles(output, "label", &sys.typography.label);
    for (name, style) in [
        ("mono-md", &sys.typography.mono.md),
        ("mono-sm", &sys.typography.mono.sm),
    ] {
        css_text_style(output, name, style);
    }
    for (name, value) in [
        ("hover", sys.state.hover),
        ("focus", sys.state.focus),
        ("press", sys.state.press),
        ("drag", sys.state.drag),
        ("disabled-content", sys.state.disabled_content),
        ("disabled-container", sys.state.disabled_container),
    ] {
        writeln!(output, "  --sys-state-{name}: {};", fraction_text(value)).expect("String write");
    }
    let motion = &sys.motion;
    for (name, value) in [
        ("instant", motion.instant),
        ("short", motion.short),
        ("medium", motion.medium),
        ("long", motion.long),
        ("xlong", motion.xlong),
        ("stagger", motion.stagger),
    ] {
        writeln!(output, "  --sys-motion-{name}: {value}ms;").expect("String write");
    }
    for (name, spring) in [
        ("snappy", &motion.spring.snappy),
        ("standard", &motion.spring.standard),
        ("weighty", &motion.spring.weighty),
        ("bouncy", &motion.spring.bouncy),
    ] {
        writeln!(
            output,
            "  --sys-motion-spring-{name}: {}, {}, {};",
            float(spring.stiffness),
            float(spring.damping),
            float(spring.mass)
        )
        .expect("String write");
    }
    for (name, profile) in profile_entries(&motion.profile) {
        writeln!(output, "  --sys-motion-{name}-duration: {}ms;\n  --sys-motion-{name}-stagger: {}ms;\n  --sys-motion-{name}-spring: {};\n  --sys-motion-{name}-category: {};", duration(profile.duration, motion), profile.stagger.map_or(0, |value| duration(value, motion)), spring_name(profile.spring), category_name(profile.category)).expect("String write");
    }
    writeln!(output, "  --sys-motion-reduced-duration-scale: {};\n  --sys-motion-reduced-prefer-fade: {};\n  --sys-motion-reduced-disable-ambient: {};\n  --sys-motion-reduced-keep-informative: {};", fraction_text(motion.reduced.duration_scale), motion.reduced.prefer_fade, motion.reduced.disable_ambient, motion.reduced.keep_informative).expect("String write");
    writeln!(output, "  --sys-density-scale: {};\n  --sys-density-min-target: {}px;\n  --sys-focus-ring-width: {}px;\n  --sys-focus-ring-color: var(--sys-color-{});\n  --sys-elevation-low: {};\n  --sys-elevation-medium: {};\n  --sys-elevation-high: {};", float(sys.density.scale), float(sys.density.min_target), float(sys.focus.ring_width), sys.focus.ring_color, sys.elevation.low, sys.elevation.medium, sys.elevation.high).expect("String write");
}
fn css_text_styles(output: &mut String, role: &str, styles: &TextSizesSource) {
    for (size, style) in [("lg", &styles.lg), ("md", &styles.md), ("sm", &styles.sm)] {
        css_text_style(output, &format!("{role}-{size}"), style);
    }
}
fn css_text_style(output: &mut String, name: &str, style: &TextStyleSource) {
    writeln!(output, "  --sys-type-{name}-family: var(--sys-type-family-{});\n  --sys-type-{name}-size: {}px;\n  --sys-type-{name}-line-height: {}px;\n  --sys-type-{name}-weight: {};\n  --sys-type-{name}-letter-spacing: {}px;\n  --sys-type-{name}-tabular-figures: {};", family_name(style.family), float(style.size), float(style.line_height), style.weight, float(style.letter_spacing), matches!(style.family, FontFamilySource::Mono)).expect("String write");
}

#[derive(Serialize)]
struct JsonExport<'a> {
    schema: &'static str,
    metadata: &'a MetaSource,
    reference: &'a ReferenceSource,
    system: &'a SystemSource,
    schemes: &'a BTreeMap<String, SchemeSource>,
    component: &'a BTreeMap<String, toml::Value>,
}
fn json(source: &TokenSource) -> String {
    let export = JsonExport {
        schema: "tabula.design-tokens/v1",
        metadata: &source.meta,
        reference: &source.reference,
        system: &source.sys,
        schemes: &source.schemes,
        component: &source.comp,
    };
    format!(
        "{}\n",
        serde_json::to_string_pretty(&export).expect("validated source serializes")
    )
}

fn scheme_colors(scheme: &SchemeSource) -> [&str; 20] {
    [
        &scheme.surface,
        &scheme.container,
        &scheme.container_high,
        &scheme.on_surface,
        &scheme.on_surface_variant,
        &scheme.outline,
        &scheme.primary,
        &scheme.on_primary,
        &scheme.success,
        &scheme.on_success,
        &scheme.danger,
        &scheme.on_danger,
        &scheme.turn_active,
        &scheme.turn_waiting,
        &scheme.legal_target,
        &scheme.illegal_target,
        &scheme.selected,
        &scheme.last_action,
        &scheme.threat,
        &scheme.hidden,
    ]
}
fn shape_values(shape: &ShapeSource) -> [(&'static str, f32); 13] {
    [
        ("none", shape.none),
        ("xs", shape.xs),
        ("sm", shape.sm),
        ("md", shape.md),
        ("lg", shape.lg),
        ("xl", shape.xl),
        ("full", shape.full),
        ("card", shape.card),
        ("board", shape.board),
        ("token", shape.token),
        ("sheet", shape.sheet),
        ("button", shape.button),
        ("chip", shape.chip),
    ]
}
fn profile_entries(profiles: &MotionProfilesSource) -> [(&'static str, &MotionProfileSource); 17] {
    [
        ("piece-move", &profiles.piece_move),
        ("card-deal", &profiles.card_deal),
        ("card-play", &profiles.card_play),
        ("tile-place", &profiles.tile_place),
        ("token-drop", &profiles.token_drop),
        ("reveal", &profiles.reveal),
        ("phase-change", &profiles.phase_change),
        ("turn-change", &profiles.turn_change),
        ("vote", &profiles.vote),
        ("score-update", &profiles.score_update),
        ("win", &profiles.win),
        ("lose", &profiles.lose),
        ("invalid", &profiles.invalid),
        ("enter", &profiles.enter),
        ("exit", &profiles.exit),
        ("drag-lift", &profiles.drag_lift),
        ("drag-drop", &profiles.drag_drop),
    ]
}
fn family_name(value: FontFamilySource) -> &'static str {
    match value {
        FontFamilySource::Display => "display",
        FontFamilySource::Text => "text",
        FontFamilySource::Mono => "mono",
    }
}
fn spring_name(value: SpringName) -> &'static str {
    match value {
        SpringName::Snappy => "snappy",
        SpringName::Standard => "standard",
        SpringName::Weighty => "weighty",
        SpringName::Bouncy => "bouncy",
    }
}
fn category_name(value: MotionCategorySource) -> &'static str {
    match value {
        MotionCategorySource::Informative => "informative",
        MotionCategorySource::Ambient => "ambient",
    }
}
fn field(value: &str) -> String {
    value.replace('-', "_")
}
fn fraction_text(value: f64) -> String {
    format!("{value}")
}
// Validation establishes `value` is finite and in 0..=1 before this conversion.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn percent(value: f64) -> u8 {
    (value * 100.0).round() as u8
}
fn float(value: f32) -> String {
    format!("{value:?}")
}
fn color_rust(value: &str) -> String {
    let (red, green, blue) = parse_hex("generated", value).expect("validated colour");
    format!("Color::rgb({red}, {green}, {blue})")
}
fn parse_hex(scheme: &str, value: &str) -> Result<(u8, u8, u8), TokenError> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(TokenError::Color {
            scheme: scheme.into(),
            value: value.into(),
        });
    };
    if hex.len() != 6 {
        return Err(TokenError::Color {
            scheme: scheme.into(),
            value: value.into(),
        });
    }
    let parse = |range| {
        u8::from_str_radix(&hex[range], 16).map_err(|_| TokenError::Color {
            scheme: scheme.into(),
            value: value.into(),
        })
    };
    Ok((parse(0..2)?, parse(2..4)?, parse(4..6)?))
}
fn format_rust(root: &Path, source: &str) -> Result<String, TokenError> {
    let id = FORMAT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = root.join(format!("target/token-generation-{id}.rs"));
    std::fs::write(&path, source).map_err(|source| TokenError::Write {
        path: path.clone(),
        source,
    })?;
    if !Command::new("rustfmt")
        .arg(&path)
        .status()
        .is_ok_and(|status| status.success())
    {
        return Err(TokenError::Rustfmt);
    }
    std::fs::read_to_string(&path).map_err(|source| TokenError::Read { path, source })
}
fn write_if_changed(path: &Path, content: &str) -> Result<(), TokenError> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    std::fs::write(path, content).map_err(|source| TokenError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rendered(source: &str) -> Vec<(&'static str, String)> {
        render_source(
            Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
            source,
        )
        .unwrap()
    }
    fn output<'a>(outputs: &'a [(&str, String)], path: &str) -> &'a str {
        outputs
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .unwrap()
            .1
            .as_str()
    }
    fn source() -> &'static str {
        include_str!("../../tokens.toml")
    }
    #[test]
    fn generation_is_idempotent() {
        assert_eq!(rendered(source()), rendered(source()));
    }
    #[test]
    fn all_major_token_families_reach_the_intended_artifacts() {
        for (needle, replacement) in [
            ("surface = \"#FFFBFF\"", "surface = \"#FEFBFF\""),
            ("size = 57.0", "size = 58.0"),
            ("md = 12", "md = 13"),
            ("card = 16.0", "card = 17.0"),
            ("medium = 280", "medium = 281"),
            ("hover = 0.08", "hover = 0.09"),
            ("scale = 1.0", "scale = 1.1"),
            ("ring-width = 3.0", "ring-width = 4.0"),
            ("low = 1", "low = 4"),
        ] {
            let before = rendered(source());
            let after = rendered(&source().replacen(needle, replacement, 1));
            assert_ne!(
                output(&before, "docs/ui/tokens.json"),
                output(&after, "docs/ui/tokens.json"),
                "JSON {needle}"
            );
            assert_ne!(
                output(&before, "crates/tabula-design/src/generated.rs"),
                output(&after, "crates/tabula-design/src/generated.rs"),
                "Rust {needle}"
            );
            assert_ne!(
                output(&before, "apps/web/style/tokens.css"),
                output(&after, "apps/web/style/tokens.css"),
                "CSS {needle}"
            );
        }
    }
    #[test]
    fn malformed_sources_fail_at_the_typed_boundary() {
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("surface = \"#FFFBFF\"", "surface = \"not-a-colour\"", 1)
            ),
            Err(TokenError::Color { .. })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("hover = 0.08", "hover = 1.1", 1)
            ),
            Err(TokenError::Fraction {
                key: "sys.state.hover"
            })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("card = 16.0", "card = -1.0", 1)
            ),
            Err(TokenError::NonNegative { .. })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("mass = 1.0", "mass = 0.0", 1)
            ),
            Err(TokenError::Positive { .. })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("min-target = 44.0", "min-target = 43.0", 1)
            ),
            Err(TokenError::Minimum {
                key: "sys.density.min-target",
                ..
            })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen(
                    "schemes = [\"light\", \"dark\", \"hc-light\", \"hc-dark\"]",
                    "schemes = [\"dark\", \"light\", \"hc-light\", \"hc-dark\"]",
                    1
                )
            ),
            Err(TokenError::SchemeMetadata { .. })
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen(
                    "team = [\"#005F73\", \"#CA6702\", \"#6D597A\", \"#127D66\", \"#B64664\", \"#385F91\", \"#79521E\", \"#5E587E\"]",
                    "team = [\"#005F73\"]",
                    1,
                )
            ),
            Err(TokenError::Toml(_))
        ));
        assert!(matches!(
            render_source(
                Path::new("."),
                &source().replacen("[sys.type.title.lg]", "# title role removed", 1)
            ),
            Err(TokenError::Toml(_))
        ));
    }
    #[test]
    fn rust_and_css_use_the_same_semantic_motion_name() {
        let outputs = rendered(source());
        assert!(output(&outputs, "crates/tabula-design/src/generated.rs").contains("piece_move:"));
        assert!(output(&outputs, "apps/web/style/tokens.css")
            .contains("--sys-motion-piece-move-duration"));
    }
}
