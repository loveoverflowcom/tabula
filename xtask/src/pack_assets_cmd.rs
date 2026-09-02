//! cargo xtask pack-assets <game> — deterministic asset-pack production.
//!
//! The filesystem-facing functions in this module are an imperative shell
//! around the pure plan_pack transformation. Source bytes are inspected
//! before planning, generated output is staged, and the generated manifest is
//! consumed by the runtime tabula-assets parser before publication.

#![allow(clippy::doc_markdown)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tabula_assets::{
    AssetFileName, AssetFileNameError, AssetIntegrityError, AssetPackManifest, AssetPackRef,
    AssetPath, AssetPathError, ManifestBindingError,
};

use crate::manifest_policy::{
    self, GameAssetBinding, GameAssetBindingError, ManifestParseError, ManifestViolation,
};

// ---------------------------------------------------------------------------
// Builder input and proof-bearing source facts
// ---------------------------------------------------------------------------

/// Builder-only source description. It deliberately contains no generated
/// path, hash, or byte-size facts.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceManifest {
    files: Vec<SourceFileSpec>,
    resources: Vec<SourceResourceSpec>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceFileSpec {
    name: String,
    source: String,
    priority: String,
    density: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceResourceSpec {
    id: String,
    variants: Vec<SourceResourceVariantSpec>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceResourceVariantSpec {
    file: String,
    region: Option<SourceRegionSpec>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceRegionSpec {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

/// A safe source path relative to assets/packs/<game>/.
///
/// This is a builder trust boundary, not an emitted AssetPath. Hostile path
/// spellings are rejected rather than normalized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PackSourcePath(String);

impl PackSourcePath {
    fn new(value: impl Into<String>) -> Result<Self, PackSourcePathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(PackSourcePathError::Empty);
        }
        if value.starts_with('/') || value.contains('\\') || value.contains(':') {
            return Err(PackSourcePathError::AbsoluteOrPlatformPath);
        }
        if value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(PackSourcePathError::WhitespaceOrControl);
        }
        for segment in value.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(PackSourcePathError::NonCanonicalSegment);
            }
            if !segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'@')
            }) {
                return Err(PackSourcePathError::InvalidCharacter);
            }
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }

    fn parent(&self) -> Option<&str> {
        self.as_str().rsplit_once('/').map(|(parent, _)| parent)
    }

    fn file_name(&self) -> &str {
        self.as_str()
            .rsplit_once('/')
            .map_or_else(|| self.as_str(), |(_, file_name)| file_name)
    }
}

/// Why a builder source path cannot be used as a relative source lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PackSourcePathError {
    #[error("source path must not be empty")]
    Empty,
    #[error("source path must be relative and use '/' separators")]
    AbsoluteOrPlatformPath,
    #[error("source path contains '.', '..', or an empty component")]
    NonCanonicalSegment,
    #[error("source path must not contain whitespace or control characters")]
    WhitespaceOrControl,
    #[error("source path contains a character outside the ASCII-safe asset grammar")]
    InvalidCharacter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InspectedSourceFile {
    spec: SourceFileSpec,
    source: PackSourcePath,
    bytes: Vec<u8>,
    byte_size: u64,
    digest: String,
}

/// Computes source facts from exact bytes without reading the filesystem.
fn inspect_source_bytes(
    spec: &SourceFileSpec,
    bytes: Vec<u8>,
) -> Result<InspectedSourceFile, PackBuildError> {
    let source = PackSourcePath::new(spec.source.clone()).map_err(|source| {
        PackBuildError::InvalidSourcePath {
            path: spec.source.clone(),
            source,
        }
    })?;
    AssetFileName::new(spec.name.clone()).map_err(|source| PackBuildError::InvalidFileName {
        name: spec.name.clone(),
        source,
    })?;
    let byte_size = u64::try_from(bytes.len()).map_err(|_| PackBuildError::ByteCountOverflow {
        name: spec.name.clone(),
    })?;
    if byte_size == 0 {
        return Err(PackBuildError::EmptySourceFile {
            name: spec.name.clone(),
        });
    }
    let digest = blake3::hash(&bytes).to_hex().to_string();
    Ok(InspectedSourceFile {
        spec: spec.clone(),
        source,
        bytes,
        byte_size,
        digest,
    })
}

