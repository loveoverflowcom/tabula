//! `cargo xtask check-no-game-ids` — the shell around
//! [`crate::game_ids_policy`].
//!
//! The set of game ids is derived from the `games/*` directory names rather
//! than hard-coded, so adding a game needs no edit to this tool.

use std::path::{Path, PathBuf};

use crate::game_ids_policy::scan_file;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "generated",
    "dist",
    "build",
];

/// Extensions worth scanning as text. Anything else (images, fonts, wasm,
/// binaries, lock files) is skipped outright.
const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "toml", "md", "txt", "json", "yml", "yaml", "html", "css", "js", "ts", "mjs", "cjs",
];

#[derive(Debug, thiserror::Error)]
pub enum CheckGameIdsError {
    #[error("running `cargo metadata`: {0}")]
    WorkspaceMetadata(#[source] cargo_metadata::Error),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn run() -> Result<bool, CheckGameIdsError> {
    let workspace_root = crate::workspace::root().map_err(CheckGameIdsError::WorkspaceMetadata)?;
    let game_ids = discover_game_ids(&workspace_root)?;

    let mut files_scanned = 0usize;
    let mut hits = Vec::new();

    walk(
        &workspace_root,
        &workspace_root,
        &mut |rel_path, abs_path| {
            if !is_scannable(rel_path) {
                return Ok(());
            }
            // Not valid UTF-8 text: treat as binary and skip.
            let Ok(contents) = std::fs::read_to_string(abs_path) else {
                return Ok(());
            };
            files_scanned += 1;
            hits.extend(scan_file(rel_path, &contents, &game_ids));
            Ok(())
        },
    )?;

    if hits.is_empty() {
        println!(
            "check-no-game-ids: {files_scanned} files scanned for {} game id(s) — all clear",
            game_ids.len()
        );
        Ok(true)
    } else {
        for hit in &hits {
            eprintln!("{hit}\n");
        }
        eprintln!(
            "check-no-game-ids: {} violation(s) across {files_scanned} files scanned",
            hits.len()
        );
        Ok(false)
    }
}

fn discover_game_ids(workspace_root: &Path) -> Result<Vec<String>, CheckGameIdsError> {
    let games_dir = workspace_root.join("games");
    let mut ids = Vec::new();
    let entries = std::fs::read_dir(&games_dir).map_err(|source| CheckGameIdsError::Io {
        path: games_dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CheckGameIdsError::Io {
            path: games_dir.clone(),
            source,
        })?;
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

fn is_scannable(rel_path: &str) -> bool {
    let file_name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    match file_name.rsplit_once('.') {
        Some((_, ext)) if ext.eq_ignore_ascii_case("lock") => false,
        Some((_, ext)) => TEXT_EXTENSIONS.contains(&ext),
        None => false,
    }
}

fn walk(
    workspace_root: &Path,
    dir: &Path,
    visit: &mut impl FnMut(&str, &Path) -> Result<(), CheckGameIdsError>,
) -> Result<(), CheckGameIdsError> {
    let entries = std::fs::read_dir(dir).map_err(|source| CheckGameIdsError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CheckGameIdsError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| CheckGameIdsError::Io {
            path: path.clone(),
            source,
        })?;

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(workspace_root, &path, visit)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(workspace_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            visit(&rel, &path)?;
        }
    }
    Ok(())
}
