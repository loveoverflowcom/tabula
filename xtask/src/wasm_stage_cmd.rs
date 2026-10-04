//! Deterministic staging of the WebAssembly browser host and gameplay bundle.
//!
//! Stages the checked-in HTML host, pinned Macroquad JS bootstrap, and compiled
//! `wasm-release` binary into a self-contained distribution directory (doc 01 §1.4).

use std::path::{Path, PathBuf};

// Every resource needed by the standalone entry and gameplay documents.
// An absent asset is a staging failure, never a silently incomplete bundle.
const HOST_FILES: &[&str] = &[
    "index.html",
    "play.html",
    "standalone.css",
    "launch-options.js",
    "setup.js",
    "bootstrap.js",
    "assets/chess-cover.png",
    "assets/chess-cover-small.png",
    "assets/chess-cover-provenance.md",
    "assets/OpenSans-Regular.ttf",
    "assets/OpenSans-Semibold.ttf",
    "assets/NotoSerif-Bold.ttf",
    "assets/OFL-OpenSans.txt",
    "assets/OFL-Noto.txt",
    "assets/LICENSE-OpenSans-Apache-2.0.txt",
];

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

    #[error("shell distribution is missing or invalid at {0}; build apps/web with TABULA_PLAY_BASE=/play first")]
    MissingShellDistribution(PathBuf),
}

/// Report summarizing successfully staged artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmStageReport {
    pub out_dir: PathBuf,
    pub html_size: u64,
    pub js_size: u64,
    pub wasm_size: u64,
    pub host_file_count: usize,
}

pub fn run(args: &[String]) -> Result<WasmStageReport, WasmStageError> {
    if let Some(argument) = args.first() {
        return Err(WasmStageError::UnexpectedArgument(argument.clone()));
    }

    let root = crate::workspace::root()?;
    let web_src_dir = root.join("apps").join("game-client").join("web");
    let wasm_src = resolve_wasm_source(&crate::workspace::target_dir()?)?;
    let out_dir = root.join("target").join("tabula-web-game");

    let tokens_src = root.join("apps/web/style/tokens.css");
    let report = stage_bundle(&web_src_dir, &tokens_src, &wasm_src, &out_dir)?;

    println!(
        "stage-wasm-game: staged browser host into {}\n  - index.html ({} bytes)\n  - mq_js_bundle.js ({} bytes)\n  - tabula-game-client.wasm ({} bytes)\n  - {} required host resources + canonical tokens.css",
        report.out_dir.display(),
        report.html_size,
        report.js_size,
        report.wasm_size,
        report.host_file_count
    );

    Ok(report)
}

