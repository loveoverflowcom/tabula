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
        if ALLOWED_GENERATED.contains(&relative.as_str())
            || !matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("rs" | "css" | "scss")
            )
        {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (line, source) in contents.lines().enumerate() {
            if has_raw_color(source) {
                violations.push(format!(
                    "{relative}:{} raw colour literal; use tabula-design semantic tokens",
                    line + 1
                ));
            }
        }
    }
    Ok(())
}

fn has_raw_color(line: &str) -> bool {
    let trimmed = line.trim_start();
    // Documentation and comments explain the policy and may legitimately show
    // examples. This enforcement intentionally scans source, not prose.
    if trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
    {
        return false;
    }
    let compact = line.replace(' ', "");
    compact.contains("Color::rgb(")
        || compact.contains("Color::rgba(")
        || compact.contains("Color{")
        || compact.to_ascii_lowercase().contains("rgb(")
        || contains_hex_literal(line)
}

fn contains_hex_literal(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'#' {
            continue;
        }
        let count = bytes[index + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        if matches!(count, 3 | 4 | 6 | 8)
            && !bytes
                .get(index + 1 + count)
                .is_some_and(u8::is_ascii_hexdigit)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_bypass_forms_but_not_commentary() {
        assert!(has_raw_color("let c = #abcdef;"));
        assert!(has_raw_color("color: #fff;"));
        assert!(has_raw_color("color: #ff000080;"));
        assert!(has_raw_color("Color::rgb(1, 2, 3)"));
        assert!(has_raw_color(
            "Color { red: 255, green: 0, blue: 0, alpha: 255 }"
        ));
        assert!(has_raw_color("color: rgb(255 0 0);"));
        assert!(!has_raw_color("// Color { red: 255 }"));
        assert!(!has_raw_color("use semantic tokens"));
    }
}
