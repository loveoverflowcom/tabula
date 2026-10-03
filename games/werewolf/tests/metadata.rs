//! Compiled metadata and capability declarations for the W2 package.

use tabula_core::Millis;
use tabula_game_api::{
    AsyncTurnPolicy, Durability, RankedSupport, SpectatorPolicy, StateSizeClass,
    SubstitutionPolicy, TurnModel, VoiceRequirement,
};
use tabula_game_werewolf::{capabilities, metadata, RULES_VERSION};

#[test]
fn compiled_metadata_matches_werewolf_manifest_identity() {
    let compiled = metadata();

    assert_eq!(compiled.id().as_str(), "com.tabula.werewolf");
    assert_eq!(compiled.version().as_str(), "0.1.0");
    assert_eq!(compiled.rules_version(), RULES_VERSION);
    assert_eq!(compiled.name_key().as_str(), "game.werewolf.name");
    assert_eq!(compiled.tagline_key().as_str(), "game.werewolf.tagline");
    assert_eq!(
        compiled.description_key().as_str(),
        "game.werewolf.description"
    );
}

#[test]
fn compiled_capabilities_preserve_werewolf_decisions() {
    let compiled = capabilities();

    assert_eq!(compiled.seats().allowed().min(), 6);
    assert_eq!(compiled.seats().allowed().max(), 20);
    assert!(!compiled.seats().symmetric());
    assert!(!compiled.seats().fill_with_bots());
    assert!(matches!(compiled.turn_model(), TurnModel::Phased));
    assert!(compiled.hidden_information());
    assert!(matches!(
        compiled.spectators(),
        SpectatorPolicy::GameControlled
    ));
    assert!(matches!(compiled.voice(), VoiceRequirement::Recommended));
    assert!(matches!(compiled.ranked(), RankedSupport::No));
    assert!(matches!(compiled.async_turns(), AsyncTurnPolicy::Disabled));
    assert!(matches!(
        compiled.substitution(),
        SubstitutionPolicy::Forbidden
    ));
    assert_eq!(compiled.reconnect().grace, Millis(60_000));
    assert!(compiled.reconnect().notify_rules);
    assert!(!compiled.pausable());
    assert!(matches!(compiled.durability(), Durability::AckAfterApply));
    assert!(!compiled.client_preview());
    assert!(matches!(compiled.state_size(), StateSizeClass::Small));
    assert_eq!(compiled.apply_budget().max_apply_micros, 2_000);
    assert_eq!(compiled.apply_budget().max_events_per_input, 64);
    assert!(compiled.max_match_duration().is_none());

    let channels: Vec<_> = compiled
        .chat()
        .channels()
        .iter()
        .map(|channel| channel.key().as_str())
        .collect();
    assert_eq!(channels, ["table", "wolves", "dead"]);
    assert!(compiled.chat().game_scoped());
}

#[test]
fn manifest_uses_recommended_voice_and_no_fixed_hard_stop() {
    let manifest = include_str!("../game.toml");
    assert!(manifest.contains("voice              = \"recommended\""));
    assert!(!manifest
        .lines()
        .any(|line| { line.trim_start().starts_with("max_match_duration") }));
}
