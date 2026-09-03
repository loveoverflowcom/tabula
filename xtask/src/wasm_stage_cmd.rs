//! Deterministic staging of the WebAssembly browser host and gameplay bundle.
//!
//! Stages the checked-in HTML host, pinned Macroquad JS bootstrap, and compiled
//! `wasm-release` binary into a self-contained distribution directory (doc 01 §1.4).

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum WasmStageError {
    #[error("failed to resolve workspace root: {0}")]
    WorkspaceRoot(#[from] cargo_metadata::Error),

    #[error("expected WASM artifact is missing at {0}\nRun 'cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release' first.")]
    MissingWasmArtifact(PathBuf),

    #[error("expected host file is missing at {0}")]
    MissingHostFile(PathBuf),

    #[error("expected bootstrap JS is missing at {0}")]
    MissingBootstrapFile(PathBuf),

    #[error("HTML host validation failed in {path}: {reason}")]
    InvalidHostHtml { path: PathBuf, reason: String },

    #[error("I/O error while staging from {src} to {dst}: {source}")]
    Io {
        src: PathBuf,
        dst: PathBuf,
        source: std::io::Error,
    },

    #[error("directory creation failed for {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("staged file {0} is empty")]
    EmptyStagedFile(PathBuf),

    #[error("unexpected argument for stage-wasm-game: {0}")]
    UnexpectedArgument(String),
}

/// Report summarizing successfully staged artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmStageReport {
    pub out_dir: PathBuf,
    pub html_size: u64,
    pub js_size: u64,
    pub wasm_size: u64,
}

pub fn run(args: &[String]) -> Result<WasmStageReport, WasmStageError> {
    if let Some(argument) = args.first() {
        return Err(WasmStageError::UnexpectedArgument(argument.clone()));
    }

    let root = crate::workspace::root()?;
    let web_src_dir = root.join("apps").join("game-client").join("web");
    let wasm_src = resolve_wasm_source(&root)?;
    let out_dir = root.join("target").join("tabula-web-game");

    let report = stage_bundle(&web_src_dir, &wasm_src, &out_dir)?;

    println!(
        "stage-wasm-game: staged browser host into {}\n  - index.html ({} bytes)\n  - mq_js_bundle.js ({} bytes)\n  - tabula-game-client.wasm ({} bytes)",
        report.out_dir.display(),
        report.html_size,
        report.js_size,
        report.wasm_size
    );

    Ok(report)
}

fn resolve_wasm_source(root: &Path) -> Result<PathBuf, WasmStageError> {
    let wasm_dir = root
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("wasm-release");

    let candidates = [
        wasm_dir.join("tabula-game-client.wasm"),
        wasm_dir.join("tabula_game_client.wasm"),
    ];

    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    Err(WasmStageError::MissingWasmArtifact(candidates[0].clone()))
}

