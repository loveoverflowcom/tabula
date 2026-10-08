//! Inert artwork and explicitly planned information for the CMP Library (ADR-0045).
//!
//! A planned descriptor is deliberately separate from `ErasedDiscoveryGame`: it
//! has no setup normalization, match factory, rollout authority or runtime vtable.

use crate::{GameMetadata, Locale, Messages};

/// Game-owned square PNG sources copied into the lightweight CMP shell by tooling
/// (doc 04 §3.3). Browsing never opens the game's runtime asset pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscoveryIcon {
    /// Opaque Compose file-resource stem, used without branching on a game id.
    pub resource_name: &'static str,
    /// Repository-relative game asset directory containing the 256/512 PNG companions.
    pub source_dir: &'static str,
}

/// Explicitly curated public information, with no supported setup or launch path
/// (docs/ui/screens/01-library.md; ADR-0045). Presence never changes module rollout.
#[derive(Debug)]
pub struct PlannedDiscoveryGame {
    pub metadata: GameMetadata,
    /// Exact supported seat counts declared by the game manifest, not a live roster.
    pub players: Vec<u8>,
    pub hidden_information: bool,
    pub icon: DiscoveryIcon,
    /// The registry owns translated informational copy just as for linked descriptors.
    pub messages: fn(Locale) -> Messages,
}

/// Owner-requested informational entries consumed only by the generated CMP
/// Library. Linked discovery and authoritative registration remain unchanged.
#[must_use]
pub fn planned_mobile_discovery_games() -> Vec<&'static PlannedDiscoveryGame> {
    vec![crate::games::werewolf::planned_discovery()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_information_does_not_enter_registered_inventories() {
        let planned = planned_mobile_discovery_games();
        assert!(!planned.is_empty());
        let registered = crate::registered_games();
        let discovery = crate::registered_discovery_games();
        for entry in planned {
            assert!(registered
                .iter()
                .all(|game| game.metadata().id() != entry.metadata.id()));
            assert!(discovery
                .iter()
                .all(|game| game.metadata().id() != entry.metadata.id()));
            for locale in Locale::ALL {
                let messages = (entry.messages)(locale);
                for key in [
                    entry.metadata.name_key(),
                    entry.metadata.tagline_key(),
                    entry.metadata.description_key(),
                ] {
                    assert!(messages
                        .iter()
                        .any(|(candidate, _)| *candidate == key.as_str()));
                }
            }
        }
    }
}
