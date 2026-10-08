//! Owner-requested planned CMP catalog information only; no module/runtime adapter.

use std::sync::LazyLock;

use tabula_core::{GameId, GameVersion, RulesVersion};
use tabula_game_api::metadata::{AssetRef, GameMetadataSpec, I18nKey};

use crate::{
    mobile_discovery::{DiscoveryIcon, PlannedDiscoveryGame},
    Category, Complexity, ContentRating, DurationRange, GameMetadata, Locale, Messages,
};

const EN: Messages = &[
    ("game.werewolf.name", "Werewolf"),
    ("game.werewolf.tagline", "Find the werewolves hiding in the village"),
    (
        "game.werewolf.description",
        "A social deduction game for six to twenty players. Discuss, observe and vote to find the werewolves hiding in the village.",
    ),
];

const VI: Messages = &[
    ("game.werewolf.name", "Ma sói"),
    ("game.werewolf.tagline", "Tìm ma sói ẩn trong ngôi làng"),
    (
        "game.werewolf.description",
        "Trò suy luận xã hội cho 6 đến 20 người. Cùng trao đổi, quan sát và bỏ phiếu để tìm ra ma sói ẩn trong ngôi làng.",
    ),
];

// The game.toml facts are verified by the mobile generator tests. No rules,
// capabilities implementation, configuration, game pack or authority is linked.
static PLANNED: LazyLock<PlannedDiscoveryGame> = LazyLock::new(|| PlannedDiscoveryGame {
    metadata: GameMetadata::from(GameMetadataSpec {
        id: GameId::new("com.tabula.werewolf").expect("registry literal is valid"),
        version: GameVersion::new("0.1.0").expect("registry literal is valid"),
        rules_version: RulesVersion(2),
        name_key: I18nKey::new("game.werewolf.name").expect("registry literal is valid"),
        tagline_key: I18nKey::new("game.werewolf.tagline").expect("registry literal is valid"),
        description_key: I18nKey::new("game.werewolf.description")
            .expect("registry literal is valid"),
        categories: vec![Category::SocialDeduction],
        tags: vec![
            "social".to_owned(),
            "deduction".to_owned(),
            "hidden_role".to_owned(),
        ],
        estimated_minutes: DurationRange::new(15, 45).expect("registry literal is ordered"),
        complexity: Complexity::Medium,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::from_static("werewolf/icon"),
        hero: AssetRef::from_static("werewolf/hero"),
        rules_url_key: None,
    }),
    players: (6..=20).collect(),
    hidden_information: true,
    icon: DiscoveryIcon {
        resource_name: "catalog_icon_werewolf",
        source_dir: "games/werewolf/assets",
    },
    messages: |locale| match locale {
        Locale::En => EN,
        Locale::Vi => VI,
    },
});

/// Public planned facts for CMP; this value cannot normalize a config or create
/// a match and does not implement `GameSetup` (ADR-0045).
#[must_use]
pub fn planned_discovery() -> &'static PlannedDiscoveryGame {
    &PLANNED
}
