//! Pure validators for `xtask check-manifests`.
//!
//! Two manifest kinds are checked:
//!
//! - Every workspace member's `Cargo.toml`: workspace-field inheritance,
//!   no accidental wildcard registry dependencies, and internal crates
//!   referenced via `{ workspace = true }` rather than a duplicated path.
//!   Game crates additionally need the documented `rules` / `presentation` /
//!   `bots` / `testkit` feature shape (doc 01 §5.1 rule 3).
//! - `game.toml`, for the games that have one: the smallest schema that
//!   catches a missing or self-contradictory manifest (doc 02 §4.3). This
//!   deliberately does NOT compare against the compiled `GameMetadata` /
//!   `GameCapabilities` statics — that needs the `metadata_from_manifest!`
//!   proc macro doc 02 §10.2 calls for, which does not exist yet. Until it
//!   does, the two representations are validated independently rather than
//!   pretending to cross-check what cannot yet be cross-checked.
//!
//! Both take manifest *text*, not a file path, so they are unit-testable
//! with literal strings and no filesystem access.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use toml::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestViolation {
    pub manifest: String,
    pub field: String,
    pub message: String,
}

impl fmt::Display for ManifestViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "manifest violation:")?;
        writeln!(f, "  manifest: {}", self.manifest)?;
        writeln!(f, "  field: {}", self.field)?;
        write!(f, "  problem: {}", self.message)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to parse {0} as TOML: {1}")]
pub struct ManifestParseError(pub String, pub toml::de::Error);

const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
const GAME_FEATURES: &[&str] = &["rules", "presentation", "bots", "testkit"];

/// Validate one workspace member's `Cargo.toml` text.
pub fn validate_cargo_manifest(
    rel_path: &str,
    toml_src: &str,
    internal_crates: &BTreeSet<String>,
) -> Result<Vec<ManifestViolation>, ManifestParseError> {
    let doc: Value =
        toml::from_str(toml_src).map_err(|e| ManifestParseError(rel_path.to_string(), e))?;
    let mut out = Vec::new();
    let violation = |field: &str, message: String| ManifestViolation {
        manifest: rel_path.to_string(),
        field: field.to_string(),
        message,
    };

    if let Some(package) = doc.get("package").and_then(Value::as_table) {
        for field in ["edition", "license", "rust-version"] {
            match package.get(field) {
                None => out.push(violation(field, format!("missing `{field}` — inherit it with `{field}.workspace = true`"))),
                Some(v) if is_workspace_inherited(v) => {}
                Some(_) => out.push(violation(
                    field,
                    format!("`{field}` is hard-coded instead of inherited via `{field}.workspace = true`"),
                )),
            }
        }
    } else {
        out.push(violation(
            "package",
            "manifest has no [package] table".to_string(),
        ));
    }

    for table_name in DEPENDENCY_TABLES {
        let Some(table) = doc.get(*table_name).and_then(Value::as_table) else {
            continue;
        };
        for (dep_name, spec) in table {
            if let Some(msg) = wildcard_registry_version(spec) {
                out.push(violation(&format!("{table_name}.{dep_name}"), msg));
            }
            if internal_crates.contains(dep_name) {
                if let Some(msg) = raw_path_instead_of_workspace(spec) {
                    out.push(violation(&format!("{table_name}.{dep_name}"), msg));
                }
            }
        }
    }

    if rel_path.starts_with("games/") {
        out.extend(validate_game_feature_shape(rel_path, &doc));
    }

    Ok(out)
}

fn is_workspace_inherited(v: &Value) -> bool {
    v.get("workspace").and_then(Value::as_bool).unwrap_or(false)
}

/// A bare registry version requirement of `"*"` is a wildcard. Path and git
/// dependencies are exempt — that is Cargo's own escape hatch for internal
/// crates, not the accidental-laxity case this check is for.
fn wildcard_registry_version(spec: &Value) -> Option<String> {
    let is_path_or_git =
        |t: &toml::map::Map<String, Value>| t.contains_key("path") || t.contains_key("git");
    match spec {
        Value::String(s) if s.trim() == "*" => {
            Some("wildcard version requirement \"*\" on a registry dependency".to_string())
        }
        Value::Table(t) if !is_path_or_git(t) => match t.get("version") {
            Some(Value::String(s)) if s.trim() == "*" => {
                Some("wildcard version requirement \"*\" on a registry dependency".to_string())
            }
            _ => None,
        },
        _ => None,
    }
}

