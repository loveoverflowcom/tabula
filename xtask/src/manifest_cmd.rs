//! `cargo xtask check-manifests` — the shell around [`crate::manifest_policy`].

use std::collections::BTreeSet;
use std::path::PathBuf;

use cargo_metadata::MetadataCommand;

use crate::manifest_policy::{
    validate_cargo_manifest, validate_game_toml, ManifestParseError, ManifestViolation,
};

#[derive(Debug, thiserror::Error)]
pub enum CheckManifestsError {
    #[error("running `cargo metadata`: {0}")]
    Metadata(#[from] cargo_metadata::Error),
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Parse(#[from] ManifestParseError),
}

pub fn run() -> Result<bool, CheckManifestsError> {
    let metadata = MetadataCommand::new().no_deps().exec()?;
    let workspace_root = metadata.workspace_root.clone();

    let internal_crates: BTreeSet<String> =
        metadata.packages.iter().map(|p| p.name.clone()).collect();
    let members: BTreeSet<&cargo_metadata::PackageId> = metadata.workspace_members.iter().collect();

    let mut violations: Vec<ManifestViolation> = Vec::new();
    let mut manifests_checked = 0usize;

    for pkg in metadata.packages.iter().filter(|p| members.contains(&p.id)) {
        let manifest_path = pkg.manifest_path.as_std_path();
        let rel_path = manifest_path
            .strip_prefix(workspace_root.as_std_path())
            .unwrap_or(manifest_path)
            .to_string_lossy()
            .replace('\\', "/");

        let src =
            std::fs::read_to_string(manifest_path).map_err(|source| CheckManifestsError::Io {
                path: manifest_path.to_path_buf(),
                source,
            })?;
        manifests_checked += 1;
        violations.extend(validate_cargo_manifest(&rel_path, &src, &internal_crates)?);

        if rel_path.starts_with("games/") {
            let game_dir = manifest_path
                .parent()
                .expect("Cargo.toml has a parent directory");
            let game_toml_path = game_dir.join("game.toml");
            if game_toml_path.exists() {
                let game_toml_rel = rel_path.replace("Cargo.toml", "game.toml");
                let expected_id =
                    format!("com.tabula.{}", pkg.name.trim_start_matches("tabula-game-"));
                let game_src = std::fs::read_to_string(&game_toml_path).map_err(|source| {
                    CheckManifestsError::Io {
                        path: game_toml_path.clone(),
                        source,
                    }
                })?;
                manifests_checked += 1;
                violations.extend(validate_game_toml(&game_toml_rel, &game_src, &expected_id)?);
            }
        }
    }

    if violations.is_empty() {
        println!("check-manifests: {manifests_checked} manifest(s) checked — all clear");
        Ok(true)
    } else {
        for v in &violations {
            eprintln!("{v}\n");
        }
        eprintln!(
            "check-manifests: {} violation(s) across {manifests_checked} manifest(s)",
            violations.len()
        );
        Ok(false)
    }
}
