//! Package bot descriptors are independent of optional policy implementations.

use tabula_core::BotLevel;
use tabula_game_api::GameModule;
use tabula_game_tiles::TilesModule;

const ALL_LEVELS: [BotLevel; 4] = [
    BotLevel::Trivial,
    BotLevel::Easy,
    BotLevel::Medium,
    BotLevel::Hard,
];

#[test]
fn package_declares_bot_levels_independently_of_the_bots_feature() {
    assert_eq!(
        TilesModule::declared_bot_levels(),
        &[BotLevel::Trivial, BotLevel::Easy]
    );
}

#[cfg(feature = "bots")]
#[test]
fn declared_bot_levels_match_factories_for_all_four_levels() {
    for level in ALL_LEVELS {
        let bot = TilesModule::bot(level);
        assert_eq!(
            bot.is_some(),
            TilesModule::declared_bot_levels().contains(&level),
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
        assert!(TilesModule::bot(level).is_none(), "{level:?}");
    }
}
