//! Pure parsing and validation for the asset-pack manifest. (doc 04 §12)
//!
//! @ai.role trust-boundary
//! @ai.domain assets.manifest
//! @ai.pure true
//! @ai.invariant validated-pack-metadata
//! @ai.invariant unique-file-names-and-paths
//! @ai.evidence tests::manifest_rejects_hostile_and_ambiguous_file_metadata

#![allow(clippy::doc_markdown)]

use std::{collections::BTreeSet, fmt, str::FromStr};

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
    pack_ref: AssetPackRef,
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
        let pack_ref = AssetPackRef::new(pack, version);
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
            pack_ref,
            game,
            files,
        })
    }

    /// Returns this manifest's exact pack reference.
    #[must_use]
    pub const fn pack_ref(&self) -> &AssetPackRef {
        &self.pack_ref
    }

    /// Returns the manifest's pack-local identity.
    #[must_use]
    pub const fn pack(&self) -> &AssetPackId {
        self.pack_ref.pack()
    }

    /// Returns this asset pack's version, distinct from rules versioning.
    #[must_use]
    pub const fn version(&self) -> &AssetPackVersion {
        self.pack_ref.version()
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

    /// Proves that this manifest matches the expected pack reference and game binding.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain assets.manifest
    /// @ai.pure true
    /// @ai.invariant expected-pack-and-game-binding
    /// @ai.evidence tests::manifest_validate_binding_partitions
    pub fn validate_binding(
        &self,
        expected_pack: &AssetPackRef,
        expected_game: &GameId,
    ) -> Result<(), ManifestBindingError> {
        if self.pack() != expected_pack.pack() {
            return Err(ManifestBindingError::PackMismatch {
                expected: expected_pack.pack().clone(),
                found: self.pack().clone(),
            });
        }
        if self.version() != expected_pack.version() {
            return Err(ManifestBindingError::VersionMismatch {
                expected: expected_pack.version().clone(),
                found: self.version().clone(),
            });
        }
        if self.game() != expected_game {
            return Err(ManifestBindingError::GameMismatch {
                expected: expected_game.clone(),
                found: self.game().clone(),
            });
        }
        Ok(())
    }
}

/// Why an [`AssetPackManifest`] failed binding validation against expected pack/game identity.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestBindingError {
    /// The manifest declares a different pack identity than requested.
    #[error("asset pack id mismatch: expected {expected}, found {found}")]
    PackMismatch {
        expected: AssetPackId,
        found: AssetPackId,
    },
    /// The manifest declares a different pack version than requested.
    #[error("asset pack version mismatch: expected {expected}, found {found}")]
    VersionMismatch {
        expected: AssetPackVersion,
        found: AssetPackVersion,
    },
    /// The manifest is bound to a different game than requested.
    #[error("game binding mismatch: expected {expected}, found {found}")]
    GameMismatch { expected: GameId, found: GameId },
}

/// A non-empty, non-whitespace, path-safe asset-pack identity.
///
/// Pack identities must be valid URI/path-safe segments containing only ASCII
/// alphanumeric characters, `'-'`, `'_'`, `'.'`, or `'~'`, and cannot be
/// `.` or `..`. They cannot contain the reserved `@` delimiter used in
/// canonical [`AssetPackRef`] strings, URI query/fragment delimiters (`?`, `#`),
/// percent-encoding characters (`%`), path separators (`/`, `\`, `:`), or
/// whitespace and control characters.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPackId(String);

impl AssetPackId {
    /// Validates a pack identity without imposing reverse-DNS game-ID syntax.
    pub fn new(value: impl Into<String>) -> Result<Self, AssetPackIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AssetPackIdError::Empty);
        }
        if value.trim().is_empty() {
            return Err(AssetPackIdError::Blank);
        }
        if value.trim() != value {
            return Err(AssetPackIdError::SurroundingWhitespace);
        }
        if value.contains('@') {
            return Err(AssetPackIdError::ReservedDelimiter);
        }
        if value == "." || value == ".." {
            return Err(AssetPackIdError::ReservedDotSegment);
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte == b'-'
                || byte == b'_'
                || byte == b'.'
                || byte == b'~'
        }) {
            return Err(AssetPackIdError::InvalidCharacter);
        }
        Ok(Self(value))
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

