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
pub struct AssetRef(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameMetadata {
    /// Reverse-DNS: `com.tabula.chess`. Unique across the registry, checked at
    /// compile time by `register!`. (doc 02 §8.1)
    pub id: GameId,

    /// Semver of the module package. Bumped for presentation/asset/bot fixes.
    pub version: GameVersion,

    /// Bumped on any state/behaviour change. A match runs one of these for life.
    pub rules_version: RulesVersion,

    // --- i18n keys, not literals ---
    pub name_key: String,
    pub tagline_key: String,
    pub description_key: String,

    pub categories: Vec<Category>,
    pub tags: Vec<String>,

    /// Drives catalog filtering and matchmaking expectations.
    pub estimated_minutes: DurationRange,

    pub complexity: Complexity,

    /// Drives voice/chat defaults and age gating. Compliance consumes this.
    pub content_rating: ContentRating,

    pub icon: AssetRef,
    pub hero: AssetRef,
    pub rules_url_key: Option<String>,
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
