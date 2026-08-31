//! # `tabula-game-chess` — deterministic standard chess rules.
//!
//! The crate owns chess legality and match transitions; it has no renderer,
//! transport, clock, filesystem, or random-number dependency.  Legal moves are
//! generated from pseudo-legal candidates and accepted only when the candidate
//! position leaves its mover's king safe (doc 02 §3, §12.1).
//!
//! Full clock arithmetic is intentionally deferred to the next Phase 1 slice.
//! [`Config::clock`] and [`State::clock`] reserve the typed state seam: that
//! work will consume only `Ctx::now`, `Input::Timer`, and `Effect::SetTimer`.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod movegen;
mod rules;
mod state;

#[cfg(feature = "bots")]
pub mod bot;

pub use movegen::{perft, FenError};
pub use rules::ChessRules;
pub use state::{
    CastlingRights, ClockConfig, ClockState, Color, Command, Config, Event, Piece, PieceKind,
    PositionKey, Square, State, Status, View, ViewEvent,
};

use std::sync::LazyLock;

use tabula_core::{BotLevel, Millis, SeatRoster};
use tabula_game_api::{
    capabilities::ChatKind, metadata::AssetRef, AsyncTurnPolicy, Budget, Category, ChatChannelSpec,
    ChatPolicy, Complexity, ConfigError, ContentRating, Durability, GameCapabilities, GameId,
    GameMetadata, GameModule, GameVersion, RankedSupport, RatingKind, ReconnectPolicy,
    RulesVersion, SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy,
    TurnModel, VoiceRequirement,
};

/// The game package around [`ChessRules`].
#[derive(Debug)]
pub struct ChessModule;

impl GameModule for ChessModule {
    type Rules = ChessRules;

    fn metadata() -> &'static GameMetadata {
        &METADATA
    }

    fn capabilities() -> &'static GameCapabilities {
        &CAPABILITIES
    }

    #[cfg(feature = "bots")]
    fn bot(level: BotLevel) -> Option<Box<dyn tabula_game_api::GameBot<ChessRules>>> {
        matches!(level, BotLevel::Trivial | BotLevel::Easy).then(|| {
            Box::new(bot::ChessBot::new(level)) as Box<dyn tabula_game_api::GameBot<ChessRules>>
        })
    }

    fn validate_config(cfg: &Config, roster: &SeatRoster) -> Result<(), ConfigError> {
        if roster.len() != 2
            || roster.get(tabula_core::SeatId(0)).is_none()
            || roster.get(tabula_core::SeatId(1)).is_none()
        {
            return Err(ConfigError::SeatCount);
        }
        if cfg.clock.is_some() {
            return Err(ConfigError::Unsupported(
                "clock controls are not implemented yet".into(),
            ));
        }
        Ok(())
    }
}

static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| GameMetadata {
    id: GameId("com.tabula.chess".to_owned()),
    version: GameVersion("0.1.0".to_owned()),
    rules_version: RulesVersion(2),
    name_key: "game.chess.name".to_owned(),
    tagline_key: "game.chess.tagline".to_owned(),
    description_key: "game.chess.description".to_owned(),
    categories: vec![Category::Abstract],
    tags: vec!["classic".to_owned(), "strategy".to_owned()],
    estimated_minutes: (10, 90),
    complexity: Complexity::Heavy,
    content_rating: ContentRating::Everyone,
    icon: AssetRef("chess/icon".to_owned()),
    hero: AssetRef("chess/hero".to_owned()),
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
        channels: vec![ChatChannelSpec {
            key: "table".to_owned(),
            kind: ChatKind::Table,
        }],
        game_scoped: false,
    },
    voice: VoiceRequirement::No,
    ranked: RankedSupport::Yes {
        rating: RatingKind::Elo,
    },
    async_turns: AsyncTurnPolicy {
        supported: false,
        turn_deadline: None,
        match_ttl: None,
    },
    reconnect: ReconnectPolicy {
        grace: Millis(60_000),
        notify_rules: true,
    },
    substitution: SubstitutionPolicy::BotOnly {
        levels: vec![BotLevel::Trivial, BotLevel::Easy],
    },
    pausable: false,
    durability: Durability::AckAfterPersist,
    client_preview: true,
    state_size: StateSizeClass::Small,
    apply_budget: Budget {
        max_apply_micros: 2_000,
        max_events_per_input: 4,
    },
    max_match_duration: Some(Millis(14_400_000)),
});
