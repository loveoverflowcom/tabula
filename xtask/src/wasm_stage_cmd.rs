//! Deterministic staging of the WebAssembly browser host and gameplay bundle.
//!
//! Stages the checked-in HTML host, pinned Macroquad JS bootstrap, and compiled
//! `wasm-release` binary into a self-contained distribution directory (doc 01 §1.4).

use std::path::{Path, PathBuf};
use std::{collections::BTreeMap, fmt::Write};

use serde::Serialize;
use sha2::{Digest, Sha256};

// Every resource needed by the standalone entry and gameplay documents.
// An absent asset is a staging failure, never a silently incomplete bundle.
const HOST_FILES: &[&str] = &[
    "index.html",
    "play.html",
    "standalone.css",
    "launch-options.js",
    "host-bridge.js",
    "setup.js",
    "bootstrap.js",
    "resources.js",
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

/// Opt-in local document shape, not registry rollout or online availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BundleKind {
    Standard,
    PrivateSimulator,
}

const SIMULATOR_HOST_FILES: &[&str] = &[
    "index.html",
    "play.html",
    "standalone.css",
    "launch-options.js",
    "werewolf-setup.js", // xtask-allow-game-id: ADR-0035 standalone leaf packaging.
    "bootstrap.js",
    "resources.js",
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

    #[error("expected WASM artifact is missing at {0}\nRun 'cargo build -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown --profile wasm-release' first.")]
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

    #[error("invalid local runtime resource: {0}")]
    InvalidResource(String),
}

/// Report summarizing successfully staged artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmStageReport {
    pub out_dir: PathBuf,
    pub html_size: u64,
    pub js_size: u64,
    pub wasm_size: u64,
    pub host_file_count: usize,
    /// Manifest-listed WASM/font/pack payloads. These are fetched on demand.
    pub runtime_resource_count: usize,
}

pub fn run(args: &[String]) -> Result<WasmStageReport, WasmStageError> {
    let kind = match args {
        [] => BundleKind::Standard,
        [flag, game] if flag == "--game" && game == "werewolf" => BundleKind::PrivateSimulator,
        _ => return Err(WasmStageError::UnexpectedArgument(args.join(" "))),
    }; // xtask-allow-game-id: ADR-0035 opt-in local leaf selector.

    let root = crate::workspace::root()?;
    let web_src_dir = root.join("apps").join("game-client").join("web");
    let wasm_src = match kind {
        BundleKind::Standard => resolve_wasm_source(&crate::workspace::target_dir()?)?,
        BundleKind::PrivateSimulator => {
            let path = crate::workspace::target_dir()?
                .join("wasm32-unknown-unknown/wasm-release/tabula-werewolf-client.wasm"); // xtask-allow-game-id: ADR-0035 separate binary artifact.
            if !path.is_file() {
                return Err(WasmStageError::MissingWasmArtifact(path));
            }
            path
        }
    };
    let out_dir = root.join("target").join(if kind == BundleKind::Standard {
        "tabula-web-game"
    } else {
        "tabula-web-werewolf"
    }); // xtask-allow-game-id: ADR-0035 opt-in standalone output.

    let tokens_src = root.join("apps/web/style/tokens.css");
    let report = stage_bundle_kind(&web_src_dir, &tokens_src, &wasm_src, &out_dir, kind)?;

    println!(
        "stage-wasm-game: staged browser host into {}\n  - index.html ({} bytes)\n  - mq_js_bundle.js ({} bytes)\n  - tabula-game-client.wasm ({} bytes)\n  - {} required host resources + canonical tokens.css\n  - {} manifest-listed, immutable runtime resources (loaded on demand)",
        report.out_dir.display(),
        report.html_size,
        report.js_size,
        report.wasm_size,
        report.host_file_count,
        report.runtime_resource_count
    );

    Ok(report)
}

pub(crate) fn resolve_wasm_source(target_dir: &Path) -> Result<PathBuf, WasmStageError> {
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
        "stage-local-play: staged separate gameplay document into {} ({} WASM bytes, {} on-demand runtime resources)",
        report.out_dir.display(),
        report.wasm_size,
        report.runtime_resource_count
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
    report.html_size = promote_integrated_entry(&candidate)?;
    std::fs::rename(&candidate, &out_dir).map_err(|source| WasmStageError::Io {
        src: candidate,
        dst: out_dir.clone(),
        source,
    })?;
    report.out_dir = out_dir;
    report.host_file_count += 1;
    Ok(report)
}

