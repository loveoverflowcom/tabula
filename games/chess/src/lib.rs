//! # `tabula-game-chess` — deterministic standard chess rules.
//!
//! The crate owns chess legality and match transitions; it has no renderer,
//! transport, wall-clock, filesystem, or random-number dependency.  Legal moves are
//! generated from pseudo-legal candidates and accepted only when the candidate
//! position leaves its mover's king safe (doc 02 §3, §12.1).
//!
//! Clock arithmetic is owned by the rules and consumes only `Ctx::now`,
//! `Input::Timer`, and timer effects. The shell schedules timer effects; it does
//! not know the meaning of Fischer or Bronstein controls.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod clock;
mod movegen;
mod rules;
mod state;

#[cfg(feature = "bots")]
pub mod bot;

pub use movegen::{perft, FenError};
pub use rules::ChessRules;
pub use state::{
    CastlingRights, ClockConfig, ClockControl, ClockState, Color, Command, Config, Event, Piece,
    PieceKind, PositionKey, Square, State, Status, View, ViewEvent,
};

use std::sync::LazyLock;

use tabula_core::{BotLevel, Millis, SeatRoster};
use tabula_game_api::{
    capabilities::ChatKind, metadata::AssetRef, AsyncTurnPolicy, BotLevels, Budget, Category,
    ChatChannelSpec, ChatPolicy, Complexity, ConfigError, ContentRating, Durability, DurationRange,
    GameCapabilities, GameId, GameMetadata, GameModule, GameRules, GameVersion, RankedSupport,
    RatingKind, ReconnectPolicy, SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass,
    SubstitutionPolicy, TurnModel, VoiceRequirement,
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
        if let Some(clock) = &cfg.clock {
            if !clock::config_is_valid(clock) {
                return Err(ConfigError::field("clock"));
            }
        }
        Ok(())
    }
}

static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| GameMetadata {
    id: GameId::new("com.tabula.chess").expect("literal is a valid game id"),
    version: GameVersion::new("0.1.0").expect("literal is valid SemVer"),
    rules_version: ChessRules::RULES_VERSION,
    name_key: "game.chess.name".to_owned(),
    tagline_key: "game.chess.tagline".to_owned(),
    description_key: "game.chess.description".to_owned(),
    categories: vec![Category::Abstract],
    tags: vec!["classic".to_owned(), "strategy".to_owned()],
    estimated_minutes: DurationRange::new(10, 90).expect("literal range is ordered"),
    complexity: Complexity::Heavy,
    content_rating: ContentRating::Everyone,
    icon: AssetRef("chess/icon".to_owned()),
    hero: AssetRef("chess/hero".to_owned()),
    rules_url_key: None,
});

static CAPABILITIES: LazyLock<GameCapabilities> = LazyLock::new(|| {
    GameCapabilities::new(
        SeatSpec::new(
            SeatCounts::range(2, 2).expect("literal range is valid"),
            None,
            true,
            false,
        ),
        TurnModel::StrictSequential,
        false,
        SpectatorPolicy::Live,
        ChatPolicy {
            channels: vec![ChatChannelSpec {
                key: "table".to_owned(),
                kind: ChatKind::Table,
            }],
            game_scoped: false,
        },
        VoiceRequirement::No,
        RankedSupport::Yes {
            rating: RatingKind::Elo,
        },
        AsyncTurnPolicy::Disabled,
        ReconnectPolicy {
            grace: Millis(60_000),
            notify_rules: true,
        },
        SubstitutionPolicy::BotOnly {
            levels: BotLevels::new(vec![BotLevel::Trivial, BotLevel::Easy])
                .expect("literal levels are non-empty and unique"),
        },
        false,
        Durability::AckAfterPersist,
        true,
        StateSizeClass::Small,
        Budget {
            max_apply_micros: 2_000,
            max_events_per_input: 4,
        },
        Some(Millis(14_400_000)),
    )
    .expect("chess capabilities are coherent")
});
