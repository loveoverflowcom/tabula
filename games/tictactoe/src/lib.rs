//! # `tabula-game-tictactoe` — the SDK smoke test and template
//!
//! **Phase 0.** (doc 07 Phase 0, doc 09 §7 step 7)
//!
//! This is not a product. It exists so that *"does the platform work?"* and
//! *"does my new game work?"* can be answered in seconds. (doc 08 §5.0)
//!
//! It is also the answer to "how do I write a game": one crate, six types, five
//! functions, one manifest, one test line.
//!
//! ## What the author of this crate did NOT write
//!
//! Adding this module to `register!` (one line) yields, with **zero platform
//! changes** (doc 02 §10.3):
//!
//! ```text
//! catalog entry, localized               replay recording + playback
//! room creation UI with config form      spectator support
//! matchmaking queue                      reconnect + resume
//! seat assignment + bot auto-fill        chat (table channel)
//! WebSocket protocol + codec negotiation rate limiting + idempotency
//! authoritative validation               event log + snapshots
//! per-viewer projection dispatch         tracing spans + metrics per command
//! timer scheduling that survives restart ranked ratings (if enabled)
//! push notifications for async turns     admin cancel/inspect tooling
//! ```
//!
//! If adding a game ever requires editing something under `crates/` or
//! `services/`, that is a platform bug. Report it rather than working around it.
//!
//! ## The acceptance gate
//!
//! `cargo xtask selfplay tictactoe --matches 10000` must pass before Phase 1
//! begins (doc 09 §7 step 9).

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod rules;
pub mod state;

#[cfg(feature = "bots")]
pub mod bot;

#[cfg(feature = "presentation")]
pub mod ui;

use std::sync::LazyLock;

use tabula_core::{BotLevel, SeatRoster};
use tabula_game_api::metadata::AssetRef;
use tabula_game_api::{
    AsyncTurnPolicy, Budget, Category, ChatPolicy, Complexity, ConfigError, ContentRating,
    Durability, GameCapabilities, GameId, GameMetadata, GameModule, GameVersion, RankedSupport,
    ReconnectPolicy, RulesVersion, SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass,
    SubstitutionPolicy, TurnModel, VoiceRequirement,
};

pub use rules::TicTacToeRules;
pub use state::{Command, Config, Event, Mark, State, Status, View, ViewEvent};

#[derive(Debug)]
pub struct TicTacToeModule;

impl GameModule for TicTacToeModule {
    type Rules = TicTacToeRules;

    fn metadata() -> &'static GameMetadata {
        &METADATA
    }

    fn capabilities() -> &'static GameCapabilities {
        &CAPABILITIES
    }

    #[cfg(feature = "bots")]
    fn bot(level: BotLevel) -> Option<Box<dyn tabula_game_api::GameBot<TicTacToeRules>>> {
        Some(Box::new(bot::Perfect::new(level)))
    }

    fn validate_config(cfg: &Config, roster: &SeatRoster) -> Result<(), ConfigError> {
        // Fail here, at match creation, rather than mid-match. The lobby shows
        // the offending field to the player, which is why the error names it.
        if roster.len() != 2 {
            return Err(ConfigError::SeatCount);
        }
        if cfg.move_timeout_ms != 0 && cfg.move_timeout_ms < 5_000 {
            return Err(ConfigError::field("move_timeout_ms"));
        }
        Ok(())
    }
}

// TODO(phase 0): replace both statics with the manifest macros from doc 02 §10.2:
//
//     static METADATA:     GameMetadata     = metadata_from_manifest!("game.toml");
//     static CAPABILITIES: GameCapabilities = capabilities_from_manifest!("game.toml");
//
// Until those proc macros exist, these hand-written values are checked against
// `game.toml` by `xtask check-manifests` — so they cannot drift, they are just
// duplicated. Writing the macro is a Phase 0 deliverable precisely because every
// future game would otherwise copy this duplication.
static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| GameMetadata {
    id: GameId("com.tabula.tictactoe".to_owned()),
    version: GameVersion("0.1.0".to_owned()),
    rules_version: RulesVersion(1),
    name_key: "game.tictactoe.name".to_owned(),
    tagline_key: "game.tictactoe.tagline".to_owned(),
    description_key: "game.tictactoe.description".to_owned(),
    categories: vec![Category::Abstract],
    tags: vec!["template".to_owned(), "tutorial".to_owned()],
    estimated_minutes: (1, 3),
    complexity: Complexity::Light,
    content_rating: ContentRating::Everyone,
    icon: AssetRef("tictactoe/icon".to_owned()),
    hero: AssetRef("tictactoe/hero".to_owned()),
    rules_url_key: None,
});

static CAPABILITIES: LazyLock<GameCapabilities> = LazyLock::new(|| GameCapabilities {
    seats: SeatSpec {
        min: 2,
        max: 2,
        allowed: SeatCounts::Range { min: 2, max: 2 },
        teams: None,
        fill_with_bots: true,
        symmetric: false,
    },
    turn_model: TurnModel::StrictSequential,
    hidden_information: false,
    spectators: SpectatorPolicy::Live,
    chat: ChatPolicy {
        channels: Vec::new(),
        game_scoped: false,
    },
    voice: VoiceRequirement::No,
    ranked: RankedSupport::No,
    async_turns: AsyncTurnPolicy {
        supported: false,
        turn_deadline: None,
        match_ttl: None,
    },
    reconnect: ReconnectPolicy {
        grace: tabula_core::Millis(60_000),
        notify_rules: false,
    },
    substitution: SubstitutionPolicy::BotOnly {
        levels: vec![BotLevel::Trivial, BotLevel::Easy],
    },
    pausable: false,
    durability: Durability::AckAfterApply,
    client_preview: true,
    state_size: StateSizeClass::Tiny,
    apply_budget: Budget {
        max_apply_micros: 200,
        max_events_per_input: 4,
    },
    max_match_duration: Some(tabula_core::Millis(600_000)),
});