/// Turn a staged standalone bundle into the integrated `/play/local/` document: keep the
/// standalone entry as `standalone.html`, promote the gameplay document to `index.html`
/// and give its static no-script fallbacks a safe escape. Returns the entry's size.
pub(crate) fn promote_integrated_entry(candidate: &Path) -> Result<u64, WasmStageError> {
    copy_file(
        &candidate.join("index.html"),
        &candidate.join("standalone.html"),
    )?;
    copy_file(&candidate.join("play.html"), &candidate.join("index.html"))?;
    // A missing/SRI-rejected bootstrap cannot rewrite these links. The plain
    // integrated document must still offer a safe escape to the shell.
    let integrated_index = candidate.join("index.html");
    let html = String::from_utf8(read_resource(&integrated_index)?)
        .map_err(|error| resource_error(format!("host UTF-8: {error}")))?;
    let html = rewrite_quoted_attribute(&html, "href", "index.html", "href=\"/games\"");
    write_resource(&integrated_index, html.as_bytes())?;
    // play.html was validated before its references were content-versioned.
    file_size(&candidate.join("index.html"))
}

/// Stages the web host, JS bootstrap, and WASM binary into `out_dir`.
pub fn stage_bundle(
    web_src_dir: &Path,
    tokens_src: &Path,
    wasm_src: &Path,
    out_dir: &Path,
) -> Result<WasmStageReport, WasmStageError> {
    stage_bundle_kind(
        web_src_dir,
        tokens_src,
        wasm_src,
        out_dir,
        BundleKind::Standard,
    )
}

/// Stage only the explicitly requested isolated-seat simulator document.
#[cfg(test)]
pub fn stage_simulator_bundle(
    web_src_dir: &Path,
    tokens_src: &Path,
    wasm_src: &Path,
    out_dir: &Path,
) -> Result<WasmStageReport, WasmStageError> {
    stage_bundle_kind(
        web_src_dir,
        tokens_src,
        wasm_src,
        out_dir,
        BundleKind::PrivateSimulator,
    )
}

fn stage_bundle_kind(
    web_src_dir: &Path,
    tokens_src: &Path,
    wasm_src: &Path,
    out_dir: &Path,
    kind: BundleKind,
) -> Result<WasmStageReport, WasmStageError> {
    let host_files = if kind == BundleKind::Standard {
        HOST_FILES
    } else {
        SIMULATOR_HOST_FILES
    };
    let source_name = |name: &str| match (kind, name) {
        (BundleKind::PrivateSimulator, "index.html") => "werewolf.html".to_owned(), // xtask-allow-game-id: ADR-0035 standalone document source.
        (BundleKind::PrivateSimulator, "play.html") => "werewolf-play.html".to_owned(), // xtask-allow-game-id: ADR-0035 standalone document source.
        _ => name.to_owned(),
    };
    let html_src = web_src_dir.join(source_name("index.html"));
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
    for relative in host_files {
        let source = web_src_dir.join(source_name(relative));
        if !source.is_file() {
            return Err(WasmStageError::MissingHostFile(source));
        }
    }
    if !tokens_src.is_file() {
        return Err(WasmStageError::MissingHostFile(tokens_src.to_path_buf()));
    }
    validate_host_html(&web_src_dir.join(source_name("play.html")), true)?;

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

    for relative in host_files {
        let source = web_src_dir.join(source_name(relative));
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

    let wasm_size = file_size(&wasm_dst)?;
    let runtime_resource_count = stage_versioned_resources(staging_dir.path(), kind)?;
    let html_size = file_size(&html_dst)?;
    let js_size = file_size(&js_dst)?;

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
        host_file_count: host_files.len(),
        runtime_resource_count,
    })
}

