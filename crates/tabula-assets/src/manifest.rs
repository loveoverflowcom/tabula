//! Pure parsing and validation for the asset-pack manifest. (doc 04 §12)
//!
//! @ai.role trust-boundary
//! @ai.domain assets.manifest
//! @ai.pure true
//! @ai.invariant validated-pack-metadata
//! @ai.invariant unique-file-names-and-paths
//! @ai.evidence tests::manifest_rejects_hostile_and_ambiguous_file_metadata

#![allow(clippy::doc_markdown)]

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use tabula_core::{
    ids::{GameIdError, GameVersionError},
    GameId, GameVersion,
};
use tabula_game_api::{AssetRef, AssetRefError};

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
    resources: Vec<AssetResource>,
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
        let mut file_densities = BTreeMap::new();
        let mut files = Vec::with_capacity(spec.files.len());
        for file in spec.files {
            let file = AssetFile::validate(file)?;
            if !names.insert(file.name.clone()) {
                return Err(ManifestError::DuplicateFileName(file.name.to_string()));
            }
            if !paths.insert(file.path.clone()) {
                return Err(ManifestError::DuplicatePath(file.path.to_string()));
            }
            file_densities.insert(
                file.name.clone(),
                (AssetFileIndex(files.len()), file.density()),
            );
            files.push(file);
        }

        if spec.resources.is_empty() {
            return Err(ManifestError::EmptyResources);
        }

        let mut resource_ids = BTreeSet::new();
        let mut resources = Vec::with_capacity(spec.resources.len());
        for resource in spec.resources {
            let resource = AssetResource::validate(resource, &file_densities)?;
            if !resource_ids.insert(resource.id.clone()) {
                return Err(ManifestError::DuplicateResourceId(resource.id.to_string()));
            }
            resources.push(resource);
        }

        Ok(Self {
            pack_ref,
            game,
            files,
            resources,
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

    /// Returns the explicit logical-resource declarations in manifest order.
    #[must_use]
    pub fn resources(&self) -> &[AssetResource] {
        &self.resources
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
    ) -> Result<BoundAssetPack<'_>, ManifestBindingError> {
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
        Ok(BoundAssetPack { manifest: self })
    }
}

/// Evidence that one exact manifest has been bound to an expected pack and game.
///
/// This scoped view is the only public path to resource resolution. It contains
/// no loaded bytes or backend handles; [`BoundAssetPack::resolve`] is a pure
/// lookup over already-validated manifest metadata.
///
/// @ai.role scoped-witness
/// @ai.domain assets.resolution
/// @ai.pure true
/// @ai.invariant expected-pack-and-game-binding
/// @ai.evidence tests::manifest_validate_binding_partitions
#[derive(Debug)]
pub struct BoundAssetPack<'a> {
    manifest: &'a AssetPackManifest,
}

impl BoundAssetPack<'_> {
    /// Resolves one logical resource to deterministic physical metadata.
    ///
    /// A valid bound manifest guarantees that a known resource has exactly one
    /// density-independent variant or a non-ambiguous density-aware variant
    /// set. No filesystem, network, cache, clock, or backend state is read.
    ///
    /// @ai.role pure-resolver
    /// @ai.domain assets.resolution
    /// @ai.pure true
    /// @ai.requires expected-pack-and-game-binding
    /// @ai.law density-selection-is-declaration-order-independent
    /// @ai.law equal-distance-selects-higher-density
    /// @ai.evidence tests::resolution_obeys_density_selection_law
    /// @ai.evidence tests::resolution_is_declaration_order_independent_and_never_infers_file_names
    pub fn resolve(
        &self,
        asset: &AssetRef,
        target_density: AssetDensity,
    ) -> Result<ResolvedAsset<'_>, AssetResolveError> {
        let resource = self
            .manifest
            .resources
            .iter()
            .find(|resource| resource.id() == asset)
            .ok_or_else(|| AssetResolveError::UnknownResource(asset.clone()))?;

        let variant = resource.select_variant(target_density);
        let file = variant.file_metadata(&self.manifest.files);

        Ok(ResolvedAsset {
            file,
            region: variant.region(),
        })
    }
}

/// Pure physical metadata selected for one logical [`AssetRef`].
///
/// `ResolvedAsset` deliberately contains neither bytes nor a renderer/audio
/// handle. Loading, decoding, cache management, and backend upload remain
/// later I/O boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedAsset<'a> {
    file: &'a AssetFile,
    region: Option<AssetPixelRegion>,
}

impl ResolvedAsset<'_> {
    /// Returns the selected validated physical file metadata.
    #[must_use]
    pub const fn file(&self) -> &AssetFile {
        self.file
    }

    /// Returns the optional structurally valid source-pixel region.
    #[must_use]
    pub const fn region(&self) -> Option<AssetPixelRegion> {
        self.region
    }
}

/// Why pure resolution could not select physical metadata.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetResolveError {
    /// The logical resource was not declared by this bound manifest.
    #[error("unknown logical asset resource {0}")]
    UnknownResource(AssetRef),
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
///
/// Names use the same ASCII-safe segment grammar as [`AssetPath`] without path
/// separators: alphanumeric characters plus `-`, `_`, `.`, `~`, and `@`.
/// They are not filesystem paths and are never trimmed or normalized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetFileName(String);

