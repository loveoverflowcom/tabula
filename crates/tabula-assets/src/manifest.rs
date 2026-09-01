//! Pure parsing and validation for the asset-pack manifest. (doc 04 §12)
//!
//! @ai.role trust-boundary
//! @ai.domain assets.manifest
//! @ai.pure true
//! @ai.invariant validated-pack-metadata
//! @ai.invariant unique-file-names-and-paths
//! @ai.evidence tests::manifest_rejects_hostile_and_ambiguous_file_metadata

#![allow(clippy::doc_markdown)]

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use tabula_core::{
    ids::{GameIdError, GameVersionError},
    GameId, GameVersion,
};

/// A validated asset-pack manifest, independent of I/O and backend handles.
///
/// Its private fields are constructed only by [`AssetPackManifest::from_toml`],
/// so users of this type receive pack metadata that has passed identity, path,
/// digest, density, size, and uniqueness validation.
///
/// @ai.role validated-domain-value
/// @ai.domain assets.manifest
/// @ai.invariant validated-pack-metadata
/// @ai.invariant unique-file-names-and-paths
/// @ai.evidence tests::valid_minimal_manifest_parses_into_validated_values
/// @ai.evidence tests::manifest_rejects_hostile_and_ambiguous_file_metadata
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetPackManifest {
    pack: AssetPackId,
    version: AssetPackVersion,
    game: GameId,
    files: Vec<AssetFile>,
}

impl AssetPackManifest {
    /// Parses TOML and returns only a fully validated manifest.
    pub fn from_toml(source: &str) -> Result<Self, ManifestError> {
        let spec: ManifestSpec = toml::from_str(source).map_err(ManifestError::Toml)?;
        Self::validate(spec)
    }

    fn validate(spec: ManifestSpec) -> Result<Self, ManifestError> {
        let pack = AssetPackId::new(spec.pack).map_err(ManifestError::InvalidPackId)?;
        let version = AssetPackVersion::new(spec.version).map_err(ManifestError::InvalidVersion)?;
        let game = GameId::new(spec.game).map_err(ManifestError::InvalidGameId)?;

        if spec.files.is_empty() {
            return Err(ManifestError::EmptyFiles);
        }

        let mut names = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut files = Vec::with_capacity(spec.files.len());
        for file in spec.files {
            let file = AssetFile::validate(file)?;
            if !names.insert(file.name.clone()) {
                return Err(ManifestError::DuplicateFileName(file.name.to_string()));
            }
            if !paths.insert(file.path.clone()) {
                return Err(ManifestError::DuplicatePath(file.path.to_string()));
            }
            files.push(file);
        }

        Ok(Self {
            pack,
            version,
            game,
            files,
        })
    }

    /// Returns the manifest's pack-local identity.
    #[must_use]
    pub const fn pack(&self) -> &AssetPackId {
        &self.pack
    }

    /// Returns this asset pack's version, distinct from rules versioning.
    #[must_use]
    pub const fn version(&self) -> &AssetPackVersion {
        &self.version
    }

    /// Returns the game to which this pack is bound.
    #[must_use]
    pub const fn game(&self) -> &GameId {
        &self.game
    }

    /// Returns the validated file metadata in manifest order.
    #[must_use]
    pub fn files(&self) -> &[AssetFile] {
        &self.files
    }
}

/// A non-empty, non-whitespace asset-pack identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPackId(String);

impl AssetPackId {
    /// Validates a pack identity without imposing game-ID syntax.
    pub fn new(value: impl Into<String>) -> Result<Self, AssetPackIdError> {
        let value = value.into();
        (!value.trim().is_empty())
            .then_some(Self(value))
            .ok_or(AssetPackIdError::Blank)
    }

    /// Returns the original validated identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetPackId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why an [`AssetPackId`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPackIdError {
    /// Pack identities must contain at least one non-whitespace character.
    #[error("asset pack id must not be blank")]
    Blank,
}

/// A `SemVer` asset-pack version, deliberately distinct from canonical rules versions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPackVersion(GameVersion);

impl AssetPackVersion {
    /// Validates the existing project `SemVer` grammar for an asset-pack version.
    pub fn new(value: impl Into<String>) -> Result<Self, GameVersionError> {
        GameVersion::new(value).map(Self)
    }

    /// Returns the `SemVer` spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for AssetPackVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A stable, manifest-local file identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetFileName(String);

impl AssetFileName {
    fn new(value: String) -> Result<Self, AssetFileNameError> {
        (!value.trim().is_empty())
            .then_some(Self(value))
            .ok_or(AssetFileNameError::Blank)
    }

