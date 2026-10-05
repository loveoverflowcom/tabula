//! # `tabula-game-werewolf` — deterministic social deduction
//!
//! **Phase 3 rules, with a bounded local presentation preview under ADR-0035.**
//! Online chat enforcement, accounts, moderation and voice remain later-phase
//! responsibilities. A local simulator is not an authenticated multiplayer room.
//!
//! [`WerewolfRules`] implements the complete `ClassicV1` preset for 6–20 seats:
//! deterministic role assignment; fixed Night → Dawn → Day → Vote → Dusk windows;
//! simultaneous wolf/Doctor/Witch resolution; Seer reports; precommitted Hunter
//! retaliation; plurality or absolute-majority voting; parity victory and capped
//! stalemate. Decisions are pure data and enter only through `GameRules::apply`.
//!
//! ## Security boundary (I-5/I-6, W-D12–W-D15)
//!
//! Clients receive [`View`] and [`ViewEvent`], never canonical [`State`]. Living
//! seats see their own role/resources; wolves know wolf teammates but never their
//! live action choices. Dead seats and Audit receive full current knowledge;
//! outside spectators see public facts, death reveals and end-game roles.
//! Private action/report/resolution events do not exist for unauthorized viewers.
//! `RolesAssigned` remains audit-only even after death or match end.
//!
//! Doctor and Witch learn their choices, not whether they prevented an attack.
//! Hunter marks are committed while alive and remain visible to their owner
//! through the round's Vote; death never opens an omniscient reactive action.
//! Canonical versions, private-action counts and match seeds are absent from View.
//!
//! ## Functional core (doc 00 §3, W-D8/W-D14/W-D18)
//!
//! All state collections are ordered. Time comes from `Ctx::now`; randomness
//! comes from the named deterministic role-assignment stream. Rejected commands
//! preserve canonical bytes. Retained history is bounded by the configured round
//! cap; replaceable ballots and lifecycle spam are not retained.
//!
//! Disconnect/idle preserves choices and fixed deadlines. Missing choices pass
//! or abstain; absence never automatically kills a seat. Substitution is forbidden
//! and [`WerewolfModule`] returns no production bots under any feature set. The
//! `testkit`-only simulation adapter consumes projected views for verification.
//!
//! Each phase transition emits absolute chat scope facts. Socket enforcement is
//! deferred, and no voice effects are emitted: the current symmetric `VoiceRoom`
//! representation cannot safely express dead-player listen-only membership.
//! The state-version/ack side channel remains a platform protocol question.
//!
//! Rules identity is version 2 with a build-derived source hash. Typed conformance,
//! semantic and privacy tests, deterministic simulation, and three exact canonical
//! replays provide bounded evidence; they do not establish online/voice readiness.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use std::sync::LazyLock;

use tabula_core::Millis;
use tabula_game_api::{
    AssetRef, AsyncTurnPolicy, Budget, Category, ChatChannelSpec, ChatKind, ChatPolicy, Complexity,
    ContentRating, Durability, DurationRange, GameCapabilities, GameCapabilitiesSpec, GameId,
    GameMetadata, GameMetadataSpec, GameVersion, I18nKey, RankedSupport, ReconnectPolicy,
    SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy, TurnModel,
    VoiceRequirement,
};

#[cfg(feature = "presentation")]
pub mod presentation;
pub mod rules;
#[cfg(feature = "testkit")]
pub mod simulation;

pub use rules::{
    checked_deadline, create_initial_state, create_initial_state_from_seed, Alignment, Ballot,
    Command, Config, ConfigValidationError, DurationError, Event, MaxRounds, MaxRoundsError,
    NightChoice, Perspective, Phase, PhaseDuration, PhaseDurations, PlayerStatus, Preset,
    PrivateKnowledge, RawConfig, RawPhaseDurations, RawState, Role, RoleCounts, RoleKnowledge,
    SeatCount, SeatCountError, SeatView, State, StateError, View, ViewEvent, VoteMode,
    WerewolfRules, WitchPotions, DEFAULT_DAWN_MS, DEFAULT_DAY_MS, DEFAULT_DUSK_MS,
    DEFAULT_MAX_ROUNDS, DEFAULT_NIGHT_MS, DEFAULT_VOTE_MS, DOMAIN_ROLES, MAX_MAX_ROUNDS,
    MAX_PHASE_DURATION_MS, MAX_SEATS, MIN_MAX_ROUNDS, MIN_PHASE_DURATION_MS, MIN_SEATS, RULES_HASH,
    RULES_VERSION,
};