impl TryFrom<String> for AssetPackId {
    type Error = AssetPackIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AssetPackId {
    type Error = AssetPackIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetPackId> for String {
    fn from(id: AssetPackId) -> Self {
        id.0
    }
}

/// Why an [`AssetPackId`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPackIdError {
    /// Pack identities cannot be empty.
    #[error("asset pack id must not be empty")]
    Empty,
    /// Pack identities must contain at least one non-whitespace character.
    #[error("asset pack id must not be blank")]
    Blank,
    /// Pack identities cannot contain leading or trailing whitespace.
    #[error("asset pack id must not contain surrounding whitespace")]
    SurroundingWhitespace,
    /// '@' is reserved as the delimiter separating pack ID from pack version.
    #[error("asset pack id must not contain reserved '@' delimiter")]
    ReservedDelimiter,
    /// Pack identities cannot be '.' or '..' dot segments.
    #[error("asset pack id must not be '.' or '..' dot segment")]
    ReservedDotSegment,
    /// Pack identities must contain only URI/path-safe characters (ASCII alphanumeric, '-', '_', '.', '~').
    #[error(
        "asset pack id must contain only ASCII alphanumeric, '-', '_', '.', or '~' characters"
    )]
    InvalidCharacter,
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

impl TryFrom<String> for AssetPackVersion {
    type Error = GameVersionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AssetPackVersion {
    type Error = GameVersionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetPackVersion> for String {
    fn from(v: AssetPackVersion) -> Self {
        v.as_str().to_string()
    }
}

/// An immutable, validated reference to a specific versioned asset pack (`pack@version`).
///
/// It combines a validated [`AssetPackId`] and [`AssetPackVersion`] without public struct
/// literals, guaranteeing that every constructed instance is valid and unambiguous.
///
/// @ai.role validated-domain-value
/// @ai.domain assets.identity
/// @ai.invariant canonical-pack-ref
/// @ai.law parse-display-round-trip
/// @ai.evidence tests::asset_pack_ref_parse_and_display_round_trip
/// @ai.evidence tests::asset_pack_ref_constructor_and_parser_partitions
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetPackRef {
    pack: AssetPackId,
    version: AssetPackVersion,
}

impl AssetPackRef {
    /// Creates a reference from already validated pack identity and version.
    #[must_use]
    pub const fn new(pack: AssetPackId, version: AssetPackVersion) -> Self {
        Self { pack, version }
    }

    /// Creates a static reference declared by code.
    ///
    /// Static declarations are developer-owned configuration. An invalid
    /// declaration is a programming error and fails loudly with a panic.
    #[track_caller]
    #[must_use]
    pub fn from_static(pack: &'static str, version: &'static str) -> Self {
        let pack = AssetPackId::new(pack).unwrap_or_else(|error| {
            panic!("static asset pack id declaration must be valid: {error}")
        });
        let version = AssetPackVersion::new(version).unwrap_or_else(|error| {
            panic!("static asset pack version declaration must be valid: {error}")
        });
        Self::new(pack, version)
    }

    /// Parses the canonical external textual representation: `pack@version`.
    pub fn parse(value: &str) -> Result<Self, AssetPackRefError> {
        let Some((pack_str, version_str)) = value.split_once('@') else {
            return Err(AssetPackRefError::MissingSeparator(value.to_string()));
        };
        if version_str.contains('@') {
            return Err(AssetPackRefError::MultipleSeparators(value.to_string()));
        }
        let pack = AssetPackId::new(pack_str).map_err(AssetPackRefError::InvalidPackId)?;
        let version =
            AssetPackVersion::new(version_str).map_err(AssetPackRefError::InvalidVersion)?;
        Ok(Self::new(pack, version))
    }

    /// Returns the pack-local identity.
    #[must_use]
    pub const fn pack(&self) -> &AssetPackId {
        &self.pack
    }

    /// Returns the asset pack version.
    #[must_use]
    pub const fn version(&self) -> &AssetPackVersion {
        &self.version
    }
}

impl fmt::Display for AssetPackRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.pack, self.version)
    }
}