pub(crate) fn remove_existing_destination(out_dir: &Path) -> Result<(), WasmStageError> {
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

pub(crate) fn copy_file(src: &Path, dst: &Path) -> Result<(), WasmStageError> {
    std::fs::copy(src, dst).map_err(|source| WasmStageError::Io {
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        source,
    })?;
    Ok(())
}

pub(crate) fn file_size(path: &Path) -> Result<u64, WasmStageError> {
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

/// A public, immutable payload admitted by the local browser host.
/// SHA-256 protects the pre-execution boundary; pack files additionally keep
/// their game-owned BLAKE3 identity and are verified by `load_verified` in Rust.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct WebResource {
    url: String,
    bytes: usize,
    sha256: String,
}

#[derive(Serialize)]
struct WebResourceManifest {
    schema: u8,
    files: BTreeMap<String, WebResource>,
}

fn resource_error(message: impl Into<String>) -> WasmStageError {
    WasmStageError::InvalidResource(message.into())
}

fn read_resource(path: &Path) -> Result<Vec<u8>, WasmStageError> {
    std::fs::read(path).map_err(|source| WasmStageError::Io {
        src: path.to_path_buf(),
        dst: path.to_path_buf(),
        source,
    })
}

fn write_resource(path: &Path, bytes: &[u8]) -> Result<(), WasmStageError> {
    std::fs::write(path, bytes).map_err(|source| WasmStageError::Io {
        src: path.to_path_buf(),
        dst: path.to_path_buf(),
        source,
    })
}

fn immutable_resource(
    directory: &Path,
    bytes: &[u8],
    extension: &str,
) -> Result<WebResource, WasmStageError> {
    if bytes.is_empty() || bytes.len() > 64 * 1024 * 1024 {
        return Err(resource_error("payload must contain 1..64 MiB"));
    }
    let digest = Sha256::digest(bytes);
    let mut sha256 = String::with_capacity(64);
    for byte in digest {
        write!(sha256, "{byte:02x}").expect("writing to a String cannot fail");
    }
    let url = format!("resources/{sha256}.{extension}");
    write_resource(&directory.join(&url), bytes)?;
    Ok(WebResource {
        url,
        bytes: bytes.len(),
        sha256,
    })
}

fn sri_sha256(resource: &WebResource) -> String {
    // A fixed-size digest encoder avoids another browser-build dependency.
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = resource
        .sha256
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let hex = std::str::from_utf8(pair).expect("SHA-256 is ASCII");
            u8::from_str_radix(hex, 16).expect("SHA-256 was generated above")
        })
        .collect();
    let mut encoded = String::from("sha256-");
    for chunk in bytes.chunks(3) {
        let value = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for shift in [18, 12, 6, 0] {
            let position = usize::try_from((value >> shift) & 63).expect("six bits fit usize");
            encoded.push(char::from(BASE64[position]));
        }
        if chunk.len() < 3 {
            encoded.pop();
            encoded.push('=');
        }
        if chunk.len() == 1 {
            let length = encoded.len();
            encoded.replace_range(length - 2..length - 1, "=");
        }
    }
    encoded
}

#[rustfmt::skip]
fn stage_game_pack(
    directory: &Path,
    files: &mut BTreeMap<String, WebResource>,
    kind: BundleKind,
) -> Result<(), WasmStageError> {
    // The local slice packages the existing game-owned manifest, not a second
    // resource-selection policy or a new delivery service (ADR-0030).
    let (manifest_text, images) = match kind {
        BundleKind::Standard => (tabula_game_chess::presentation::assets::MANIFEST, tabula_game_chess::presentation::assets::ALL_IMAGES), // xtask-allow-game-id: local standalone packaging of the game-owned pack.
        BundleKind::PrivateSimulator => (tabula_game_werewolf::presentation::assets::MANIFEST, tabula_game_werewolf::presentation::assets::ALL_IMAGES), // xtask-allow-game-id: ADR-0035 local standalone pack declaration.
    };
    let manifest = tabula_assets::AssetPackManifest::from_toml(manifest_text).map_err(|error| resource_error(format!("local pack manifest: {error}")))?;
    for file in manifest.files() {
        let bytes = images.iter() // xtask-allow-game-id: local standalone packaging of the game-owned pack.
            .find(|(name, _)| *name == file.name().as_str())
            .map(|(_, bytes)| *bytes)
            .ok_or_else(|| resource_error(format!("local pack file missing: {}", file.name())))?;
        file.verify_bytes(bytes)
            .map_err(|error| resource_error(format!("local pack integrity: {error}")))?;
        files.insert(file.path().as_str().to_owned(), immutable_resource(directory, bytes, "png")?);
    }
    Ok(())
}