    /// Returns this file's manifest-local name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetFileName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why an [`AssetFileName`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetFileNameError {
    /// File names must contain at least one non-whitespace character.
    #[error("asset file name must not be blank")]
    Blank,
}

/// A validated, relative, canonical pack path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath(String);

impl AssetPath {
    fn new(value: String) -> Result<Self, AssetPathError> {
        if value.is_empty() {
            return Err(AssetPathError::Empty);
        }
        if value.starts_with('/') || value.contains('\\') || value.contains(':') {
            return Err(AssetPathError::NotRelative);
        }
        if value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(AssetPathError::NonCanonicalSegment);
        }
        Ok(Self(value))
    }

    /// Returns the validated pack-relative path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why an [`AssetPath`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPathError {
    /// A pack path cannot be empty.
    #[error("asset path must not be empty")]
    Empty,
    /// A pack path must not be an absolute, URL-like, or Windows path.
    #[error("asset path must be a relative pack path")]
    NotRelative,
    /// Dot, parent, and empty segments are rejected rather than normalized.
    #[error("asset path contains a non-canonical segment")]
    NonCanonicalSegment,
}

/// A validated BLAKE3 digest stored as its fixed-size bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetContentHash([u8; blake3::OUT_LEN]);

impl AssetContentHash {
    fn new(value: &str) -> Result<Self, AssetContentHashError> {
        if value.len() != blake3::OUT_LEN * 2 {
            return Err(AssetContentHashError::InvalidLength { found: value.len() });
        }
        let mut bytes = [0_u8; blake3::OUT_LEN];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let high = hex_nibble(pair[0]).ok_or(AssetContentHashError::InvalidHex)?;
            let low = hex_nibble(pair[1]).ok_or(AssetContentHashError::InvalidHex)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }

    /// Returns the digest bytes used by a future integrity verifier.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; blake3::OUT_LEN] {
        &self.0
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for AssetContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Why an [`AssetContentHash`] failed structural validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetContentHashError {
    /// A BLAKE3 digest has exactly 32 bytes, encoded as 64 hex characters.
    #[error("BLAKE3 hash must contain 64 hex characters, found {found}")]
    InvalidLength { found: usize },
    /// The fixed-length digest contained a non-hexadecimal character.
    #[error("BLAKE3 hash must contain only hexadecimal characters")]
    InvalidHex,
}

/// A positive declared asset byte size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetByteSize(u64);

impl AssetByteSize {
    fn new(value: u64) -> Result<Self, AssetByteSizeError> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(AssetByteSizeError::Zero)
    }

    /// Returns the positive declared size in bytes.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Why an [`AssetByteSize`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetByteSizeError {
    /// Zero-byte assets have no current manifest use case.
    #[error("asset byte size must be greater than zero")]
    Zero,
}

/// The closed loading priorities committed by doc 04 §12.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetPriority {
    /// Needed before the first useful frame.
    Critical,
    /// Loaded during early play.
    High,
    /// Loaded lazily by a future resolver.
    Low,
}

impl AssetPriority {
    fn parse(value: String) -> Result<Self, ManifestError> {
        match value.as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "low" => Ok(Self::Low),
            _ => Err(ManifestError::UnknownPriority(value)),
        }
    }
}

/// A supported raster density, restricted to the documented 1x, 2x, and 3x values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetDensity(u8);

impl AssetDensity {
    fn new(value: u64) -> Result<Self, AssetDensityError> {
        match value {
            1..=3 => match u8::try_from(value) {
                Ok(value) => Ok(Self(value)),
                Err(_) => Err(AssetDensityError::Unsupported { value }),
            },
            _ => Err(AssetDensityError::Unsupported { value }),
        }
    }

    /// Returns the validated density multiplier.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Why an [`AssetDensity`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetDensityError {
    /// Only the 1x, 2x, and 3x variants currently have consumers.
    #[error("asset density must be 1, 2, or 3; found {value}")]
    Unsupported { value: u64 },
}

/// One validated file entry in an [`AssetPackManifest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetFile {
    name: AssetFileName,
    path: AssetPath,
    hash: AssetContentHash,
    bytes: AssetByteSize,
    priority: AssetPriority,
    density: Option<AssetDensity>,
}

impl AssetFile {
    fn validate(spec: AssetFileSpec) -> Result<Self, ManifestError> {
        Ok(Self {
            name: AssetFileName::new(spec.name).map_err(ManifestError::InvalidFileName)?,
            path: AssetPath::new(spec.path).map_err(ManifestError::InvalidPath)?,
            hash: AssetContentHash::new(&spec.hash).map_err(ManifestError::InvalidHash)?,
            bytes: AssetByteSize::new(spec.bytes).map_err(ManifestError::InvalidByteSize)?,
            priority: AssetPriority::parse(spec.priority)?,
            density: spec
                .density
                .map(AssetDensity::new)
                .transpose()
                .map_err(ManifestError::InvalidDensity)?,
        })
    }