fn raw_path_instead_of_workspace(spec: &Value) -> Option<String> {
    match spec {
        Value::Table(t) if t.contains_key("path") && !is_workspace_inherited(spec) => {
            Some("an internal crate must be referenced via `{ workspace = true }`, not a duplicated `path`".to_string())
        }
        _ => None,
    }
}

fn validate_game_feature_shape(rel_path: &str, doc: &Value) -> Vec<ManifestViolation> {
    let mut out = Vec::new();
    let violation = |field: &str, message: String| ManifestViolation {
        manifest: rel_path.to_string(),
        field: field.to_string(),
        message,
    };

    let Some(features) = doc.get("features").and_then(Value::as_table) else {
        out.push(violation(
            "features",
            "game crates must declare a [features] table (doc 01 §5.1 rule 3)".to_string(),
        ));
        return out;
    };

    for required in GAME_FEATURES {
        if !features.contains_key(*required) {
            out.push(violation(
                "features",
                format!("missing the `{required}` feature — every game crate uses the same rules/presentation/bots/testkit shape"),
            ));
        }
    }

    match features.get("default").and_then(Value::as_array) {
        Some(arr) if arr.iter().any(|v| v.as_str() == Some("rules")) => {}
        _ => out.push(violation(
            "features.default",
            "the default feature set must include \"rules\" so a bare `cargo build` compiles the game".to_string(),
        )),
    }

    out
}

use tabula_assets::{AssetPackRef, AssetPackRefError};
use tabula_core::{ids::GameIdError, GameId};

// ---------------------------------------------------------------------------
// game.toml
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct GameToml {
    id: Option<String>,
    version: Option<String>,
    rules_version: Option<i64>,
    name_key: Option<String>,
    categories: Option<Vec<String>>,
    estimated_minutes: Option<(u32, u32)>,
    seats: Option<SeatsToml>,
    capabilities: Option<CapabilitiesToml>,
    assets: Option<AssetsToml>,
}