/// Version every runtime payload and every static host dependency. HTML stays
/// mutable/no-store, with immutable script/style references (including SRI).
/// Fixed-name diagnostic copies are retained but never referenced by the host.
fn stage_versioned_resources(directory: &Path, kind: BundleKind) -> Result<usize, WasmStageError> {
    let resources = directory.join("resources");
    std::fs::create_dir(&resources).map_err(|source| WasmStageError::CreateDir {
        path: resources,
        source,
    })?;
    let mut files = BTreeMap::new();
    for (alias, extension) in [
        ("tabula-game-client.wasm", "wasm"),
        ("assets/OpenSans-Regular.ttf", "ttf"),
        ("assets/OpenSans-Semibold.ttf", "ttf"),
        ("assets/NotoSerif-Bold.ttf", "ttf"),
    ] {
        files.insert(
            alias.to_owned(),
            immutable_resource(
                directory,
                &read_resource(&directory.join(alias))?,
                extension,
            )?,
        );
    }
    stage_game_pack(directory, &mut files, kind)?;
    if files.len() > 32 || files.values().map(|file| file.bytes).sum::<usize>() > 150 * 1024 * 1024
    {
        return Err(resource_error(
            "local manifest exceeds its entry/byte budget",
        ));
    }
    let runtime_resource_count = files.len();
    let manifest = WebResourceManifest {
        schema: 1,
        files: files.clone(),
    };
    let json = serde_json::to_string(&manifest)
        .map_err(|error| resource_error(format!("manifest serialization: {error}")))?;
    let manifest_script = format!("window.TabulaResourceManifest={json};\n");
    write_resource(
        &directory.join("resource-manifest.js"),
        manifest_script.as_bytes(),
    )?;

    let mut references = files;
    let mut static_files = vec![
        "mq_js_bundle.js",
        "resource-manifest.js",
        "resources.js",
        "launch-options.js",
        "bootstrap.js",
        "tokens.css",
    ];
    if kind == BundleKind::Standard {
        static_files.extend([
            "host-bridge.js",
            "setup.js",
            "assets/chess-cover.png",
            "assets/chess-cover-small.png",
        ]);
    }
    // xtask-allow-game-id: existing standalone cover packaging only.
    else {
        static_files.push("werewolf-setup.js");
    } // xtask-allow-game-id: ADR-0035 standalone setup.
    for relative in static_files {
        let extension = Path::new(relative)
            .extension()
            .and_then(|value| value.to_str())
            .ok_or_else(|| resource_error("static host dependency has no extension"))?;
        references.insert(
            relative.to_owned(),
            immutable_resource(
                directory,
                &read_resource(&directory.join(relative))?,
                extension,
            )?,
        );
    }
    // A hashed stylesheet lives beside its fonts in resources/, so its font
    // URLs are relative to that directory, not the mutable HTML entry.
    let mut css = String::from_utf8(read_resource(&directory.join("standalone.css"))?)
        .map_err(|error| resource_error(format!("stylesheet UTF-8: {error}")))?;
    for alias in [
        "assets/OpenSans-Regular.ttf",
        "assets/OpenSans-Semibold.ttf",
        "assets/NotoSerif-Bold.ttf",
    ] {
        css = css.replace(
            alias,
            references[alias].url.trim_start_matches("resources/"),
        );
    }
    references.insert(
        "standalone.css".to_owned(),
        immutable_resource(directory, css.as_bytes(), "css")?,
    );
    rewrite_host_references(directory, &references)?;
    Ok(runtime_resource_count)
}

fn rewrite_host_references(
    directory: &Path,
    references: &BTreeMap<String, WebResource>,
) -> Result<(), WasmStageError> {
    for relative in ["index.html", "play.html"] {
        let path = directory.join(relative);
        let mut html = String::from_utf8(read_resource(&path)?)
            .map_err(|error| resource_error(format!("host UTF-8: {error}")))?;
        for (alias, resource) in references {
            match Path::new(alias)
                .extension()
                .and_then(|value| value.to_str())
            {
                Some(extension @ ("js" | "css")) => {
                    let attribute = if extension == "js" { "src" } else { "href" };
                    html = rewrite_attribute(&html, attribute, alias, resource);
                    if html.contains(alias) {
                        return Err(resource_error(format!(
                            "unversioned host reference remains: {relative}: {alias}"
                        )));
                    }
                }
                Some("png") => html = html.replace(alias, &resource.url),
                _ => {}
            }
        }
        write_resource(&path, html.as_bytes())?;
    }
    Ok(())
}