    /// Returns the manifest-local file identity.
    #[must_use]
    pub const fn name(&self) -> &AssetFileName {
        &self.name
    }

    /// Returns the validated pack-relative path.
    #[must_use]
    pub const fn path(&self) -> &AssetPath {
        &self.path
    }

    /// Returns the manifest-declared BLAKE3 digest.
    #[must_use]
    pub const fn hash(&self) -> &AssetContentHash {
        &self.hash
    }

    /// Returns the manifest-declared positive byte size.
    #[must_use]
    pub const fn bytes(&self) -> AssetByteSize {
        self.bytes
    }

    /// Returns this file's loading priority.
    #[must_use]
    pub const fn priority(&self) -> AssetPriority {
        self.priority
    }

    /// Returns raster density, or `None` for non-raster assets.
    #[must_use]
    pub const fn density(&self) -> Option<AssetDensity> {
        self.density
    }
}

/// A focused failure while parsing or validating a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// TOML could not be decoded into the raw manifest representation.
    #[error("invalid asset manifest TOML: {0}")]
    Toml(#[source] toml::de::Error),
    /// The pack identity failed its local contract.
    #[error("invalid asset pack id: {0}")]
    InvalidPackId(#[source] AssetPackIdError),
    /// The pack version failed the project's SemVer contract.
    #[error("invalid asset pack version: {0}")]
    InvalidVersion(#[source] GameVersionError),
    /// The game binding failed the shared identity contract.
    #[error("invalid game binding: {0}")]
    InvalidGameId(#[source] GameIdError),
    /// A file name was empty or whitespace-only.
    #[error("invalid asset file name: {0}")]
    InvalidFileName(#[source] AssetFileNameError),
    /// A file path was not a safe relative pack path.
    #[error("invalid asset path: {0}")]
    InvalidPath(#[source] AssetPathError),
    /// A file hash was not exactly one structurally valid BLAKE3 digest.
    #[error("invalid asset hash: {0}")]
    InvalidHash(#[source] AssetContentHashError),
    /// A file declared an invalid byte size.
    #[error("invalid asset byte size: {0}")]
    InvalidByteSize(#[source] AssetByteSizeError),
    /// A file used a priority outside the closed contract.
    #[error("unknown asset priority {0:?}")]
    UnknownPriority(String),
    /// A raster density was outside the supported domain.
    #[error("invalid asset density: {0}")]
    InvalidDensity(#[source] AssetDensityError),
    /// A pack without files cannot resolve any presentation assets.
    #[error("asset manifest must contain at least one file")]
    EmptyFiles,
    /// Two entries named the same manifest-local file.
    #[error("duplicate asset file name {0:?}")]
    DuplicateFileName(String),
    /// Two entries named the same pack path, making addressing ambiguous.
    #[error("duplicate asset path {0:?}")]
    DuplicatePath(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSpec {
    pack: String,
    version: String,
    game: String,
    files: Vec<AssetFileSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetFileSpec {
    name: String,
    path: String,
    hash: String,
    bytes: u64,
    priority: String,
    density: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::{
        AssetContentHash, AssetContentHashError, AssetPackManifest, AssetPriority, ManifestError,
    };

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(file: &str) -> String {
        format!(
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\n\n[[files]]\n{file}"
        )
    }

    fn file() -> String {
        format!(
            "name = \"pieces@2x.atlas\"\npath = \"sample/1.0.0/pieces@2x.png\"\nhash = \"{HASH}\"\nbytes = 412003\npriority = \"critical\"\ndensity = 2"
        )
    }

    fn file_with(field: &str, replacement: &str) -> String {
        file().replacen(field, replacement, 1)
    }

    #[test]
    fn valid_minimal_manifest_parses_into_validated_values() {
        let parsed = AssetPackManifest::from_toml(&manifest(&file())).unwrap();

        assert_eq!(parsed.pack().as_str(), "sample");
        assert_eq!(parsed.version().as_str(), "1.0.0");
        assert_eq!(parsed.game().as_str(), "com.example.sample");
        assert_eq!(parsed.files().len(), 1);
        assert_eq!(parsed.files()[0].priority(), AssetPriority::Critical);
        assert_eq!(parsed.files()[0].density().unwrap().get(), 2);
        assert_eq!(parsed.files()[0].hash().to_string(), HASH);
    }

    #[test]
    fn manifest_rejects_blank_pack_and_invalid_game_binding() {
        let blank = manifest(&file()).replacen("pack = \"sample\"", "pack = \"   \"", 1);
        assert!(matches!(
            AssetPackManifest::from_toml(&blank),
            Err(ManifestError::InvalidPackId(_))
        ));

        let game =
            manifest(&file()).replacen("game = \"com.example.sample\"", "game = \"sample\"", 1);
        assert!(matches!(
            AssetPackManifest::from_toml(&game),
            Err(ManifestError::InvalidGameId(_))
        ));
    }

    #[test]
    fn density_partitions_allow_exactly_one_through_three() {
        for density in [1, 2, 3] {
            let source = manifest(&file_with("density = 2", &format!("density = {density}")));
            assert!(AssetPackManifest::from_toml(&source).is_ok(), "{density}");
        }
        for density in [0, 4, 255] {
            let source = manifest(&file_with("density = 2", &format!("density = {density}")));
            assert!(matches!(
                AssetPackManifest::from_toml(&source),
                Err(ManifestError::InvalidDensity(_))
            ));
        }
    }

    #[test]
    fn hash_length_and_character_partitions_are_rejected() {
        for hash in ["0".repeat(63), "0".repeat(65), "g".repeat(64)] {
            let source = manifest(&file_with(
                &format!("hash = \"{HASH}\""),
                &format!("hash = \"{hash}\""),
            ));
            assert!(matches!(
                AssetPackManifest::from_toml(&source),
                Err(ManifestError::InvalidHash(_))
            ));
        }
    }

    #[test]
    fn hash_validation_rejects_non_ascii_at_exact_byte_length_without_panicking() {
        let hash = format!("€{}", "0".repeat(61));
        assert_eq!(hash.len(), 64);
        assert_eq!(
            AssetContentHash::new(&hash),
            Err(AssetContentHashError::InvalidHex)
        );
    }

    #[test]
    fn manifest_schema_rejects_unknown_top_level_and_file_fields() {
        let unknown_top_level = format!("unsupported = true\n{}", manifest(&file()));
        assert!(matches!(
            AssetPackManifest::from_toml(&unknown_top_level),
            Err(ManifestError::Toml(_))
        ));

        let unknown_file_field = manifest(&format!("{}\nunsupported = true", file()));
        assert!(matches!(
            AssetPackManifest::from_toml(&unknown_file_field),
            Err(ManifestError::Toml(_))
        ));
    }

    #[test]
    fn manifest_rejects_hostile_and_ambiguous_file_metadata() {
        for path in [
            "../secret",
            "foo/../../secret",
            "/path.png",
            "C:\\\\path.png",
            "http://example.com/file",
            "https://example.com/file",
            "foo/./bar",
        ] {
            let source = manifest(&file_with(
                "path = \"sample/1.0.0/pieces@2x.png\"",
                &format!("path = \"{path}\""),
            ));
            assert!(matches!(
                AssetPackManifest::from_toml(&source),
                Err(ManifestError::InvalidPath(_))
            ));
        }

        let first = file();
        let duplicate_name = format!(
            "{first}\n[[files]]\n{}",
            file_with(
                "path = \"sample/1.0.0/pieces@2x.png\"",
                "path = \"sample/1.0.0/other.png\"",
            )
        );
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest(&duplicate_name)),
            Err(ManifestError::DuplicateFileName(_))
        ));

        let duplicate_path = format!(
            "{first}\n[[files]]\n{}",
            file_with("name = \"pieces@2x.atlas\"", "name = \"other.atlas\"")
        );
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest(&duplicate_path)),
            Err(ManifestError::DuplicatePath(_))
        ));
    }

    #[test]
    fn unknown_priority_and_zero_byte_size_are_rejected() {
        let priority = manifest(&file_with(
            "priority = \"critical\"",
            "priority = \"medium\"",
        ));
        assert!(matches!(
            AssetPackManifest::from_toml(&priority),
            Err(ManifestError::UnknownPriority(value)) if value == "medium"
        ));

        let bytes = manifest(&file_with("bytes = 412003", "bytes = 0"));
        assert!(matches!(
            AssetPackManifest::from_toml(&bytes),
            Err(ManifestError::InvalidByteSize(_))
        ));
    }

    #[test]
    fn malformed_or_pathological_toml_is_rejected_without_validation_panics() {
        for source in [
            String::new(),
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"".to_owned(),
            "pack = \"sample\"\npack = \"alternate\"".to_owned(),
            manifest(""),
            manifest(&file_with("bytes = 412003", "bytes = 18446744073709551616")),
        ] {
            assert!(AssetPackManifest::from_toml(&source).is_err());
        }
    }

    #[test]
    fn manifest_without_files_is_not_a_resolvable_asset_pack() {
        let source =
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\nfiles = []";

        assert!(matches!(
            AssetPackManifest::from_toml(source),
            Err(ManifestError::EmptyFiles)
        ));
    }
}
