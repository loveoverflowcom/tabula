//! Integration evidence for the generic canonical authority bridge (ADR-0039).
#![cfg(feature = "game-chess")]

use serde::{Deserialize, Serialize};
use tabula_core::{
    canonical_decode, canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Millis,
    Occupant, RuleError, RuleErrorCode, SeatChange, SeatEntry, SeatId, SeatRoster, SpectatorTier,
    TimerId, UserId, Viewer,
};
use tabula_game_api::{
    AdminInput, ConfigError, Ctx, Effect, GameCapabilities, GameMetadata, GameModule, GameRules,
    Init, InitError, Input, Outcome,
};
use tabula_game_chess::{ChessModule, ChessRules, ClockConfig, ClockControl, Command, Config};
use tabula_registry::{ClientViewer, CreatedMatch, ErasedInput, RuntimeError, TypedMatch};

fn roster(seats: &[u8]) -> SeatRoster {
    SeatRoster::new(
        seats
            .iter()
            .map(|seat| SeatEntry {
                seat: SeatId(*seat),
                occupant: Occupant::Human(UserId(u128::from(*seat) + 1)),
                team: None,
            })
            .collect(),
    )
    .unwrap()
}

fn seed() -> MatchSeed {
    MatchSeed::from_bytes([51; 32])
}

// Tests consume the creation receipt before applying, so changed initial
// authorities can never be passed to an actor as a fresh CreatedMatch.
struct TestAuthority {
    runtime: Box<dyn tabula_registry::ErasedMatch>,
    effects: Vec<Effect>,
}

impl core::fmt::Debug for TestAuthority {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("TestAuthority")
            .field("identity", self.runtime.identity())
            .finish_non_exhaustive()
    }
}

impl TestAuthority {
    fn from_created(created: CreatedMatch, expected_seed: &MatchSeed) -> Self {
        let (runtime, bound_seed, _events, effects) = created.into_parts();
        assert_eq!(&bound_seed, expected_seed);
        Self { runtime, effects }
    }

    fn runtime(&self) -> &dyn tabula_registry::ErasedMatch {
        self.runtime.as_ref()
    }
    fn runtime_mut(&mut self) -> &mut dyn tabula_registry::ErasedMatch {
        self.runtime.as_mut()
    }
    fn effects(&self) -> &[Effect] {
        &self.effects
    }
}

fn authority<M: GameModule>(
    config: &[u8],
    roster: &SeatRoster,
    seed: MatchSeed,
) -> Result<TestAuthority, RuntimeError> {
    let expected_seed = seed.clone();
    TypedMatch::<M>::create(config, roster, seed)
        .map(|created| TestAuthority::from_created(created, &expected_seed))
}

fn create() -> TestAuthority {
    let game = tabula_registry::registered_games()
        .into_iter()
        .find(|game| game.metadata().id() == ChessModule::metadata().id())
        .expect("approved module is registered");
    let created = game
        .create_match(
            &canonical_encode(&Config::default()).unwrap(),
            &roster(&[0, 1]),
            seed(),
        )
        .unwrap();
    TestAuthority::from_created(created, &seed())
}

fn player(seat: u8, command: Command) -> ErasedInput {
    ErasedInput::Player {
        seat: SeatId(seat),
        payload: canonical_encode(&command).unwrap(),
    }
}

#[test]
fn catalog_factory_binds_module_identity_and_projected_initial_state() {
    let created = create();
    let identity = created.runtime().identity();
    let metadata = ChessModule::metadata();
    assert_eq!(&identity.game, metadata.id());
    assert_eq!(&identity.game_version, metadata.version());
    assert_eq!(identity.rules_version, ChessRules::RULES_VERSION);
    assert_eq!(identity.rules_hash, ChessModule::rules_hash());
    assert_ne!(identity.rules_hash, [0; 32]);
    assert_eq!(created.runtime().roster().len(), 2);

    let mut rng = DetRng::for_input(&seed(), InputIndex(0));
    let typed = ChessRules::create(
        &Config::default(),
        &roster(&[0, 1]),
        &mut Ctx {
            now: LogicalTime(0),
            index: InputIndex(0),
            rng: &mut rng,
            budget: ChessModule::capabilities().apply_budget(),
        },
    )
    .unwrap();
    for (client, viewer) in [
        (ClientViewer::Seat(SeatId(0)), Viewer::Seat(SeatId(0))),
        (ClientViewer::Seat(SeatId(1)), Viewer::Seat(SeatId(1))),
        (
            ClientViewer::Spectator,
            Viewer::Spectator(SpectatorTier::Live),
        ),
    ] {
        let expected = canonical_encode(&ChessRules::project(&typed.state, viewer)).unwrap();
        assert_eq!(created.runtime().project(client).unwrap(), expected);
        assert_ne!(expected, canonical_encode(&typed.state).unwrap());
    }
    assert_eq!(
        created.runtime().state_hash(),
        ChessRules::state_hash(&typed.state)
    );
}