// ---------------------------------------------------------------------------
// Pure build plan
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct PackBuildPlan {
    manifest_toml: String,
    files: Vec<PlannedOutputFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlannedOutputFile {
    path: AssetPath,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct FilePlan {
    runtime: Vec<RuntimeFileSpec>,
    output: Vec<PlannedOutputFile>,
    density_by_name: BTreeMap<String, Option<u64>>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeManifestSpec {
    pack: String,
    version: String,
    game: String,
    files: Vec<RuntimeFileSpec>,
    resources: Vec<RuntimeResourceSpec>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeFileSpec {
    name: String,
    path: String,
    hash: String,
    bytes: u64,
    priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    density: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeResourceSpec {
    id: String,
    variants: Vec<RuntimeResourceVariantSpec>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeResourceVariantSpec {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<RuntimeRegionSpec>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeRegionSpec {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

/// Creates the canonical full-digest physical path for one exact source.
///
/// The path commits to pack identity, version, source parent, source filename
/// stem, and the complete lowercase BLAKE3 digest. It does not read files or
/// consult filesystem metadata.
///
/// @ai.role canonicalization
/// @ai.domain assets.pack-build
/// @ai.pure true
/// @ai.invariant content-path-commits-to-bytes
/// @ai.evidence tests::content_addressed_path_commits_to_exact_bytes
fn content_addressed_path(
    pack: &AssetPackRef,
    source: &PackSourcePath,
    digest: &str,
) -> Result<AssetPath, PackBuildError> {
    if digest.len() != blake3::OUT_LEN * 2
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(PackBuildError::InvalidDigest(digest.to_string()));
    }

    let source_name = source.file_name();
    let output_name = source_name
        .rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map_or_else(
            || format!("{source_name}.b3-{digest}"),
            |(stem, extension)| format!("{stem}.b3-{digest}.{extension}"),
        );
    let prefix = format!("{}/{}/", pack.pack(), pack.version());
    let path = match source.parent() {
        Some(parent) => format!("{prefix}{parent}/{output_name}"),
        None => format!("{prefix}{output_name}"),
    };
    AssetPath::new(path.clone()).map_err(|source_error| PackBuildError::InvalidGeneratedPath {
        path,
        source: source_error,
    })
}

/// Builds a complete runtime manifest and its stable output list from already
/// inspected plain values. This is the functional core of pack-assets.
///
/// All source declarations are canonicalized before TOML serialization. No
/// filesystem, environment, clock, random source, or directory iteration is
/// consulted here.
///
/// @ai.role functional-core
/// @ai.domain assets.pack-build
/// @ai.pure true
/// @ai.invariant deterministic-pack-plan
/// @ai.invariant content-path-commits-to-bytes
/// @ai.law declaration-order-independence
/// @ai.evidence tests::declaration_order_does_not_change_the_pack_plan
/// @ai.evidence tests::one_byte_changes_content_identity
/// @ai.evidence tests::generated_manifest_round_trips_through_runtime_parser
fn plan_pack(
    binding: &GameAssetBinding,
    source_manifest: &SourceManifest,
    inspected: Vec<InspectedSourceFile>,
) -> Result<PackBuildPlan, PackBuildError> {
    let files_by_name = collect_inspected_files(source_manifest, inspected)?;
    let file_plan = plan_files(binding, source_manifest, &files_by_name)?;
    let runtime_resources = plan_resources(source_manifest, &file_plan.density_by_name);
    let runtime_manifest = RuntimeManifestSpec {
        pack: binding.pack.pack().to_string(),
        version: binding.pack.version().to_string(),
        game: binding.game.to_string(),
        files: file_plan.runtime,
        resources: runtime_resources,
    };
    let manifest_toml =
        toml::to_string(&runtime_manifest).map_err(PackBuildError::ManifestSerialization)?;
    Ok(PackBuildPlan {
        manifest_toml,
        files: file_plan.output,
    })
}

fn collect_inspected_files(
    source_manifest: &SourceManifest,
    inspected: Vec<InspectedSourceFile>,
) -> Result<BTreeMap<String, InspectedSourceFile>, PackBuildError> {
    let mut files_by_name = BTreeMap::new();
    let mut declared_names = BTreeSet::new();
    let mut declared_sources = BTreeSet::new();
    for declaration in &source_manifest.files {
        if !declared_names.insert(declaration.name.clone()) {
            return Err(PackBuildError::DuplicateFileName(declaration.name.clone()));
        }
        if !declared_sources.insert(declaration.source.clone()) {
            return Err(PackBuildError::DuplicateSourcePath(
                declaration.source.clone(),
            ));
        }
    }

    for file in inspected {
        let duplicate_name = file.spec.name.clone();
        if files_by_name.insert(duplicate_name.clone(), file).is_some() {
            return Err(PackBuildError::DuplicateFileName(duplicate_name));
        }
    }

    if files_by_name.len() != source_manifest.files.len() {
        let missing = source_manifest
            .files
            .iter()
            .find(|declaration| !files_by_name.contains_key(&declaration.name))
            .map_or_else(
                || "<unknown>".to_string(),
                |declaration| declaration.name.clone(),
            );
        return Err(PackBuildError::MissingInspection(missing));
    }
    Ok(files_by_name)
}

fn plan_files(
    binding: &GameAssetBinding,
    source_manifest: &SourceManifest,
    files_by_name: &BTreeMap<String, InspectedSourceFile>,
) -> Result<FilePlan, PackBuildError> {
    let mut generated_paths = BTreeSet::new();
    let mut runtime_files = Vec::with_capacity(files_by_name.len());
    let mut planned_files = Vec::with_capacity(files_by_name.len());
    let mut density_by_name = BTreeMap::new();
    for file in files_by_name.values() {
        if !source_manifest
            .files
            .iter()
            .any(|declaration| declaration == &file.spec)
        {
            return Err(PackBuildError::UnexpectedInspection(file.spec.name.clone()));
        }
        if file.source.as_str() != file.spec.source {
            return Err(PackBuildError::InspectionMismatch(file.spec.name.clone()));
        }
        let path = content_addressed_path(&binding.pack, &file.source, &file.digest)?;
        if !generated_paths.insert(path.clone()) {
            return Err(PackBuildError::DuplicateGeneratedPath(path.to_string()));
        }
        density_by_name.insert(file.spec.name.clone(), file.spec.density);
        runtime_files.push(RuntimeFileSpec {
            name: file.spec.name.clone(),
            path: path.to_string(),
            hash: file.digest.clone(),
            bytes: file.byte_size,
            priority: file.spec.priority.clone(),
            density: file.spec.density,
        });
        planned_files.push(PlannedOutputFile {
            path,
            bytes: file.bytes.clone(),
        });
    }

    runtime_files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.name.cmp(&right.name))
    });
    planned_files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(FilePlan {
        runtime: runtime_files,
        output: planned_files,
        density_by_name,
    })
}

fn plan_resources(
    source_manifest: &SourceManifest,
    density_by_name: &BTreeMap<String, Option<u64>>,
) -> Vec<RuntimeResourceSpec> {
    let mut runtime_resources: Vec<_> = source_manifest
        .resources
        .iter()
        .map(|resource| {
            let mut variants: Vec<_> = resource
                .variants
                .iter()
                .map(|variant| RuntimeResourceVariantSpec {
                    file: variant.file.clone(),
                    region: variant.region.as_ref().map(|region| RuntimeRegionSpec {
                        x: region.x,
                        y: region.y,
                        width: region.width,
                        height: region.height,
                    }),
                })
                .collect();
            variants.sort_by_key(|variant| {
                (
                    density_by_name
                        .get(&variant.file)
                        .copied()
                        .flatten()
                        .unwrap_or(0),
                    variant.file.clone(),
                    region_sort_key(variant.region.as_ref()),
                )
            });
            RuntimeResourceSpec {
                id: resource.id.clone(),
                variants,
            }
        })
        .collect();
    runtime_resources.sort_by(|left, right| left.id.cmp(&right.id));
    runtime_resources
}

fn region_sort_key(region: Option<&RuntimeRegionSpec>) -> (u8, u64, u64, u64, u64) {
    region.map_or((0, 0, 0, 0, 0), |region| {
        (1, region.x, region.y, region.width, region.height)
    })
}

/// Errors produced by the pure source-fact and plan transformations.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PackBuildError {
    #[error("invalid source path {path:?}: {source}")]
    InvalidSourcePath {
        path: String,
        source: PackSourcePathError,
    },
    #[error("invalid manifest file name {name:?}: {source}")]
    InvalidFileName {
        name: String,
        source: AssetFileNameError,
    },
    #[error("source asset {name:?} is empty")]
    EmptySourceFile { name: String },
    #[error("source asset {name:?} is too large to represent as a manifest byte count")]
    ByteCountOverflow { name: String },
    #[error("invalid BLAKE3 digest fact {0:?}")]
    InvalidDigest(String),
    #[error("generated asset path {path:?} is invalid: {source}")]
    InvalidGeneratedPath {
        path: String,
        source: AssetPathError,
    },
    #[error("duplicate source file name {0:?}")]
    DuplicateFileName(String),
    #[error("duplicate source path {0:?}")]
    DuplicateSourcePath(String),
    #[error("duplicate generated asset path {0:?}")]
    DuplicateGeneratedPath(String),
    #[error("no inspected facts were supplied for source file {0:?}")]
    MissingInspection(String),
    #[error("inspected fact does not belong to the source manifest: {0:?}")]
    UnexpectedInspection(String),
    #[error("inspected source path does not match its source declaration: {0:?}")]
    InspectionMismatch(String),
    #[error("could not serialize generated runtime manifest: {0}")]
    ManifestSerialization(#[source] toml::ser::Error),
}

// ---------------------------------------------------------------------------
// Imperative shell
// ---------------------------------------------------------------------------

/// Runs the command using the current repository as its input root.
pub(crate) fn run() -> Result<(), PackAssetsError> {
    let mut args = env::args().skip(2);
    let Some(game) = args.next() else {
        return Err(PackAssetsError::Usage);
    };
    if args.next().is_some() {
        return Err(PackAssetsError::Usage);
    }
    let repository_root = env::current_dir().map_err(PackAssetsError::CurrentDirectory)?;
    let output_root = repository_root.join("target").join("asset-packs");
    let summary = build_pack(&repository_root, &output_root, &game)?;
    println!(
        "pack-assets: published {}@{} ({} file(s)) to {}",
        summary.pack,
        summary.version,
        summary.file_count,
        summary.output.display()
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BuildSummary {
    pack: String,
    version: String,
    file_count: usize,
    output: PathBuf,
}

/// Builds and publishes one pack beneath an explicitly supplied output root.
///
/// The final output path is untouched until source inspection, planning,
/// runtime-manifest parsing, binding validation, and per-file integrity checks
/// all succeed. Publication replaces only the exact generated pack-version
/// directory; it does not claim crash-atomic replacement on every OS.
///
/// @ai.role build-orchestrator
/// @ai.domain assets.pack-build
/// @ai.pure false
/// @ai.requires deterministic-pack-plan
fn build_pack(
    repository_root: &Path,
    output_root: &Path,
    game: &str,
) -> Result<BuildSummary, PackAssetsError> {
    let game = GameDirectoryName::new(game)?;
    let game_root = repository_root.join("games").join(game.as_str());
    let game_manifest_path = game_root.join("game.toml");
    let game_manifest_source = read_text(&game_manifest_path)?;
    let game_manifest_rel = format!("games/{}/game.toml", game.as_str());
    let game_manifest =
        manifest_policy::parse_game_toml(&game_manifest_rel, &game_manifest_source)?;
    let expected_game_id = format!("com.tabula.{}", game.as_str());
    let violations = manifest_policy::validate_game_document(
        &game_manifest_rel,
        &game_manifest,
        &expected_game_id,
    );
    if !violations.is_empty() {
        return Err(PackAssetsError::InvalidGameManifest {
            path: game_manifest_path,
            violations,
        });
    }
    let binding = game_manifest.asset_binding()?;

    let pack_root = repository_root
        .join("assets")
        .join("packs")
        .join(game.as_str());
    let source_manifest_path = pack_root.join("pack.source.toml");
    let source_manifest_source = read_text(&source_manifest_path)?;
    let source_manifest: SourceManifest =
        toml::from_str(&source_manifest_source).map_err(|source| {
            PackAssetsError::InvalidSourceManifest {
                path: source_manifest_path.clone(),
                source,
            }
        })?;

    let inspected = inspect_source_files(&pack_root, &source_manifest)?;
    let plan = plan_pack(&binding, &source_manifest, inspected).map_err(PackAssetsError::Plan)?;
    publish_staged_pack(output_root, &binding, &plan)
}

fn inspect_source_files(
    pack_root: &Path,
    source_manifest: &SourceManifest,
) -> Result<Vec<InspectedSourceFile>, PackAssetsError> {
    let mut inspected = Vec::with_capacity(source_manifest.files.len());
    for spec in &source_manifest.files {
        let source = PackSourcePath::new(spec.source.clone()).map_err(|source| {
            PackAssetsError::Plan(PackBuildError::InvalidSourcePath {
                path: spec.source.clone(),
                source,
            })
        })?;
        let bytes = read_regular_source_file(pack_root, &source)?;
        inspected.push(inspect_source_bytes(spec, bytes).map_err(PackAssetsError::Plan)?);
    }
    Ok(inspected)
}

fn read_regular_source_file(
    pack_root: &Path,
    source: &PackSourcePath,
) -> Result<Vec<u8>, PackAssetsError> {
    let root_metadata =
        fs::symlink_metadata(pack_root).map_err(|source_error| PackAssetsError::SourceIo {
            path: pack_root.to_path_buf(),
            source: source_error,
        })?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(PackAssetsError::SourceRootNotDirectory(
            pack_root.to_path_buf(),
        ));
    }

    let mut current = pack_root.to_path_buf();
    let components: Vec<_> = source.as_str().split('/').collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|source_error| PackAssetsError::SourceIo {
                path: current.clone(),
                source: source_error,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(PackAssetsError::SourceSymlink(current));
        }
        let is_last = index + 1 == components.len();
        if is_last {
            if !metadata.is_file() {
                return Err(PackAssetsError::SourceNotRegular(current));
            }
        } else if !metadata.is_dir() {
            return Err(PackAssetsError::SourceAncestorNotDirectory(current));
        }
    }
    fs::read(&current).map_err(|source_error| PackAssetsError::SourceIo {
        path: current,
        source: source_error,
    })
}

fn publish_staged_pack(
    output_root: &Path,
    binding: &GameAssetBinding,
    plan: &PackBuildPlan,
) -> Result<BuildSummary, PackAssetsError> {
    fs::create_dir_all(output_root).map_err(|source| PackAssetsError::OutputIo {
        path: output_root.to_path_buf(),
        source,
    })?;
    let stage = tempfile::tempdir_in(output_root).map_err(|source| PackAssetsError::StageIo {
        path: output_root.to_path_buf(),
        source,
    })?;
    let stage_root = stage.path();
    for planned in &plan.files {
        let path = stage_root.join(planned.path.as_str());
        write_staged_file(&path, &planned.bytes)?;
    }
    let manifest_path = stage_root
        .join(binding.pack.pack().as_str())
        .join(binding.pack.version().as_str())
        .join("pack.toml");
    write_staged_file(&manifest_path, plan.manifest_toml.as_bytes())?;

    let generated_manifest_source = read_text(&manifest_path)?;
    let manifest = AssetPackManifest::from_toml(&generated_manifest_source).map_err(|source| {
        PackAssetsError::GeneratedManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;
    let _bound = manifest
        .validate_binding(&binding.pack, &binding.game)
        .map_err(|source| PackAssetsError::Binding {
            path: manifest_path.clone(),
            source,
        })?;
    for file in manifest.files() {
        let path = stage_root.join(file.path().as_str());
        let bytes = read_staged_file(&path)?;
        file.verify_bytes(&bytes)
            .map_err(|source| PackAssetsError::Integrity { path, source })?;
    }

    let final_pack_root = output_root.join(binding.pack.pack().as_str());
    fs::create_dir_all(&final_pack_root).map_err(|source| PackAssetsError::OutputIo {
        path: final_pack_root.clone(),
        source,
    })?;
    let final_version_root = final_pack_root.join(binding.pack.version().as_str());
    match fs::symlink_metadata(&final_version_root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(PackAssetsError::ExistingOutputNotDirectory(
                    final_version_root,
                ));
            }
            fs::remove_dir_all(&final_version_root).map_err(|source| {
                PackAssetsError::OutputIo {
                    path: final_version_root.clone(),
                    source,
                }
            })?;
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(PackAssetsError::OutputIo {
                path: final_version_root,
                source,
            });
        }
    }
    let staged_version_root = stage_root
        .join(binding.pack.pack().as_str())
        .join(binding.pack.version().as_str());
    fs::rename(&staged_version_root, &final_version_root).map_err(|source| {
        PackAssetsError::Publish {
            from: staged_version_root,
            to: final_version_root.clone(),
            source,
        }
    })?;

    Ok(BuildSummary {
        pack: binding.pack.pack().to_string(),
        version: binding.pack.version().to_string(),
        file_count: manifest.files().len(),
        output: final_version_root,
    })
}

fn write_staged_file(path: &Path, bytes: &[u8]) -> Result<(), PackAssetsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PackAssetsError::StageIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| PackAssetsError::StageIo {
        path: path.to_path_buf(),
        source,
    })
}

fn read_staged_file(path: &Path) -> Result<Vec<u8>, PackAssetsError> {
    fs::read(path).map_err(|source| PackAssetsError::StageIo {
        path: path.to_path_buf(),
        source,
    })
}

fn read_text(path: &Path) -> Result<String, PackAssetsError> {
    fs::read_to_string(path).map_err(|source| PackAssetsError::InputIo {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GameDirectoryName(String);

impl GameDirectoryName {
    fn new(value: &str) -> Result<Self, PackAssetsError> {
        let source = PackSourcePath::new(value)
            .map_err(|_| PackAssetsError::InvalidGameDirectory(value.to_string()))?;
        if source.as_str().contains('/') {
            return Err(PackAssetsError::InvalidGameDirectory(value.to_string()));
        }
        Ok(Self(source.as_str().to_string()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors from command argument handling, source I/O, validation, staging,
/// and final publication.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PackAssetsError {
    #[error("usage: cargo xtask pack-assets <game>")]
    Usage,
    #[error("invalid game directory {0:?}")]
    InvalidGameDirectory(String),
    #[error("could not determine the repository directory: {0}")]
    CurrentDirectory(#[source] io::Error),
    #[error("could not read input file {path}: {source}")]
    InputIo { path: PathBuf, source: io::Error },
    #[error("could not parse game manifest: {0}")]
    GameManifestParse(#[from] ManifestParseError),
    #[error("game manifest {path} is invalid: {violations:?}")]
    InvalidGameManifest {
        path: PathBuf,
        violations: Vec<ManifestViolation>,
    },
    #[error("could not extract game/pack identity: {0}")]
    GameBinding(#[from] GameAssetBindingError),
    #[error("could not parse source manifest {path}: {source}")]
    InvalidSourceManifest {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not inspect source path {path}: {source}")]
    SourceIo { path: PathBuf, source: io::Error },
    #[error("source root is not a regular directory: {0}")]
    SourceRootNotDirectory(PathBuf),
    #[error("source path uses a symlink, which is not allowed: {0}")]
    SourceSymlink(PathBuf),
    #[error("source path is not a regular file: {0}")]
    SourceNotRegular(PathBuf),
    #[error("source path ancestor is not a directory: {0}")]
    SourceAncestorNotDirectory(PathBuf),
    #[error("asset build planning failed: {0}")]
    Plan(#[source] PackBuildError),
    #[error("could not create output directory {path}: {source}")]
    OutputIo { path: PathBuf, source: io::Error },
    #[error("could not create or write staging path {path}: {source}")]
    StageIo { path: PathBuf, source: io::Error },
    #[error("generated manifest at {path} was rejected: {source}")]
    GeneratedManifest {
        path: PathBuf,
        source: tabula_assets::ManifestError,
    },
    #[error("generated manifest at {path} did not bind to the requested game pack: {source}")]
    Binding {
        path: PathBuf,
        source: ManifestBindingError,
    },
    #[error("staged asset file {path} failed integrity verification: {source}")]
    Integrity {
        path: PathBuf,
        source: AssetIntegrityError,
    },
    #[error("existing output is not a replaceable directory: {0}")]
    ExistingOutputNotDirectory(PathBuf),
    #[error("could not publish staged pack from {from} to {to}: {source}")]
    Publish {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
}

// ---------------------------------------------------------------------------
// Tests — each test maps to a verification-ledger claim in PR #32.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tabula_assets::AssetPackRef;
    use tabula_core::GameId;
    use tabula_game_api::AssetRefError;

    fn binding() -> GameAssetBinding {
        GameAssetBinding {
            game: GameId::new("com.tabula.sample").unwrap(),
            pack: AssetPackRef::parse("sample@1.0.0").unwrap(),
        }
    }

    fn file(name: &str, source: &str, bytes: &[u8]) -> (SourceFileSpec, InspectedSourceFile) {
        let spec = SourceFileSpec {
            name: name.to_string(),
            source: source.to_string(),
            priority: "critical".to_string(),
            density: Some(1),
        };
        let inspected = inspect_source_bytes(&spec, bytes.to_vec()).unwrap();
        (spec, inspected)
    }

    fn source_manifest(
        files: Vec<SourceFileSpec>,
        resources: Vec<SourceResourceSpec>,
    ) -> SourceManifest {
        SourceManifest { files, resources }
    }

    fn resource(id: &str, variants: Vec<SourceResourceVariantSpec>) -> SourceResourceSpec {
        SourceResourceSpec {
            id: id.to_string(),
            variants,
        }
    }

    fn whole_file(file: &str) -> SourceResourceVariantSpec {
        SourceResourceVariantSpec {
            file: file.to_string(),
            region: None,
        }
    }

    #[test]
    fn content_addressed_path_commits_to_exact_bytes() {
        let (spec, inspected) = file(
            "fixture.bin",
            "data/fixture.bin",
            b"pack-builder-fixture-v1",
        );
        let expected_digest = "18e35c2fba91736622f8a1de6751901a869545ffee8635964588ccf3b9600324";
        assert_eq!(inspected.digest, expected_digest);
        let path =
            content_addressed_path(&binding().pack, &inspected.source, &inspected.digest).unwrap();
        assert_eq!(inspected.byte_size, 23);
        assert_eq!(
            path.as_str(),
            format!("sample/1.0.0/data/fixture.b3-{expected_digest}.bin")
        );

        let source = source_manifest(
            vec![spec],
            vec![resource("data/fixture", vec![whole_file("fixture.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let manifest = AssetPackManifest::from_toml(&plan.manifest_toml).unwrap();
        assert_eq!(manifest.files()[0].hash().to_string(), expected_digest);
        assert_eq!(manifest.files()[0].bytes().get(), 23);
        assert!(manifest.files()[0]
            .path()
            .as_str()
            .contains(expected_digest));
    }

    #[test]
    fn one_byte_changes_content_identity() {
        let (spec_a, inspected_a) = file("fixture.bin", "fixture.bin", b"same-size-payload-a");
        let (spec_b, inspected_b) = file("fixture.bin", "fixture.bin", b"same-size-payload-b");
        assert_eq!(inspected_a.byte_size, inspected_b.byte_size);
        let path_a =
            content_addressed_path(&binding().pack, &inspected_a.source, &inspected_a.digest)
                .unwrap();
        let path_b =
            content_addressed_path(&binding().pack, &inspected_b.source, &inspected_b.digest)
                .unwrap();
        assert_ne!(inspected_a.digest, inspected_b.digest);
        assert_ne!(path_a, path_b);

        let source_a = source_manifest(
            vec![spec_a],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let source_b = source_manifest(
            vec![spec_b],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let plan_a = plan_pack(&binding(), &source_a, vec![inspected_a]).unwrap();
        let plan_b = plan_pack(&binding(), &source_b, vec![inspected_b]).unwrap();
        assert_ne!(plan_a.manifest_toml, plan_b.manifest_toml);
    }

    #[test]
    fn declaration_order_does_not_change_the_pack_plan() {
        let (a_spec, a) = file("a.bin", "textures/a.bin", b"a");
        let (b_spec, b) = file("b.bin", "textures/b.bin", b"b");
        let resource_a = resource("pieces/a", vec![whole_file("a.bin")]);
        let resource_b = resource("pieces/b", vec![whole_file("b.bin")]);
        let first = source_manifest(
            vec![a_spec.clone(), b_spec.clone()],
            vec![resource_a.clone(), resource_b.clone()],
        );
        let reversed = source_manifest(vec![b_spec, a_spec], vec![resource_b, resource_a]);
        let first_plan = plan_pack(&binding(), &first, vec![a.clone(), b.clone()]).unwrap();
        let reversed_plan = plan_pack(&binding(), &reversed, vec![b, a]).unwrap();
        assert_eq!(first_plan.manifest_toml, reversed_plan.manifest_toml);
        let first_paths: BTreeSet<_> = first_plan
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        let reversed_paths: BTreeSet<_> = reversed_plan
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(first_paths, reversed_paths);
    }

    #[test]
    fn generated_manifest_round_trips_through_runtime_parser() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"runtime-round-trip");
        let source = source_manifest(
            vec![spec],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let manifest = AssetPackManifest::from_toml(&plan.manifest_toml).unwrap();
        let file = &manifest.files()[0];
        let planned = plan
            .files
            .iter()
            .find(|planned| planned.path == *file.path())
            .unwrap();
        assert!(file.verify_bytes(&planned.bytes).is_ok());
    }

    #[test]
    fn hostile_source_paths_are_rejected_without_normalization() {
        for path in [
            "../escape.png",
            "/absolute.png",
            "a/../b.png",
            "a//b.png",
            r"platform\\backslash.png",
            "a/./b.png",
        ] {
            assert!(
                PackSourcePath::new(path).is_err(),
                "accepted hostile path {path:?}"
            );
        }
    }

    #[test]
    fn source_manifest_rejects_unknown_builder_fields() {
        let source = r#"
            [[files]]
            name = "fixture.bin"
            source = "fixture.bin"
            priority = "critical"
            density = 1
            hash = "must-not-be-an-input"

            [[resources]]
            id = "fixture"
            [[resources.variants]]
            file = "fixture.bin"
        "#;
        assert!(toml::from_str::<SourceManifest>(source).is_err());
    }

    fn write_repository_fixture(root: &Path, source_manifest: &str, source_bytes: Option<&[u8]>) {
        fs::create_dir_all(root.join("games/sample")).unwrap();
        fs::create_dir_all(root.join("assets/packs/sample")).unwrap();
        fs::write(root.join("games/sample/game.toml"), game_manifest()).unwrap();
        fs::write(
            root.join("assets/packs/sample/pack.source.toml"),
            source_manifest,
        )
        .unwrap();
        if let Some(bytes) = source_bytes {
            fs::write(root.join("assets/packs/sample/fixture.bin"), bytes).unwrap();
        }
    }

    fn game_manifest() -> &'static str {
        r#"
            id = "com.tabula.sample"
            version = "0.1.0"
            rules_version = 1
            name_key = "game.sample.name"
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
            pack = "sample@1.0.0"
        "#
    }

    fn valid_source_manifest() -> &'static str {
        r#"
            [[files]]
            name = "fixture.bin"
            source = "fixture.bin"
            priority = "critical"
            density = 1

            [[resources]]
            id = "fixture"
            [[resources.variants]]
            file = "fixture.bin"
        "#
    }

    #[test]
    fn missing_source_does_not_publish_a_final_pack() {
        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), None);
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(result, Err(PackAssetsError::SourceIo { .. })));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn zero_byte_source_is_rejected_before_publication() {
        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), Some(&[]));
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::Plan(
                PackBuildError::EmptySourceFile { .. }
            ))
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn repeated_builds_have_identical_observable_output() {
        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), Some(b"repeatable"));
        let output = root.path().join("target/asset-packs");
        let first = build_pack(root.path(), &output, "sample").unwrap();
        let first_files = read_tree(&first.output);
        let second = build_pack(root.path(), &output, "sample").unwrap();
        let second_files = read_tree(&second.output);
        assert_eq!(first_files, second_files);
    }

    #[test]
    fn published_manifest_and_files_pass_runtime_integrity_contract() {
        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), Some(b"published"));
        let output = root.path().join("target/asset-packs");
        let summary = build_pack(root.path(), &output, "sample").unwrap();
        let manifest_source = fs::read_to_string(summary.output.join("pack.toml")).unwrap();
        let manifest = AssetPackManifest::from_toml(&manifest_source).unwrap();
        let _bound = manifest
            .validate_binding(&binding().pack, &binding().game)
            .unwrap();
        for file in manifest.files() {
            let bytes = fs::read(output.join(file.path().as_str())).unwrap();
            file.verify_bytes(&bytes).unwrap();
        }
    }

    #[test]
    fn invalid_generated_manifest_does_not_publish_a_final_pack() {
        let root = tempfile::tempdir().unwrap();
        let invalid_source =
            valid_source_manifest().replace("file = \"fixture.bin\"", "file = \"missing.bin\"");
        write_repository_fixture(root.path(), &invalid_source, Some(b"manifest-invalid"));
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::GeneratedManifest { .. })
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn duplicate_logical_resources_are_rejected_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let invalid_source = format!(
            "{}\n[[resources]]\nid = \"fixture\"\n[[resources.variants]]\nfile = \"fixture.bin\"\n",
            valid_source_manifest()
        );
        write_repository_fixture(root.path(), &invalid_source, Some(b"duplicate-resource"));
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::GeneratedManifest { .. })
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn invalid_density_is_rejected_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let invalid_source = valid_source_manifest().replace("density = 1", "density = 4");
        write_repository_fixture(root.path(), &invalid_source, Some(b"invalid-density"));
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::GeneratedManifest { .. })
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn corrupted_staged_bytes_are_rejected_before_publication() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"staged-integrity");
        let source = source_manifest(
            vec![spec],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let mut corrupted_plan = plan.clone();
        corrupted_plan.files[0].bytes[0] ^= 0x01;
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("target/asset-packs");
        let result = publish_staged_pack(&output, &binding(), &corrupted_plan);
        assert!(matches!(result, Err(PackAssetsError::Integrity { .. })));
        assert!(!output.join("sample/1.0.0").exists());
    }

    fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        collect_tree(root, root, &mut files);
        files
    }

    fn collect_tree(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collect_tree(root, &path, files);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, fs::read(path).unwrap());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_source_ancestor_is_rejected_without_publication() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(
            root.path(),
            &valid_source_manifest().replace("fixture.bin", "outside/fixture.bin"),
            None,
        );
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("fixture.bin"), b"outside").unwrap();
        symlink(
            outside.path(),
            root.path().join("assets/packs/sample/outside"),
        )
        .unwrap();
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(result, Err(PackAssetsError::SourceSymlink(_))));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_source_file_is_rejected_without_publication() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), None);
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("fixture.bin");
        fs::write(&outside_file, b"outside").unwrap();
        symlink(
            &outside_file,
            root.path().join("assets/packs/sample/fixture.bin"),
        )
        .unwrap();
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(result, Err(PackAssetsError::SourceSymlink(_))));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn invalid_game_directory_cannot_escape_repository_inputs() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("target/asset-packs");
        for game in ["../outside", "/absolute", r"platform\\path"] {
            assert!(matches!(
                build_pack(root.path(), &output, game),
                Err(PackAssetsError::InvalidGameDirectory(_))
            ));
        }
    }

    #[test]
    fn source_manifest_invalid_resource_is_left_to_runtime_parser() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"resource-check");
        let source = source_manifest(
            vec![spec],
            vec![resource("fixture", vec![whole_file("unknown.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let error = AssetPackManifest::from_toml(&plan.manifest_toml).unwrap_err();
        assert!(matches!(
            error,
            tabula_assets::ManifestError::UnknownResourceFile { .. }
        ));
    }

    #[test]
    fn direct_duplicate_source_declarations_are_rejected_deterministically() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"duplicate");
        let source = source_manifest(
            vec![spec.clone(), spec],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let error = plan_pack(&binding(), &source, vec![inspected.clone(), inspected]).unwrap_err();
        assert!(matches!(error, PackBuildError::DuplicateFileName(_)));
    }

    #[test]
    fn missing_inspection_identifies_the_declared_file() {
        let (spec, _) = file("fixture.bin", "fixture.bin", b"missing-inspection");
        let source = source_manifest(
            vec![spec],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let error = plan_pack(&binding(), &source, Vec::new()).unwrap_err();
        assert!(matches!(
            error,
            PackBuildError::MissingInspection(name) if name == "fixture.bin"
        ));
    }

    #[test]
    fn source_file_path_helper_rejects_invalid_digest_facts() {
        let source = PackSourcePath::new("fixture.bin").unwrap();
        assert!(matches!(
            content_addressed_path(&binding().pack, &source, "short"),
            Err(PackBuildError::InvalidDigest(_))
        ));
        assert!(matches!(
            content_addressed_path(&binding().pack, &source, &"A".repeat(64)),
            Err(PackBuildError::InvalidDigest(_))
        ));

        let extensionless = PackSourcePath::new(".fixture").unwrap();
        let digest = "0".repeat(64);
        let path = content_addressed_path(&binding().pack, &extensionless, &digest).unwrap();
        assert_eq!(path.as_str(), format!("sample/1.0.0/.fixture.b3-{digest}"));
    }

    #[test]
    fn asset_ref_error_is_not_silently_rewritten_by_the_builder() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"invalid-resource");
        let source = source_manifest(
            vec![spec],
            vec![resource("bad@resource", vec![whole_file("fixture.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let error = AssetPackManifest::from_toml(&plan.manifest_toml).unwrap_err();
        assert!(matches!(
            error,
            tabula_assets::ManifestError::InvalidResourceId(AssetRefError::ReservedAt)
        ));
    }

    #[test]
    fn unexpected_inspection_is_rejected_even_when_counts_match() {
        let (declared_spec, declared) = file("declared.bin", "declared.bin", b"declared");
        let (missing_spec, _) = file("missing.bin", "missing.bin", b"missing");
        let (unexpected_spec, unexpected) = file("unexpected.bin", "unexpected.bin", b"unexpected");
        let source = source_manifest(
            vec![declared_spec, missing_spec],
            vec![resource("declared", vec![whole_file("declared.bin")])],
        );
        let error = plan_pack(&binding(), &source, vec![declared, unexpected]).unwrap_err();
        assert!(matches!(
            error,
            PackBuildError::UnexpectedInspection(name) if name == unexpected_spec.name
        ));
    }

    #[test]
    fn resource_regions_have_stable_canonical_order() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"regions");
        let reversed_regions = vec![
            SourceResourceVariantSpec {
                file: "fixture.bin".to_string(),
                region: Some(SourceRegionSpec {
                    x: 64,
                    y: 2,
                    width: 8,
                    height: 9,
                }),
            },
            SourceResourceVariantSpec {
                file: "fixture.bin".to_string(),
                region: Some(SourceRegionSpec {
                    x: 0,
                    y: 1,
                    width: 8,
                    height: 9,
                }),
            },
        ];
        let source = source_manifest(vec![spec], vec![resource("regions", reversed_regions)]);
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let manifest: toml::Value = toml::from_str(&plan.manifest_toml).unwrap();
        let variants = manifest["resources"][0]["variants"].as_array().unwrap();
        assert_eq!(variants[0]["region"]["x"].as_integer(), Some(0));
        assert_eq!(variants[1]["region"]["x"].as_integer(), Some(64));
    }

    #[test]
    fn non_directory_source_ancestor_is_rejected_without_publication() {
        let root = tempfile::tempdir().unwrap();
        let invalid_source = valid_source_manifest().replace(
            "source = \"fixture.bin\"",
            "source = \"fixture.bin/nested\"",
        );
        write_repository_fixture(root.path(), &invalid_source, Some(b"ancestor-file"));
        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::SourceAncestorNotDirectory(_))
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }

    #[test]
    fn regular_file_source_root_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let pack_root = root.path().join("pack-root");
        fs::write(&pack_root, b"not a directory").unwrap();
        let source = PackSourcePath::new("fixture.bin").unwrap();
        let result = read_regular_source_file(&pack_root, &source);
        assert!(matches!(
            result,
            Err(PackAssetsError::SourceRootNotDirectory(path)) if path == pack_root
        ));
    }

    #[test]
    fn existing_output_file_is_not_replaced() {
        let (spec, inspected) = file("fixture.bin", "fixture.bin", b"existing-output");
        let source = source_manifest(
            vec![spec],
            vec![resource("fixture", vec![whole_file("fixture.bin")])],
        );
        let plan = plan_pack(&binding(), &source, vec![inspected]).unwrap();
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("target/asset-packs");
        fs::create_dir_all(output.join("sample")).unwrap();
        let occupied = output.join("sample/1.0.0");
        fs::write(&occupied, b"keep me").unwrap();

        let result = publish_staged_pack(&output, &binding(), &plan);
        assert!(matches!(
            result,
            Err(PackAssetsError::ExistingOutputNotDirectory(path)) if path == occupied
        ));
        assert_eq!(fs::read(occupied).unwrap(), b"keep me");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_source_root_is_rejected_without_publication() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        write_repository_fixture(root.path(), valid_source_manifest(), None);
        let outside = tempfile::tempdir().unwrap();
        let outside_pack = outside.path().join("sample");
        fs::create_dir(&outside_pack).unwrap();
        fs::write(
            outside_pack.join("pack.source.toml"),
            valid_source_manifest(),
        )
        .unwrap();
        fs::write(outside_pack.join("fixture.bin"), b"outside-root").unwrap();
        let pack_root = root.path().join("assets/packs/sample");
        fs::remove_dir_all(&pack_root).unwrap();
        symlink(&outside_pack, &pack_root).unwrap();

        let output = root.path().join("target/asset-packs");
        let result = build_pack(root.path(), &output, "sample");
        assert!(matches!(
            result,
            Err(PackAssetsError::SourceRootNotDirectory(_))
        ));
        assert!(!output.join("sample/1.0.0").exists());
    }
}
