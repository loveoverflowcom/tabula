//! # `tabula-game-werewolf` — the social and scale benchmark
//!
//! > ## PHASE 3 (rules, headless) → PHASE 7 (presentation + online)
//! >
//! > The rules skeleton lands in Phase 3 so the contract is stressed by a
//! > phased, many-seat, heavily-redacted game **before** the protocol is frozen.
//! > Everything social — scoped chat enforcement, moderation, the party model —
//! > waits for Phase 7, after the web shell exists.
//!
//! Werewolf is the game that forces three platform capabilities nothing else
//! needs: **event non-existence**, **game-driven communication scoping**, and
//! **6–20 seats with phases instead of turns**. (doc 08 §5)
//!
//! ## Scope (doc 08 §5)
//!
//! ```text
//! IN:  6–20 seats; roles Villager, Werewolf, Seer, Doctor, Hunter, Witch
//!      (configurable preset sets per seat count); phases Night → Dawn →
//!      Day discussion → Vote → Dusk; night actions; majority/plurality voting
//!      with configurable ties; death reveals role; win conditions (wolves
//!      eliminated / wolves reach parity); dead players become spectators-with-
//!      full-vision; chat scopes per phase; voice scopes per phase; per-phase
//!      timers; NO bot substitution
//! OUT: moderator-run mode, advanced roles (Cupid/lovers, Jester, Alpha),
//!      custom rulesets beyond presets, cross-match reputation — Phase 9+
//! ```
//!
//! ## Contract sketch (doc 02 §12.3)
//!
//! ```rust,ignore
//! struct State {
//!     phase: Phase,                                 // Night, Dawn, Day, Vote, Dusk, Ended
//!     phase_ends_at: LogicalTime,
//!     roles: BTreeMap<SeatId, Role>,                // SECRET
//!     alive: BTreeSet<SeatId>,
//!     night_actions: BTreeMap<SeatId, NightAction>, // SECRET until resolution
//!     votes: BTreeMap<SeatId, SeatId>,              // public in most variants
//!     revealed: BTreeMap<SeatId, Role>,             // becomes public on death
//!     speech: Option<SpeechToken>,
//!     config: Ruleset,
//! }
//! ```
//!
//! Note `BTreeMap`/`BTreeSet` throughout, never `HashMap` (I-2). With 20 seats
//! and iteration that drives vote resolution, nondeterministic ordering would
//! change match outcomes.
//!
//! ## The five contract lessons
//!
//! 1. **`view_event` returns `None` for real.** `NightActionSubmitted` must not
//!    even be *known to exist* by other players — otherwise timing analysis
//!    reveals who acted. `Saved` (doctor) is invisible to everyone but the
//!    doctor until dawn. This is the only game that forces the platform to
//!    support hiding an event's existence.
//! 2. **`RolesAssigned` uses `Audience::ServerOnly`** — in the canonical log for
//!    replay and audit, reaching no client until deaths reveal roles.
//! 3. **Phases are timers.** Each transition emits `Effect::SetTimer` plus
//!    `Effect::SetChatScopes` and `Effect::SetVoiceScopes`:
//!    ```rust,ignore
//!    effects.push(Effect::SetChatScopes(ChatScopes::new()
//!        .allow("table",  Speak::None,                 Listen::None)
//!        .allow("wolves", Speak::Seats(&wolves_alive), Listen::Seats(&wolves_alive))
//!        .allow("dead",   Speak::Seats(&dead),         Listen::Seats(&dead))));
//!    ```
//!    The platform enforces it **at the socket**: the chat service refuses a
//!    message from a seat without `Speak`. The game never touches a socket or an
//!    SFU, and the platform learns nothing about werewolf.
//! 4. **`substitution = Forbidden`.** A werewolf seat carries secret knowledge;
//!    handing it to a bot or another human would leak it or destroy the social
//!    game. A disconnected player is handled by rules — auto-abstain, or death at
//!    dawn per the ruleset.
//! 5. **`spectators = GameControlled`.** Dead players are `Viewer::Seat(_)` and
//!    see everything; outsiders are `Viewer::Spectator(_)` and see only public
//!    information. **This is precisely why `Viewer` is an enum with a seat
//!    variant rather than an `Option<SeatId>`.**
//!
//! `durability = AckAfterApply`: no ranked stakes, and snappy voting matters more
//! than a 50 ms loss window that the phase timer recovers from anyway.
//!
//! ## Failure signals (doc 08 §5)
//!
//! - Any villager socket frame derived from a night action — **critical**.
//! - Chat scope enforcement needing game knowledge.
//! - The platform needing to know phase names.
//!
//! ## Acceptance (doc 08 §5)
//!
//! ```text
//! [ ] 20-seat golden integration match with per-seat projection assertions at
//!     every phase
//! [ ] socket-level chat scope tests (a wolf message never appears on a villager
//!     socket)
//! [ ] projection scan green at 6, 12, and 20 seats, for every role set
//! [ ] disconnect during night, during vote, and during dawn — all per ruleset
//! [ ] vote-burst load scenario (L2) within latency SLO
//! [ ] twelve-human playtest with a leak review by a SECOND engineer
//! [ ] dead-player vision correct; outside spectator vision correct; different
//! [ ] voice scope enforcement verified at the SFU (Phase 8)
//! ```

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

pub mod rules;

pub use rules::{
    checked_deadline, create_initial_state, create_initial_state_from_seed, Alignment, Ballot,
    Config, ConfigValidationError, DurationError, Event, MaxRounds, MaxRoundsError, NightChoice,
    Phase, PhaseDuration, PhaseDurations, PlayerStatus, Preset, RawConfig, RawPhaseDurations,
    RawState, Role, RoleCounts, SeatCount, SeatCountError, State, StateError, VoteMode,
    WitchPotions, DEFAULT_DAWN_MS, DEFAULT_DAY_MS, DEFAULT_DUSK_MS, DEFAULT_MAX_ROUNDS,
    DEFAULT_NIGHT_MS, DEFAULT_VOTE_MS, DOMAIN_ROLES, MAX_MAX_ROUNDS, MAX_PHASE_DURATION_MS,
    MAX_SEATS, MIN_MAX_ROUNDS, MIN_PHASE_DURATION_MS, MIN_SEATS, RULES_HASH, RULES_VERSION,
};

/// Compiled catalog metadata, kept independent from the incomplete W7 module adapter.
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

/// Compiled platform capabilities, kept independent from the incomplete W7 module adapter.
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

/// Returns compiled catalog metadata without requiring a complete [`tabula_game_api::GameModule`].
#[must_use]
pub fn metadata() -> &'static GameMetadata {
    &METADATA
}

/// Returns compiled platform capabilities without requiring a complete [`tabula_game_api::GameModule`].
#[must_use]
pub fn capabilities() -> &'static GameCapabilities {
    &CAPABILITIES
}