#[test]
fn creation_rejects_bad_config_encoding_bounds_and_nonstandard_rosters() {
    for bytes in [vec![], vec![1], vec![2, 0, 0]] {
        assert!(matches!(
            authority::<ChessModule>(&bytes, &roster(&[0, 1]), seed()),
            Err(RuntimeError::Malformed(_))
        ));
    }
    let mut trailing = canonical_encode(&Config::default()).unwrap();
    trailing.push(0);
    assert!(matches!(
        authority::<ChessModule>(&trailing, &roster(&[0, 1]), seed()),
        Err(RuntimeError::Malformed(_))
    ));
    assert!(matches!(
        authority::<ChessModule>(&vec![0; 16 * 1024 + 1], &roster(&[0, 1]), seed()),
        Err(RuntimeError::LimitExceeded)
    ));
    for seats in [&[0][..], &[7, 42][..]] {
        assert!(matches!(
            authority::<ChessModule>(
                &canonical_encode(&Config::default()).unwrap(),
                &roster(seats),
                seed(),
            ),
            Err(RuntimeError::Config(ConfigError::SeatCount))
        ));
    }
    let invalid = Config {
        clock: Some(ClockConfig {
            initial: Millis(0),
            control: ClockControl::Fischer {
                increment: Millis(0),
            },
        }),
    };
    assert!(matches!(
        authority::<ChessModule>(
            &canonical_encode(&invalid).unwrap(),
            &roster(&[0, 1]),
            seed(),
        ),
        Err(RuntimeError::Config(_))
    ));
}

#[test]
fn malformed_illegal_and_absent_seats_preserve_authority_and_rng() {
    let mut created = create();
    let before = created.runtime().state_hash();
    let projection = created.runtime().project(ClientViewer::Spectator).unwrap();
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    let original_rng = rng.clone();

    let mut trailing = canonical_encode(&Command::Resign).unwrap();
    trailing.push(0);
    let noncanonical_variant = vec![1, 0, 0x81, 0]; // overlong varint for Resign
    for payload in [vec![], vec![1, 0, 0xff], trailing, noncanonical_variant] {
        assert!(matches!(
            created.runtime_mut().apply(
                ErasedInput::Player {
                    seat: SeatId(0),
                    payload,
                },
                LogicalTime(10),
                InputIndex(1),
                &mut rng,
            ),
            Err(RuntimeError::Malformed(_))
        ));
        assert_eq!(created.runtime().state_hash(), before);
        assert_eq!(
            created.runtime().project(ClientViewer::Spectator).unwrap(),
            projection
        );
    }
    assert!(matches!(
        created.runtime_mut().apply(
            player(
                0,
                Command::Move {
                    from: 255,
                    to: 254,
                    promotion: None
                }
            ),
            LogicalTime(10),
            InputIndex(1),
            &mut rng,
        ),
        Err(RuntimeError::Rule(RuleError {
            code: RuleErrorCode::IllegalMove,
            ..
        }))
    ));
    for input in [
        player(7, Command::Resign),
        ErasedInput::Seat {
            seat: SeatId(7),
            change: SeatChange::Disconnected,
        },
    ] {
        assert!(matches!(
            created
                .runtime_mut()
                .apply(input, LogicalTime(10), InputIndex(1), &mut rng),
            Err(RuntimeError::UnknownSeat(SeatId(7)))
        ));
    }
    assert!(matches!(
        created.runtime().project(ClientViewer::Seat(SeatId(7))),
        Err(RuntimeError::UnknownSeat(SeatId(7)))
    ));
    assert!(matches!(
        created
            .runtime()
            .view_events(&[], ClientViewer::Seat(SeatId(7))),
        Err(RuntimeError::UnknownSeat(SeatId(7)))
    ));
    assert_eq!(created.runtime().state_hash(), before);
    assert_eq!(
        created.runtime().project(ClientViewer::Spectator).unwrap(),
        projection
    );
    let mut expected_rng = original_rng;
    assert_eq!(rng.next_u64(), expected_rng.next_u64());
}

