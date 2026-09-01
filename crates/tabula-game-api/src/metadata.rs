//! `GameMetadata` — identity and presentation-neutral description. (doc 02 §4.1)
//!
//! Everything here is safe to show in a catalog to anyone.
//!
//! ## i18n keys, never literals
//!
//! `name_key`, `tagline_key`, `description_key` are keys the shell localises.
//! A literal here means the game ships in exactly one language forever, and
//! "no hardcoded strings" is on the per-game definition of done (doc 08 §7.1).

use serde::{Deserialize, Serialize};
use tabula_core::{GameId, GameVersion, RulesVersion};

/// Reference to an asset inside a game's pack, e.g. `pieces/white-knight`.
///
/// TODO(phase 3): the real type lives in `tabula-assets` (created in Phase 3).
/// A local newtype here keeps `GameMetadata` complete without dragging the asset
/// crate into the rules tier — `tabula-game-api` may not depend on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetRef(String);

/// Why an [`AssetRef`] cannot identify an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetRefError {
    #[error("asset reference must not be empty")]
    Empty,
}

impl AssetRef {
    /// Creates the minimal Phase-2 asset reference. Path, pack, and hash
    /// semantics remain owned by `tabula-assets` in Phase 3.
    pub fn new(value: impl Into<String>) -> Result<Self, AssetRefError> {
        let value = value.into();
        (!value.is_empty())
            .then_some(Self(value))
            .ok_or(AssetRefError::Empty)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetRef {
    type Error = AssetRefError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AssetRef {
    type Error = AssetRefError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AssetRef> for String {
    fn from(value: AssetRef) -> Self {
        value.0
    }
}

/// A localization lookup identity, rather than display text.
///
/// The Phase-2 contract establishes only that a key is non-empty, not a
/// particular grammar. Unicode keys remain valid; leading and trailing
/// whitespace is rejected rather than silently normalized.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct I18nKey(String);

/// Why an [`I18nKey`] cannot identify a localization entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum I18nKeyError {
    #[error("i18n key must not be empty")]
    Empty,
    #[error("i18n key must not be whitespace-only")]
    WhitespaceOnly,
    #[error("i18n key must not have leading or trailing whitespace")]
    SurroundingWhitespace,
}

impl I18nKey {
    /// Establishes the stable facts required of a localization key.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain game.metadata
    /// @ai.invariant non-empty-trimmed-i18n-key
    /// @ai.evidence crate::metadata::tests::i18n_key_constructor_partitions
    /// @ai.evidence crate::metadata::tests::metadata_deserialization_cannot_bypass_key_validation
    #[allow(clippy::doc_markdown)]
    pub fn new(value: impl Into<String>) -> Result<Self, I18nKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(I18nKeyError::Empty);
        }
        if value.trim().is_empty() {
            return Err(I18nKeyError::WhitespaceOnly);
        }
        if value.trim() != value {
            return Err(I18nKeyError::SurroundingWhitespace);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for I18nKey {
    type Error = I18nKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for I18nKey {
    type Error = I18nKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<I18nKey> for String {
    fn from(value: I18nKey) -> Self {
        value.0
    }
}

/// Validated catalog metadata authoring input.
///
/// Leaf values such as localization keys and asset references are validated
/// before this spec is accepted. [`GameMetadata`] keeps its fields private so
/// callers cannot skip those proof barriers with a struct literal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameMetadataSpec {
    pub id: GameId,
    pub version: GameVersion,
    pub rules_version: RulesVersion,
    pub name_key: I18nKey,
    pub tagline_key: I18nKey,
    pub description_key: I18nKey,
    pub categories: Vec<Category>,
    pub tags: Vec<String>,
    pub estimated_minutes: DurationRange,
    pub complexity: Complexity,
    pub content_rating: ContentRating,
    pub icon: AssetRef,
    pub hero: AssetRef,
    pub rules_url_key: Option<I18nKey>,
}

/// Metadata that has crossed the catalog authoring boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "GameMetadataSpec")]
pub struct GameMetadata {
    /// Reverse-DNS: `com.tabula.chess`. Unique across the registry, checked at
    /// compile time by `register!`. (doc 02 §8.1)
    id: GameId,

    /// Semver of the module package. Bumped for presentation/asset/bot fixes.
    version: GameVersion,

    /// Bumped on any state/behaviour change. A match runs one of these for life.
    rules_version: RulesVersion,

    // --- i18n keys, not literals ---
    name_key: I18nKey,
    tagline_key: I18nKey,
    description_key: I18nKey,

    categories: Vec<Category>,
    tags: Vec<String>,

    /// Drives catalog filtering and matchmaking expectations.
    estimated_minutes: DurationRange,

    complexity: Complexity,

    /// Drives voice/chat defaults and age gating. Compliance consumes this.
    content_rating: ContentRating,

    icon: AssetRef,
    hero: AssetRef,
    rules_url_key: Option<I18nKey>,
}

impl From<GameMetadataSpec> for GameMetadata {
    fn from(spec: GameMetadataSpec) -> Self {
        Self {
            id: spec.id,
            version: spec.version,
            rules_version: spec.rules_version,
            name_key: spec.name_key,
            tagline_key: spec.tagline_key,
            description_key: spec.description_key,
            categories: spec.categories,
            tags: spec.tags,
            estimated_minutes: spec.estimated_minutes,
            complexity: spec.complexity,
            content_rating: spec.content_rating,
            icon: spec.icon,
            hero: spec.hero,
            rules_url_key: spec.rules_url_key,
        }
    }
}

impl GameMetadata {
    #[must_use]
    pub const fn id(&self) -> &GameId {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> &GameVersion {
        &self.version
    }

    #[must_use]
    pub const fn rules_version(&self) -> RulesVersion {
        self.rules_version
    }

    #[must_use]
    pub const fn name_key(&self) -> &I18nKey {
        &self.name_key
    }

    #[must_use]
    pub const fn tagline_key(&self) -> &I18nKey {
        &self.tagline_key
    }

    #[must_use]
    pub const fn description_key(&self) -> &I18nKey {
        &self.description_key
    }

    #[must_use]
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    #[must_use]
    pub const fn estimated_minutes(&self) -> DurationRange {
        self.estimated_minutes
    }

    #[must_use]
    pub const fn complexity(&self) -> Complexity {
        self.complexity
    }

    #[must_use]
    pub const fn content_rating(&self) -> ContentRating {
        self.content_rating
    }

    #[must_use]
    pub const fn icon(&self) -> &AssetRef {
        &self.icon
    }

    #[must_use]
    pub const fn hero(&self) -> &AssetRef {
        &self.hero
    }

    #[must_use]
    pub const fn rules_url_key(&self) -> Option<&I18nKey> {
        self.rules_url_key.as_ref()
    }
}

/// A non-inverted catalog duration estimate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "(u16, u16)", into = "(u16, u16)")]
pub struct DurationRange {
    min: u16,
    max: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("estimated duration minimum must not exceed maximum")]
pub struct DurationRangeError;

impl DurationRange {
    pub fn new(min: u16, max: u16) -> Result<Self, DurationRangeError> {
        (min <= max)
            .then_some(Self { min, max })
            .ok_or(DurationRangeError)
    }

    #[must_use]
    pub const fn min(self) -> u16 {
        self.min
    }

    #[must_use]
    pub const fn max(self) -> u16 {
        self.max
    }
}

impl TryFrom<(u16, u16)> for DurationRange {
    type Error = DurationRangeError;

    fn try_from(value: (u16, u16)) -> Result<Self, Self::Error> {
        Self::new(value.0, value.1)
    }
}

impl From<DurationRange> for (u16, u16) {
    fn from(value: DurationRange) -> Self {
        (value.min, value.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{canonical_decode, canonical_encode};

    fn metadata_spec(name_key: I18nKey) -> GameMetadataSpec {
        GameMetadataSpec {
            id: GameId::new("com.tabula.test").unwrap(),
            version: GameVersion::new("1.0.0").unwrap(),
            rules_version: RulesVersion(1),
            name_key,
            tagline_key: I18nKey::new("test.tagline").unwrap(),
            description_key: I18nKey::new("test.description").unwrap(),
            categories: vec![Category::Abstract],
            tags: vec!["test".to_owned()],
            estimated_minutes: DurationRange::new(1, 2).unwrap(),
            complexity: Complexity::Light,
            content_rating: ContentRating::Everyone,
            icon: AssetRef::new("test/icon").unwrap(),
            hero: AssetRef::new("test/hero").unwrap(),
            rules_url_key: None,
        }
    }

    #[derive(Serialize)]
    struct RawMetadata {
        id: GameId,
        version: GameVersion,
        rules_version: RulesVersion,
        name_key: String,
        tagline_key: String,
        description_key: String,
        categories: Vec<Category>,
        tags: Vec<String>,
        estimated_minutes: DurationRange,
        complexity: Complexity,
        content_rating: ContentRating,
        icon: String,
        hero: String,
        rules_url_key: Option<String>,
    }

    fn raw_metadata_with_name(name_key: &str) -> RawMetadata {
        RawMetadata {
            id: GameId::new("com.tabula.test").unwrap(),
            version: GameVersion::new("1.0.0").unwrap(),
            rules_version: RulesVersion(1),
            name_key: name_key.to_owned(),
            tagline_key: "test.tagline".to_owned(),
            description_key: "test.description".to_owned(),
            categories: vec![Category::Abstract],
            tags: vec!["test".to_owned()],
            estimated_minutes: DurationRange::new(1, 2).unwrap(),
            complexity: Complexity::Light,
            content_rating: ContentRating::Everyone,
            icon: "test/icon".to_owned(),
            hero: "test/hero".to_owned(),
            rules_url_key: None,
        }
    }

    #[test]
    fn i18n_key_constructor_partitions() {
        assert!(I18nKey::new("game.test.name").is_ok());
        assert!(I18nKey::new("日本語のキー").is_ok());
        assert_eq!(I18nKey::new(""), Err(I18nKeyError::Empty));
        assert_eq!(I18nKey::new(" \t"), Err(I18nKeyError::WhitespaceOnly));
        assert_eq!(
            I18nKey::new(" test.name"),
            Err(I18nKeyError::SurroundingWhitespace)
        );
        assert_eq!(
            I18nKey::new("test.name "),
            Err(I18nKeyError::SurroundingWhitespace)
        );

        let invalid = canonical_encode(&String::from(" ")).unwrap();
        assert!(canonical_decode::<I18nKey>(&invalid).is_err());
    }

    #[test]
    fn asset_ref_rejects_empty_values() {
        assert!(AssetRef::new("test/icon").is_ok());
        assert_eq!(AssetRef::new(""), Err(AssetRefError::Empty));

        let invalid = canonical_encode(&String::new()).unwrap();
        assert!(canonical_decode::<AssetRef>(&invalid).is_err());
    }

    #[test]
    fn metadata_deserialization_cannot_bypass_key_validation() {
        let invalid = canonical_encode(&raw_metadata_with_name(" ")).unwrap();
        assert!(canonical_decode::<GameMetadata>(&invalid).is_err());
    }

    #[test]
    fn metadata_spec_and_validated_metadata_keep_the_same_wire_shape() {
        let spec = metadata_spec(I18nKey::new("test.name").unwrap());
        let metadata = GameMetadata::from(spec.clone());
        assert_eq!(
            canonical_encode(&spec).unwrap(),
            canonical_encode(&metadata).unwrap()
        );
    }

    #[test]
    fn duration_range_constructor_and_deserialization_reject_inversion() {
        assert!(DurationRange::new(0, 0).is_ok());
        assert!(DurationRange::new(1, 90).is_ok());
        assert_eq!(DurationRange::new(90, 10), Err(DurationRangeError));

        let invalid = canonical_encode(&(90u16, 10u16)).unwrap();
        assert!(canonical_decode::<DurationRange>(&invalid).is_err());
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Abstract,
    Cards, // xtask-allow-game-id: a genre, not the games/cards package
    SocialDeduction,
    TilePlacement,
    Party,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    Light,
    Medium,
    Heavy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentRating {
    Everyone,
    Teen,
    Mature,
}