/// Stages the web host, JS bootstrap, and WASM binary into `out_dir`.
pub fn stage_bundle(
    web_src_dir: &Path,
    wasm_src: &Path,
    out_dir: &Path,
) -> Result<WasmStageReport, WasmStageError> {
    let html_src = web_src_dir.join("index.html");
    let js_src = web_src_dir.join("mq_js_bundle.js");

    // The destination is workspace-owned output. Invalidate it before every
    // attempt so a failed current stage cannot leave a previous bundle to be
    // served accidentally.
    remove_existing_destination(out_dir)?;

    if !wasm_src.is_file() {
        return Err(WasmStageError::MissingWasmArtifact(wasm_src.to_path_buf()));
    }
    if !html_src.is_file() {
        return Err(WasmStageError::MissingHostFile(html_src));
    }
    if !js_src.is_file() {
        return Err(WasmStageError::MissingBootstrapFile(js_src));
    }

    validate_host_html(&html_src)?;

    let parent = out_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| WasmStageError::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    let staging_dir = tempfile::Builder::new()
        .prefix(".tabula-web-game-")
        .tempdir_in(parent)
        .map_err(|source| WasmStageError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;

    let html_dst = staging_dir.path().join("index.html");
    let js_dst = staging_dir.path().join("mq_js_bundle.js");
    let wasm_dst = staging_dir.path().join("tabula-game-client.wasm");

    copy_file(&html_src, &html_dst)?;
    copy_file(&js_src, &js_dst)?;
    copy_file(wasm_src, &wasm_dst)?;

    validate_host_html(&html_dst)?;
    let html_size = file_size(&html_dst)?;
    let js_size = file_size(&js_dst)?;
    let wasm_size = file_size(&wasm_dst)?;

    std::fs::rename(staging_dir.path(), out_dir).map_err(|source| WasmStageError::Io {
        src: staging_dir.path().to_path_buf(),
        dst: out_dir.to_path_buf(),
        source,
    })?;

    Ok(WasmStageReport {
        out_dir: out_dir.to_path_buf(),
        html_size,
        js_size,
        wasm_size,
    })
}

fn remove_existing_destination(out_dir: &Path) -> Result<(), WasmStageError> {
    match std::fs::symlink_metadata(out_dir) {
        Ok(_) => std::fs::remove_dir_all(out_dir).map_err(|source| WasmStageError::Io {
            src: out_dir.to_path_buf(),
            dst: out_dir.to_path_buf(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(WasmStageError::Io {
            src: out_dir.to_path_buf(),
            dst: out_dir.to_path_buf(),
            source,
        }),
    }
}

fn validate_host_html(path: &Path) -> Result<(), WasmStageError> {
    let content = std::fs::read_to_string(path).map_err(|source| WasmStageError::Io {
        src: path.to_path_buf(),
        dst: path.to_path_buf(),
        source,
    })?;

    if !content.contains("id=\"glcanvas\"") && !content.contains("id='glcanvas'") {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing canvas with id \"glcanvas\"".into(),
        });
    }

    if !content.contains("mq_js_bundle.js") {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing script reference to \"mq_js_bundle.js\"".into(),
        });
    }

    if !content.contains("tabula-game-client.wasm") {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing load reference to \"tabula-game-client.wasm\"".into(),
        });
    }

    // Must not load mutable scripts over remote HTTP(S) CDN.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<script")
            && (trimmed.contains("http://") || trimmed.contains("https://"))
        {
            return Err(WasmStageError::InvalidHostHtml {
                path: path.to_path_buf(),
                reason: format!("forbidden remote script URL found: {trimmed}"),
            });
        }
    }

    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), WasmStageError> {
    std::fs::copy(src, dst).map_err(|source| WasmStageError::Io {
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn file_size(path: &Path) -> Result<u64, WasmStageError> {
    let metadata = std::fs::metadata(path).map_err(|source| WasmStageError::Io {
        src: path.to_path_buf(),
        dst: path.to_path_buf(),
        source,
    })?;
    let size = metadata.len();
    if size == 0 {
        return Err(WasmStageError::EmptyStagedFile(path.to_path_buf()));
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const VALID_WASM: &[u8] = b"\0asm\x01\0\0\0";

    fn write_valid_html(path: &Path) {
        let content = r#"<!DOCTYPE html>
<html>
<head><title>Tabula</title></head>
<body>
    <canvas id="glcanvas"></canvas>
    <script src="mq_js_bundle.js"></script>
    <script>load("tabula-game-client.wasm");</script>
</body>
</html>"#;
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn stage_bundle_produces_all_expected_artifacts() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");

        write_valid_html(&html_path);
        std::fs::write(&js_path, "/* mock js */").unwrap();
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let report = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap();

        assert_eq!(report.out_dir, out_dir.path());
        assert!(out_dir.path().join("index.html").is_file());
        assert!(out_dir.path().join("mq_js_bundle.js").is_file());
        assert!(out_dir.path().join("tabula-game-client.wasm").is_file());
        assert_eq!(report.wasm_size, 8);

        let mut entries: Vec<_> = std::fs::read_dir(out_dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                std::ffi::OsString::from("index.html"),
                std::ffi::OsString::from("mq_js_bundle.js"),
                std::ffi::OsString::from("tabula-game-client.wasm"),
            ]
        );
    }

    #[test]
    fn stage_bundle_fails_when_wasm_is_missing() {
        let web_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        write_valid_html(&html_path);
        std::fs::write(&js_path, "/* mock js */").unwrap();

        let missing_wasm = web_dir.path().join("nonexistent.wasm");
        let err = stage_bundle(web_dir.path(), &missing_wasm, out_dir.path()).unwrap_err();

        assert!(matches!(err, WasmStageError::MissingWasmArtifact(_)));
    }

    #[test]
    fn stage_bundle_fails_when_html_is_missing() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let err = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::MissingHostFile(_)));
    }

    #[test]
    fn stage_bundle_fails_when_js_is_missing() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");
        write_valid_html(&html_path);
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let err = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::MissingBootstrapFile(_)));
    }

    #[test]
    fn stage_bundle_rejects_html_with_remote_cdn_scripts() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");

        let cdn_html = r#"<!DOCTYPE html>
<html>
<body>
    <canvas id="glcanvas"></canvas>
    <script src="https://cdn.example.com/mq_js_bundle.js"></script>
    <script>load("tabula-game-client.wasm");</script>
</body>
</html>"#;
        std::fs::write(&html_path, cdn_html).unwrap();
        std::fs::write(&js_path, "/* mock js */").unwrap();
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let err = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::InvalidHostHtml { .. }));
    }

    #[test]
    fn stage_bundle_overwrites_stale_files_deterministically() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");

        write_valid_html(&html_path);
        std::fs::write(&js_path, "/* mock js v2 */").unwrap();
        std::fs::write(&wasm_path, b"\0asm\x01\0\0\0v2").unwrap();

        // Pre-populate out_dir with stale files
        std::fs::write(out_dir.path().join("index.html"), "stale").unwrap();
        std::fs::write(out_dir.path().join("mq_js_bundle.js"), "stale").unwrap();
        std::fs::write(out_dir.path().join("tabula-game-client.wasm"), "stale").unwrap();
        std::fs::write(out_dir.path().join("stale.txt"), "stale").unwrap();

        let report = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap();
        assert_eq!(report.wasm_size, 10);
        assert_eq!(
            std::fs::read(out_dir.path().join("tabula-game-client.wasm")).unwrap(),
            b"\0asm\x01\0\0\0v2"
        );
        assert!(!out_dir.path().join("stale.txt").exists());
        assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 3);
    }

    #[test]
    fn stage_bundle_rejects_empty_staged_file() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");

        write_valid_html(&html_path);
        std::fs::write(&js_path, "/* mock js */").unwrap();
        std::fs::write(&wasm_path, b"").unwrap();

        let err = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::EmptyStagedFile(_)));
        assert!(!out_dir.path().exists());
    }

    #[test]
    fn failed_stage_removes_previous_bundle() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let html_path = web_dir.path().join("index.html");
        let js_path = web_dir.path().join("mq_js_bundle.js");
        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");

        write_valid_html(&html_path);
        std::fs::write(&js_path, "/* mock js */").unwrap();
        std::fs::write(&wasm_path, VALID_WASM).unwrap();
        stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap();

        std::fs::remove_file(&wasm_path).unwrap();
        let err = stage_bundle(web_dir.path(), &wasm_path, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::MissingWasmArtifact(_)));
        assert!(!out_dir.path().exists());
    }
}