/// Rewrite canonical quoted attributes, accepting both quote styles and HTML
/// whitespace around `=`. Anything left referring to a mutable dependency is
/// rejected above rather than silently bypassing versioning/SRI.
fn rewrite_attribute(html: &str, attribute: &str, alias: &str, resource: &WebResource) -> String {
    let markup = format!(
        "{attribute}=\"{}\" integrity=\"{}\" crossorigin=\"anonymous\"",
        resource.url,
        sri_sha256(resource)
    );
    rewrite_quoted_attribute(html, attribute, alias, &markup)
}

fn rewrite_quoted_attribute(html: &str, attribute: &str, alias: &str, markup: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut copied = 0;
    let mut cursor = 0;
    while let Some(offset) = html[cursor..].find(attribute) {
        let start = cursor + offset;
        cursor = start + attribute.len();
        if !html[..start].ends_with(char::is_whitespace) {
            continue;
        }
        let tail = html[cursor..].trim_start();
        let Some(tail) = tail.strip_prefix('=') else {
            continue;
        };
        let tail = tail.trim_start();
        let Some(quote @ ('\'' | '"')) = tail.chars().next() else {
            continue;
        };
        let Some(end) = tail[1..].find(quote) else {
            continue;
        };
        if &tail[1..=end] != alias {
            continue;
        }
        let value_end = html.len() - tail.len() + end + 2;
        result.push_str(&html[copied..start]);
        result.push_str(markup);
        copied = value_end;
        cursor = value_end;
    }
    result.push_str(&html[copied..]);
    result
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
        // Top-level entries: the HOST_FILES roots (including the host bridge), the bootstrap,
        // WASM, tokens, manifest script and the hashed resources directory.
        assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 14);
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
        let play = web.path().join("play.html");
        let html = std::fs::read_to_string(&play).unwrap()
            + "<a id=\"cancel-load\" href=\"index.html\">Return</a><a id='error-back' href = 'index.html'>Return</a>";
        std::fs::write(&play, html).unwrap();
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
        assert_eq!(html.matches("href=\"/games\"").count(), 2);
        assert!(!html.contains("href=\"index.html\""));
        assert!(!html.contains("href = 'index.html'"));
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

    #[test]
    fn content_names_and_sri_match_a_published_sha256_vector() {
        let directory = tempdir().unwrap();
        std::fs::create_dir(directory.path().join("resources")).unwrap();
        let resource = immutable_resource(directory.path(), b"abc", "wasm").unwrap();
        assert_eq!(
            resource.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sri_sha256(&resource),
            "sha256-ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
        assert_eq!(
            resource,
            immutable_resource(directory.path(), b"abc", "wasm").unwrap()
        );
        assert_ne!(
            resource.url,
            immutable_resource(directory.path(), b"abd", "wasm")
                .unwrap()
                .url
        );
        for original in [
            "<script src='bootstrap.js'></script>",
            "<script src = \"bootstrap.js\"></script>",
            "<script\n src\t=\t'bootstrap.js'></script>",
        ] {
            let rewritten = rewrite_attribute(original, "src", "bootstrap.js", &resource);
            assert!(rewritten.contains(&resource.url));
            assert!(rewritten.contains("integrity=\"sha256-"));
            assert!(!rewritten.contains("bootstrap.js"));
        }
        let style = rewrite_attribute(
            "<link href = 'standalone.css'>",
            "href",
            "standalone.css",
            &resource,
        );
        assert!(style.contains(&resource.url) && style.contains("integrity=\"sha256-"));
    }

    #[test]
    fn simulator_stage_uses_only_its_pack_and_keeps_runtime_identity() {
        let root = crate::workspace::root().unwrap();
        let dir = tempdir().unwrap();
        let wasm = dir.path().join("input.wasm");
        std::fs::write(&wasm, VALID_WASM).unwrap();
        let out = dir.path().join("simulator");
        let report = stage_simulator_bundle(
            &root.join("apps/game-client/web"),
            &root.join("apps/web/style/tokens.css"),
            &wasm,
            &out,
        )
        .unwrap();
        assert_eq!(report.host_file_count, SIMULATOR_HOST_FILES.len());
        assert_eq!(report.runtime_resource_count, 18);
        assert!(!out.join("assets/chess-cover.png").exists()); // xtask-allow-game-id: packaging isolation assertion.
        assert!(!out.join("setup.js").exists());
        assert!(!out.join("host-bridge.js").exists());
        let entry = std::fs::read_to_string(out.join("index.html")).unwrap();
        let setup = std::fs::read(out.join("werewolf-setup.js")).unwrap(); // xtask-allow-game-id: ADR-0035 standalone setup isolation.
        let setup_url = format!("resources/{:x}.js", Sha256::digest(&setup));
        assert!(entry.contains(&format!("src=\"{setup_url}\" integrity=\"sha256-")));
        assert!(!entry.contains("src=\"setup.js\""));
        let play = std::fs::read_to_string(out.join("play.html")).unwrap();
        assert!(play.contains("data-runtime=\"werewolf\"")); // xtask-allow-game-id: standalone identity assertion.
        assert!(!play.contains("src=\"bootstrap.js\""));
        let text = std::fs::read_to_string(out.join("resource-manifest.js")).unwrap();
        let value: serde_json::Value = serde_json::from_str(
            text.trim_start_matches("window.TabulaResourceManifest=")
                .trim_end_matches(";\n"),
        )
        .unwrap();
        let files = value["files"].as_object().unwrap();
        assert_eq!(files.len(), 18);
        assert_eq!(
            files
                .keys()
                .filter(|key| key.starts_with("werewolf/0.1.0/"))
                .count(),
            14
        ); // xtask-allow-game-id: pack isolation assertion.
        assert!(!files.keys().any(|key| key.starts_with("chess/"))); // xtask-allow-game-id: pack isolation assertion.
        for entry in files.values() {
            let bytes = std::fs::read(out.join(entry["url"].as_str().unwrap())).unwrap();
            assert_eq!(bytes.len() as u64, entry["bytes"].as_u64().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                entry["sha256"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn staging_pins_every_runtime_payload_and_static_host_reference() {
        let directory = tempdir().unwrap();
        write_valid_html(&directory.path().join("index.html"));
        let html = "<canvas id=\"glcanvas\"></canvas><script src=\"mq_js_bundle.js\"></script><script src=\"resource-manifest.js\"></script><script src=\"resources.js\"></script><script src=\"launch-options.js\"></script><script src=\"bootstrap.js\"></script><link rel=\"stylesheet\" href=\"standalone.css\">";
        std::fs::write(directory.path().join("play.html"), html).unwrap();
        std::fs::write(
            directory.path().join("mq_js_bundle.js"),
            "/* pinned bootstrap */",
        )
        .unwrap();
        std::fs::write(directory.path().join("tabula-game-client.wasm"), VALID_WASM).unwrap();
        std::fs::write(directory.path().join("tokens.css"), "/* tokens */").unwrap();
        std::fs::write(
            directory.path().join("standalone.css"),
            "@font-face{src:url('assets/OpenSans-Regular.ttf')}",
        )
        .unwrap();
        assert_eq!(
            stage_versioned_resources(directory.path(), BundleKind::Standard).unwrap(),
            8
        );
        let text = std::fs::read_to_string(directory.path().join("resource-manifest.js")).unwrap();
        let value: serde_json::Value = serde_json::from_str(
            text.trim_start_matches("window.TabulaResourceManifest=")
                .trim_end_matches(";\n"),
        )
        .unwrap();
        assert_eq!(value["schema"], 1);
        let files = value["files"].as_object().unwrap();
        assert_eq!(files.len(), 8);
        for entry in files.values() {
            let bytes =
                std::fs::read(directory.path().join(entry["url"].as_str().unwrap())).unwrap();
            assert_eq!(bytes.len() as u64, entry["bytes"].as_u64().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                entry["sha256"].as_str().unwrap()
            );
        }
        let play = std::fs::read_to_string(directory.path().join("play.html")).unwrap();
        for alias in [
            "mq_js_bundle.js",
            "resource-manifest.js",
            "resources.js",
            "launch-options.js",
            "bootstrap.js",
            "standalone.css",
        ] {
            assert!(
                !play.contains(&format!("=\"{alias}\"")),
                "mutable reference: {alias}"
            );
        }
        assert_eq!(play.matches("integrity=\"sha256-").count(), 6);
        let font = files["assets/OpenSans-Regular.ttf"]["url"]
            .as_str()
            .unwrap();
        let expected = font.trim_start_matches("resources/");
        let css: Vec<_> = std::fs::read_dir(directory.path().join("resources"))
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "css"))
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect();
        assert!(css
            .iter()
            .any(|css| css.contains(expected) && !css.contains("assets/OpenSans-Regular.ttf")));
    }
}