impl FromStr for AssetPackRef {
    type Err = AssetPackRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for AssetPackRef {
    type Error = AssetPackRefError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl TryFrom<&str> for AssetPackRef {
    type Error = AssetPackRefError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<AssetPackRef> for String {
    fn from(value: AssetPackRef) -> Self {
        value.to_string()
    }
}

/// Why an [`AssetPackRef`] failed validation during parsing.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPackRefError {
    /// The string does not contain the required '@' separator.
    #[error("missing '@' separator in asset pack reference: {0:?}")]
    MissingSeparator(String),
    /// The string contains multiple '@' delimiters.
    #[error("multiple '@' separators in asset pack reference: {0:?}")]
    MultipleSeparators(String),
    /// The pack identity portion failed validation.
    #[error("invalid asset pack id in reference: {0}")]
    InvalidPackId(#[source] AssetPackIdError),
    /// The version portion failed SemVer validation.
    #[error("invalid asset pack version in reference: {0}")]
    InvalidVersion(#[source] GameVersionError),
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

    #[test]
    fn asset_pack_id_constructor_partitions() {
        use super::{AssetPackId, AssetPackIdError};

        for valid in [
            "sample",
            "alpha",
            "beta",
            "sample-pack_1",
            "pack2",
            "sample.name",
            "sample~1",
            "a-b_c.d~e",
            "v1",
        ] {
            let id = AssetPackId::new(valid).unwrap();
            assert_eq!(id.as_str(), valid);
            assert_eq!(id.to_string(), valid);
        }

        assert_eq!(AssetPackId::new(""), Err(AssetPackIdError::Empty));
        assert_eq!(AssetPackId::new("   "), Err(AssetPackIdError::Blank));
        assert_eq!(
            AssetPackId::new(" sample"),
            Err(AssetPackIdError::SurroundingWhitespace)
        );
        assert_eq!(
            AssetPackId::new("sample "),
            Err(AssetPackIdError::SurroundingWhitespace)
        );
        assert_eq!(
            AssetPackId::new("sample@legacy"),
            Err(AssetPackIdError::ReservedDelimiter)
        );
        assert_eq!(
            AssetPackId::new("."),
            Err(AssetPackIdError::ReservedDotSegment)
        );
        assert_eq!(
            AssetPackId::new(".."),
            Err(AssetPackIdError::ReservedDotSegment)
        );

        for invalid_char in [
            "sample?debug",
            "sample#fragment",
            "sample%2Fother",
            "sample\nlegacy",
            "sample\tlegacy",
            "sample\rlegacy",
            "sample/pack",
            "sample\\pack",
            "sample:pack",
            "sample pack",
            "sample€",
            "sample!name",
            "sample$name",
            "sample*name",
        ] {
            assert_eq!(
                AssetPackId::new(invalid_char),
                Err(AssetPackIdError::InvalidCharacter),
                "testing {invalid_char}"
            );
        }
    }

    #[test]
    fn asset_pack_ref_valid_parsing_and_construction() {
        use super::{AssetPackId, AssetPackRef, AssetPackVersion};

        // Valid refs
        for (valid, expected_pack, expected_version) in [
            ("sample@0.1.0", "sample", "0.1.0"),
            ("alpha@1.2.3", "alpha", "1.2.3"),
            ("beta@2.0.0-alpha.1", "beta", "2.0.0-alpha.1"),
            ("alpha@1.2.3+build.7", "alpha", "1.2.3+build.7"),
            ("sample.pack~1@1.0.0", "sample.pack~1", "1.0.0"),
        ] {
            let parsed = AssetPackRef::parse(valid).unwrap();
            assert_eq!(parsed.pack().as_str(), expected_pack);
            assert_eq!(parsed.version().as_str(), expected_version);
            assert_eq!(parsed.to_string(), valid);
        }

        // Infallible constructor from validated parts
        let pack = AssetPackId::new("sample").unwrap();
        let version = AssetPackVersion::new("0.1.0").unwrap();
        let direct = AssetPackRef::new(pack.clone(), version.clone());
        assert_eq!(direct.pack(), &pack);
        assert_eq!(direct.version(), &version);
    }

    #[test]
    fn asset_pack_ref_invalid_separator_and_version_partitions() {
        use super::{AssetPackIdError, AssetPackRef, AssetPackRefError};
        use tabula_core::ids::GameVersionError;

        // Invalid separator shapes
        assert!(matches!(
            AssetPackRef::parse("sample"),
            Err(AssetPackRefError::MissingSeparator(_))
        ));
        assert!(matches!(
            AssetPackRef::parse("@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(AssetPackIdError::Empty))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample@"),
            Err(AssetPackRefError::InvalidVersion(GameVersionError))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample@@1.0.0"),
            Err(AssetPackRefError::MultipleSeparators(_))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample@1.0.0@extra"),
            Err(AssetPackRefError::MultipleSeparators(_))
        ));

        // Invalid version
        for invalid_ver in [
            "sample@1",
            "sample@1.2",
            "sample@01.2.3",
            "sample@v1.2.3",
            "sample@not-semver",
        ] {
            assert!(
                matches!(
                    AssetPackRef::parse(invalid_ver),
                    Err(AssetPackRefError::InvalidVersion(GameVersionError))
                ),
                "testing {invalid_ver}"
            );
        }
    }

    #[test]
    fn asset_pack_ref_invalid_pack_id_partitions() {
        use super::{AssetPackIdError, AssetPackRef, AssetPackRefError};

        assert!(matches!(
            AssetPackRef::parse("   @1.0.0"),
            Err(AssetPackRefError::InvalidPackId(AssetPackIdError::Blank))
        ));
        assert!(matches!(
            AssetPackRef::parse(" sample@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::SurroundingWhitespace
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample @1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::SurroundingWhitespace
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample?debug@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::InvalidCharacter
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample#frag@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::InvalidCharacter
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample%20pack@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::InvalidCharacter
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("sample/sub@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::InvalidCharacter
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("../sample@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::InvalidCharacter
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse("..@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::ReservedDotSegment
            ))
        ));
        assert!(matches!(
            AssetPackRef::parse(".@1.0.0"),
            Err(AssetPackRefError::InvalidPackId(
                AssetPackIdError::ReservedDotSegment
            ))
        ));
    }

    #[test]
    fn asset_pack_ref_parse_and_display_round_trip() {
        use super::AssetPackRef;

        for canonical in [
            "sample@0.1.0",
            "alpha@1.2.3",
            "beta@2.0.0-alpha.1",
            "alpha@1.2.3+build.7",
            "my-custom-pack_1@0.0.1",
        ] {
            let parsed = AssetPackRef::parse(canonical).unwrap();
            let displayed = parsed.to_string();
            assert_eq!(displayed, canonical);
            let reparsed = AssetPackRef::parse(&displayed).unwrap();
            assert_eq!(parsed, reparsed);
            assert_eq!(format!("{parsed}"), canonical);
        }
    }

    #[test]
    fn asset_pack_ref_serde_round_trip_and_validation_barrier() {
        use super::AssetPackRef;
        use tabula_core::{canonical_decode, canonical_encode};

        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Config {
            pack: AssetPackRef,
        }

        let pack_ref = AssetPackRef::parse("sample@0.1.0").unwrap();

        // Round-trip through canonical postcard encoding
        let encoded = canonical_encode(&pack_ref).unwrap();
        let decoded: AssetPackRef = canonical_decode(&encoded).unwrap();
        assert_eq!(decoded, pack_ref);

        // Round-trip through TOML
        let config = Config {
            pack: pack_ref.clone(),
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("pack = \"sample@0.1.0\""));
        let deserialized_config: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized_config, config);

        // Deserialization cannot bypass validation
        for invalid in [
            "pack = \"sample\"",
            "pack = \"@0.1.0\"",
            "pack = \"sample@\"",
            "pack = \"sample@bad\"",
            "pack = \"sample@@1.0.0\"",
            "pack = \" sample@1.0.0\"",
            "pack = \"sample/sub@1.0.0\"",
            "pack = \"\"",
        ] {
            assert!(
                toml::from_str::<Config>(invalid).is_err(),
                "should reject {invalid}"
            );
        }

        // Postcard raw invalid byte strings cannot forge AssetPackRef
        let invalid_postcard = canonical_encode(&String::from("sample@@1.0.0")).unwrap();
        assert!(canonical_decode::<AssetPackRef>(&invalid_postcard).is_err());
    }

    #[test]
    fn asset_pack_ref_from_static_panics_on_invalid_literals() {
        use super::AssetPackRef;

        let valid = AssetPackRef::from_static("sample", "0.1.0");
        assert_eq!(valid.to_string(), "sample@0.1.0");

        let panic_on_id = std::panic::catch_unwind(|| {
            let _ = AssetPackRef::from_static(" sample", "0.1.0");
        });
        assert!(panic_on_id.is_err());

        let panic_on_delimiter = std::panic::catch_unwind(|| {
            let _ = AssetPackRef::from_static("sample@bad", "0.1.0");
        });
        assert!(panic_on_delimiter.is_err());

        let panic_on_version = std::panic::catch_unwind(|| {
            let _ = AssetPackRef::from_static("sample", "bad_version");
        });
        assert!(panic_on_version.is_err());
    }

    #[test]
    fn manifest_exposes_pack_ref_matching_pack_and_version() {
        use super::AssetPackRef;

        let parsed = AssetPackManifest::from_toml(&manifest(&file())).unwrap();
        let pack_ref = parsed.pack_ref();

        assert_eq!(pack_ref.pack(), parsed.pack());
        assert_eq!(pack_ref.version(), parsed.version());
        assert_eq!(pack_ref, &AssetPackRef::from_static("sample", "1.0.0"));
    }

    #[test]
    fn manifest_validate_binding_partitions() {
        use super::{AssetPackId, AssetPackRef, AssetPackVersion, ManifestBindingError};
        use tabula_core::GameId;

        let parsed = AssetPackManifest::from_toml(&manifest(&file())).unwrap();
        let expected_pack = AssetPackRef::from_static("sample", "1.0.0");
        let expected_game = GameId::new("com.example.sample").unwrap();

        // Exact match passes
        assert_eq!(
            parsed.validate_binding(&expected_pack, &expected_game),
            Ok(())
        );

        // Mutation A: wrong version -> VersionMismatch
        let wrong_version_pack = AssetPackRef::from_static("sample", "0.2.0");
        assert_eq!(
            parsed.validate_binding(&wrong_version_pack, &expected_game),
            Err(ManifestBindingError::VersionMismatch {
                expected: AssetPackVersion::new("0.2.0").unwrap(),
                found: AssetPackVersion::new("1.0.0").unwrap(),
            })
        );

        // Mutation B: wrong pack name -> PackMismatch
        let wrong_pack = AssetPackRef::from_static("alternate", "1.0.0");
        assert_eq!(
            parsed.validate_binding(&wrong_pack, &expected_game),
            Err(ManifestBindingError::PackMismatch {
                expected: AssetPackId::new("alternate").unwrap(),
                found: AssetPackId::new("sample").unwrap(),
            })
        );

        // Mutation C: wrong game binding -> GameMismatch
        let wrong_game = GameId::new("com.example.alternate").unwrap();
        assert_eq!(
            parsed.validate_binding(&expected_pack, &wrong_game),
            Err(ManifestBindingError::GameMismatch {
                expected: GameId::new("com.example.alternate").unwrap(),
                found: GameId::new("com.example.sample").unwrap(),
            })
        );
    }
}
