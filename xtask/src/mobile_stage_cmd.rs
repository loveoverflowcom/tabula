//! Staging of the first-party game bundle the mobile `GameHost` serves (ADR-0033).
//!
//! The mobile host presents the **existing** integrated `/play/local/` document: the same
//! pinned HTML, hashed scripts with SRI, manifest-listed WASM/fonts/pack and verified loader
//! that `stage-local-play` stages beside the web shell. This command adds no second runtime
//! and no second resource policy; it only places that bundle where the Android/iOS builds
//! package it, and writes `tabula-games.json`, the list of games the bundle can launch.
//!
//! The list is derived from the registry (I-9): the registry decides which game has a local
//! document and what its validated default launch query is. Nothing here, and nothing in the
//! shell, parses or invents game options.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tabula_registry::{
    resolve_launch_with_locale, ConfigDraft, I18nKey, LaunchMode, Locale, Localizer,
    RuntimeBinding, SetupRequest,
};

use crate::wasm_stage_cmd::{
    promote_integrated_entry, remove_existing_destination, resolve_wasm_source, stage_bundle,
    WasmStageError,
};

/// Where the packaged game document lives, relative to the bundle root. The mobile hosts map
/// the app's virtual origin onto this tree, so the page sees `/play/local/` exactly as on web.
const ENTRY: &str = "/play/local/";
/// The registry binding for that document base; the same one the web shell is built with.
const PLAY_BASE: &str = "/play";
const MANIFEST: &str = "tabula-games.json";