fn assert_projected_chess_outputs(
    runtime: &dyn tabula_registry::ErasedMatch,
    state: &tabula_game_chess::State,
    actual_events: &[Vec<u8>],
    expected_events: &[tabula_game_chess::Event],
) {
    for (client, viewer) in [
        (ClientViewer::Seat(SeatId(0)), Viewer::Seat(SeatId(0))),
        (ClientViewer::Seat(SeatId(1)), Viewer::Seat(SeatId(1))),
        (
            ClientViewer::Spectator,
            Viewer::Spectator(SpectatorTier::Live),
        ),
    ] {
        assert_eq!(
            runtime.project(client).unwrap(),
            canonical_encode(&ChessRules::project(state, viewer)).unwrap()
        );
        let expected_events = expected_events
            .iter()
            .filter_map(|event| ChessRules::view_event(state, event, viewer))
            .map(|event| canonical_encode(&event).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            runtime.view_events(actual_events, client).unwrap(),
            expected_events
        );
    }
}

#[test]
fn real_checkmate_transition_matches_typed_rules_input_events_and_views() {
    let mut created = create();
    let identity = created.runtime().identity().clone();
    let roster = roster(&[0, 1]);
    let mut rng = DetRng::for_input(&seed(), InputIndex(0));
    let mut typed = ChessRules::create(
        &Config::default(),
        &roster,
        &mut Ctx {
            now: LogicalTime(0),
            index: InputIndex(0),
            rng: &mut rng,
            budget: ChessModule::capabilities().apply_budget(),
        },
    )
    .unwrap()
    .state;
    let script = [(0, 13, 21), (1, 52, 36), (0, 14, 30), (1, 59, 31)];
    let mut end_count = 0;
    for (offset, (seat, from, to)) in script.into_iter().enumerate() {
        let index = InputIndex(u64::try_from(offset).unwrap() + 1);
        let now = LogicalTime(index.0 * 10);
        let command = Command::Move {
            from,
            to,
            promotion: None,
        };
        let input = Input::Player {
            seat: SeatId(seat),
            command,
        };
        let mut rng = DetRng::for_input(&seed(), index);
        let mut expected_rng = rng.clone();
        let expected = ChessRules::apply(
            &mut typed,
            input.clone(),
            &mut Ctx {
                now,
                index,
                rng: &mut expected_rng,
                budget: ChessModule::capabilities().apply_budget(),
            },
        )
        .unwrap();
        let actual = created
            .runtime_mut()
            .apply(player(seat, command), now, index, &mut rng)
            .unwrap();
        assert_eq!(actual.canonical_input, canonical_encode(&input).unwrap());
        let decoded: Input<Command> = canonical_decode(&actual.canonical_input).unwrap();
        assert_eq!(canonical_encode(&decoded).unwrap(), actual.canonical_input);
        assert_eq!(
            actual.events,
            expected
                .events
                .iter()
                .map(|event| canonical_encode(event).unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            canonical_encode(&actual.effects).unwrap(),
            canonical_encode(&expected.effects.into_vec()).unwrap()
        );
        end_count += actual
            .effects
            .iter()
            .filter(|effect| matches!(effect, Effect::EndMatch { .. }))
            .count();
        assert_projected_chess_outputs(
            created.runtime(),
            &typed,
            &actual.events,
            expected.events.as_slice(),
        );
        assert_eq!(
            created.runtime().state_hash(),
            ChessRules::state_hash(&typed)
        );
        assert_eq!(created.runtime().identity(), &identity);
        assert_eq!(rng.next_u64(), expected_rng.next_u64());
    }
    assert_eq!(end_count, 1);
    let terminal = created.runtime().state_hash();
    let mut rng = DetRng::for_input(&seed(), InputIndex(5));
    assert!(matches!(
        created.runtime_mut().apply(
            player(0, Command::Resign),
            LogicalTime(50),
            InputIndex(5),
            &mut rng
        ),
        Err(RuntimeError::Rule(RuleError {
            code: RuleErrorCode::MatchOver,
            ..
        }))
    ));
    assert_eq!(created.runtime().state_hash(), terminal);
}

#[test]
fn platform_inputs_enter_the_same_canonical_stream() {
    let mut created = create();
    let opening = created.runtime().state_hash();
    let seat = SeatId(0);
    let change = SeatChange::Disconnected;
    let timer = TimerId(u16::MAX);
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    let transition = created
        .runtime_mut()
        .apply(
            ErasedInput::Seat {
                seat,
                change: change.clone(),
            },
            LogicalTime(10),
            InputIndex(1),
            &mut rng,
        )
        .unwrap();
    assert_eq!(
        transition.canonical_input,
        canonical_encode(&Input::<Command>::Seat { seat, change }).unwrap()
    );
    assert!(transition.events.is_empty());
    assert_eq!(created.runtime().state_hash(), opening);
    let transition = created
        .runtime_mut()
        .apply(
            ErasedInput::Timer { timer },
            LogicalTime(20),
            InputIndex(2),
            &mut rng,
        )
        .unwrap();
    assert_eq!(
        transition.canonical_input,
        canonical_encode(&Input::<Command>::Timer { timer }).unwrap()
    );
    assert_eq!(created.runtime().state_hash(), opening);
    let before = created.runtime().state_hash();
    assert!(matches!(
        created.runtime_mut().apply(
            ErasedInput::Admin(AdminInput::Pause),
            LogicalTime(30),
            InputIndex(3),
            &mut rng
        ),
        Err(RuntimeError::Rule(RuleError {
            code: RuleErrorCode::Unsupported,
            ..
        }))
    ));
    assert_eq!(created.runtime().state_hash(), before);
    let cancel = AdminInput::Cancel {
        reason: tabula_core::AbortReason::OperatorCancelled,
    };
    let transition = created
        .runtime_mut()
        .apply(
            ErasedInput::Admin(cancel.clone()),
            LogicalTime(30),
            InputIndex(3),
            &mut rng,
        )
        .unwrap();
    assert_eq!(
        transition.canonical_input,
        canonical_encode(&Input::<Command>::Admin(cancel)).unwrap()
    );
    assert!(transition
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));
}

