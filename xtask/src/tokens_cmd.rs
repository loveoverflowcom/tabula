//! Token generation and freshness checking. (doc 04 §8.1)
//!
//! `TokenSource` is deliberately typed: every generated output derives from
//! the same parsed source instead of duplicating values in Rust or CSS.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("workspace root cannot be derived")]
    Root,
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("writing {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("tokens.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("scheme `{scheme}` is missing `{key}")]
    Missing { scheme: String, key: &'static str },
    #[error("scheme `{scheme}` has invalid hexadecimal colour `{value}`")]
    Color { scheme: String, value: String },
    #[error("scheme `{scheme}` requires exactly eight `{key}` colours")]
    ColorArray { scheme: String, key: &'static str },
    #[error("token `{key}` must be a finite fraction in 0..=1")]
    Fraction { key: &'static str },
    #[error("sys.space.scale must contain at least 9 values")]
    SpaceScale,
    #[error("rustfmt failed while formatting generated Rust")]
    Rustfmt,
}

const SCHEMES: [(&str, &str, &str); 4] = [
    ("light", "LIGHT", "ThemeKind::Light"),
    ("dark", "DARK", "ThemeKind::Dark"),
    (
        "hc-light",
        "HIGH_CONTRAST_LIGHT",
        "ThemeKind::HighContrastLight",
    ),
    (
        "hc-dark",
        "HIGH_CONTRAST_DARK",
        "ThemeKind::HighContrastDark",
    ),
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

#[derive(Debug, Deserialize, Serialize)]
struct TokenSource {
    meta: MetaSource,
    #[serde(rename = "ref")]
    reference: ReferenceSource,
    sys: SystemSource,
    schemes: BTreeMap<String, BTreeMap<String, toml::Value>>,
    comp: BTreeMap<String, toml::Value>,
}
#[derive(Debug, Deserialize, Serialize)]
struct MetaSource {
    identity: String,
    schemes: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
struct ReferenceSource {
    palette: BTreeMap<String, String>,
}
#[derive(Debug, Deserialize, Serialize)]
struct SystemSource {
    color: BTreeMap<String, String>,
    space: SpaceSource,
    shape: ShapeSource,
    #[serde(rename = "type")]
    typography: TypeSource,
    state: StateSource,
    motion: MotionSource,
    density: DensitySource,
    elevation: ElevationSource,
    focus: FocusSource,
}
#[derive(Debug, Deserialize, Serialize)]
struct SpaceSource {
    scale: Vec<u16>,
}
#[derive(Debug, Deserialize, Serialize)]
struct ShapeSource {
    scale: Vec<String>,
    semantic: Vec<String>,
    values: ShapeValues,
}
#[derive(Debug, Deserialize, Serialize)]
struct ShapeValues {
    card: f32,
    board: f32,
    button: f32,
    chip: f32,
}
#[derive(Debug, Deserialize, Serialize)]
struct TypeSource {
    roles: Vec<String>,
    sizes: Vec<String>,
    mono: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
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
struct MotionSource {
    instant: u16,
    short: u16,
    medium: u16,
    long: u16,
    xlong: u16,
    stagger: u16,
    spring: BTreeMap<String, SpringSource>,
    reduced: ReducedSource,
}
#[derive(Debug, Deserialize, Serialize)]
struct SpringSource {
    stiffness: f32,
    damping: f32,
    mass: f32,
}
#[derive(Debug, Deserialize, Serialize)]
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
struct DensitySource {
    scale: f32,
    #[serde(rename = "min-target")]
    min_target: f32,
}
#[derive(Debug, Deserialize, Serialize)]
struct ElevationSource {
    low: u8,
    medium: u8,
    high: u8,
}
#[derive(Debug, Deserialize, Serialize)]
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
    println!("gen-tokens: emitted Rust, CSS, and JSON from tokens.toml");
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
    let source: TokenSource = toml::from_str(text)?;
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
    if source.sys.space.scale.len() < 9 {
        return Err(TokenError::SpaceScale);
    }
    for (name, _, _) in SCHEMES {
        let scheme = source.schemes.get(name).ok_or(TokenError::Missing {
            scheme: name.into(),
            key: "scheme",
        })?;
        for key in COLOR_KEYS {
            parse_hex(name, color(scheme, name, key)?)?;
        }
        for key in ["team", "seat-marker"] {
            let values = color_array(scheme, name, key)?;
            if values.len() != 8 {
                return Err(TokenError::ColorArray {
                    scheme: name.into(),
                    key,
                });
            }
            for value in values {
                parse_hex(name, value)?;
            }
        }
    }
    for (key, value) in [
        ("sys.state.hover", source.sys.state.hover),
        ("sys.state.focus", source.sys.state.focus),
        ("sys.state.press", source.sys.state.press),
        ("sys.state.drag", source.sys.state.drag),
        (
            "sys.motion.reduced.duration-scale",
            source.sys.motion.reduced.duration_scale,
        ),
    ] {
        fraction(key, value)?;
    }
    for key in ["snappy", "standard", "weighty", "bouncy"] {
        if !source.sys.motion.spring.contains_key(key) {
            return Err(TokenError::Missing {
                scheme: "sys.motion.spring".into(),
                key,
            });
        }
    }
    if !COLOR_KEYS.contains(&source.sys.focus.ring_color.as_str()) {
        return Err(TokenError::Missing {
            scheme: "sys.focus".into(),
            key: "ring-color",
        });
    }
    Ok(())
}

fn rust(source: &TokenSource) -> String {
    let sys = &source.sys;
    let mut output = String::from("// @generated by `cargo xtask gen-tokens`; do not edit.\nuse crate::{Color, ColorTokens, Density, ElevationTokens, FocusTokens, MotionTokens, ReducedMotion, ShapeTokens, SpaceTokens, Spring, StateLayerTokens, Theme, ThemeKind};\n\n");
    output.push_str("const fn theme(kind: ThemeKind, color: ColorTokens) -> Theme {\n    Theme { kind, shape: ShapeTokens { card: ");
    write!(
        output,
        "{}, board: {}, button: {}, chip: {}",
        float(sys.shape.values.card),
        float(sys.shape.values.board),
        float(sys.shape.values.button),
        float(sys.shape.values.chip)
    )
    .expect("String write");
    output.push_str(" }, space: SpaceTokens { xs: ");
    write!(
        output,
        "{}, sm: {}, md: {}, lg: {}, xl: {}",
        sys.space.scale[2],
        sys.space.scale[3],
        sys.space.scale[5],
        sys.space.scale[7],
        sys.space.scale[8]
    )
    .expect("String write");
    write!(output, " }}, elevation: ElevationTokens {{ low: {}, medium: {}, high: {} }}, motion: MotionTokens {{ ", sys.elevation.low, sys.elevation.medium, sys.elevation.high).expect("String write");
    write!(
        output,
        "instant_ms: {}, short_ms: {}, medium_ms: {}, long_ms: {}, xlong_ms: {}, stagger_ms: {}, ",
        sys.motion.instant,
        sys.motion.short,
        sys.motion.medium,
        sys.motion.long,
        sys.motion.xlong,
        sys.motion.stagger
    )
    .expect("String write");
    for key in ["snappy", "standard", "weighty", "bouncy"] {
        let spring = &sys.motion.spring[key];
        write!(
            output,
            "spring_{key}: Spring {{ stiffness: {}, damping: {}, mass: {} }}, ",
            float(spring.stiffness),
            float(spring.damping),
            float(spring.mass)
        )
        .expect("String write");
    }
    write!(output, "reduced: ReducedMotion {{ duration_scale_percent: {}, prefer_fade: {}, disable_ambient: {}, keep_informative: {} }}", percent(sys.motion.reduced.duration_scale), sys.motion.reduced.prefer_fade, sys.motion.reduced.disable_ambient, sys.motion.reduced.keep_informative).expect("String write");
    write!(output, " }}, state: StateLayerTokens {{ hover: {}, focus: {}, press: {}, drag: {} }}, density: Density {{ scale: {}, min_target: {} }}, focus: FocusTokens {{ ring_width: {}, ring_color: color.{} }}, color }}\n}}\n", percent(sys.state.hover), percent(sys.state.focus), percent(sys.state.press), percent(sys.state.drag), float(sys.density.scale), float(sys.density.min_target), float(sys.focus.ring_width), sys.focus.ring_color.replace('-', "_")).expect("String write");
    for (name, constant, kind) in SCHEMES {
        let scheme = &source.schemes[name];
        writeln!(
            output,
            "pub const {constant}: Theme = theme({kind}, {});",
            color_tokens(scheme, name)
        )
        .expect("String write");
    }
    output
}
fn color_tokens(scheme: &BTreeMap<String, toml::Value>, name: &str) -> String {
    let mut result = String::from("ColorTokens {");
    for (field, key) in [
        ("surface", "surface"),
        ("surface_container", "container"),
        ("surface_container_high", "container-high"),
        ("on_surface", "on-surface"),
        ("on_surface_variant", "on-surface-variant"),
        ("outline", "outline"),
        ("primary", "primary"),
        ("on_primary", "on-primary"),
        ("success", "success"),
        ("on_success", "on-success"),
        ("danger", "danger"),
        ("on_danger", "on-danger"),
        ("turn_active", "turn-active"),
        ("turn_waiting", "turn-waiting"),
        ("legal_target", "legal-target"),
        ("illegal_target", "illegal-target"),
        ("selected", "selected"),
        ("last_action", "last-action"),
        ("threat", "threat"),
        ("hidden", "hidden"),
    ] {
        write!(
            result,
            " {field}: {},",
            color_rust(color(scheme, name, key).expect("validated scheme"))
        )
        .expect("String write");
    }
    for (field, key) in [("team", "team"), ("seat_marker", "seat-marker")] {
        let values = color_array(scheme, name, key).expect("validated scheme");
        write!(
            result,
            " {field}: [{}],",
            values
                .iter()
                .map(|value| color_rust(value))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .expect("String write");
    }
    result.push_str(" }");
    result
}
fn css(source: &TokenSource) -> String {
    let mut output = String::from(
        "/* @generated by `cargo xtask gen-tokens` from tokens.toml; do not edit. */\n",
    );
    for (index, (name, _, _)) in SCHEMES.iter().enumerate() {
        let selector = match *name {
            "light" => ":root",
            "dark" => ":root[data-theme=\"dark\"]",
            "hc-light" => ":root[data-theme=\"hc-light\"]",
            _ => ":root[data-theme=\"hc-dark\"]",
        };
        writeln!(output, "{selector} {{").expect("String write");
        let scheme = &source.schemes[*name];
        for key in COLOR_KEYS {
            writeln!(
                output,
                "  --sys-color-{key}: {};",
                color(scheme, name, key).expect("validated scheme")
            )
            .expect("String write");
        }
        writeln!(output, "  --sys-space-md: {}px;\n  --sys-shape-button: {}px;\n  --sys-motion-medium: {}ms;\n  --sys-state-hover: {};\n  --sys-density-min-target: {}px;\n}}", source.sys.space.scale[5], source.sys.shape.values.button, source.sys.motion.medium, source.sys.state.hover, source.sys.density.min_target).expect("String write");
        if index + 1 != SCHEMES.len() {
            output.push('\n');
        }
    }
    output
}
fn json(source: &TokenSource) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(source).expect("token source is serializable")
    )
}
fn color<'a>(
    scheme: &'a BTreeMap<String, toml::Value>,
    name: &str,
    key: &'static str,
) -> Result<&'a str, TokenError> {
    scheme
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or(TokenError::Missing {
            scheme: name.into(),
            key,
        })
}
fn color_array<'a>(
    scheme: &'a BTreeMap<String, toml::Value>,
    name: &str,
    key: &'static str,
) -> Result<Vec<&'a str>, TokenError> {
    scheme
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or(TokenError::Missing {
            scheme: name.into(),
            key,
        })?
        .iter()
        .map(|value| {
            value.as_str().ok_or(TokenError::Missing {
                scheme: name.into(),
                key,
            })
        })
        .collect()
}
fn fraction(key: &'static str, value: f64) -> Result<(), TokenError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(TokenError::Fraction { key })
    }
}
// `validate` establishes this input is finite and in 0..=1 before generation.
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
    let (r, g, b) = parse_hex("generated", value).expect("validated colour");
    format!("Color::rgb({r}, {g}, {b})")
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
    #[test]
    fn motion_medium_changes_all_generated_outputs() {
        let source = include_str!("../../tokens.toml");
        let changed = source.replacen("medium  = 280", "medium  = 320", 1);
        let before = rendered(source);
        let after = rendered(&changed);
        for path in [
            "crates/tabula-design/src/generated.rs",
            "apps/web/style/tokens.css",
            "docs/ui/tokens.json",
        ] {
            assert_ne!(output(&before, path), output(&after, path), "{path}");
        }
    }
    #[test]
    fn state_hover_changes_all_generated_outputs() {
        let source = include_str!("../../tokens.toml");
        let changed = source.replacen("hover              = 0.08", "hover              = 0.10", 1);
        let before = rendered(source);
        let after = rendered(&changed);
        for path in [
            "crates/tabula-design/src/generated.rs",
            "apps/web/style/tokens.css",
            "docs/ui/tokens.json",
        ] {
            assert_ne!(output(&before, path), output(&after, path), "{path}");
        }
    }
    #[test]
    fn density_min_target_changes_all_generated_outputs() {
        let source = include_str!("../../tokens.toml");
        let changed = source.replacen("min-target = 44.0", "min-target = 48.0", 1);
        let before = rendered(source);
        let after = rendered(&changed);
        for path in [
            "crates/tabula-design/src/generated.rs",
            "apps/web/style/tokens.css",
            "docs/ui/tokens.json",
        ] {
            assert_ne!(output(&before, path), output(&after, path), "{path}");
        }
    }
}