#[derive(Debug, Deserialize)]
struct SeatsToml {
    min: Option<u32>,
    max: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CapabilitiesToml {
    turn_model: Option<String>,
    spectators: Option<String>,
    durability: Option<String>,
    state_size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssetsToml {
    pack: Option<String>,
    #[allow(dead_code)]
    size_kb: Option<u64>,
}

const TURN_MODELS: &[&str] = &["strict_sequential", "simultaneous", "phased", "free_form"];
const SPECTATOR_POLICIES: &[&str] = &["forbidden", "live", "delayed", "game_controlled"];
const DURABILITIES: &[&str] = &["ack_after_apply", "ack_after_persist"];
const STATE_SIZES: &[&str] = &["tiny", "small", "medium", "large"];

/// The typed game and pinned asset-pack identity extracted from one validated
/// `game.toml`. The builder consumes this evidence instead of maintaining a
/// second source of truth for either identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GameAssetBinding {
    pub(crate) game: GameId,
    pub(crate) pack: AssetPackRef,
}

impl GameToml {
    pub(crate) fn asset_binding(&self) -> Result<GameAssetBinding, GameAssetBindingError> {
        let game = self
            .id
            .clone()
            .ok_or(GameAssetBindingError::MissingGameId)
            .and_then(|id| GameId::new(id).map_err(GameAssetBindingError::InvalidGameId))?;
        let pack = self
            .assets
            .as_ref()
            .and_then(|assets| assets.pack.clone())
            .ok_or(GameAssetBindingError::MissingPack)
            .and_then(|pack| {
                AssetPackRef::parse(&pack).map_err(GameAssetBindingError::InvalidPackRef)
            })?;
        Ok(GameAssetBinding { game, pack })
    }
}

/// Why a game manifest could not provide the typed identity a pack build needs.
#[derive(Debug, thiserror::Error)]
pub(crate) enum GameAssetBindingError {
    #[error("game.toml is missing required `id`")]
    MissingGameId,
    #[error("game.toml contains an invalid game id: {0}")]
    InvalidGameId(#[source] GameIdError),
    #[error("game.toml is missing required [assets].pack")]
    MissingPack,
    #[error("game.toml contains an invalid asset pack reference: {0}")]
    InvalidPackRef(#[source] AssetPackRefError),
}

pub(crate) fn parse_game_toml(
    rel_path: &str,
    toml_src: &str,
) -> Result<GameToml, ManifestParseError> {
    toml::from_str(toml_src).map_err(|e| ManifestParseError(rel_path.to_string(), e))
}

/// Validate `game.toml` text against the smallest schema that catches a
/// missing field or a self-contradictory value. `expected_id` is
/// `com.tabula.<game directory name>`.
pub fn validate_game_toml(
    rel_path: &str,
    toml_src: &str,
    expected_id: &str,
) -> Result<Vec<ManifestViolation>, ManifestParseError> {
    let doc = parse_game_toml(rel_path, toml_src)?;
    Ok(validate_game_document(rel_path, &doc, expected_id))
}

pub(crate) fn validate_game_document(
    rel_path: &str,
    doc: &GameToml,
    expected_id: &str,
) -> Vec<ManifestViolation> {
    let mut out = Vec::new();
    let violation = |field: &str, message: String| ManifestViolation {
        manifest: rel_path.to_string(),
        field: field.to_string(),
        message,
    };

    match &doc.id {
        None => out.push(violation("id", "missing required field `id`".to_string())),
        Some(id) if id != expected_id => out.push(violation(
            "id",
            format!("`id` is \"{id}\", expected \"{expected_id}\" (reverse-DNS, doc 02 §4.1)"),
        )),
        Some(_) => {}
    }
    for (field, present) in [
        ("version", doc.version.is_some()),
        ("rules_version", doc.rules_version.is_some()),
        ("name_key", doc.name_key.is_some()),
        ("categories", doc.categories.is_some()),
        ("estimated_minutes", doc.estimated_minutes.is_some()),
    ] {
        if !present {
            out.push(violation(
                field,
                format!("missing required field `{field}`"),
            ));
        }
    }

    match &doc.seats {
        None => out.push(violation(
            "seats",
            "missing required [seats] table".to_string(),
        )),
        Some(seats) => match (seats.min, seats.max) {
            (Some(min), Some(max)) if min >= 1 && min <= max => {}
            (Some(min), Some(max)) => out.push(violation(
                "seats",
                format!("seats.min ({min}) must be >= 1 and <= seats.max ({max})"),
            )),
            _ => out.push(violation(
                "seats",
                "[seats] must declare both `min` and `max`".to_string(),
            )),
        },
    }

    match &doc.capabilities {
        None => out.push(violation(
            "capabilities",
            "missing required [capabilities] table".to_string(),
        )),
        Some(caps) => {
            out.extend(check_enum(
                rel_path,
                "capabilities.turn_model",
                caps.turn_model.as_ref(),
                TURN_MODELS,
            ));
            out.extend(check_enum(
                rel_path,
                "capabilities.spectators",
                caps.spectators.as_ref(),
                SPECTATOR_POLICIES,
            ));
            out.extend(check_enum(
                rel_path,
                "capabilities.durability",
                caps.durability.as_ref(),
                DURABILITIES,
            ));
            out.extend(check_enum(
                rel_path,
                "capabilities.state_size",
                caps.state_size.as_ref(),
                STATE_SIZES,
            ));
        }
    }

    match &doc.assets {
        None => out.push(violation(
            "assets",
            "missing required [assets] table".to_string(),
        )),
        Some(assets) => match &assets.pack {
            None => out.push(violation(
                "assets.pack",
                "missing required field `assets.pack`".to_string(),
            )),
            Some(pack) => {
                if let Err(err) = AssetPackRef::parse(pack) {
                    out.push(violation(
                        "assets.pack",
                        format!("invalid asset pack reference \"{pack}\": {err}"),
                    ));
                }
            }
        },
    }

    out
}

fn check_enum(
    rel_path: &str,
    field: &str,
    value: Option<&String>,
    allowed: &[&str],
) -> Option<ManifestViolation> {
    match value {
        None => Some(ManifestViolation {
            manifest: rel_path.to_string(),
            field: field.to_string(),
            message: format!("missing required field `{field}`"),
        }),
        Some(v) if !allowed.contains(&v.as_str()) => Some(ManifestViolation {
            manifest: rel_path.to_string(),
            field: field.to_string(),
            message: format!("`{v}` is not one of {allowed:?}"),
        }),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crates() -> BTreeSet<String> {
        ["tabula-core", "tabula-game-api", "tabula-presentation"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn valid_game_crate_manifest_passes() {
        let src = r#"
            [package]
            name = "tabula-game-tictactoe"
            version = "0.1.0"
            edition.workspace = true
            rust-version.workspace = true
            license.workspace = true

            [features]
            default = ["rules"]
            rules = []
            presentation = []
            bots = []
            testkit = []

            [dependencies]
            tabula-core = { workspace = true }
            serde = { version = "1", features = ["derive"] }
        "#;
        let violations =
            validate_cargo_manifest("games/tictactoe/Cargo.toml", src, &crates()).unwrap();
        assert!(
            violations.is_empty(),
            "expected a clean manifest, got {violations:?}"
        );
    }

    #[test]
    fn missing_workspace_inherited_fields_fails() {
        let src = r#"
            [package]
            name = "tabula-core"
            version = "0.1.0"
            edition = "2021"
        "#;
        let violations =
            validate_cargo_manifest("crates/tabula-core/Cargo.toml", src, &crates()).unwrap();
        assert!(violations.iter().any(|v| v.field == "edition"));
        assert!(violations.iter().any(|v| v.field == "license"));
        assert!(violations.iter().any(|v| v.field == "rust-version"));
    }

    #[test]
    fn wildcard_registry_dependency_fails() {
        let src = r#"
            [package]
            name = "tabula-core"
            version = "0.1.0"
            edition.workspace = true
            rust-version.workspace = true
            license.workspace = true

            [dependencies]
            serde = "*"
        "#;
        let violations =
            validate_cargo_manifest("crates/tabula-core/Cargo.toml", src, &crates()).unwrap();
        assert!(violations
            .iter()
            .any(|v| v.field.contains("serde") && v.message.contains("wildcard")));
    }

    #[test]
    fn path_dependency_on_internal_crate_instead_of_workspace_fails() {
        let src = r#"
            [package]
            name = "tabula-game-api"
            version = "0.1.0"
            edition.workspace = true
            rust-version.workspace = true
            license.workspace = true

            [dependencies]
            tabula-core = { path = "../../crates/tabula-core" }
        "#;
        let violations =
            validate_cargo_manifest("crates/tabula-game-api/Cargo.toml", src, &crates()).unwrap();
        assert!(violations
            .iter()
            .any(|v| v.field.contains("tabula-core") && v.message.contains("workspace = true")));
    }

    #[test]
    fn game_crate_missing_presentation_feature_fails() {
        let src = r#"
            [package]
            name = "tabula-game-chess"
            version = "0.0.0"
            edition.workspace = true
            rust-version.workspace = true
            license.workspace = true

            [features]
            default = ["rules"]
            rules = []
            bots = []
            testkit = []
        "#;
        let violations = validate_cargo_manifest("games/chess/Cargo.toml", src, &crates()).unwrap();
        assert!(violations
            .iter()
            .any(|v| v.message.contains("presentation")));
    }

    #[test]
    fn valid_game_toml_passes() {
        let src = r#"
            id = "com.tabula.tictactoe"
            version = "0.1.0"
            rules_version = 1
            name_key = "game.tictactoe.name"
            categories = ["abstract"]
            estimated_minutes = [1, 3]

            [seats]
            min = 2
            max = 2

            [capabilities]
            turn_model = "strict_sequential"
            hidden_information = false
            spectators = "live"
            durability = "ack_after_apply"
            state_size = "tiny"

            [assets]
            pack = "tictactoe@0.1.0"
            size_kb = 40
        "#;
        let violations =
            validate_game_toml("games/tictactoe/game.toml", src, "com.tabula.tictactoe").unwrap();
        assert!(
            violations.is_empty(),
            "expected a clean game.toml, got {violations:?}"
        );
    }

    #[test]
    fn game_toml_missing_metadata_fails() {
        let src = r#"
            id = "com.tabula.tictactoe"
            version = "0.1.0"
        "#;
        let violations =
            validate_game_toml("games/tictactoe/game.toml", src, "com.tabula.tictactoe").unwrap();
        assert!(violations.iter().any(|v| v.field == "rules_version"));
        assert!(violations.iter().any(|v| v.field == "seats"));
        assert!(violations.iter().any(|v| v.field == "capabilities"));
        assert!(violations.iter().any(|v| v.field == "assets"));
    }

    #[test]
    fn game_toml_invalid_capability_enum_fails() {
        let src = r#"
            id = "com.tabula.tictactoe"
            version = "0.1.0"
            rules_version = 1
            name_key = "game.tictactoe.name"
            categories = ["abstract"]
            estimated_minutes = [1, 3]

            [seats]
            min = 2
            max = 2

            [capabilities]
            turn_model = "whenever_i_feel_like_it"
            hidden_information = false
            spectators = "live"
            durability = "ack_after_apply"
            state_size = "tiny"

            [assets]
            pack = "tictactoe@0.1.0"
        "#;
        let violations =
            validate_game_toml("games/tictactoe/game.toml", src, "com.tabula.tictactoe").unwrap();
        assert!(violations
            .iter()
            .any(|v| v.field == "capabilities.turn_model"));
    }

    #[test]
    fn game_toml_id_mismatch_fails() {
        let src = r#"
            id = "com.tabula.wrongname"
            version = "0.1.0"
            rules_version = 1
            name_key = "game.tictactoe.name"
            categories = ["abstract"]
            estimated_minutes = [1, 3]

            [seats]
            min = 2
            max = 2

            [capabilities]
            turn_model = "strict_sequential"
            hidden_information = false
            spectators = "live"
            durability = "ack_after_apply"
            state_size = "tiny"

            [assets]
            pack = "tictactoe@0.1.0"
        "#;
        let violations =
            validate_game_toml("games/tictactoe/game.toml", src, "com.tabula.tictactoe").unwrap();
        assert!(violations.iter().any(|v| v.field == "id"));
    }

    #[test]
    fn seats_min_greater_than_max_fails() {
        let src = r#"
            id = "com.tabula.tictactoe"
            version = "0.1.0"
            rules_version = 1
            name_key = "game.tictactoe.name"
            categories = ["abstract"]
            estimated_minutes = [1, 3]

            [seats]
            min = 5
            max = 2

            [capabilities]
            turn_model = "strict_sequential"
            hidden_information = false
            spectators = "live"
            durability = "ack_after_apply"
            state_size = "tiny"

            [assets]
            pack = "tictactoe@0.1.0"
        "#;
        let violations =
            validate_game_toml("games/tictactoe/game.toml", src, "com.tabula.tictactoe").unwrap();
        assert!(violations.iter().any(|v| v.field == "seats"));
    }

    #[test]
    fn game_toml_invalid_asset_pack_ref_fails() {
        let base = r#"
            id = "com.tabula.chess"
            version = "0.1.0"
            rules_version = 3
            name_key = "game.chess.name"
            categories = ["abstract"]
            estimated_minutes = [10, 90]

            [seats]
            min = 2
            max = 2

            [capabilities]
            turn_model = "strict_sequential"
            hidden_information = false
            spectators = "live"
            durability = "ack_after_persist"
            state_size = "small"
        "#;

        for invalid_pack in [
            "chess",
            "@0.1.0",
            "chess@",
            "chess@bad",
            "chess@not-semver",
            "chess@@1.0.0",
            " chess@1.0.0",
            "chess@1.0.0 ",
            "chess/sub@1.0.0",
        ] {
            let src = format!("{base}\n[assets]\npack = \"{invalid_pack}\"");
            let violations =
                validate_game_toml("games/chess/game.toml", &src, "com.tabula.chess").unwrap();
            assert!(
                violations.iter().any(|v| v.field == "assets.pack"
                    && v.message.contains("invalid asset pack reference")),
                "expected rejection of pack = \"{invalid_pack}\", got {violations:?}"
            );
        }

        // Valid pack passes
        let valid_src = format!("{base}\n[assets]\npack = \"chess@0.1.0\"");
        let violations =
            validate_game_toml("games/chess/game.toml", &valid_src, "com.tabula.chess").unwrap();
        assert!(
            violations.is_empty(),
            "expected clean validation for valid pack, got {violations:?}"
        );
    }

    #[test]
    fn committed_game_manifests_declare_valid_asset_packs_matching_presenters() {
        use tabula_game_chess::presentation::ChessPresentation;
        use tabula_presentation::GamePresentation;

        let chess_toml = include_str!("../../games/chess/game.toml");
        let violations =
            validate_game_toml("games/chess/game.toml", chess_toml, "com.tabula.chess").unwrap();
        assert!(
            violations.is_empty(),
            "chess game.toml must be valid: {violations:?}"
        );

        let doc: GameToml = toml::from_str(chess_toml).unwrap();
        let manifest_pack_str = doc
            .assets
            .as_ref()
            .and_then(|a| a.pack.as_ref())
            .expect("chess game.toml must declare [assets].pack");
        let manifest_pack = AssetPackRef::parse(manifest_pack_str)
            .expect("chess game.toml asset pack must be valid");
        assert_eq!(ChessPresentation::asset_pack(), manifest_pack);

        let tictactoe_toml = include_str!("../../games/tictactoe/game.toml");
        let violations = validate_game_toml(
            "games/tictactoe/game.toml",
            tictactoe_toml,
            "com.tabula.tictactoe",
        )
        .unwrap();
        assert!(
            violations.is_empty(),
            "tictactoe game.toml must be valid: {violations:?}"
        );
    }
}