#[derive(Clone, Debug, Deserialize)]
struct FragileState {
    value: u8,
    fail_encode: bool,
}

impl Serialize for FragileState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.fail_encode {
            return Err(serde::ser::Error::custom("deliberately broken test state"));
        }
        (self.value, self.fail_encode).serialize(serializer)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct FragileEvent {
    padding: Vec<u8>,
    fail_encode: bool,
}

impl Serialize for FragileEvent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.fail_encode {
            return Err(serde::ser::Error::custom("deliberately broken test event"));
        }
        (&self.padding, self.fail_encode).serialize(serializer)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum FaultCommand {
    RejectAfterMutation,
    FailState,
    FailEvent,
    TooManyEvents,
    OversizedEvent,
    Commit,
}

#[derive(Debug)]
struct FaultRules;

impl GameRules for FaultRules {
    type State = FragileState;
    type Command = FaultCommand;
    type Event = FragileEvent;
    type View = u8;
    type ViewEvent = u8;
    type Config = bool;
    const RULES_VERSION: tabula_core::RulesVersion = ChessRules::RULES_VERSION;

    fn create(_: &bool, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: FragileState {
                value: 0,
                fail_encode: false,
            },
            events: core::iter::empty().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn apply(
        state: &mut FragileState,
        input: Input<FaultCommand>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        let Input::Player { command, .. } = input else {
            return Ok(Outcome::empty());
        };
        state.value = 1;
        ctx.rng.next_u64();
        let mut events = Vec::new();
        match command {
            FaultCommand::RejectAfterMutation => {
                return Err(RuleError::code(RuleErrorCode::IllegalMove))
            }
            FaultCommand::FailState => state.fail_encode = true,
            FaultCommand::FailEvent => events.push(FragileEvent {
                padding: vec![],
                fail_encode: true,
            }),
            FaultCommand::TooManyEvents => events.resize(
                65,
                FragileEvent {
                    padding: vec![],
                    fail_encode: false,
                },
            ),
            FaultCommand::OversizedEvent => events.push(FragileEvent {
                padding: vec![0; 16 * 1024],
                fail_encode: false,
            }),
            FaultCommand::Commit => {}
        }
        Ok(Outcome {
            events: events.into_iter().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn project(state: &FragileState, _: Viewer) -> u8 {
        state.value
    }
    fn view_event(_: &FragileState, _: &FragileEvent, _: Viewer) -> Option<u8> {
        Some(1)
    }
}

#[derive(Debug)]
struct FaultModule;

impl GameModule for FaultModule {
    type Rules = FaultRules;
    fn metadata() -> &'static GameMetadata {
        ChessModule::metadata()
    }
    fn capabilities() -> &'static GameCapabilities {
        ChessModule::capabilities()
    }
    fn validate_config(valid: &bool, _: &SeatRoster) -> Result<(), ConfigError> {
        if *valid {
            Ok(())
        } else {
            Err(ConfigError::field("valid"))
        }
    }
}

#[test]
fn defensive_candidates_contain_partial_mutation_rng_and_serialization_failures() {
    let mut created =
        authority::<FaultModule>(&canonical_encode(&true).unwrap(), &roster(&[7, 42]), seed())
            .unwrap();
    let initial = created.runtime().state_hash();
    let initial_view = created
        .runtime()
        .project(ClientViewer::Seat(SeatId(7)))
        .unwrap();
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    for command in [
        FaultCommand::RejectAfterMutation,
        FaultCommand::FailState,
        FaultCommand::FailEvent,
        FaultCommand::TooManyEvents,
        FaultCommand::OversizedEvent,
    ] {
        let original_rng = rng.clone();
        let result = created.runtime_mut().apply(
            ErasedInput::Player {
                seat: SeatId(7),
                payload: canonical_encode(&command).unwrap(),
            },
            LogicalTime(0),
            InputIndex(1),
            &mut rng,
        );
        assert!(match command {
            FaultCommand::RejectAfterMutation => matches!(result, Err(RuntimeError::Rule(_))),
            FaultCommand::FailState | FaultCommand::FailEvent =>
                matches!(result, Err(RuntimeError::Serialization(_))),
            FaultCommand::TooManyEvents | FaultCommand::OversizedEvent =>
                matches!(result, Err(RuntimeError::LimitExceeded)),
            FaultCommand::Commit => unreachable!(),
        });
        assert_eq!(created.runtime().state_hash(), initial);
        assert_eq!(
            created
                .runtime()
                .project(ClientViewer::Seat(SeatId(7)))
                .unwrap(),
            initial_view
        );
        let mut actual_probe = rng.clone();
        let mut expected_probe = original_rng;
        assert_eq!(actual_probe.next_u64(), expected_probe.next_u64());
    }
    let mut expected_rng = rng.clone();
    expected_rng.next_u64();
    created
        .runtime_mut()
        .apply(
            ErasedInput::Player {
                seat: SeatId(42),
                payload: canonical_encode(&FaultCommand::Commit).unwrap(),
            },
            LogicalTime(0),
            InputIndex(1),
            &mut rng,
        )
        .unwrap();
    assert_ne!(created.runtime().state_hash(), initial);
    assert_eq!(
        created
            .runtime()
            .project(ClientViewer::Seat(SeatId(7)))
            .unwrap(),
        canonical_encode(&1u8).unwrap()
    );
    assert_eq!(rng.next_u64(), expected_rng.next_u64());
}

#[derive(Debug)]
struct BoundaryRules;

impl GameRules for BoundaryRules {
    type State = u8;
    type Command = Vec<u8>;
    type Event = Vec<u8>;
    type View = Vec<u8>;
    type ViewEvent = Vec<u8>;
    type Config = Vec<u8>;
    const RULES_VERSION: tabula_core::RulesVersion = ChessRules::RULES_VERSION;

    fn create(_: &Vec<u8>, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: 0,
            events: core::iter::empty().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn apply(
        state: &mut u8,
        input: Input<Vec<u8>>,
        _: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        let Input::Player { command, .. } = input else {
            return Ok(Outcome::empty());
        };
        *state = command
            .first()
            .copied()
            .ok_or_else(|| RuleError::code(RuleErrorCode::IllegalMove))?;
        Ok(Outcome {
            events: [command].into_iter().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn project(state: &u8, _: Viewer) -> Vec<u8> {
        match state {
            1 => vec![0; 512 * 1024 - 5],
            2 => vec![0; 512 * 1024 - 4],
            _ => Vec::new(),
        }
    }

    fn view_event(_: &u8, event: &Vec<u8>, _: Viewer) -> Option<Vec<u8>> {
        Some(event.clone())
    }
}

#[derive(Debug)]
struct BoundaryModule;

impl GameModule for BoundaryModule {
    type Rules = BoundaryRules;
    fn metadata() -> &'static GameMetadata {
        ChessModule::metadata()
    }
    fn capabilities() -> &'static GameCapabilities {
        ChessModule::capabilities()
    }
    fn validate_config(cfg: &Vec<u8>, _: &SeatRoster) -> Result<(), ConfigError> {
        if cfg.is_empty() {
            Err(ConfigError::field("payload"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn exact_config_command_event_and_projection_limits_are_inclusive() {
    // Two-byte canonical prefix plus two-byte Vec length for this payload.
    let config = canonical_encode(&vec![0u8; 16 * 1024 - 4]).unwrap();
    assert_eq!(config.len(), tabula_protocol::MAX_GAME_PAYLOAD_BYTES);
    let mut created = authority::<BoundaryModule>(&config, &roster(&[7]), seed()).unwrap();
    let oversized_config = canonical_encode(&vec![0u8; 16 * 1024 - 3]).unwrap();
    assert!(matches!(
        authority::<BoundaryModule>(&oversized_config, &roster(&[7]), seed()),
        Err(RuntimeError::LimitExceeded)
    ));
    let command = canonical_encode(&vec![1u8; 16 * 1024 - 4]).unwrap();
    assert_eq!(command.len(), tabula_protocol::MAX_GAME_PAYLOAD_BYTES);
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    let transition = created
        .runtime_mut()
        .apply(
            ErasedInput::Player {
                seat: SeatId(7),
                payload: command.clone(),
            },
            LogicalTime(0),
            InputIndex(1),
            &mut rng,
        )
        .unwrap();
    assert_eq!(transition.events.as_slice(), std::slice::from_ref(&command));
    let redacted = created
        .runtime()
        .view_events(&transition.events, ClientViewer::Seat(SeatId(7)))
        .unwrap();
    assert_eq!(redacted, [command]);
    let at_limit = created.runtime().project(ClientViewer::Spectator).unwrap();
    assert_eq!(at_limit.len(), tabula_protocol::MAX_VIEW_BYTES);
    let hash = created.runtime().state_hash();
    let too_large_command = canonical_encode(&vec![2u8; 16 * 1024 - 3]).unwrap();
    assert!(matches!(
        created.runtime_mut().apply(
            ErasedInput::Player {
                seat: SeatId(7),
                payload: too_large_command
            },
            LogicalTime(0),
            InputIndex(2),
            &mut rng
        ),
        Err(RuntimeError::LimitExceeded)
    ));
    assert_eq!(created.runtime().state_hash(), hash);
    assert_eq!(
        created.runtime().project(ClientViewer::Spectator).unwrap(),
        at_limit
    );
    created
        .runtime_mut()
        .apply(
            ErasedInput::Player {
                seat: SeatId(7),
                payload: canonical_encode(&vec![2u8]).unwrap(),
            },
            LogicalTime(0),
            InputIndex(2),
            &mut rng,
        )
        .unwrap();
    assert!(matches!(
        created.runtime().project(ClientViewer::Spectator),
        Err(RuntimeError::LimitExceeded)
    ));
}

#[test]
fn module_validation_is_called_before_permissive_rules_creation() {
    assert!(matches!(
        authority::<FaultModule>(&canonical_encode(&false).unwrap(), &roster(&[7]), seed()),
        Err(RuntimeError::Config(_))
    ));
}

#[test]
fn initial_timer_effect_and_expiry_stay_game_owned() {
    let config = Config {
        clock: Some(ClockConfig {
            initial: Millis(1000),
            control: ClockControl::Fischer {
                increment: Millis(0),
            },
        }),
    };
    let mut created = authority::<ChessModule>(
        &canonical_encode(&config).unwrap(),
        &roster(&[0, 1]),
        seed(),
    )
    .unwrap();
    let timer = created
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::SetTimer { id, delay } => {
                assert_eq!(*delay, Millis(1000));
                Some(*id)
            }
            _ => None,
        })
        .expect("real rules request an initial timer");
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    let transition = created
        .runtime_mut()
        .apply(
            ErasedInput::Timer { timer },
            LogicalTime(1000),
            InputIndex(1),
            &mut rng,
        )
        .unwrap();
    assert_eq!(
        transition.canonical_input,
        canonical_encode(&Input::<Command>::Timer { timer }).unwrap()
    );
    assert_eq!(
        transition
            .effects
            .iter()
            .filter(|effect| matches!(effect, Effect::EndMatch { .. }))
            .count(),
        1
    );
    assert!(transition
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::CancelTimer { .. })));
}

#[test]
fn redaction_rejects_malformed_and_oversized_event_sequences() {
    let created = authority::<BoundaryModule>(
        &canonical_encode(&vec![0u8]).unwrap(),
        &roster(&[7]),
        seed(),
    )
    .unwrap();
    let event = canonical_encode(&vec![1u8]).unwrap();
    let maximum = vec![event.clone(); tabula_protocol::MAX_EVENTS];
    assert_eq!(
        created
            .runtime()
            .view_events(&maximum, ClientViewer::Spectator)
            .unwrap(),
        maximum
    );
    let mut excessive = maximum;
    excessive.push(event.clone());
    assert!(matches!(
        created
            .runtime()
            .view_events(&excessive, ClientViewer::Spectator),
        Err(RuntimeError::LimitExceeded)
    ));
    let mut trailing = event;
    trailing.push(0);
    assert!(matches!(
        created
            .runtime()
            .view_events(&[trailing], ClientViewer::Spectator),
        Err(RuntimeError::Malformed(_))
    ));
    assert!(matches!(
        created.runtime().view_events(
            &[vec![0; tabula_protocol::MAX_EVENT_BYTES + 1]],
            ClientViewer::Spectator
        ),
        Err(RuntimeError::LimitExceeded)
    ));
}

#[derive(Debug)]
struct MismatchedModule;

impl GameModule for MismatchedModule {
    type Rules = FaultRules;
    fn metadata() -> &'static GameMetadata {
        static METADATA: std::sync::LazyLock<GameMetadata> = std::sync::LazyLock::new(|| {
            let mut spec: tabula_game_api::GameMetadataSpec =
                canonical_decode(&canonical_encode(ChessModule::metadata()).unwrap()).unwrap();
            spec.rules_version = tabula_core::RulesVersion(ChessRules::RULES_VERSION.0 + 1);
            GameMetadata::from(spec)
        });
        &METADATA
    }
    fn capabilities() -> &'static GameCapabilities {
        ChessModule::capabilities()
    }
    fn validate_config(_: &bool, _: &SeatRoster) -> Result<(), ConfigError> {
        Ok(())
    }
}

#[test]
fn creation_rejects_metadata_rules_version_drift() {
    assert!(matches!(
        authority::<MismatchedModule>(&canonical_encode(&true).unwrap(), &roster(&[7]), seed()),
        Err(RuntimeError::IdentityMismatch)
    ));
}

#[test]
#[cfg(feature = "test-support")]
fn approved_clocked_fixture_uses_logical_elapsed_time() {
    let fixture = tabula_registry::runtime::test_support::approved_clocked_fixture();
    let created = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap();
    assert!(matches!(
        created.effects(),
        [Effect::SetTimer {
            id: TimerId(1),
            delay: Millis(1000)
        }]
    ));
    let mut authority = TestAuthority::from_created(created, &fixture.seed);
    let (seat, payload) = &fixture.commands[0];
    let mut rng = DetRng::for_input(&fixture.seed, InputIndex(1));
    let transition = authority
        .runtime_mut()
        .apply(
            ErasedInput::Player {
                seat: *seat,
                payload: payload.clone(),
            },
            LogicalTime(10),
            InputIndex(1),
            &mut rng,
        )
        .unwrap();
    assert_eq!(
        transition.events,
        [vec![1, 0, 0, 0, 13, 21, 0, 0], vec![1, 0, 1, 0, 222, 7]]
    );
    assert!(matches!(
        transition.effects.as_slice(),
        [Effect::SetTimer {
            id: TimerId(1),
            delay: Millis(1000)
        }]
    ));
}