fn resolve_wasm_source(target_dir: &Path) -> Result<PathBuf, WasmStageError> {
    let wasm_dir = target_dir
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

/// Stage the separate local gameplay document beside an already-built shell.
///
/// ADR-011/0030: `/play/local/` is static HTML, never a Leptos route. Only
/// workspace-owned build output is replaced; shell files and standalone source
/// remain unchanged. A failed attempt retires the previous local play bundle.
pub fn run_local(args: &[String]) -> Result<WasmStageReport, WasmStageError> {
    if let Some(argument) = args.first() {
        return Err(WasmStageError::UnexpectedArgument(argument.clone()));
    }
    let root = crate::workspace::root()?;
    let shell_dist = root.join("apps/web/dist");
    let out_dir = shell_dist.join("play/local");
    remove_existing_destination(&out_dir)?;
    let wasm_src = resolve_wasm_source(&crate::workspace::target_dir()?)?;
    let report = stage_local_bundle(
        &root.join("apps/game-client/web"),
        &root.join("apps/web/style/tokens.css"),
        &wasm_src,
        &shell_dist,
    )?;
    println!(
        "stage-local-play: staged separate gameplay document into {} ({} WASM bytes)",
        report.out_dir.display(),
        report.wasm_size
    );
    Ok(report)
}

/// Build a complete local document before atomically publishing it in shell output.
pub fn stage_local_bundle(
    web_src_dir: &Path,
    tokens_src: &Path,
    wasm_src: &Path,
    shell_dist: &Path,
) -> Result<WasmStageReport, WasmStageError> {
    let out_dir = shell_dist.join("play/local");
    remove_existing_destination(&out_dir)?;
    let shell_index = shell_dist.join("index.html");
    if !shell_index.is_file() || file_size(&shell_index)? == 0 {
        return Err(WasmStageError::MissingShellDistribution(
            shell_dist.to_path_buf(),
        ));
    }
    let parent = out_dir.parent().expect("local output has a parent");
    std::fs::create_dir_all(parent).map_err(|source| WasmStageError::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    let temporary = tempfile::Builder::new()
        .prefix(".tabula-local-play-")
        .tempdir_in(parent)
        .map_err(|source| WasmStageError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    let candidate = temporary.path().join("bundle");
    let mut report = stage_bundle(web_src_dir, tokens_src, wasm_src, &candidate)?;
    copy_file(
        &candidate.join("index.html"),
        &candidate.join("standalone.html"),
    )?;
    copy_file(&candidate.join("play.html"), &candidate.join("index.html"))?;
    validate_host_html(&candidate.join("index.html"), true)?;
    report.html_size = file_size(&candidate.join("index.html"))?;
    std::fs::rename(&candidate, &out_dir).map_err(|source| WasmStageError::Io {
        src: candidate,
        dst: out_dir.clone(),
        source,
    })?;
    report.out_dir = out_dir;
    report.host_file_count += 1;
    Ok(report)
}

/// Stages the web host, JS bootstrap, and WASM binary into `out_dir`.
pub fn stage_bundle(
    web_src_dir: &Path,
    tokens_src: &Path,
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

    validate_host_html(&html_src, false)?;
    for relative in HOST_FILES {
        let source = web_src_dir.join(relative);
        if !source.is_file() {
            return Err(WasmStageError::MissingHostFile(source));
        }
    }
    if !tokens_src.is_file() {
        return Err(WasmStageError::MissingHostFile(tokens_src.to_path_buf()));
    }
    validate_host_html(&web_src_dir.join("play.html"), true)?;

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

    for relative in HOST_FILES {
        let source = web_src_dir.join(relative);
        let destination = staging_dir.path().join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|source| WasmStageError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        copy_file(&source, &destination)?;
        file_size(&destination)?;
    }
    let tokens_dst = staging_dir.path().join("tokens.css");
    copy_file(tokens_src, &tokens_dst)?;
    file_size(&tokens_dst)?;
    copy_file(&js_src, &js_dst)?;
    copy_file(wasm_src, &wasm_dst)?;

    validate_host_html(&html_dst, false)?;
    validate_host_html(&staging_dir.path().join("play.html"), true)?;
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
        host_file_count: HOST_FILES.len(),
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

fn validate_host_html(path: &Path, gameplay: bool) -> Result<(), WasmStageError> {
    let content = std::fs::read_to_string(path).map_err(|source| WasmStageError::Io {
        src: path.to_path_buf(),
        dst: path.to_path_buf(),
        source,
    })?;

    if gameplay && !content.contains("id=\"glcanvas\"") && !content.contains("id='glcanvas'") {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing canvas with id \"glcanvas\"".into(),
        });
    }

    if gameplay && !content.contains("mq_js_bundle.js") {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing script reference to \"mq_js_bundle.js\"".into(),
        });
    }

    if gameplay
        && !content.contains("src=\"bootstrap.js\"")
        && !content.contains("src='bootstrap.js'")
    {
        return Err(WasmStageError::InvalidHostHtml {
            path: path.to_path_buf(),
            reason: "missing script reference to \"bootstrap.js\"".into(),
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
    <script src="bootstrap.js"></script>
</body>
</html>"#;
        std::fs::write(path, content).unwrap();
        let root = path.parent().unwrap();
        for relative in HOST_FILES {
            if *relative == "index.html" {
                continue;
            }
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(
                &target,
                if *relative == "play.html" {
                    content
                } else {
                    "fixture resource"
                },
            )
            .unwrap();
        }
        std::fs::write(root.join("canonical.css"), "/* canonical tokens fixture */").unwrap();
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

        let report = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap();

        assert_eq!(report.out_dir, out_dir.path());
        assert!(out_dir.path().join("index.html").is_file());
        assert!(out_dir.path().join("mq_js_bundle.js").is_file());
        assert!(out_dir.path().join("tabula-game-client.wasm").is_file());
        assert_eq!(report.wasm_size, 8);

        for relative in HOST_FILES {
            assert!(out_dir.path().join(relative).is_file());
        }
        assert!(out_dir.path().join("tokens.css").is_file());
        assert_eq!(
            std::fs::read_to_string(out_dir.path().join("tokens.css")).unwrap(),
            "/* canonical tokens fixture */"
        );
        assert_eq!(report.host_file_count, HOST_FILES.len());
    }

    #[test]
    fn stage_bundle_fails_when_required_resource_is_missing() {
        let web_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();
        write_valid_html(&web_dir.path().join("index.html"));
        std::fs::write(web_dir.path().join("mq_js_bundle.js"), "bootstrap").unwrap();
        let wasm = web_dir.path().join("game.wasm");
        std::fs::write(&wasm, VALID_WASM).unwrap();
        let missing = web_dir.path().join("assets/chess-cover-small.png");
        std::fs::remove_file(&missing).unwrap();
        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm,
            out_dir.path(),
        )
        .unwrap_err();
        assert!(matches!(err, WasmStageError::MissingHostFile(path) if path == missing));
        assert!(!out_dir.path().exists());
    }

    #[test]
    fn stage_bundle_fails_when_canonical_tokens_are_missing_or_empty() {
        let web_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();
        write_valid_html(&web_dir.path().join("index.html"));
        std::fs::write(web_dir.path().join("mq_js_bundle.js"), "bootstrap").unwrap();
        let wasm = web_dir.path().join("game.wasm");
        std::fs::write(&wasm, VALID_WASM).unwrap();
        let tokens = web_dir.path().join("canonical.css");
        std::fs::remove_file(&tokens).unwrap();
        let err = stage_bundle(web_dir.path(), &tokens, &wasm, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::MissingHostFile(path) if path == tokens));
        std::fs::write(&tokens, "").unwrap();
        let err = stage_bundle(web_dir.path(), &tokens, &wasm, out_dir.path()).unwrap_err();
        assert!(matches!(err, WasmStageError::EmptyStagedFile(_)));
        assert!(!out_dir.path().exists());
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
        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &missing_wasm,
            out_dir.path(),
        )
        .unwrap_err();

        assert!(matches!(err, WasmStageError::MissingWasmArtifact(_)));
    }

    #[test]
    fn stage_bundle_fails_when_html_is_missing() {
        let web_dir = tempdir().unwrap();
        let wasm_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let wasm_path = wasm_dir.path().join("tabula-game-client.wasm");
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap_err();
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

        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap_err();
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
    <script src="bootstrap.js"></script>
</body>
</html>"#;
        std::fs::write(&html_path, cdn_html).unwrap();
        std::fs::write(&js_path, "/* mock js */").unwrap();
        std::fs::write(&wasm_path, VALID_WASM).unwrap();

        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap_err();
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

        let report = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap();
        assert_eq!(report.wasm_size, 10);
        assert_eq!(
            std::fs::read(out_dir.path().join("tabula-game-client.wasm")).unwrap(),
            b"\0asm\x01\0\0\0v2"
        );
        assert!(!out_dir.path().join("stale.txt").exists());
        assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 10);
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

        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap_err();
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
        stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap();

        std::fs::remove_file(&wasm_path).unwrap();
        let err = stage_bundle(
            web_dir.path(),
            &web_dir.path().join("canonical.css"),
            &wasm_path,
            out_dir.path(),
        )
        .unwrap_err();
        assert!(matches!(err, WasmStageError::MissingWasmArtifact(_)));
        assert!(!out_dir.path().exists());
    }

    #[test]
    fn local_stage_promotes_gameplay_without_overwriting_shell() {
        let web = tempdir().unwrap();
        let dist = tempdir().unwrap();
        write_valid_html(&web.path().join("index.html"));
        std::fs::write(web.path().join("index.html"), "standalone setup").unwrap();
        std::fs::write(web.path().join("mq_js_bundle.js"), "bootstrap").unwrap();
        let wasm = web.path().join("game.wasm");
        std::fs::write(&wasm, VALID_WASM).unwrap();
        std::fs::write(dist.path().join("index.html"), "Leptos shell").unwrap();
        std::fs::write(dist.path().join("app.js"), "shell bootstrap").unwrap();
        let report = stage_local_bundle(
            web.path(),
            &web.path().join("canonical.css"),
            &wasm,
            dist.path(),
        )
        .unwrap();
        assert_eq!(report.out_dir, dist.path().join("play/local"));
        let html = std::fs::read_to_string(report.out_dir.join("index.html")).unwrap();
        assert!(html.contains("glcanvas"));
        assert_eq!(
            std::fs::read_to_string(report.out_dir.join("standalone.html")).unwrap(),
            "standalone setup"
        );
        assert_eq!(
            std::fs::read_to_string(dist.path().join("index.html")).unwrap(),
            "Leptos shell"
        );
        assert_eq!(
            std::fs::read_to_string(dist.path().join("app.js")).unwrap(),
            "shell bootstrap"
        );
        assert_eq!(report.host_file_count, HOST_FILES.len() + 1);
        assert_eq!(
            std::fs::read(report.out_dir.join("tabula-game-client.wasm")).unwrap(),
            VALID_WASM
        );
    }

    #[test]
    fn local_stage_failure_invalidates_only_previous_game_document() {
        let web = tempdir().unwrap();
        let dist = tempdir().unwrap();
        let output = dist.path().join("play/local");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("index.html"), "stale game").unwrap();
        std::fs::write(dist.path().join("index.html"), "Leptos shell").unwrap();
        let missing = web.path().join("missing.wasm");
        let error = stage_local_bundle(
            web.path(),
            &web.path().join("canonical.css"),
            &missing,
            dist.path(),
        )
        .unwrap_err();
        assert!(matches!(error, WasmStageError::MissingWasmArtifact(_)));
        assert!(!output.exists());
        assert_eq!(
            std::fs::read_to_string(dist.path().join("index.html")).unwrap(),
            "Leptos shell"
        );
    }

    #[test]
    fn local_stage_requires_a_built_shell() {
        let web = tempdir().unwrap();
        let dist = tempdir().unwrap();
        let error = stage_local_bundle(
            web.path(),
            &web.path().join("canonical.css"),
            &web.path().join("game.wasm"),
            dist.path(),
        )
        .unwrap_err();
        assert!(matches!(error, WasmStageError::MissingShellDistribution(_)));
        assert!(!dist.path().join("play/local").exists());
    }
}