impl AssetFileName {
    /// Validates a manifest-local file identity.
    ///
    /// @ai.role proof-constructor
    /// @ai.domain assets.file-name
    /// @ai.pure true
    /// @ai.invariant canonical-manifest-file-name
    /// @ai.evidence tests::asset_file_name_constructor_partitions
    pub fn new(value: impl Into<String>) -> Result<Self, AssetFileNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AssetFileNameError::Empty);
        }
        if value.trim().is_empty() {
            return Err(AssetFileNameError::Blank);
        }
        if has_whitespace_or_control(&value) {
            return Err(AssetFileNameError::WhitespaceOrControl);
        }
        if value.contains('/') || value.contains('\\') || value.contains(':') {
            return Err(AssetFileNameError::PathLike);
        }
        if !is_asset_segment(&value) {
            return Err(AssetFileNameError::InvalidCharacter);
        }
        Ok(Self(value))
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

impl TryFrom<String> for AssetFileName {
    type Error = AssetFileNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AssetFileName {
    type Error = AssetFileNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetFileName> for String {
    fn from(value: AssetFileName) -> Self {
        value.0
    }
}

/// Why an [`AssetFileName`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetFileNameError {
    /// File names cannot be empty.
    #[error("asset file name must not be empty")]
    Empty,
    /// File names must contain at least one non-whitespace character.
    #[error("asset file name must not be blank")]
    Blank,
    /// File names cannot contain whitespace or control characters.
    #[error("asset file name must not contain whitespace or control characters")]
    WhitespaceOrControl,
    /// Path separators, drive delimiters, and other path-like spellings are not file identities.
    #[error("asset file name must not contain path separators or ':'")]
    PathLike,
    /// File names must use the explicit ASCII-safe asset segment grammar.
    #[error("asset file name contains a character outside the ASCII-safe asset grammar")]
    InvalidCharacter,
}

/// A validated, relative, canonical pack path.
///
/// Each segment uses the ASCII-safe grammar of alphanumeric characters plus
/// `-`, `_`, `.`, `~`, and `@`; `/` is the only separator. Input is rejected
/// rather than trimmed, decoded, or normalized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath(String);

impl AssetPath {
    /// Validates a canonical relative pack path.
    ///
    /// @ai.role proof-constructor
    /// @ai.domain assets.path
    /// @ai.pure true
    /// @ai.invariant canonical-relative-pack-path
    /// @ai.evidence tests::asset_path_constructor_partitions
    pub fn new(value: impl Into<String>) -> Result<Self, AssetPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AssetPathError::Empty);
        }
        if value.starts_with('/') || value.contains('\\') || value.contains(':') {
            return Err(AssetPathError::AbsoluteOrPlatformPath);
        }
        if has_whitespace_or_control(&value) {
            return Err(AssetPathError::WhitespaceOrControl);
        }
        for segment in value.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(AssetPathError::NonCanonicalSegment);
            }
            if !is_asset_segment(segment) {
                return Err(AssetPathError::InvalidCharacter);
            }
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

impl TryFrom<String> for AssetPath {
    type Error = AssetPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AssetPath {
    type Error = AssetPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetPath> for String {
    fn from(value: AssetPath) -> Self {
        value.0
    }
}

/// Why an [`AssetPath`] failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPathError {
    /// A pack path cannot be empty.
    #[error("asset path must not be empty")]
    Empty,
    /// A pack path must not be absolute, URL-like, or platform-specific.
    #[error("asset path must be a relative, platform-neutral pack path")]
    AbsoluteOrPlatformPath,
    /// Dot, parent, and empty segments are rejected rather than normalized.
    #[error("asset path contains a non-canonical segment")]
    NonCanonicalSegment,
    /// Pack paths cannot contain whitespace or control characters.
    #[error("asset path must not contain whitespace or control characters")]
    WhitespaceOrControl,
    /// Pack path segments must use the explicit ASCII-safe asset grammar.
    #[error("asset path contains a character outside the ASCII-safe asset grammar")]
    InvalidCharacter,
}

fn has_whitespace_or_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
}

fn is_asset_segment(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'@')
    })
}

/// A validated BLAKE3 digest stored as its fixed-size bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetContentHash([u8; blake3::OUT_LEN]);

impl AssetContentHash {
    /// Validates a 64-character hexadecimal BLAKE3 digest string.
    pub fn new(value: &str) -> Result<Self, AssetContentHashError> {
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

    /// Creates an [`AssetContentHash`] directly from raw 32-byte BLAKE3 digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; blake3::OUT_LEN]) -> Self {
        Self(bytes)
    }

    /// Returns the digest bytes used by an integrity verifier.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; blake3::OUT_LEN] {
        &self.0
    }
}