#[derive(Debug, thiserror::Error)]
pub enum MobileStageError {
    #[error(transparent)]
    Stage(#[from] WasmStageError),
    #[error("failed to resolve workspace root: {0}")]
    WorkspaceRoot(#[from] cargo_metadata::Error),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the registry offers no launchable local game for this bundle")]
    NoLaunchableGame,
    #[error("unexpected argument for stage-mobile-game: {0}")]
    UnexpectedArgument(String),
    #[error("games manifest serialization: {0}")]
    Manifest(#[from] serde_json::Error),
}

/// One game this bundle can launch. `query` is the registry's validated handoff query,
/// opaque to the mobile shell, which appends nothing and parses nothing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BundledGame {
    pub id: String,
    pub entry: String,
    pub query: String,
    pub names: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct GamesManifest<'a> {
    schema: u8,
    games: &'a [BundledGame],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileStageReport {
    pub out_dir: PathBuf,
    pub games: Vec<BundledGame>,
    /// Files removed because the play document never references them.
    pub pruned_files: usize,
    pub wasm_size: u64,
    pub runtime_resource_count: usize,
}

/// No catalog text is needed to launch; display names come from each game's own tables.
struct NoText;
impl Localizer for NoText {
    fn text(&self, _key: &I18nKey) -> Option<&str> {
        None
    }
}

/// The games the staged bundle can start: registry games with a local hot-seat document whose
/// default configuration validates, restricted to the game this bundle actually packages.
pub fn bundled_games() -> Result<Vec<BundledGame>, MobileStageError> {
    // The pack and WASM staged by `stage_bundle` are one game's; listing any other game
    // would offer a launch the bundle cannot serve.
    let packaged = <tabula_game_chess::ChessModule as tabula_game_api::GameModule>::metadata() // xtask-allow-game-id: local packaging of the game-owned pack.
        .id()
        .as_str()
        .to_owned();
    let catalog = tabula_registry::catalog(&NoText);
    let mut games = Vec::new();
    for entry in catalog.entries() {
        if entry.id().as_str() != packaged {
            continue;
        }
        let game = entry.game();
        let request = SetupRequest {
            mode: LaunchMode::LocalHotSeat,
            seats: 2,
            bot_level: None,
            draft: ConfigDraft::with_defaults(game.form()),
        };
        let Ok(config) = game.normalize(&request) else {
            continue;
        };
        let Ok(handoff) =
            resolve_launch_with_locale(RuntimeBinding::bound(PLAY_BASE), &config, Locale::Vi)
        else {
            continue;
        };
        let Some(query) = handoff
            .url
            .strip_prefix(ENTRY)
            .and_then(|rest| rest.strip_prefix('?'))
        else {
            continue;
        };
        let name_key = game.metadata().name_key().as_str();
        let names = Locale::ALL
            .into_iter()
            .filter_map(|locale| {
                game.messages(locale)
                    .iter()
                    .find(|(key, _)| *key == name_key)
                    .map(|(_, text)| (locale.tag().to_owned(), (*text).to_owned()))
            })
            .collect();
        games.push(BundledGame {
            id: entry.id().as_str().to_owned(),
            entry: ENTRY.to_owned(),
            query: query.to_owned(),
            names,
        });
    }
    if games.is_empty() {
        return Err(MobileStageError::NoLaunchableGame);
    }
    Ok(games)
}

pub fn run(args: &[String]) -> Result<MobileStageReport, MobileStageError> {
    if let Some(argument) = args.first() {
        return Err(MobileStageError::UnexpectedArgument(argument.clone()));
    }
    let root = crate::workspace::root()?;
    let wasm_src = resolve_wasm_source(&crate::workspace::target_dir()?)?;
    let out_dir = root.join("target").join("tabula-mobile-game");
    let report = stage_mobile_bundle(
        &root.join("apps/game-client/web"),
        &root.join("apps/web/style/tokens.css"),
        &wasm_src,
        &out_dir,
    )?;
    println!(
        "stage-mobile-game: staged {} into {}\n  - {} game(s): {}\n  - {} WASM bytes, {} manifest-listed runtime resources (loaded on demand, SHA-256 verified)",
        MANIFEST,
        report.out_dir.display(),
        report.games.len(),
        report
            .games
            .iter()
            .map(|game| game.id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        report.wasm_size,
        report.runtime_resource_count
    );
    Ok(report)
}

/// Content-addressed names a text points at: exactly 64 hex digits then `.<ext>`. [`Naming::Url`]
/// requires the `resources/` prefix, so a pack file alias that merely contains a BLAKE3 hash is not
/// mistaken for a file; [`Naming::Relative`] is for a stylesheet in `resources/` naming its siblings.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Naming {
    Url,
    Relative,
}

fn referenced_resources(text: &str, naming: Naming) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_hexdigit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_hexdigit() {
            at += 1;
        }
        if at - start != 64 || bytes.get(at) != Some(&b'.') {
            continue;
        }
        if naming == Naming::Url && !text[..start].ends_with("resources/") {
            continue;
        }
        let extension: String = text[at + 1..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !extension.is_empty() {
            found.insert(format!("resources/{}.{extension}", &text[start..at]));
        }
    }
    found
}

fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// Reduce the staged web document to what the app serves: `index.html` and the content-addressed
/// files it (and `resource-manifest.js`) reference. The standalone setup page, its cover art and the
/// fixed-name diagnostic copies are never fetched by the play document, and an app package should not
/// carry them. Fails if a reference is missing, so a pruning bug cannot ship a broken document.
fn prune_to_referenced(document: &Path) -> Result<usize, MobileStageError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| MobileStageError::Io { path, source }
    };
    let index = std::fs::read_to_string(document.join("index.html"))
        .map_err(io(&document.join("index.html")))?;
    let manifest = std::fs::read_to_string(document.join("resource-manifest.js"))
        .map_err(io(&document.join("resource-manifest.js")))?;
    let mut keep: BTreeSet<String> = referenced_resources(&index, Naming::Url)
        .union(&referenced_resources(&manifest, Naming::Url))
        .cloned()
        .collect();
    // Stylesheets name their fonts and images; whatever they reference stays too.
    let stylesheets: Vec<String> = keep
        .iter()
        .filter(|name| has_extension(name, "css"))
        .cloned()
        .collect();
    for name in stylesheets {
        if let Ok(css) = std::fs::read_to_string(document.join(&name)) {
            keep.extend(referenced_resources(&css, Naming::Relative));
        }
    }
    if let Some(missing) = keep.iter().find(|name| !document.join(name).is_file()) {
        return Err(WasmStageError::InvalidResource(format!(
            "referenced resource is missing: {missing}"
        ))
        .into());
    }
    let mut pruned = 0;
    let mut stack = vec![document.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).map_err(io(&directory))? {
            let path = entry.map_err(io(&directory))?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(document)
                .expect("walked below the document");
            let name = relative.to_string_lossy().replace('\\', "/");
            if name != "index.html" && !keep.contains(&name) {
                std::fs::remove_file(&path).map_err(io(&path))?;
                pruned += 1;
            }
        }
    }
    // Directories emptied by pruning (for example `assets/`) are not part of the package.
    let assets = document.join("assets");
    if assets.is_dir()
        && std::fs::read_dir(&assets)
            .map_err(io(&assets))?
            .next()
            .is_none()
    {
        std::fs::remove_dir(&assets).map_err(io(&assets))?;
    }
    Ok(pruned)
}