/// Compiled catalog metadata, used by the module adapter.
static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| {
    GameMetadata::from(GameMetadataSpec {
        id: GameId::new("com.tabula.werewolf").expect("literal is a valid game id"),
        version: GameVersion::new("0.1.0").expect("literal is valid SemVer"),
        rules_version: RULES_VERSION,
        name_key: I18nKey::new("game.werewolf.name").expect("literal is a valid i18n key"),
        tagline_key: I18nKey::new("game.werewolf.tagline").expect("literal is a valid i18n key"),
        description_key: I18nKey::new("game.werewolf.description")
            .expect("literal is a valid i18n key"),
        categories: vec![Category::SocialDeduction],
        tags: vec![
            "social".to_owned(),
            "deduction".to_owned(),
            "hidden_role".to_owned(),
        ],
        estimated_minutes: DurationRange::new(15, 45).expect("literal range is ordered"),
        complexity: Complexity::Medium,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::new("werewolf/icon").expect("literal is a valid asset reference"),
        hero: AssetRef::new("werewolf/hero").expect("literal is a valid asset reference"),
        rules_url_key: None,
    })
});

/// Compiled platform capabilities, used by the module adapter.
static CAPABILITIES: LazyLock<GameCapabilities> = LazyLock::new(|| {
    GameCapabilities::try_from(GameCapabilitiesSpec {
        seats: SeatSpec::new(
            SeatCounts::range(rules::MIN_SEATS, rules::MAX_SEATS).expect("literal range is valid"),
            None,
            false,
            false,
        ),
        turn_model: TurnModel::Phased,
        hidden_information: true,
        spectators: SpectatorPolicy::GameControlled,
        chat: ChatPolicy::new(
            vec![
                ChatChannelSpec::new("table", ChatKind::Table)
                    .expect("literal is a valid chat channel"),
                ChatChannelSpec::new("wolves", ChatKind::Team)
                    .expect("literal is a valid chat channel"),
                ChatChannelSpec::new("dead", ChatKind::Dead)
                    .expect("literal is a valid chat channel"),
            ],
            true,
        )
        .expect("werewolf chat channels are unique"),
        voice: VoiceRequirement::Recommended,
        ranked: RankedSupport::No,
        async_turns: AsyncTurnPolicy::Disabled,
        reconnect: ReconnectPolicy {
            grace: Millis(60_000),
            notify_rules: true,
        },
        substitution: SubstitutionPolicy::Forbidden,
        pausable: false,
        durability: Durability::AckAfterApply,
        client_preview: false,
        state_size: StateSizeClass::Small,
        apply_budget: Budget {
            max_apply_micros: 2_000,
            max_events_per_input: 64,
        },
        max_match_duration: None,
    })
    .expect("werewolf capabilities are coherent")
});

/// Returns compiled catalog metadata for the module adapter.
#[must_use]
pub fn metadata() -> &'static GameMetadata {
    &METADATA
}

/// Returns compiled platform capabilities for the module adapter.
#[must_use]
pub fn capabilities() -> &'static GameCapabilities {
    &CAPABILITIES
}

/// Package adapter for `ClassicV1`; production bot substitution is forbidden (W-D18).
#[derive(Debug)]
pub struct WerewolfModule;
impl tabula_game_api::GameModule for WerewolfModule {
    type Rules = WerewolfRules;
    fn metadata() -> &'static GameMetadata {
        metadata()
    }
    fn capabilities() -> &'static GameCapabilities {
        capabilities()
    }
    fn validate_config(
        cfg: &Config,
        roster: &tabula_core::SeatRoster,
    ) -> Result<(), tabula_game_api::ConfigError> {
        cfg.validate_roster(roster).map(|_| ())
    }
}