impl From<[u8; blake3::OUT_LEN]> for AssetContentHash {
    fn from(bytes: [u8; blake3::OUT_LEN]) -> Self {
        Self::from_bytes(bytes)
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
    /// Validates a positive declared asset byte size.
    pub fn new(value: u64) -> Result<Self, AssetByteSizeError> {
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

impl fmt::Display for AssetByteSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl TryFrom<u64> for AssetByteSize {
    type Error = AssetByteSizeError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetByteSize> for u64 {
    fn from(size: AssetByteSize) -> Self {
        size.get()
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
    /// Validates a discrete 1x, 2x, or 3x density target.
    pub fn new(value: u8) -> Result<Self, AssetDensityError> {
        match value {
            1..=3 => Ok(Self(value)),
            _ => Err(AssetDensityError::Unsupported {
                value: u64::from(value),
            }),
        }
    }

    /// Returns the validated density multiplier.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for AssetDensity {
    type Error = AssetDensityError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<u64> for AssetDensity {
    type Error = AssetDensityError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        u8::try_from(value)
            .map_err(|_| AssetDensityError::Unsupported { value })
            .and_then(Self::new)
    }
}

impl fmt::Display for AssetDensity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.get())
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
                .map(AssetDensity::try_from)
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

/// A structurally valid source-pixel rectangle within a physical asset file.
///
/// This type proves non-zero extents and non-overflowing pixel arithmetic. It
/// does not prove the rectangle is inside a decoded image; that requires image
/// metadata and belongs to a future decoding boundary. (doc 04 §12)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetPixelRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl AssetPixelRegion {
    /// Validates source-pixel origin, extent, and endpoint arithmetic.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, AssetPixelRegionError> {
        if width == 0 {
            return Err(AssetPixelRegionError::ZeroWidth);
        }
        if height == 0 {
            return Err(AssetPixelRegionError::ZeroHeight);
        }
        if x.checked_add(width).is_none() {
            return Err(AssetPixelRegionError::HorizontalOverflow);
        }
        if y.checked_add(height).is_none() {
            return Err(AssetPixelRegionError::VerticalOverflow);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// Returns the non-negative horizontal source-pixel origin.
    #[must_use]
    pub const fn x(self) -> u32 {
        self.x
    }

    /// Returns the non-negative vertical source-pixel origin.
    #[must_use]
    pub const fn y(self) -> u32 {
        self.y
    }

    /// Returns the strictly positive source-pixel width.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the strictly positive source-pixel height.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    fn validate(spec: AssetPixelRegionSpec) -> Result<Self, ManifestError> {
        let x = u32::try_from(spec.x).map_err(|_| ManifestError::PixelRegionOutOfRange {
            coordinate: "x",
            value: spec.x,
        })?;
        let y = u32::try_from(spec.y).map_err(|_| ManifestError::PixelRegionOutOfRange {
            coordinate: "y",
            value: spec.y,
        })?;
        let width =
            u32::try_from(spec.width).map_err(|_| ManifestError::PixelRegionOutOfRange {
                coordinate: "width",
                value: spec.width,
            })?;
        let height =
            u32::try_from(spec.height).map_err(|_| ManifestError::PixelRegionOutOfRange {
                coordinate: "height",
                value: spec.height,
            })?;
        Self::new(x, y, width, height).map_err(ManifestError::InvalidPixelRegion)
    }
}

/// Why an [`AssetPixelRegion`] failed structural validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetPixelRegionError {
    /// A source region must span at least one pixel horizontally.
    #[error("asset pixel region width must be greater than zero")]
    ZeroWidth,
    /// A source region must span at least one pixel vertically.
    #[error("asset pixel region height must be greater than zero")]
    ZeroHeight,
    /// Horizontal source-pixel endpoint arithmetic overflowed `u32`.
    #[error("asset pixel region horizontal endpoint overflows u32")]
    HorizontalOverflow,
    /// Vertical source-pixel endpoint arithmetic overflowed `u32`.
    #[error("asset pixel region vertical endpoint overflows u32")]
    VerticalOverflow,
}

/// One explicit mapping from a logical [`AssetRef`] to physical file variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetResource {
    id: AssetRef,
    variants: AssetResourceVariants,
}

impl AssetResource {
    fn validate(
        spec: AssetResourceSpec,
        files: &BTreeMap<AssetFileName, (AssetFileIndex, Option<AssetDensity>)>,
    ) -> Result<Self, ManifestError> {
        let id = AssetRef::new(spec.id).map_err(ManifestError::InvalidResourceId)?;
        let mut raw_variants = spec.variants.into_iter();
        let Some(first_spec) = raw_variants.next() else {
            return Err(ManifestError::EmptyResourceVariants(id));
        };

        let (first_variant, first_density) =
            AssetResourceVariant::validate(first_spec, &id, files)?;
        let mut variants = match first_density {
            None => AssetResourceVariants::DensityIndependent(first_variant),
            Some(density) => AssetResourceVariants::DensityAware(NonEmptyVariants::new(
                DensityAwareResourceVariant {
                    variant: first_variant,
                    density,
                },
            )),
        };
        let mut densities = BTreeSet::new();
        if let Some(density) = first_density {
            let _ = densities.insert(density);
        }
        for raw_variant in raw_variants {
            let (variant, density) = AssetResourceVariant::validate(raw_variant, &id, files)?;
            match (&mut variants, density) {
                (AssetResourceVariants::DensityIndependent(_), Some(_))
                | (AssetResourceVariants::DensityAware(_), None) => {
                    return Err(ManifestError::MixedResourceDensityMode(id));
                }
                (AssetResourceVariants::DensityIndependent(_), None) => {
                    return Err(ManifestError::MultipleDensitylessResourceVariants(id));
                }
                (AssetResourceVariants::DensityAware(variants), Some(density)) => {
                    if !densities.insert(density) {
                        return Err(ManifestError::DuplicateResourceDensity { id, density });
                    }
                    variants.push(DensityAwareResourceVariant { variant, density });
                }
            }
        }

        Ok(Self { id, variants })
    }

