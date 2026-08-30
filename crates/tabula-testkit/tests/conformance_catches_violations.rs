//! Proves the FIXTURE-driven conformance suite (`crate::conformance`) enforces
//! the invariants it documents, not just the lower-level `determinism` module
//! already exercised by `harness_catches_violations.rs`.
//!
//! Same discipline as that file: each test builds a deliberately broken game
//! plus a fixture for it, and asserts that the relevant check panics naming
//! the invariant it broke. A conformance suite that cannot be observed to
//! fail is not known to enforce anything.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use tabula_core::{
    GameId, GameVersion, MatchSeed, Millis, Occupant, RuleError, RuleErrorCode, RulesVersion,
    SeatEntry, SeatId, SeatRoster, UserId, Viewer,
};
use tabula_game_api::{
    AsyncTurnPolicy, Budget, Category, ChatPolicy, Complexity, ConfigError, ContentRating, Ctx,
    Durability, GameCapabilities, GameMetadata, GameModule, GameRules, Init, InitError, Input,
    LegalCommands, Outcome, RankedSupport, ReconnectPolicy, SeatCounts, SeatSpec, SpectatorPolicy,
    StateSizeClass, SubstitutionPolicy, TurnModel, VoiceRequirement,
};
use tabula_testkit::{GameTestFixture, TerminalScenario};

fn assert_check_rejects(invariant: &str, f: impl FnOnce()) {
    let result = catch_unwind(AssertUnwindSafe(f));
    let Err(payload) = result else {
        panic!(
            "the fixture-driven conformance check ACCEPTED a game that violates {invariant}. \
             The suite is not enforcing anything."
        );
    };
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>");
    assert!(
        message.contains(invariant),
        "the check rejected the game but did not name {invariant}. A failure that does not \
         say which invariant broke is a failure someone will paper over.\nmessage was: {message}"
    );
}

fn one_seat_roster() -> SeatRoster {
    SeatRoster {
        seats: smallvec![SeatEntry {
            seat: SeatId(0),
            occupant: Occupant::Human(UserId(1)),
            team: None,
        }],
    }
}

fn minimal_capabilities() -> GameCapabilities {
    GameCapabilities {
        seats: SeatSpec {
            min: 1,
            max: 1,
            allowed: SeatCounts::Range { min: 1, max: 1 },
            teams: None,
            fill_with_bots: false,
            symmetric: true,
        },
        turn_model: TurnModel::FreeForm,
        hidden_information: false,
        spectators: SpectatorPolicy::Forbidden,
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
            grace: Millis(0),
            notify_rules: false,
        },
        substitution: SubstitutionPolicy::Forbidden,
        pausable: false,
        durability: Durability::AckAfterApply,
        client_preview: true,
        state_size: StateSizeClass::Tiny,
        apply_budget: Budget::default(),
        max_match_duration: None,
    }
}

