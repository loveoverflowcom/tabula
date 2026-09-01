//! Mechanical enforcement of the no-raw-colors presentation policy. (doc 04 §8.1)

use std::path::{Path, PathBuf};

const ROOTS: [&str; 3] = ["apps", "games", "crates/tabula-presentation"];
const ALLOWED_GENERATED: [&str; 1] = ["apps/web/style/tokens.css"];

#[derive(Debug, thiserror::Error)]
pub enum ColorCheckError {
    #[error("walking {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawColorViolation {
    RustConstructor,
    RustStructLiteral,
    HexLiteral,
    CssFunction,
}

impl RawColorViolation {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::RustConstructor => "raw Color constructor; use tabula-design semantic tokens",
            Self::RustStructLiteral => {
                "raw Color struct literal; use tabula-design semantic tokens"
            }
            Self::HexLiteral => "raw hex colour literal; use tabula-design semantic tokens",
            Self::CssFunction => "raw CSS colour function; use tabula-design semantic tokens",
        }
    }
}

pub fn run() -> Result<bool, ColorCheckError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace child");
    let mut violations = Vec::new();
    for relative in ROOTS {
        walk(root, &root.join(relative), &mut violations)?;
    }
    if violations.is_empty() {
        println!("check-no-raw-colors: all presentation sources use semantic tokens");
        Ok(true)
    } else {
        for violation in &violations {
            eprintln!("{violation}");
        }
        Ok(false)
    }
}

fn walk(root: &Path, dir: &Path, violations: &mut Vec<String>) -> Result<(), ColorCheckError> {
    for entry in std::fs::read_dir(dir).map_err(|source| ColorCheckError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ColorCheckError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|source| ColorCheckError::Io {
                path: path.clone(),
                source,
            })?
            .is_dir()
        {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            walk(root, &path, violations)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED_GENERATED.contains(&relative.as_str()) {
            continue;
        }

        let is_rust = path.extension().and_then(|e| e.to_str()) == Some("rs");
        let is_css = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("css" | "scss")
        );

        if !is_rust && !is_css {
            continue;
        }

        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };

        for (line_idx, line) in contents.lines().enumerate() {
            let violation = if is_rust {
                classify_rust_line(line)
            } else {
                classify_css_line(line)
            };

            if let Some(kind) = violation {
                violations.push(format!("{relative}:{} {}", line_idx + 1, kind.message()));
            }
        }
    }
    Ok(())
}

/// Classifies whether a single line of Rust code contains a forbidden raw color literal or constructor.
#[must_use]
pub fn classify_rust_line(line: &str) -> Option<RawColorViolation> {
    let trimmed = line.trim();
    // Documentation and comments explain the policy and may legitimately show examples.
    if trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
    {
        return None;
    }

    // Strip trailing line comment if present.
    let code = if let Some((before, _)) = line.split_once("//") {
        before.trim()
    } else {
        trimmed
    };

    if code.is_empty() {
        return None;
    }

    // Check for raw Color constructors.
    if has_rust_color_constructor(code) {
        return Some(RawColorViolation::RustConstructor);
    }

    // Check for struct literal instantiation: Color { red: ..., green: ..., ... }
    if has_rust_color_struct_literal(code) {
        return Some(RawColorViolation::RustStructLiteral);
    }

    // Check for hex color literals (e.g. #abcdef in macros or expressions).
    if contains_hex_color_literal(code) {
        return Some(RawColorViolation::HexLiteral);
    }

    None
}

/// Classifies whether a single line of CSS / SCSS contains a forbidden raw color literal or function.
#[must_use]
pub fn classify_css_line(line: &str) -> Option<RawColorViolation> {
    let trimmed = line.trim();
    if trimmed.starts_with("/*") || trimmed.starts_with('*') || trimmed.starts_with("*/") {
        return None;
    }

    let code = strip_css_comments(trimmed);
    let code = code.trim();
    if code.is_empty() {
        return None;
    }

    if has_css_color_function(code) {
        return Some(RawColorViolation::CssFunction);
    }

    if contains_hex_color_literal(code) {
        return Some(RawColorViolation::HexLiteral);
    }

    None
}

fn has_rust_color_constructor(code: &str) -> bool {
    let compact = code.replace(' ', "");
    compact.contains("Color::rgb(")
        || compact.contains("Color::rgba(")
        || compact.contains("Color::new(")
        || compact.contains("Color::from_rgb(")
        || compact.contains("Color::from_rgba(")
        || compact.contains("Color::from_hex(")
        || compact.contains("Color::from_rgba_f32(")
        || compact.contains("Color::from_rgb_f32(")
}

fn has_rust_color_struct_literal(code: &str) -> bool {
    let compact = code.replace(' ', "");
    compact.contains("Color{red:")
        || compact.contains("Color{green:")
        || compact.contains("Color{blue:")
        || compact.contains("Color{alpha:")
        || compact.contains("Color{r:")
        || compact.contains("Color{g:")
        || compact.contains("Color{b:")
        || compact.contains("Color{a:")
}