    /// Returns the resource's canonical logical identity.
    #[must_use]
    pub const fn id(&self) -> &AssetRef {
        &self.id
    }

    /// Returns the validated physical alternatives for this resource.
    #[must_use]
    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Returns a physical alternative by its stable manifest declaration index.
    #[must_use]
    pub fn variant(&self, index: usize) -> Option<&AssetResourceVariant> {
        self.variants.get(index)
    }

    fn select_variant(&self, target_density: AssetDensity) -> &AssetResourceVariant {
        self.variants.select(target_density)
    }
}

/// The validated density mode of one [`AssetResource`].
///
/// The private representation makes an empty resource, mixed density modes,
/// and a density-aware variant without a density unrepresentable after parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
enum AssetResourceVariants {
    DensityIndependent(AssetResourceVariant),
    DensityAware(NonEmptyVariants<DensityAwareResourceVariant>),
}

impl AssetResourceVariants {
    fn len(&self) -> usize {
        match self {
            Self::DensityIndependent(_) => 1,
            Self::DensityAware(variants) => variants.len(),
        }
    }

    fn get(&self, index: usize) -> Option<&AssetResourceVariant> {
        match self {
            Self::DensityIndependent(variant) => (index == 0).then_some(variant),
            Self::DensityAware(variants) => variants.get(index).map(|variant| &variant.variant),
        }
    }

    fn select(&self, target_density: AssetDensity) -> &AssetResourceVariant {
        match self {
            Self::DensityIndependent(variant) => variant,
            Self::DensityAware(variants) => {
                &variants
                    .min_by_key(|variant| {
                        (
                            u8::abs_diff(variant.density.get(), target_density.get()),
                            Reverse(variant.density),
                        )
                    })
                    .variant
            }
        }
    }
}

/// A private non-empty collection used only after the resource validation boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
struct NonEmptyVariants<T> {
    first: T,
    rest: Vec<T>,
}

impl<T> NonEmptyVariants<T> {
    fn new(first: T) -> Self {
        Self {
            first,
            rest: Vec::new(),
        }
    }

    fn push(&mut self, value: T) {
        self.rest.push(value);
    }

    fn len(&self) -> usize {
        self.rest.len() + 1
    }

    fn get(&self, index: usize) -> Option<&T> {
        if index == 0 {
            Some(&self.first)
        } else {
            self.rest.get(index - 1)
        }
    }

    fn min_by_key<K: Ord>(&self, key: impl Fn(&T) -> K) -> &T {
        self.rest.iter().fold(&self.first, |best, candidate| {
            if key(candidate) < key(best) {
                candidate
            } else {
                best
            }
        })
    }
}

/// A density-aware resource alternative with a density proven from its referenced file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DensityAwareResourceVariant {
    variant: AssetResourceVariant,
    density: AssetDensity,
}

/// One physical file and optional source-pixel region of an [`AssetResource`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetResourceVariant {
    file: AssetFileName,
    file_index: AssetFileIndex,
    region: Option<AssetPixelRegion>,
}

impl AssetResourceVariant {
    fn validate(
        spec: AssetResourceVariantSpec,
        resource: &AssetRef,
        files: &BTreeMap<AssetFileName, (AssetFileIndex, Option<AssetDensity>)>,
    ) -> Result<(Self, Option<AssetDensity>), ManifestError> {
        let file = AssetFileName::new(spec.file).map_err(ManifestError::InvalidResourceFileName)?;
        let (file_index, density) =
            files
                .get(&file)
                .copied()
                .ok_or_else(|| ManifestError::UnknownResourceFile {
                    resource: resource.clone(),
                    file: file.clone(),
                })?;
        let region = spec.region.map(AssetPixelRegion::validate).transpose()?;
        Ok((
            Self {
                file,
                file_index,
                region,
            },
            density,
        ))
    }

    /// Returns the referenced manifest-local physical file identity.
    #[must_use]
    pub const fn file(&self) -> &AssetFileName {
        &self.file
    }

    /// Returns optional structural atlas metadata for this variant.
    #[must_use]
    pub const fn region(&self) -> Option<AssetPixelRegion> {
        self.region
    }

    fn file_metadata<'a>(&self, files: &'a [AssetFile]) -> &'a AssetFile {
        self.file_index.resolve(files)
    }
}

/// A manifest-private proof that a resource variant refers to one declared file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AssetFileIndex(usize);