fn minimal_metadata(id: &str) -> GameMetadata {
    GameMetadata {
        id: GameId(id.to_owned()),
        version: GameVersion("0.0.0".to_owned()),
        rules_version: RulesVersion(1),
        name_key: "test.name".to_owned(),
        tagline_key: "test.tagline".to_owned(),
        description_key: "test.description".to_owned(),
        categories: vec![Category::Abstract],
        tags: Vec::new(),
        estimated_minutes: (1, 1),
        complexity: Complexity::Light,
        content_rating: ContentRating::Everyone,
        icon: tabula_game_api::metadata::AssetRef("icon".to_owned()),
        hero: tabula_game_api::metadata::AssetRef("hero".to_owned()),
        rules_url_key: None,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Nothing;

// ---------------------------------------------------------------------------
// Stable identity: an empty GameId
// ---------------------------------------------------------------------------

mod empty_id {
    use super::*;

    struct Rules;
    impl GameRules for Rules {
        type State = Nothing;
        type Command = Nothing;
        type Event = Nothing;
        type View = Nothing;
        type ViewEvent = Nothing;
        type Config = Nothing;

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: Nothing,
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            _: &mut Nothing,
            _: Input<Nothing>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            Ok(Outcome::empty())
        }

        fn project(_: &Nothing, _: Viewer) -> Nothing {
            Nothing
        }
        fn view_event(_: &Nothing, _: &Nothing, _: Viewer) -> Option<Nothing> {
            None
        }
    }

    struct Module;
    static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| minimal_metadata(""));
    static CAPS: LazyLock<GameCapabilities> = LazyLock::new(minimal_capabilities);
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS
        }
        fn validate_config(_: &Nothing, _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() -> Nothing {
            Nothing
        }
        fn roster() -> SeatRoster {
            one_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([1u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Nothing>> {
            vec![Input::Player {
                seat: SeatId(0),
                command: Nothing,
            }]
        }
    }

    #[test]
    fn conformance_catches_an_empty_game_id() {
        assert_check_rejects("stable game identity", || {
            tabula_testkit::conformance::identity::check::<Fixture>();
        });
    }
}

// ---------------------------------------------------------------------------
// Terminal-state behavior: a game that keeps accepting commands after
// emitting Effect::EndMatch
// ---------------------------------------------------------------------------

mod ignores_terminality {
    use super::*;

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct Counter {
        moves: u32,
    }

    struct Rules;
    impl GameRules for Rules {
        type State = Counter;
        type Command = Nothing;
        type Event = Nothing;
        type View = Nothing;
        type ViewEvent = Nothing;
        type Config = Nothing;

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: Counter::default(),
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            state: &mut Counter,
            _: Input<Nothing>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            state.moves += 1;
            let mut effects = SmallVec::new();
            // The bug: emit EndMatch on move 1, but never actually stop
            // accepting further commands (no MatchOver guard anywhere).
            if state.moves == 1 {
                effects.push(tabula_game_api::Effect::EndMatch {
                    outcome: tabula_core::MatchOutcome {
                        kind: tabula_core::OutcomeKind::Draw,
                        standings: SmallVec::new(),
                        summary: "test".into(),
                    },
                });
            }
            Ok(Outcome {
                events: SmallVec::new(),
                effects,
            })
        }

        fn project(_: &Counter, _: Viewer) -> Nothing {
            Nothing
        }
        fn view_event(_: &Counter, _: &Nothing, _: Viewer) -> Option<Nothing> {
            None
        }
    }

    struct Module;
    static METADATA: LazyLock<GameMetadata> =
        LazyLock::new(|| minimal_metadata("com.tabula.testfixture.ignoresterminality"));
    static CAPS: LazyLock<GameCapabilities> = LazyLock::new(minimal_capabilities);
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS
        }
        fn validate_config(_: &Nothing, _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    fn command() -> Input<Nothing> {
        Input::Player {
            seat: SeatId(0),
            command: Nothing,
        }
    }

    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() -> Nothing {
            Nothing
        }
        fn roster() -> SeatRoster {
            one_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([2u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Nothing>> {
            vec![command()]
        }
        fn terminal() -> Option<TerminalScenario<Nothing>> {
            Some(TerminalScenario {
                script: vec![command()],
                post_terminal: command(),
            })
        }
    }

    #[test]
    fn conformance_catches_a_game_that_ignores_its_own_terminality() {
        assert_check_rejects("terminal-state behavior", || {
            tabula_testkit::conformance::terminal::check::<Fixture>();
        });
    }
}

// ---------------------------------------------------------------------------
// legal_commands sanity: a command enumerated as legal that `apply` rejects
// ---------------------------------------------------------------------------

mod dishonest_legal_commands {
    use super::*;

    #[derive(Copy, Clone, Debug, Serialize, Deserialize)]
    enum Command {
        AlwaysLegalAccordingToTheGame,
    }

    struct Rules;
    impl GameRules for Rules {
        type State = Nothing;
        type Command = Command;
        type Event = Nothing;
        type View = Nothing;
        type ViewEvent = Nothing;
        type Config = Nothing;

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: Nothing,
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            _: &mut Nothing,
            _: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            // The bug: `apply` disagrees with `legal_commands` below and
            // rejects the one command that was advertised as legal.
            Err(RuleError::code(RuleErrorCode::IllegalMove))
        }

        fn project(_: &Nothing, _: Viewer) -> Nothing {
            Nothing
        }
        fn view_event(_: &Nothing, _: &Nothing, _: Viewer) -> Option<Nothing> {
            None
        }

        fn legal_commands(_: &Nothing, _: SeatId) -> LegalCommands<Command> {
            LegalCommands::Enumerated(vec![Command::AlwaysLegalAccordingToTheGame])
        }
    }

    struct Module;
    static METADATA: LazyLock<GameMetadata> =
        LazyLock::new(|| minimal_metadata("com.tabula.testfixture.dishonestlegal"));
    static CAPS: LazyLock<GameCapabilities> = LazyLock::new(minimal_capabilities);
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS
        }
        fn validate_config(_: &Nothing, _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() -> Nothing {
            Nothing
        }
        fn roster() -> SeatRoster {
            one_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([3u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            // `apply` always rejects, so a script must stay empty of
            // "successful" assumptions; determinism/replay only need it
            // non-empty, which a single rejected input satisfies.
            vec![Input::Player {
                seat: SeatId(0),
                command: Command::AlwaysLegalAccordingToTheGame,
            }]
        }
    }

    #[test]
    fn conformance_catches_a_legal_command_that_apply_rejects() {
        assert_check_rejects("legal_commands sanity", || {
            tabula_testkit::conformance::commands::check_legal::<Fixture>();
        });
    }
}

// ---------------------------------------------------------------------------
// State hash sensitivity: a constant/placeholder state_hash override
// ---------------------------------------------------------------------------

mod placeholder_hash {
    use super::*;

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct Counter {
        n: u32,
    }

    struct Rules;
    impl GameRules for Rules {
        type State = Counter;
        type Command = Nothing;
        type Event = Nothing;
        type View = Nothing;
        type ViewEvent = Nothing;
        type Config = Nothing;

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: Counter::default(),
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            state: &mut Counter,
            _: Input<Nothing>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            state.n += 1;
            Ok(Outcome::empty())
        }

        fn project(_: &Counter, _: Viewer) -> Nothing {
            Nothing
        }
        fn view_event(_: &Counter, _: &Nothing, _: Viewer) -> Option<Nothing> {
            None
        }

        // The bug this whole check exists to catch.
        fn state_hash(_: &Counter) -> tabula_core::StateHash {
            tabula_core::StateHash([0u8; 32])
        }
    }

    struct Module;
    static METADATA: LazyLock<GameMetadata> =
        LazyLock::new(|| minimal_metadata("com.tabula.testfixture.placeholderhash"));
    static CAPS: LazyLock<GameCapabilities> = LazyLock::new(minimal_capabilities);
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS
        }
        fn validate_config(_: &Nothing, _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() -> Nothing {
            Nothing
        }
        fn roster() -> SeatRoster {
            one_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([4u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Nothing>> {
            vec![Input::Player {
                seat: SeatId(0),
                command: Nothing,
            }]
        }
    }

    #[test]
    fn conformance_catches_a_placeholder_state_hash() {
        assert_check_rejects("state hash sensitivity", || {
            tabula_testkit::conformance::hashing::check::<Fixture>();
        });
    }
}
