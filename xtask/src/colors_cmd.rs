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

        let file_violations = if is_rust {
            scan_rust_source(&contents)
        } else {
            scan_css_source(&contents)
        };

        for (line_number, kind) in file_violations {
            violations.push(format!("{relative}:{line_number} {}", kind.message()));
        }
    }
    Ok(())
}

fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn line_number_at_offset(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

enum RustLexerState {
    Normal,
    LineComment,
    BlockComment(usize),
    StringLiteral,
    RawStringLiteral(usize),
}

enum CssLexerState {
    Normal,
    BlockComment,
    DoubleQuoteString,
    SingleQuoteString,
}

/// Sanitizes Rust source code by replacing comments and string/char literals with spaces,
/// preserving exact character offsets and line numbers.
#[must_use]
pub fn sanitize_rust_source(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut state = RustLexerState::Normal;

    while i < n {
        let ch = chars[i];
        match state {
            RustLexerState::Normal => {
                step_rust_normal(&chars, n, &mut i, &mut out, &mut state);
            }
            RustLexerState::LineComment => {
                if ch == '\n' {
                    out.push('\n');
                    state = RustLexerState::Normal;
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            RustLexerState::BlockComment(depth) => {
                step_rust_block_comment(ch, &chars, n, &mut i, &mut out, &mut state, depth);
            }
            RustLexerState::StringLiteral => {
                step_rust_string_literal(ch, &chars, n, &mut i, &mut out, &mut state);
            }
            RustLexerState::RawStringLiteral(hashes) => {
                step_rust_raw_string(ch, &chars, n, &mut i, &mut out, &mut state, hashes);
            }
        }
    }
    out
}

fn step_rust_normal(
    chars: &[char],
    n: usize,
    i: &mut usize,
    out: &mut String,
    state: &mut RustLexerState,
) {
    let ch = chars[*i];
    if ch == '/' && *i + 1 < n && chars[*i + 1] == '/' {
        out.push(' ');
        out.push(' ');
        *i += 2;
        *state = RustLexerState::LineComment;
    } else if ch == '/' && *i + 1 < n && chars[*i + 1] == '*' {
        out.push(' ');
        out.push(' ');
        *i += 2;
        *state = RustLexerState::BlockComment(1);
    } else if ch == '"' {
        out.push(' ');
        *i += 1;
        *state = RustLexerState::StringLiteral;
    } else if ch == 'r' && (*i + 1 < n && (chars[*i + 1] == '"' || chars[*i + 1] == '#')) {
        let mut hashes = 0;
        let mut j = *i + 1;
        while j < n && chars[j] == '#' {
            hashes += 1;
            j += 1;
        }
        if j < n && chars[j] == '"' {
            for _ in *i..=j {
                out.push(' ');
            }
            *i = j + 1;
            *state = RustLexerState::RawStringLiteral(hashes);
        } else {
            out.push(ch);
            *i += 1;
        }
    } else if ch == '\'' {
        step_rust_char_literal(chars, n, i, out);
    } else {
        out.push(ch);
        *i += 1;
    }
}

fn step_rust_char_literal(chars: &[char], n: usize, i: &mut usize, out: &mut String) {
    if *i + 2 < n && chars[*i + 1] != '\\' && chars[*i + 2] == '\'' {
        out.push(' ');
        out.push(' ');
        out.push(' ');
        *i += 3;
    } else if *i + 3 < n && chars[*i + 1] == '\\' && chars[*i + 3] == '\'' {
        out.push(' ');
        out.push(' ');
        out.push(' ');
        out.push(' ');
        *i += 4;
    } else {
        out.push(chars[*i]);
        *i += 1;
    }
}

fn step_rust_block_comment(
    ch: char,
    chars: &[char],
    n: usize,
    i: &mut usize,
    out: &mut String,
    state: &mut RustLexerState,
    depth: usize,
) {
    if ch == '/' && *i + 1 < n && chars[*i + 1] == '*' {
        out.push(' ');
        out.push(' ');
        *i += 2;
        *state = RustLexerState::BlockComment(depth + 1);
    } else if ch == '*' && *i + 1 < n && chars[*i + 1] == '/' {
        out.push(' ');
        out.push(' ');
        *i += 2;
        if depth <= 1 {
            *state = RustLexerState::Normal;
        } else {
            *state = RustLexerState::BlockComment(depth - 1);
        }
    } else {
        if ch == '\n' {
            out.push('\n');
        } else {
            out.push(' ');
        }
        *i += 1;
    }
}

fn step_rust_string_literal(
    ch: char,
    _chars: &[char],
    n: usize,
    i: &mut usize,
    out: &mut String,
    state: &mut RustLexerState,
) {
    if ch == '\\' && *i + 1 < n {
        out.push(' ');
        out.push(' ');
        *i += 2;
    } else if ch == '"' {
        out.push(' ');
        *i += 1;
        *state = RustLexerState::Normal;
    } else {
        if ch == '\n' {
            out.push('\n');
        } else {
            out.push(' ');
        }
        *i += 1;
    }
}

fn step_rust_raw_string(
    ch: char,
    chars: &[char],
    n: usize,
    i: &mut usize,
    out: &mut String,
    state: &mut RustLexerState,
    hashes: usize,
) {
    if ch == '"' {
        let mut match_hashes = true;
        if *i + hashes < n {
            for k in 0..hashes {
                if chars[*i + 1 + k] != '#' {
                    match_hashes = false;
                    break;
                }
            }
        } else {
            match_hashes = false;
        }
        if match_hashes {
            for _ in 0..=hashes {
                out.push(' ');
            }
            *i += 1 + hashes;
            *state = RustLexerState::Normal;
        } else {
            out.push(' ');
            *i += 1;
        }
    } else {
        if ch == '\n' {
            out.push('\n');
        } else {
            out.push(' ');
        }
        *i += 1;
    }
}

/// Sanitizes CSS/SCSS source code by replacing comments and strings with spaces,
/// preserving exact character offsets and line numbers.
#[must_use]
pub fn sanitize_css_source(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut state = CssLexerState::Normal;

    while i < n {
        let ch = chars[i];
        match state {
            CssLexerState::Normal => {
                if ch == '/' && i + 1 < n && chars[i + 1] == '*' {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    state = CssLexerState::BlockComment;
                } else if ch == '"' {
                    out.push(' ');
                    i += 1;
                    state = CssLexerState::DoubleQuoteString;
                } else if ch == '\'' {
                    out.push(' ');
                    i += 1;
                    state = CssLexerState::SingleQuoteString;
                } else {
                    out.push(ch);
                    i += 1;
                }
            }
            CssLexerState::BlockComment => {
                if ch == '*' && i + 1 < n && chars[i + 1] == '/' {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    state = CssLexerState::Normal;
                } else {
                    if ch == '\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            CssLexerState::DoubleQuoteString => {
                if ch == '\\' && i + 1 < n {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if ch == '"' {
                    out.push(' ');
                    i += 1;
                    state = CssLexerState::Normal;
                } else {
                    if ch == '\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            CssLexerState::SingleQuoteString => {
                if ch == '\\' && i + 1 < n {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if ch == '\'' {
                    out.push(' ');
                    i += 1;
                    state = CssLexerState::Normal;
                } else {
                    if ch == '\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
        }
    }
    out
}

/// Scans a full Rust source file and returns all violations with their 1-indexed line numbers.
#[must_use]
pub fn scan_rust_source(source: &str) -> Vec<(usize, RawColorViolation)> {
    let sanitized = sanitize_rust_source(source);
    let line_starts = compute_line_starts(source);
    let bytes = sanitized.as_bytes();
    let n = bytes.len();
    let mut violations = Vec::new();

    let mut i = 0;
    while i < n {
        if bytes[i] == b'C'
            && i + 5 <= n
            && &bytes[i..i + 5] == b"Color"
            && (i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_'))
            && (i + 5 == n || (!bytes[i + 5].is_ascii_alphanumeric() && bytes[i + 5] != b'_'))
        {
            scan_rust_color_ident(&sanitized, &line_starts, i, &mut violations);
        } else if bytes[i] == b'#' {
            scan_rust_hex_literal(&sanitized, &line_starts, i, &mut violations);
        }
        i += 1;
    }

    violations
}

fn scan_rust_color_ident(
    sanitized: &str,
    line_starts: &[usize],
    i: usize,
    violations: &mut Vec<(usize, RawColorViolation)>,
) {
    let bytes = sanitized.as_bytes();
    let n = bytes.len();

    // 1. Check for constructors: Color :: <method>
    let mut j = i + 5;
    while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j + 2 <= n && &bytes[j..j + 2] == b"::" {
        j += 2;
        while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        let method_start = j;
        while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
        }
        let method = &sanitized[method_start..j];
        if matches!(
            method,
            "rgb"
                | "rgba"
                | "new"
                | "from_rgb"
                | "from_rgba"
                | "from_hex"
                | "from_rgba_f32"
                | "from_rgb_f32"
        ) {
            let line = line_number_at_offset(line_starts, i);
            violations.push((line, RawColorViolation::RustConstructor));
        }
    }

    // 2. Check for struct literal instantiation: Color { ... }
    let mut prev_word = "";
    let mut k = i;
    while k > 0 && bytes[k - 1].is_ascii_whitespace() {
        k -= 1;
    }
    let word_end = k;
    while k > 0 && (bytes[k - 1].is_ascii_alphanumeric() || bytes[k - 1] == b'_') {
        k -= 1;
    }
    if word_end > k {
        prev_word = &sanitized[k..word_end];
    }

    if !matches!(
        prev_word,
        "enum" | "struct" | "impl" | "trait" | "for" | "type"
    ) {
        let mut j = i + 5;
        while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < n && bytes[j] == b'{' {
            let brace_start = j;
            let mut depth = 1;
            let mut brace_end = j + 1;
            while brace_end < n && depth > 0 {
                if bytes[brace_end] == b'{' {
                    depth += 1;
                } else if bytes[brace_end] == b'}' {
                    depth -= 1;
                }
                brace_end += 1;
            }
            let body = &sanitized[brace_start + 1..brace_end - 1];
            if has_rust_color_struct_fields(body) {
                let line = line_number_at_offset(line_starts, i);
                violations.push((line, RawColorViolation::RustStructLiteral));
            }
        }
    }
}

fn scan_rust_hex_literal(
    sanitized: &str,
    line_starts: &[usize],
    i: usize,
    violations: &mut Vec<(usize, RawColorViolation)>,
) {
    let bytes = sanitized.as_bytes();
    let n = bytes.len();

    if i == 0
        || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'-' && bytes[i - 1] != b'_')
    {
        let mut j = i + 1;
        if j < n && (bytes[j] == b'[' || (bytes[j] == b'!' && j + 1 < n && bytes[j + 1] == b'[')) {
            return;
        }
        let hex_start = j;
        while j < n && bytes[j].is_ascii_hexdigit() {
            j += 1;
        }
        let hex_count = j - hex_start;
        if matches!(hex_count, 3 | 4 | 6 | 8)
            && (j == n
                || (!bytes[j].is_ascii_alphanumeric() && bytes[j] != b'-' && bytes[j] != b'_'))
        {
            let line = line_number_at_offset(line_starts, i);
            violations.push((line, RawColorViolation::HexLiteral));
        }
    }
}

fn has_rust_color_struct_fields(body: &str) -> bool {
    let bytes = body.as_bytes();
    let n = bytes.len();
    let fields = ["red", "green", "blue", "alpha", "r", "g", "b", "a"];
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_alphabetic()
            && (i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_'))
        {
            let start = i;
            let mut j = i;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &body[start..j];
            if fields.contains(&word) {
                let mut k = j;
                while k < n && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < n && bytes[k] == b':' {
                    return true;
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
}

/// Scans a full CSS/SCSS source file and returns all violations with their 1-indexed line numbers.
#[must_use]
pub fn scan_css_source(source: &str) -> Vec<(usize, RawColorViolation)> {
    let sanitized = sanitize_css_source(source);
    let line_starts = compute_line_starts(source);
    let bytes = sanitized.as_bytes();
    let n = bytes.len();
    let mut violations = Vec::new();

    let functions = [
        "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch", "color",
    ];

    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_alphabetic()
            && (i == 0
                || (!bytes[i - 1].is_ascii_alphanumeric()
                    && bytes[i - 1] != b'-'
                    && bytes[i - 1] != b'_'))
        {
            let start = i;
            let mut j = i;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            let word = sanitized[start..j].to_ascii_lowercase();
            if functions.contains(&word.as_str()) {
                let mut k = j;
                while k < n && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < n && bytes[k] == b'(' {
                    let line = line_number_at_offset(&line_starts, start);
                    violations.push((line, RawColorViolation::CssFunction));
                }
            }
            i = j;
            continue;
        } else if bytes[i] == b'#' {
            let hash_offset = i;
            if i == 0
                || (!bytes[i - 1].is_ascii_alphanumeric()
                    && bytes[i - 1] != b'-'
                    && bytes[i - 1] != b'_')
            {
                let mut j = i + 1;
                let hex_start = j;
                while j < n && bytes[j].is_ascii_hexdigit() {
                    j += 1;
                }
                let hex_count = j - hex_start;
                if matches!(hex_count, 3 | 4 | 6 | 8)
                    && (j == n
                        || (!bytes[j].is_ascii_alphanumeric()
                            && bytes[j] != b'-'
                            && bytes[j] != b'_'))
                {
                    let line = line_number_at_offset(&line_starts, hash_offset);
                    violations.push((line, RawColorViolation::HexLiteral));
                }
            }
        }
        i += 1;
    }

    violations
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
        let code = concat!(
            "pub enum Color {\n",
            "    White,\n",
            "    Black,\n",
            "}\n",
            "impl Color {\n",
            "    pub const fn is_white(self) -> bool { true }\n",
            "}\n",
            "impl<T> Display for Color {\n",
            "    fn fmt(&self, f: &mut Formatter<'_>) -> Result {}\n",
            "}\n",
            "use chess::Color;\n",
            "use tabula_design::Color;\n",
            "fn side(color: Color) {}\n",
            "struct ColorProfile;\n",
            "let color_name = \"Color\";\n",
            "pub color: Color,\n",
            "let c: Color = theme.color.surface;\n",
            "match color {\n",
            "    Color::White => 0,\n",
            "    Color::Black => 1,\n",
            "}\n"
        );
        assert_eq!(scan_rust_source(code), vec![]);
    }

    #[test]
    fn multiline_rust_struct_literals_are_rejected() {
        let code = concat!(
            "fn build_color() {\n",
            "    let color = Color {\n",
            "        r: 1.0,\n",
            "        g: 0.0,\n",
            "        b: 0.0,\n",
            "        a: 1.0,\n",
            "    };\n",
            "}\n"
        );
        let violations = scan_rust_source(code);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0], (2, RawColorViolation::RustStructLiteral));
    }

    #[test]
    fn single_line_rust_struct_literals_are_rejected() {
        assert_eq!(
            scan_rust_source("let c = Color { red: 255, green: 0, blue: 0, alpha: 255 };"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("let c = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { green: 255 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { blue: 255 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { alpha: 255 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { g: 1.0 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { b: 1.0 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
        assert_eq!(
            scan_rust_source("Color { a: 1.0 }"),
            vec![(1, RawColorViolation::RustStructLiteral)]
        );
    }

    #[test]
    fn rust_raw_color_constructors_are_rejected() {
        assert_eq!(
            scan_rust_source("let c = Color::rgb(255, 255, 255);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color :: rgba ( 255 , 0 , 0 , 128 ) ;"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::new(1.0, 0.0, 0.0, 1.0);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::from_rgb(10, 20, 30);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::from_rgba(10, 20, 30, 40);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::from_hex(\"#fff\");"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::from_rgba_f32(1.0, 0.0, 0.0, 1.0);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
        assert_eq!(
            scan_rust_source("let c = Color::from_rgb_f32(1.0, 0.0, 0.0);"),
            vec![(1, RawColorViolation::RustConstructor)]
        );
    }

    #[test]
    fn multiline_block_comments_in_rust_and_css_are_exempt() {
        let rust_code = concat!(
            "/*\n",
            "let c = Color::rgb(1, 2, 3);\n",
            "let color = Color {\n",
            "    r: 1.0,\n",
            "    g: 0.0,\n",
            "};\n",
            "*/\n",
            "// let x = Color::rgb(1, 2, 3);\n"
        );
        assert_eq!(scan_rust_source(rust_code), vec![]);

        let css_code = concat!(
            "/*\n",
            "color: #fff;\n",
            "background: rgb(255 0 0);\n",
            "*/\n",
            "/* --sys-color-primary: #7B4DFF */\n"
        );
        assert_eq!(scan_css_source(css_code), vec![]);
    }

    #[test]
    fn string_literals_in_rust_and_css_are_exempt() {
        let rust_code = concat!(
            "let example = \"Color::rgb(1, 2, 3)\";\n",
            "let raw_example = r#\"Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }\"#;\n",
            "let hex_str = \"#ffffff\";\n"
        );
        assert_eq!(scan_rust_source(rust_code), vec![]);

        let css_code = concat!(
            ".badge::after {\n",
            "    content: \"#ffffff\";\n",
            "}\n",
            ".icon {\n",
            "    background-image: url(\"rgb(1,2,3).png\");\n",
            "}\n"
        );
        assert_eq!(scan_css_source(css_code), vec![]);
    }

    #[test]
    fn rust_hex_literals_in_code_are_rejected() {
        assert_eq!(
            scan_rust_source("let c = #abcdef;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_rust_source("let c = #fff;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_rust_source("#12345678"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        // Attributes must not be rejected:
        assert_eq!(
            scan_rust_source("#[derive(Debug)]\n#![forbid(unsafe_code)]"),
            vec![]
        );
    }

    #[test]
    fn css_hex_color_literals_are_rejected() {
        assert_eq!(
            scan_css_source("#fff"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_css_source("color: #fff;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_css_source("background: #abcdef;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_css_source("border-color: #ff000080;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_css_source("border-color: #1234;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
        assert_eq!(
            scan_css_source("color: #FFFFFF;"),
            vec![(1, RawColorViolation::HexLiteral)]
        );
    }

    #[test]
    fn css_id_selectors_and_tokens_are_allowed() {
        assert_eq!(scan_css_source("#app { display: flex; }"), vec![]);
        assert_eq!(scan_css_source("#main { width: 100%; }"), vec![]);
        assert_eq!(scan_css_source("#button-container { margin: 0; }"), vec![]);
        assert_eq!(scan_css_source("#app_root { margin: 0; }"), vec![]);
        assert_eq!(scan_css_source("#feed-btn { margin: 0; }"), vec![]);
        assert_eq!(scan_css_source("#feed_btn { margin: 0; }"), vec![]);
        assert_eq!(
            scan_css_source(".btn { background: var(--sys-color-primary); }"),
            vec![]
        );
        assert_eq!(
            scan_css_source("border-radius: var(--sys-shape-button);"),
            vec![]
        );
    }

    #[test]
    fn css_color_functions_are_rejected() {
        assert_eq!(
            scan_css_source("color: rgb(255 0 0);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: rgba(255 0 0 / 50%);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: hsl(0 100% 50%);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: hsla(0 100% 50% / 50%);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: hwb(12 50% 0%);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: lab(50% 40 59.5);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: lch(50% 70 300);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: oklab(0.6 0.1 0.1);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: oklch(0.6 0.25 150);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: color(display-p3 1 0 0);"),
            vec![(1, RawColorViolation::CssFunction)]
        );
        assert_eq!(
            scan_css_source("color: RGB(255, 0, 0);"),
            vec![(1, RawColorViolation::CssFunction)]
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

        // Rust file with domain Color (valid) and multiline raw color (invalid):
        let domain_rs = app_dir.join("domain_types.rs");
        std::fs::write(
            &domain_rs,
            "pub enum Color { White, Black }\nimpl Color {}\nlet s = \"#fff\";",
        )
        .unwrap();

        let raw_rs = app_dir.join("raw_colors.rs");
        std::fs::write(
            &raw_rs,
            "fn test() {\n    let _ = Color {\n        r: 1.0,\n        g: 0.0,\n        b: 0.0,\n        a: 1.0,\n    };\n}",
        )
        .unwrap();

        let mut violations = Vec::new();
        walk(root, &root.join("apps"), &mut violations).expect("walk success");

        assert_eq!(violations.len(), 2);
        assert!(violations
            .iter()
            .any(|v| v.contains("custom.css:1 raw hex colour literal")));
        assert!(violations
            .iter()
            .any(|v| v.contains("raw_colors.rs:2 raw Color struct literal")));
        assert!(!violations.iter().any(|v| v.contains("tokens.css")));
        assert!(!violations.iter().any(|v| v.contains("domain_types.rs")));
    }
}