/// Build the whole bundle in a sibling temporary directory and publish it with one rename,
/// so a failed attempt never leaves a previous or half-written bundle for Gradle/Xcode.
pub fn stage_mobile_bundle(
    web_src_dir: &Path,
    tokens_src: &Path,
    wasm_src: &Path,
    out_dir: &Path,
) -> Result<MobileStageReport, MobileStageError> {
    remove_existing_destination(out_dir)?;
    let games = bundled_games()?;
    let parent = out_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| MobileStageError::Io { path, source }
    };
    std::fs::create_dir_all(parent).map_err(io(parent))?;
    let temporary = tempfile::Builder::new()
        .prefix(".tabula-mobile-game-")
        .tempdir_in(parent)
        .map_err(io(parent))?;
    let bundle = temporary.path().join("bundle");
    let document = bundle.join("play/local");
    std::fs::create_dir_all(document.parent().expect("document has a parent"))
        .map_err(io(&bundle))?;
    // `stage_bundle` replaces its destination, so it creates `document` itself.
    let staged = stage_bundle(web_src_dir, tokens_src, wasm_src, &document)?;
    promote_integrated_entry(&document)?;
    let pruned = prune_to_referenced(&document)?;
    let manifest = serde_json::to_string_pretty(&GamesManifest {
        schema: 1,
        games: &games,
    })?;
    std::fs::write(bundle.join(MANIFEST), format!("{manifest}\n")).map_err(io(&bundle))?;
    std::fs::rename(&bundle, out_dir).map_err(io(out_dir))?;
    Ok(MobileStageReport {
        out_dir: out_dir.to_path_buf(),
        pruned_files: pruned,
        games,
        wasm_size: staged.wasm_size,
        runtime_resource_count: staged.runtime_resource_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest as _;
    use tempfile::tempdir;

    const WASM: &[u8] = b"\0asm\x01\0\0\0";

    /// Every file below `document`, relative and `/`-separated, sorted.
    fn package_files(document: &Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut stack = vec![document.to_path_buf()];
        while let Some(directory) = stack.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path.strip_prefix(document).unwrap();
                    files.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        files.sort();
        files
    }

    fn real_web_dir() -> PathBuf {
        crate::workspace::root()
            .expect("workspace root")
            .join("apps/game-client/web")
    }

    fn tokens() -> PathBuf {
        crate::workspace::root()
            .expect("workspace root")
            .join("apps/web/style/tokens.css")
    }

    #[test]
    fn registry_derives_exactly_the_packaged_games_validated_launch() {
        let games = bundled_games().unwrap();
        assert_eq!(games.len(), 1, "only the packaged game is launchable");
        let game = &games[0];
        assert_eq!(game.entry, "/play/local/");
        // The same handoff the web shell navigates to, not a hand-built string.
        assert!(game
            .query
            .starts_with(&format!("game={}&mode=local&seats=2", game.id)));
        assert!(game.query.contains("&source=tabula&return_to=%2Fgames%2F"));
        assert!(game.query.ends_with("&locale=vi"));
        assert_eq!(game.names.len(), 2, "both required locales have a name");
        assert!(game.names.values().all(|name| !name.is_empty()));
    }

    #[test]
    fn stages_the_integrated_document_with_pinned_bridge_and_a_games_list() {
        let wasm_dir = tempdir().unwrap();
        let wasm = wasm_dir.path().join("tabula-game-client.wasm");
        std::fs::write(&wasm, WASM).unwrap();
        let out = tempdir().unwrap();
        let out_dir = out.path().join("tabula-mobile-game");

        let report = stage_mobile_bundle(&real_web_dir(), &tokens(), &wasm, &out_dir).unwrap();

        let document = out_dir.join("play/local");
        let html = std::fs::read_to_string(document.join("index.html")).unwrap();
        // The bridge is hashed, SRI-pinned like every other script, and loads before bootstrap.
        let hashed = |name: &str| {
            let bytes = std::fs::read(real_web_dir().join(name)).unwrap();
            let digest = sha2::Sha256::digest(bytes);
            let hex = digest.iter().fold(String::new(), |mut hex, byte| {
                use std::fmt::Write as _;
                write!(hex, "{byte:02x}").unwrap();
                hex
            });
            html.find(&format!("resources/{hex}.js"))
                .unwrap_or_else(|| panic!("{name} is not referenced by its content hash"))
        };
        assert!(hashed("host-bridge.js") < hashed("bootstrap.js"));
        assert!(
            !html.contains("host-bridge.js"),
            "unversioned bridge reference remains"
        );
        assert!(html.matches("integrity=\"sha256-").count() >= 5);
        assert!(
            !html.contains("href=\"index.html\""),
            "setup fallback must not dangle"
        );
        // Only the entry document and what it references are packaged.
        let files = package_files(&document);
        assert!(files.contains(&"index.html".to_owned()));
        assert!(
            files
                .iter()
                .all(|name| name == "index.html" || name.starts_with("resources/")),
            "unexpected package files: {files:?}"
        );
        let mut referenced = referenced_resources(&html, Naming::Url);
        for name in files.iter().filter(|name| has_extension(name, "js")) {
            let script = std::fs::read_to_string(document.join(name)).unwrap();
            if script.contains("TabulaResourceManifest") {
                referenced.extend(referenced_resources(&script, Naming::Url));
            }
        }
        for name in files.iter().filter(|name| has_extension(name, "css")) {
            let css = std::fs::read_to_string(document.join(name)).unwrap();
            referenced.extend(referenced_resources(&css, Naming::Relative));
        }
        assert!(
            files
                .iter()
                .filter(|name| name.starts_with("resources/"))
                .all(|name| referenced.contains(name)),
            "unreferenced resource packaged"
        );
        assert!(
            report.pruned_files > 10,
            "the standalone setup assets and fixed-name copies were pruned"
        );
        assert!(!document.join("standalone.html").exists() && !document.join("assets").exists());

        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out_dir.join(MANIFEST)).unwrap())
                .unwrap();
        assert_eq!(manifest["schema"], 1);
        assert_eq!(
            manifest["games"].as_array().unwrap().len(),
            report.games.len()
        );
        assert_eq!(report.wasm_size, WASM.len() as u64);
    }

    #[test]
    fn a_failed_stage_invalidates_the_previous_bundle() {
        let out = tempdir().unwrap();
        let out_dir = out.path().join("tabula-mobile-game");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(out_dir.join("stale.txt"), "previous").unwrap();
        let missing = out.path().join("absent.wasm");

        let error = stage_mobile_bundle(&real_web_dir(), &tokens(), &missing, &out_dir)
            .expect_err("a missing WASM artifact must fail");

        assert!(matches!(
            error,
            MobileStageError::Stage(WasmStageError::MissingWasmArtifact(_))
        ));
        assert!(
            !out_dir.exists(),
            "no previous or partial bundle may remain"
        );
    }

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn references_need_the_resources_prefix_unless_a_stylesheet_names_its_siblings() {
        let text = format!(
            "src=\"resources/{HASH_A}.js\" alias \"chess/0.1.0/pieces@1x.b3-{HASH_B}.png\" url({HASH_C}.ttf)"
        );
        let url = referenced_resources(&text, Naming::Url);
        assert_eq!(
            url.into_iter().collect::<Vec<_>>(),
            vec![format!("resources/{HASH_A}.js")],
            "a BLAKE3 pack alias is not a file"
        );
        let relative = referenced_resources(&text, Naming::Relative);
        assert!(relative.contains(&format!("resources/{HASH_C}.ttf")));
        assert!(!referenced_resources("resources/abc.js", Naming::Url).contains("resources/abc.js"));
    }

    #[test]
    fn pruning_keeps_exactly_what_is_referenced_and_fails_on_a_dangling_reference() {
        let dir = tempdir().unwrap();
        let document = dir.path();
        std::fs::create_dir_all(document.join("resources")).unwrap();
        std::fs::create_dir_all(document.join("assets")).unwrap();
        let write = |name: &str| std::fs::write(document.join(name), "x").unwrap();
        std::fs::write(
            document.join("index.html"),
            format!("<script src=\"resources/{HASH_A}.js\">"),
        )
        .unwrap();
        std::fs::write(
            document.join("resource-manifest.js"),
            format!("{{\"url\":\"resources/{HASH_B}.wasm\"}}"),
        )
        .unwrap();
        write(&format!("resources/{HASH_A}.js"));
        write(&format!("resources/{HASH_B}.wasm"));
        write(&format!("resources/{HASH_C}.png")); // unreferenced
        write("assets/OpenSans-Regular.ttf"); // fixed-name copy
        write("standalone.html");

        let pruned = prune_to_referenced(document).unwrap();

        assert_eq!(
            pruned, 4,
            "manifest script, pngs/fonts and the setup page go"
        );
        assert_eq!(
            package_files(document),
            vec![
                "index.html".to_owned(),
                format!("resources/{HASH_A}.js"),
                format!("resources/{HASH_B}.wasm"),
            ]
        );
        assert!(!document.join("assets").exists());

        std::fs::remove_file(document.join(format!("resources/{HASH_B}.wasm"))).unwrap();
        std::fs::write(
            document.join("resource-manifest.js"),
            format!("resources/{HASH_B}.wasm"),
        )
        .unwrap();
        assert!(
            prune_to_referenced(document).is_err(),
            "a dangling reference must fail the stage"
        );
    }

    #[test]
    fn unexpected_arguments_are_rejected() {
        assert!(matches!(
            run(&["--anything".to_owned()]),
            Err(MobileStageError::UnexpectedArgument(_))
        ));
    }
}