impl AssetFileIndex {
    fn resolve(self, files: &[AssetFile]) -> &AssetFile {
        &files[self.0]
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
    /// A logical resource ID did not satisfy the canonical `AssetRef` grammar.
    #[error("invalid logical asset resource id: {0}")]
    InvalidResourceId(#[source] AssetRefError),
    /// A resource referenced an invalid manifest-local file identity.
    #[error("invalid resource file name: {0}")]
    InvalidResourceFileName(#[source] AssetFileNameError),
    /// A resource declared an invalid structural atlas region.
    #[error("invalid asset pixel region: {0}")]
    InvalidPixelRegion(#[source] AssetPixelRegionError),
    /// A source-pixel field did not fit the physical pixel coordinate domain.
    #[error("asset pixel region {coordinate} is outside u32: {value}")]
    PixelRegionOutOfRange {
        coordinate: &'static str,
        value: u64,
    },
    /// A pack without files cannot resolve any presentation assets.
    #[error("asset manifest must contain at least one file")]
    EmptyFiles,
    /// A pack without logical resource declarations cannot satisfy presenters.
    #[error("asset manifest must contain at least one resource")]
    EmptyResources,
    /// Two entries named the same manifest-local file.
    #[error("duplicate asset file name {0:?}")]
    DuplicateFileName(String),
    /// Two entries named the same pack path, making addressing ambiguous.
    #[error("duplicate asset path {0:?}")]
    DuplicatePath(String),
    /// Two declarations used the same logical resource identity.
    #[error("duplicate logical asset resource {0}")]
    DuplicateResourceId(String),
    /// A resource must explicitly map to at least one declared file.
    #[error("logical asset resource {0} must contain at least one variant")]
    EmptyResourceVariants(AssetRef),
    /// A resource variant named no declared physical file.
    #[error("logical asset resource {resource} references undeclared file {file}")]
    UnknownResourceFile {
        resource: AssetRef,
        file: AssetFileName,
    },
    /// A resource cannot mix density-aware and density-independent files.
    #[error("logical asset resource {0} mixes density-aware and density-independent variants")]
    MixedResourceDensityMode(AssetRef),
    /// A density-aware resource cannot have two variants at the same density.
    #[error("logical asset resource {id} has duplicate {density}x variants")]
    DuplicateResourceDensity { id: AssetRef, density: AssetDensity },
    /// A density-independent resource has exactly one physical variant.
    #[error("logical asset resource {0} has multiple density-independent variants")]
    MultipleDensitylessResourceVariants(AssetRef),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSpec {
    pack: String,
    version: String,
    game: String,
    files: Vec<AssetFileSpec>,
    resources: Vec<AssetResourceSpec>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetResourceSpec {
    id: String,
    variants: Vec<AssetResourceVariantSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetResourceVariantSpec {
    file: String,
    region: Option<AssetPixelRegionSpec>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetPixelRegionSpec {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

#[cfg(test)]
mod tests {
    use super::{
        AssetContentHash, AssetContentHashError, AssetPackManifest, AssetPriority, ManifestError,
    };

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(file: &str) -> String {
        format!(
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\n\n[[files]]\n{file}\n\n[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@2x.atlas\""
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

    fn file_named(name: &str, density: Option<u8>) -> String {
        let density = density.map_or_else(String::new, |density| format!("\ndensity = {density}"));
        format!(
            "name = \"{name}\"\npath = \"sample/1.0.0/{name}.bin\"\nhash = \"{HASH}\"\nbytes = 100\npriority = \"critical\"{density}"
        )
    }

    fn manifest_with(files: &[String], resources: &str) -> String {
        format!(
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\n\n[[files]]\n{}\n\n{resources}",
            files.join("\n\n[[files]]\n")
        )
    }

    fn resource(id: &str, files: &[&str]) -> String {
        let variants = files
            .iter()
            .map(|file| format!("[[resources.variants]]\nfile = \"{file}\""))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("[[resources]]\nid = \"{id}\"\n\n{variants}")
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
    fn asset_file_name_constructor_partitions() {
        use super::{AssetFileName, AssetFileNameError};

        for valid in ["pieces@2x.atlas", "move.ogg", "board-background"] {
            let name = AssetFileName::new(valid).unwrap();
            assert_eq!(name.as_str(), valid);
            assert_eq!(name.to_string(), valid);
        }

        assert_eq!(AssetFileName::new(""), Err(AssetFileNameError::Empty));
        assert_eq!(AssetFileName::new("   "), Err(AssetFileNameError::Blank));

        for whitespace in [
            " move.ogg",
            "move.ogg ",
            "piece\nname",
            "piece\tname",
            "piece\rname",
        ] {
            assert_eq!(
                AssetFileName::new(whitespace),
                Err(AssetFileNameError::WhitespaceOrControl),
                "testing {whitespace:?}"
            );
        }

        for path_like in ["foo/bar.png", r"foo\bar.png", "foo:bar"] {
            assert_eq!(
                AssetFileName::new(path_like),
                Err(AssetFileNameError::PathLike),
                "testing {path_like:?}"
            );
        }

        for invalid in ["foo?bar", "foo#bar", "foo%2Fbar", "foo€bar"] {
            assert_eq!(
                AssetFileName::new(invalid),
                Err(AssetFileNameError::InvalidCharacter),
                "testing {invalid:?}"
            );
        }
    }

    #[test]
    fn asset_path_constructor_partitions() {
        use super::{AssetPath, AssetPathError};

        let valid_paths = [
            "sample/1.0.0/pieces@2x.png".to_owned(),
            ["ch", "ess/1.0.0/move.ogg"].concat(),
            "foo/bar-baz_1.png".to_owned(),
            "foo/a.b~c@3x.png".to_owned(),
        ];
        for valid in valid_paths {
            let path = AssetPath::new(valid.as_str()).unwrap();
            assert_eq!(path.as_str(), valid.as_str());
            assert_eq!(path.to_string(), valid);
        }

        assert_eq!(AssetPath::new(""), Err(AssetPathError::Empty));

        for absolute_or_platform in [
            "/path.png",
            r"C:\path.png",
            "C:/path.png",
            r"\\server\share",
        ] {
            assert_eq!(
                AssetPath::new(absolute_or_platform),
                Err(AssetPathError::AbsoluteOrPlatformPath),
                "testing {absolute_or_platform:?}"
            );
        }

        for non_canonical in [
            "../secret",
            "foo/../../secret",
            "foo/./bar",
            "foo//bar",
            "./foo",
            "foo/.",
            "foo/..",
        ] {
            assert_eq!(
                AssetPath::new(non_canonical),
                Err(AssetPathError::NonCanonicalSegment),
                "testing {non_canonical:?}"
            );
        }

        for invalid in [
            "foo/bar.png?x=1",
            "foo/bar.png#fragment",
            "foo/%2e%2e/bar",
            "foo/%2F/bar",
        ] {
            assert_eq!(
                AssetPath::new(invalid),
                Err(AssetPathError::InvalidCharacter),
                "testing {invalid:?}"
            );
        }

        for whitespace in [
            "foo/bar.png ",
            "foo/ bar.png",
            "foo/\tbar.png",
            "foo/bar\n.png",
            "foo/bar\r.png",
        ] {
            assert_eq!(
                AssetPath::new(whitespace),
                Err(AssetPathError::WhitespaceOrControl),
                "testing {whitespace:?}"
            );
        }
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

        for name in [
            "",
            "   ",
            " move.ogg",
            "move.ogg ",
            "foo/bar",
            r"foo\bar",
            "foo:bar",
            "foo\nbar",
            "foo\tbar",
        ] {
            let source = manifest(&file_with(
                "name = \"pieces@2x.atlas\"",
                &format!("name = {name:?}"),
            ));
            assert!(
                matches!(
                    AssetPackManifest::from_toml(&source),
                    Err(ManifestError::InvalidFileName(_))
                ),
                "testing {name:?}"
            );
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
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\nfiles = []\nresources = []";

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
        assert!(parsed
            .validate_binding(&expected_pack, &expected_game)
            .is_ok());

        // Mutation A: wrong version -> VersionMismatch
        let wrong_version_pack = AssetPackRef::from_static("sample", "0.2.0");
        assert_eq!(
            parsed
                .validate_binding(&wrong_version_pack, &expected_game)
                .unwrap_err(),
            ManifestBindingError::VersionMismatch {
                expected: AssetPackVersion::new("0.2.0").unwrap(),
                found: AssetPackVersion::new("1.0.0").unwrap(),
            }
        );

        // Mutation B: wrong pack name -> PackMismatch
        let wrong_pack = AssetPackRef::from_static("alternate", "1.0.0");
        assert_eq!(
            parsed
                .validate_binding(&wrong_pack, &expected_game)
                .unwrap_err(),
            ManifestBindingError::PackMismatch {
                expected: AssetPackId::new("alternate").unwrap(),
                found: AssetPackId::new("sample").unwrap(),
            }
        );

        // Mutation C: wrong game binding -> GameMismatch
        let wrong_game = GameId::new("com.example.alternate").unwrap();
        assert_eq!(
            parsed
                .validate_binding(&expected_pack, &wrong_game)
                .unwrap_err(),
            ManifestBindingError::GameMismatch {
                expected: GameId::new("com.example.alternate").unwrap(),
                found: GameId::new("com.example.sample").unwrap(),
            }
        );
    }

    #[test]
    fn resource_validation_rejects_empty_duplicate_and_ambiguous_declarations() {
        use super::AssetRef;

        let one_x = file_named("pieces@1x.atlas", Some(1));
        let two_x = file_named("pieces@2x.atlas", Some(2));
        let audio = file_named("move.ogg", None);

        let no_resources = format!(
            "pack = \"sample\"\nversion = \"1.0.0\"\ngame = \"com.example.sample\"\nresources = []\n\n[[files]]\n{one_x}"
        );
        assert!(matches!(
            AssetPackManifest::from_toml(&no_resources),
            Err(ManifestError::EmptyResources)
        ));

        let duplicate = format!(
            "{}\n\n{}",
            resource("pieces/white-knight", &["pieces@1x.atlas"]),
            resource("pieces/white-knight", &["pieces@1x.atlas"])
        );
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(std::slice::from_ref(&one_x), &duplicate)),
            Err(ManifestError::DuplicateResourceId(_))
        ));

        let zero_variants = "[[resources]]\nid = \"pieces/white-knight\"\nvariants = []";
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(std::slice::from_ref(&one_x), zero_variants)),
            Err(ManifestError::EmptyResourceVariants(id)) if id == AssetRef::from_static("pieces/white-knight")
        ));

        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                std::slice::from_ref(&one_x),
                &resource("pieces/white-knight", &["missing.atlas"])
            )),
            Err(ManifestError::UnknownResourceFile { .. })
        ));

        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[one_x.clone(), audio.clone()],
                &resource("pieces/white-knight", &["pieces@1x.atlas", "move.ogg"])
            )),
            Err(ManifestError::MixedResourceDensityMode(_))
        ));

        let another_two_x = file_named("alternate@2x.atlas", Some(2));
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[two_x.clone(), another_two_x],
                &resource(
                    "pieces/white-knight",
                    &["pieces@2x.atlas", "alternate@2x.atlas"]
                )
            )),
            Err(ManifestError::DuplicateResourceDensity { .. })
        ));

        let another_audio = file_named("other.ogg", None);
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[audio, another_audio],
                &resource("audio/move", &["move.ogg", "other.ogg"])
            )),
            Err(ManifestError::MultipleDensitylessResourceVariants(_))
        ));

        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[one_x],
                &resource("../white-knight", &["pieces@1x.atlas"])
            )),
            Err(ManifestError::InvalidResourceId(_))
        ));
    }

    #[test]
    fn resource_validation_accepts_whole_files_density_variants_and_atlas_sharing() {
        let one_x = file_named("pieces@1x.atlas", Some(1));
        let two_x = file_named("pieces@2x.atlas", Some(2));
        let three_x = file_named("pieces@3x.atlas", Some(3));
        let board = file_named("board.png", None);
        let resources = [
            "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 64, height = 64 }\n\n[[resources.variants]]\nfile = \"pieces@2x.atlas\"\nregion = { x = 0, y = 0, width = 128, height = 128 }\n\n[[resources.variants]]\nfile = \"pieces@3x.atlas\"\nregion = { x = 0, y = 0, width = 192, height = 192 }".to_owned(),
            "[[resources]]\nid = \"pieces/white-queen\"\n\n[[resources.variants]]\nfile = \"pieces@2x.atlas\"\nregion = { x = 64, y = 0, width = 64, height = 64 }".to_owned(),
            resource("pieces/single", &["pieces@1x.atlas"]),
            resource("board/background", &["board.png"]),
        ]
        .join("\n\n");
        let manifest = AssetPackManifest::from_toml(&manifest_with(
            &[one_x, two_x, three_x, board],
            &resources,
        ))
        .unwrap();

        assert_eq!(manifest.resources().len(), 4);
        assert_eq!(manifest.resources()[0].variant_count(), 3);
        assert_eq!(
            manifest.resources()[1]
                .variant(0)
                .unwrap()
                .region()
                .unwrap()
                .width(),
            64
        );
    }

    #[test]
    fn pixel_region_validation_rejects_degenerate_overflowing_and_unknown_input() {
        use super::AssetPixelRegionError;

        assert_eq!(
            super::AssetPixelRegion::new(0, 0, 0, 1),
            Err(AssetPixelRegionError::ZeroWidth)
        );
        assert_eq!(
            super::AssetPixelRegion::new(0, 0, 1, 0),
            Err(AssetPixelRegionError::ZeroHeight)
        );
        assert_eq!(
            super::AssetPixelRegion::new(u32::MAX, 0, 1, 1),
            Err(AssetPixelRegionError::HorizontalOverflow)
        );
        assert_eq!(
            super::AssetPixelRegion::new(0, u32::MAX, 1, 1),
            Err(AssetPixelRegionError::VerticalOverflow)
        );

        let invalid_region = "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 0, height = 1 }";
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[file_named("pieces@1x.atlas", Some(1))],
                invalid_region
            )),
            Err(ManifestError::InvalidPixelRegion(
                AssetPixelRegionError::ZeroWidth
            ))
        ));

        let overflowing_region = "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 4294967295, y = 0, width = 1, height = 1 }";
        assert!(matches!(
            AssetPackManifest::from_toml(&manifest_with(
                &[file_named("pieces@1x.atlas", Some(1))],
                overflowing_region
            )),
            Err(ManifestError::InvalidPixelRegion(
                AssetPixelRegionError::HorizontalOverflow
            ))
        ));

        let unknown_resource_field = "[[resources]]\nid = \"pieces/white-knight\"\nunsupported = true\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"";
        let unknown_variant_field = "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nunsupported = true";
        let unknown_region_field = "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 1, height = 1, unsupported = true }";
        for source in [
            unknown_resource_field,
            unknown_variant_field,
            unknown_region_field,
        ] {
            assert!(matches!(
                AssetPackManifest::from_toml(&manifest_with(
                    &[file_named("pieces@1x.atlas", Some(1))],
                    source
                )),
                Err(ManifestError::Toml(_))
            ));
        }
    }

    #[test]
    fn resolution_obeys_density_selection_law() {
        use super::{AssetDensity, AssetRef};
        use tabula_core::GameId;

        let files = [
            file_named("pieces@1x.atlas", Some(1)),
            file_named("pieces@2x.atlas", Some(2)),
            file_named("pieces@3x.atlas", Some(3)),
            file_named("move.ogg", None),
        ];
        let resources = [
            "[[resources]]\nid = \"pieces/all\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 64, height = 64 }\n\n[[resources.variants]]\nfile = \"pieces@2x.atlas\"\nregion = { x = 10, y = 20, width = 128, height = 128 }\n\n[[resources.variants]]\nfile = \"pieces@3x.atlas\"\nregion = { x = 30, y = 40, width = 192, height = 192 }".to_owned(),
            "[[resources]]\nid = \"pieces/tie\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 64, height = 64 }\n\n[[resources.variants]]\nfile = \"pieces@3x.atlas\"\nregion = { x = 30, y = 40, width = 192, height = 192 }".to_owned(),
            resource("pieces/low", &["pieces@1x.atlas", "pieces@2x.atlas"]),
            resource("pieces/high", &["pieces@2x.atlas", "pieces@3x.atlas"]),
            resource("audio/move", &["move.ogg"]),
        ]
        .join("\n\n");
        let manifest = AssetPackManifest::from_toml(&manifest_with(&files, &resources)).unwrap();
        let bound = manifest
            .validate_binding(
                &super::AssetPackRef::from_static("sample", "1.0.0"),
                &GameId::new("com.example.sample").unwrap(),
            )
            .unwrap();
        let resolve = |id: &str, target| {
            let resolved = bound
                .resolve(
                    &AssetRef::new(id).unwrap(),
                    AssetDensity::new(target).unwrap(),
                )
                .unwrap();
            (
                resolved.file().name().as_str().to_owned(),
                resolved.region(),
            )
        };

        let one_x = resolve("pieces/all", 1);
        assert_eq!(one_x.0, "pieces@1x.atlas");
        assert_eq!(one_x.1.unwrap().width(), 64);
        let two_x = resolve("pieces/all", 2);
        assert_eq!(two_x.0, "pieces@2x.atlas");
        assert_eq!(two_x.1.unwrap().width(), 128);
        assert_eq!(two_x.1.unwrap().x(), 10);
        let three_x = resolve("pieces/all", 3);
        assert_eq!(three_x.0, "pieces@3x.atlas");
        assert_eq!(three_x.1.unwrap().width(), 192);
        assert_eq!(three_x.1.unwrap().x(), 30);
        let fallback = resolve("pieces/tie", 2);
        assert_eq!(fallback.0, "pieces@3x.atlas");
        assert_eq!(fallback.1.unwrap().width(), 192);
        assert_eq!(resolve("pieces/low", 3).0, "pieces@2x.atlas");
        assert_eq!(resolve("pieces/high", 1).0, "pieces@2x.atlas");
        assert_eq!(resolve("audio/move", 1).0, "move.ogg");
        assert_eq!(resolve("audio/move", 3).0, "move.ogg");
    }

    #[test]
    fn resolution_is_declaration_order_independent_and_never_infers_file_names() {
        use super::{AssetDensity, AssetPackRef, AssetRef, AssetResolveError};
        use tabula_core::GameId;

        let files = [
            file_named("pieces@1x.atlas", Some(1)),
            file_named("pieces@3x.atlas", Some(3)),
        ];
        let first = AssetPackManifest::from_toml(&manifest_with(
            &files,
            "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 64, height = 64 }\n\n[[resources.variants]]\nfile = \"pieces@3x.atlas\"\nregion = { x = 30, y = 40, width = 192, height = 192 }",
        ))
        .unwrap();
        let reversed = AssetPackManifest::from_toml(&manifest_with(
            &files,
            "[[resources]]\nid = \"pieces/white-knight\"\n\n[[resources.variants]]\nfile = \"pieces@3x.atlas\"\nregion = { x = 30, y = 40, width = 192, height = 192 }\n\n[[resources.variants]]\nfile = \"pieces@1x.atlas\"\nregion = { x = 0, y = 0, width = 64, height = 64 }",
        ))
        .unwrap();
        let game = GameId::new("com.example.sample").unwrap();
        let pack = AssetPackRef::from_static("sample", "1.0.0");
        let target = AssetDensity::new(2).unwrap();
        let first_bound = first.validate_binding(&pack, &game).unwrap();
        let first_result = first_bound
            .resolve(&AssetRef::from_static("pieces/white-knight"), target)
            .unwrap();
        let reversed_bound = reversed.validate_binding(&pack, &game).unwrap();
        let reversed_result = reversed_bound
            .resolve(&AssetRef::from_static("pieces/white-knight"), target)
            .unwrap();
        assert_eq!(first_result, reversed_result);
        assert_eq!(first_result.file().name().as_str(), "pieces@3x.atlas");
        assert_eq!(first_result.region().unwrap().width(), 192);
        assert_eq!(first_result.region().unwrap().x(), 30);

        let no_inference = AssetPackManifest::from_toml(&manifest_with(
            &[file_named("white-knight", Some(1))],
            &resource("other/resource", &["white-knight"]),
        ))
        .unwrap();
        assert_eq!(
            no_inference
                .validate_binding(&pack, &game)
                .unwrap()
                .resolve(&AssetRef::from_static("pieces/white-knight"), target),
            Err(AssetResolveError::UnknownResource(AssetRef::from_static(
                "pieces/white-knight"
            )))
        );
    }
}