fn has_css_color_function(code: &str) -> bool {
    let compact = code.replace(' ', "").to_ascii_lowercase();
    compact.contains("rgb(")
        || compact.contains("rgba(")
        || compact.contains("hsl(")
        || compact.contains("hsla(")
        || compact.contains("hwb(")
        || compact.contains("lab(")
        || compact.contains("lch(")
        || compact.contains("oklab(")
        || compact.contains("oklch(")
        || compact.contains("color(")
}

fn strip_css_comments(mut s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    while let Some(start) = s.find("/*") {
        result.push_str(&s[..start]);
        if let Some(end) = s[start + 2..].find("*/") {
            s = &s[start + 2 + end + 2..];
        } else {
            s = "";
            break;
        }
    }
    result.push_str(s);
    result
}

fn contains_hex_color_literal(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'#' {
            continue;
        }
        // If preceded by an alphanumeric char, `-`, or `_`, this is part of an identifier, anchor, or custom token.
        if index > 0
            && (bytes[index - 1].is_ascii_alphanumeric()
                || bytes[index - 1] == b'-'
                || bytes[index - 1] == b'_')
        {
            continue;
        }
        let hex_count = bytes[index + 1..]
            .iter()
            .take_while(|b| b.is_ascii_hexdigit())
            .count();
        if !matches!(hex_count, 3 | 4 | 6 | 8) {
            continue;
        }
        // Check following character: if it's alphanumeric, `-`, or `_`, it's an ID selector like `#button-container`.
        if let Some(&next_byte) = bytes.get(index + 1 + hex_count) {
            if next_byte.is_ascii_alphanumeric() || next_byte == b'-' || next_byte == b'_' {
                continue;
            }
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn violation_messages_are_non_empty_and_descriptive() {
        for violation in [
            RawColorViolation::RustConstructor,
            RawColorViolation::RustStructLiteral,
            RawColorViolation::HexLiteral,
            RawColorViolation::CssFunction,
        ] {
            let msg = violation.message();
            assert!(!msg.is_empty());
            assert!(msg.contains("tabula-design semantic tokens"));
        }
    }

    #[test]
    fn domain_color_enum_and_impl_are_allowed() {
        assert_eq!(classify_rust_line("pub enum Color {"), None);
        assert_eq!(classify_rust_line("enum Color {"), None);
        assert_eq!(classify_rust_line("    White,"), None);
        assert_eq!(classify_rust_line("    Black,"), None);
        assert_eq!(classify_rust_line("}"), None);
        assert_eq!(classify_rust_line("impl Color {"), None);
        assert_eq!(classify_rust_line("impl<T> Display for Color {"), None);
        assert_eq!(classify_rust_line("use chess::Color;"), None);
        assert_eq!(classify_rust_line("use tabula_design::Color;"), None);
        assert_eq!(classify_rust_line("fn side(color: Color) {}"), None);
        assert_eq!(classify_rust_line("struct ColorProfile;"), None);
        assert_eq!(classify_rust_line("let color_name = \"Color\";"), None);
        assert_eq!(classify_rust_line("pub color: Color,"), None);
        assert_eq!(
            classify_rust_line("let c: Color = theme.color.surface;"),
            None
        );
        assert_eq!(
            classify_rust_line("match color { Color::White => 0, Color::Black => 1 }"),
            None
        );
    }

    #[test]
    fn rust_raw_color_constructors_are_rejected() {
        assert_eq!(
            classify_rust_line("let c = Color::rgb(255, 255, 255);"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color :: rgba ( 255 , 0 , 0 , 128 ) ;"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::new(1.0, 0.0, 0.0, 1.0);"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::from_rgb(10, 20, 30);"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::from_rgba(10, 20, 30, 40);"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::from_hex(\"#fff\");"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::from_rgba_f32(1.0, 0.0, 0.0, 1.0);"),
            Some(RawColorViolation::RustConstructor)
        );
        assert_eq!(
            classify_rust_line("let c = Color::from_rgb_f32(1.0, 0.0, 0.0);"),
            Some(RawColorViolation::RustConstructor)
        );
    }

    #[test]
    fn rust_raw_color_struct_literals_are_rejected() {
        assert_eq!(
            classify_rust_line("Color { red: 255, green: 0, blue: 0, alpha: 255 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { green: 255 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { blue: 255 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { alpha: 255 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { g: 1.0 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { b: 1.0 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
        assert_eq!(
            classify_rust_line("Color { a: 1.0 }"),
            Some(RawColorViolation::RustStructLiteral)
        );
    }

    #[test]
    fn rust_hex_literals_in_code_are_rejected() {
        assert_eq!(
            classify_rust_line("let c = #abcdef;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_rust_line("let c = #fff;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_rust_line("#12345678"),
            Some(RawColorViolation::HexLiteral)
        );
    }

    #[test]
    fn css_hex_color_literals_are_rejected() {
        assert_eq!(
            classify_css_line("#fff"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_css_line("color: #fff;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_css_line("background: #abcdef;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_css_line("border-color: #ff000080;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_css_line("border-color: #1234;"),
            Some(RawColorViolation::HexLiteral)
        );
        assert_eq!(
            classify_css_line("color: #FFFFFF;"),
            Some(RawColorViolation::HexLiteral)
        );
    }

    #[test]
    fn css_id_selectors_and_tokens_are_allowed() {
        assert_eq!(classify_css_line("#app { display: flex; }"), None);
        assert_eq!(classify_css_line("#main { width: 100%; }"), None);
        assert_eq!(classify_css_line("#button-container { margin: 0; }"), None);
        assert_eq!(classify_css_line("#app_root { margin: 0; }"), None);
        assert_eq!(classify_css_line("#feed-btn { margin: 0; }"), None);
        assert_eq!(classify_css_line("#feed_btn { margin: 0; }"), None);
        assert_eq!(
            classify_css_line(".btn { background: var(--sys-color-primary); }"),
            None
        );
        assert_eq!(
            classify_css_line("border-radius: var(--sys-shape-button);"),
            None
        );
    }

    #[test]
    fn css_color_functions_are_rejected() {
        assert_eq!(
            classify_css_line("color: rgb(255 0 0);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: rgba(255 0 0 / 50%);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: hsl(0 100% 50%);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: hsla(0 100% 50% / 50%);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: hwb(12 50% 0%);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: lab(50% 40 59.5);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: lch(50% 70 300);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: oklab(0.6 0.1 0.1);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: oklch(0.6 0.25 150);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: color(display-p3 1 0 0);"),
            Some(RawColorViolation::CssFunction)
        );
        assert_eq!(
            classify_css_line("color: RGB(255, 0, 0);"),
            Some(RawColorViolation::CssFunction)
        );
    }

    #[test]
    fn comments_in_rust_and_css_are_exempt() {
        assert_eq!(classify_rust_line("// Color::rgb(1, 2, 3)"), None);
        assert_eq!(classify_rust_line("/// Color { red: 255 }"), None);
        assert_eq!(classify_rust_line("/* Color::new(1, 1, 1, 1) */"), None);
        assert_eq!(classify_rust_line(" * Color::rgb(1, 2, 3)"), None);
        assert_eq!(classify_rust_line("*/"), None);
        assert_eq!(
            classify_rust_line("let x = 42; // Color::rgb(1, 2, 3)"),
            None
        );

        assert_eq!(classify_css_line("/* color: #fff; */"), None);
        assert_eq!(classify_css_line("/* rgb(255, 0, 0) */"), None);
        assert_eq!(classify_css_line(" * hsl(0 100% 50%)"), None);
        assert_eq!(classify_css_line("*/"), None);
        assert_eq!(
            classify_css_line("/* --sys-color-primary: #7B4DFF */"),
            None
        );
        assert_eq!(
            classify_css_line("color: /* comment1 */ var(--sys-color-primary) /* comment2 */;"),
            None
        );
    }

    #[test]
    fn walk_detects_violations_and_respects_exemptions() {
        let temp_dir = tempfile::tempdir().expect("tempdir creation");
        let root = temp_dir.path();

        let app_dir = root.join("apps").join("web");
        std::fs::create_dir_all(&app_dir).expect("create app dir");
        let style_dir = app_dir.join("style");
        std::fs::create_dir_all(&style_dir).expect("create style dir");

        // Allowed generated file:
        let generated_css = style_dir.join("tokens.css");
        std::fs::write(&generated_css, ":root { --sys-color-primary: #7B4DFF; }").unwrap();

        // Custom css with violation:
        let custom_css = style_dir.join("custom.css");
        std::fs::write(&custom_css, ".btn { color: #fff; }").unwrap();

        // Rust file with domain Color (valid) and raw color (invalid):
        let domain_rs = app_dir.join("domain_types.rs");
        std::fs::write(&domain_rs, "pub enum Color { White, Black }\nimpl Color {}").unwrap();

        let raw_rs = app_dir.join("raw_colors.rs");
        std::fs::write(&raw_rs, "fn test() { let _ = Color::rgb(1, 2, 3); }").unwrap();

        let mut violations = Vec::new();
        walk(root, &root.join("apps"), &mut violations).expect("walk success");

        assert_eq!(violations.len(), 2);
        assert!(violations
            .iter()
            .any(|v| v.contains("custom.css:1 raw hex colour literal")));
        assert!(violations
            .iter()
            .any(|v| v.contains("raw_colors.rs:1 raw Color constructor")));
        assert!(!violations.iter().any(|v| v.contains("tokens.css")));
        assert!(!violations.iter().any(|v| v.contains("domain_types.rs")));
    }
}
