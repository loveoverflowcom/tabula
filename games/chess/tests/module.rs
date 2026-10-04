//! Package bot descriptors are independent of optional policy implementations.

use tabula_core::{BotLevel, SeatRoster};
use tabula_game_api::{ConfigError, GameCapabilities, GameMetadata, GameModule};
use tabula_game_chess::{ChessModule, ChessRules, Config};

const ALL_LEVELS: [BotLevel; 4] = [
    BotLevel::Trivial,
    BotLevel::Easy,
    BotLevel::Medium,
    BotLevel::Hard,
];

#[test]
fn package_declares_bot_levels_independently_of_the_bots_feature() {
    assert_eq!(
        ChessModule::declared_bot_levels(),
        &[BotLevel::Trivial, BotLevel::Easy]
    );
}

#[cfg(feature = "bots")]
#[test]
fn declared_bot_levels_match_factories_for_all_four_levels() {
    for level in ALL_LEVELS {
        let bot = ChessModule::bot(level);
        assert_eq!(
            bot.is_some(),
            ChessModule::declared_bot_levels().contains(&level),
            "{level:?} descriptor and factory must agree"
        );
        if let Some(bot) = bot {
            assert_eq!(bot.level(), level);
        }
    }
}

#[cfg(not(feature = "bots"))]
#[test]
fn rules_only_build_does_not_construct_bot_factories() {
    for level in ALL_LEVELS {
        assert!(ChessModule::bot(level).is_none(), "{level:?}");
    }
}

/// A module that has not opted into the package bot inventory contract.
struct DefaultBotInventoryModule;

impl GameModule for DefaultBotInventoryModule {
    type Rules = ChessRules;

    fn metadata() -> &'static GameMetadata {
        ChessModule::metadata()
    }

    fn capabilities() -> &'static GameCapabilities {
        ChessModule::capabilities()
    }

    fn validate_config(cfg: &Config, roster: &SeatRoster) -> Result<(), ConfigError> {
        ChessModule::validate_config(cfg, roster)
    }
}

#[test]
fn module_bot_inventory_defaults_to_empty() {
    assert!(DefaultBotInventoryModule::declared_bot_levels().is_empty());
    for level in ALL_LEVELS {
        assert!(DefaultBotInventoryModule::bot(level).is_none(), "{level:?}");
    }
}
