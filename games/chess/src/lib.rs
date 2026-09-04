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

// `rules` is the complete canonical module tree. Package, bot, and
// presentation code may depend on it, but canonical rules never reach back
// into those noncanonical sources.
mod rules;

#[cfg(feature = "bots")]
pub mod bot;

#[cfg(feature = "presentation")]
pub mod presentation;

pub use rules::{
    perft, CastlingRights, ChessRules, ClockConfig, ClockControl, ClockState, Color, Command,
    Config, Event, FenError, Piece, PieceKind, PositionKey, Square, State, Status, View, ViewEvent,
};

use std::sync::LazyLock;

use tabula_core::{BotLevel, Millis, SeatRoster};
use tabula_game_api::{
    capabilities::ChatKind, metadata::AssetRef, AsyncTurnPolicy, BotLevels, Budget, Category,
    ChatChannelSpec, ChatPolicy, Complexity, ConfigError, ContentRating, Durability, DurationRange,
    GameCapabilities, GameCapabilitiesSpec, GameId, GameMetadata, GameMetadataSpec, GameModule,
    GameRules, GameVersion, I18nKey, RankedSupport, RatingKind, ReconnectPolicy, SeatCounts,
    SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy, TurnModel, VoiceRequirement,
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
            if !rules::config_is_valid(clock) {
                return Err(ConfigError::field("clock"));
            }
        }
        Ok(())
    }
}

static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| {
    GameMetadata::from(GameMetadataSpec {
        id: GameId::new("com.tabula.chess").expect("literal is a valid game id"),
        version: GameVersion::new("0.1.0").expect("literal is valid SemVer"),
        rules_version: ChessRules::RULES_VERSION,
        name_key: I18nKey::new("game.chess.name").expect("literal is a valid i18n key"),
        tagline_key: I18nKey::new("game.chess.tagline").expect("literal is a valid i18n key"),
        description_key: I18nKey::new("game.chess.description")
            .expect("literal is a valid i18n key"),
        categories: vec![Category::Abstract],
        tags: vec!["classic".to_owned(), "strategy".to_owned()],
        estimated_minutes: DurationRange::new(10, 90).expect("literal range is ordered"),
        complexity: Complexity::Heavy,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::new("chess/icon").expect("literal is a valid asset reference"),
        hero: AssetRef::new("chess/hero").expect("literal is a valid asset reference"),
        rules_url_key: None,
    })
});

static CAPABILITIES: LazyLock<GameCapabilities> = LazyLock::new(|| {
    GameCapabilities::try_from(GameCapabilitiesSpec {
        seats: SeatSpec::new(
            SeatCounts::range(2, 2).expect("literal range is valid"),
            None,
            true,
            false,
        ),
        turn_model: TurnModel::StrictSequential,
        hidden_information: false,
        spectators: SpectatorPolicy::Live,
        chat: ChatPolicy::new(
            vec![ChatChannelSpec::new("table", ChatKind::Table)
                .expect("literal is a valid chat channel")],
            false,
        )
        .expect("chess chat channels are unique"),
        voice: VoiceRequirement::No,
        ranked: RankedSupport::Yes {
            rating: RatingKind::Elo,
        },
        async_turns: AsyncTurnPolicy::Disabled,
        reconnect: ReconnectPolicy {
            grace: Millis(60_000),
            notify_rules: true,
        },
        substitution: SubstitutionPolicy::BotOnly {
            levels: BotLevels::new(vec![BotLevel::Trivial, BotLevel::Easy])
                .expect("literal levels are non-empty and unique"),
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
    })
    .expect("chess capabilities are coherent")
});
