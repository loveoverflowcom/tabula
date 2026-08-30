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

    /// `(min, max)` — drives catalog filtering and matchmaking expectations.
    pub estimated_minutes: (u16, u16),

    pub complexity: Complexity,

    /// Drives voice/chat defaults and age gating. Compliance consumes this.
    pub content_rating: ContentRating,

    pub icon: AssetRef,
    pub hero: AssetRef,
    pub rules_url_key: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Abstract,
    Cards,
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
