//! Server-only recovery bridge evidence for I-5/I-16 and doc 05 §§7–8.
#![cfg(feature = "game-chess")]

use serde::{Deserialize, Serialize};
use tabula_core::{
    canonical_decode, canonical_encode, CanonicalError, DetRng, GameId, GameVersion, InputIndex,
    LogicalTime, MatchSeed, Occupant, RuleError, RuleErrorCode, RulesVersion, SeatChange,
    SeatEntry, SeatId, SeatRoster, TimerId, UserId, Viewer,
};
use tabula_game_api::{
    AdminInput, ConfigError, Ctx, GameCapabilities, GameMetadata, GameModule, GameRules, Init,
    InitError, Input, Outcome,
};
use tabula_game_chess::ChessModule;
use tabula_registry::{
    runtime::{MAX_RUNTIME_INPUT_BYTES, MAX_RUNTIME_PAYLOAD_BYTES, MAX_RUNTIME_STATE_BYTES},
    ClientViewer, ErasedInput, ErasedMatch, RuntimeError, TypedMatch,
};

fn roster() -> SeatRoster {
    SeatRoster::new(
        vec![SeatEntry {
            seat: SeatId(7),
            occupant: Occupant::Human(UserId(8)),
            team: None,
        }]
        .into(),
    )
    .unwrap()
}

fn seed() -> MatchSeed {
    MatchSeed::from_bytes([91; 32])
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ContextState {
    records: Vec<(u64, u64, u64, u8)>,
    padding: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ContextCommand {
    Record { payload: Vec<u8> },
    Grow { bytes: u32 },
    Reject,
}

#[derive(Debug)]
struct ContextRules;

impl GameRules for ContextRules {
    type State = ContextState;
    type Command = ContextCommand;
    type Event = (u64, u64, u64, u8);
    type View = u64;
    type ViewEvent = u8;
    type Config = u32;
    const RULES_VERSION: RulesVersion = <ChessModule as GameModule>::Rules::RULES_VERSION;

    fn create(padding: &u32, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: ContextState {
                padding: vec![0; *padding as usize],
                ..ContextState::default()
            },
            events: core::iter::empty().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn apply(
        state: &mut ContextState,
        input: Input<ContextCommand>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        let draw = ctx.rng.next_u64();
        let kind = match input {
            Input::Player { command, .. } => match command {
                ContextCommand::Record { .. } => 0,
                ContextCommand::Grow { bytes } => {
                    state.padding = vec![0; bytes as usize];
                    return Ok(Outcome::empty());
                }
                ContextCommand::Reject => {
                    state.records.push((0, 0, 0, 255));
                    return Err(RuleError::code(RuleErrorCode::IllegalMove));
                }
            },
            Input::Timer { .. } => 1,
            Input::Seat { .. } => 2,
            Input::Admin(_) => 3,
        };
        let record = (ctx.now.0, ctx.index.0, draw, kind);
        state.records.push(record);
        Ok(Outcome {
            events: [record].into_iter().collect(),
            effects: core::iter::empty().collect(),
        })
    }

    fn project(state: &ContextState, _: Viewer) -> u64 {
        state.records.len() as u64
    }

    fn view_event(_: &ContextState, event: &Self::Event, _: Viewer) -> Option<u8> {
        Some(event.3)
    }
}

#[derive(Debug)]
struct ContextModule;

impl GameModule for ContextModule {
    type Rules = ContextRules;
    fn metadata() -> &'static GameMetadata {
        ChessModule::metadata()
    }
    fn capabilities() -> &'static GameCapabilities {
        ChessModule::capabilities()
    }
    fn validate_config(padding: &u32, _: &SeatRoster) -> Result<(), ConfigError> {
        if *padding > 2 * 1024 * 1024 {
            Err(ConfigError::field("padding"))
        } else {
            Ok(())
        }
    }
}

fn create(padding: u32) -> Box<dyn ErasedMatch> {
    TypedMatch::<ContextModule>::create(&canonical_encode(&padding).unwrap(), &roster(), seed())
        .unwrap()
        .into_parts()
        .0
}

fn canonical_record(payload: Vec<u8>) -> Vec<u8> {
    canonical_encode(&Input::Player {
        seat: SeatId(7),
        command: ContextCommand::Record { payload },
    })
    .unwrap()
}

#[test]
fn replay_preserves_exact_time_original_index_rng_and_all_input_variants() {
    let mut live = create(0);
    let mut replayed = create(0);
    let inputs = [
        ErasedInput::Player {
            seat: SeatId(7),
            payload: canonical_encode(&ContextCommand::Record { payload: vec![] }).unwrap(),
        },
        ErasedInput::Timer { timer: TimerId(19) },
        ErasedInput::Seat {
            seat: SeatId(7),
            change: SeatChange::Disconnected,
        },
        ErasedInput::Admin(AdminInput::Pause),
    ];
    // Original indices deliberately have gaps; compacting them changes RNG (I-4).
    for (ordinal, input) in inputs.into_iter().enumerate() {
        let index = InputIndex([1, 4, 9, 20][ordinal]);
        let now = LogicalTime([10, 100, 100, 1000][ordinal]);
        let mut live_rng = DetRng::for_input(&seed(), index);
        let mut replay_rng = DetRng::for_input(&seed(), index);
        let mut expected_rng = DetRng::for_input(&seed(), index);
        let expected_record = (
            now.0,
            index.0,
            expected_rng.next_u64(),
            u8::try_from(ordinal).unwrap(),
        );
        let recorded = live.apply(input, now, index, &mut live_rng).unwrap();
        let actual = replayed
            .replay(&recorded.canonical_input, now, index, &mut replay_rng)
            .unwrap();
        assert_eq!(actual.canonical_input, recorded.canonical_input);
        assert_eq!(actual.events, [canonical_encode(&expected_record).unwrap()]);
        assert_eq!(actual.events, recorded.events);
        assert_eq!(
            canonical_encode(&actual.effects).unwrap(),
            canonical_encode(&recorded.effects).unwrap()
        );
        assert_eq!(replayed.snapshot().unwrap(), live.snapshot().unwrap());
        assert_eq!(replayed.state_hash(), live.state_hash());
        let expected_probe = expected_rng.next_u64();
        assert_eq!(replay_rng.next_u64(), expected_probe);
        assert_eq!(live_rng.next_u64(), expected_probe);
    }
    let actual: ContextState = canonical_decode(&replayed.snapshot().unwrap()).unwrap();
    assert_eq!(actual.records.len(), 4);
    assert_eq!(replayed.creation_config(), canonical_encode(&0u32).unwrap());
    assert_eq!(
        replayed.project(ClientViewer::Spectator).unwrap(),
        canonical_encode(&4u64).unwrap()
    );
}

#[test]
fn restore_rejects_every_foreign_identity_axis_before_snapshot_decode() {
    let runtime = create(0);
    for axis in 0..4 {
        let mut identity = runtime.identity().clone();
        match axis {
            0 => identity.game = GameId::new("com.test.unregistered".to_owned()).unwrap(),
            1 => identity.game_version = GameVersion::new("999.0.0".to_owned()).unwrap(),
            2 => identity.rules_version = RulesVersion(identity.rules_version.0 + 1),
            3 => identity.rules_hash[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            TypedMatch::<ContextModule>::restore(
                &identity,
                runtime.creation_config(),
                &roster(),
                &[]
            ),
            Err(RuntimeError::RestoreIdentityMismatch)
        ));
    }
}

#[test]
fn restore_checks_snapshot_canonical_encoding_version_shape_and_exact_bytes() {
    let runtime = create(0);
    let snapshot = runtime.snapshot().unwrap();
    assert_eq!(snapshot, [1, 0, 0, 0]);
    let restored = TypedMatch::<ContextModule>::restore(
        runtime.identity(),
        runtime.creation_config(),
        &roster(),
        &snapshot,
    )
    .unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_eq!(restored.state_hash(), runtime.state_hash());
    assert_eq!(restored.creation_config(), runtime.creation_config());
    for bytes in [
        vec![],
        vec![1],
        vec![1, 0],
        vec![1, 0, 255],
        vec![1, 0, 0, 0, 0],
        vec![1, 0, 0x80, 0, 0],
    ] {
        assert!(
            matches!(
                TypedMatch::<ContextModule>::restore(
                    runtime.identity(),
                    runtime.creation_config(),
                    &roster(),
                    &bytes
                ),
                Err(RuntimeError::Malformed(_))
            ),
            "accepted malformed/noncanonical snapshot: {bytes:?}"
        );
    }
    let mut future = snapshot;
    future[..2].copy_from_slice(&2u16.to_le_bytes());
    assert!(matches!(
        TypedMatch::<ContextModule>::restore(
            runtime.identity(),
            runtime.creation_config(),
            &roster(),
            &future
        ),
        Err(RuntimeError::Malformed(CanonicalError::EncodingVersion {
            found: 2,
            ..
        }))
    ));
    assert!(matches!(
        TypedMatch::<ContextModule>::restore(
            runtime.identity(),
            runtime.creation_config(),
            &roster(),
            &vec![0; MAX_RUNTIME_STATE_BYTES + 1]
        ),
        Err(RuntimeError::LimitExceeded)
    ));
    assert!(matches!(
        TypedMatch::<ContextModule>::restore(runtime.identity(), &[1, 0], &roster(), &[1, 0, 0, 0]),
        Err(RuntimeError::Malformed(_))
    ));
    assert!(matches!(
        TypedMatch::<ContextModule>::restore(
            runtime.identity(),
            &canonical_encode(&u32::MAX).unwrap(),
            &roster(),
            &[1, 0, 0, 0]
        ),
        Err(RuntimeError::Config(_))
    ));
}

#[test]
fn snapshot_state_bound_is_inclusive_at_create_restore_and_apply() {
    // Prefix 2 + records length 1 + padding length varint 3 = 6 bytes.
    let maximum_padding = u32::try_from(MAX_RUNTIME_STATE_BYTES - 6).unwrap();
    let mut runtime = create(maximum_padding);
    let snapshot = runtime.snapshot().unwrap();
    assert_eq!(snapshot.len(), MAX_RUNTIME_STATE_BYTES);
    let restored = TypedMatch::<ContextModule>::restore(
        runtime.identity(),
        runtime.creation_config(),
        &roster(),
        &snapshot,
    )
    .unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert!(matches!(
        TypedMatch::<ContextModule>::create(
            &canonical_encode(&(maximum_padding + 1)).unwrap(),
            &roster(),
            seed()
        ),
        Err(RuntimeError::LimitExceeded)
    ));
    let before = runtime.state_hash();
    let mut rng = DetRng::for_input(&seed(), InputIndex(2));
    let mut original_rng = rng.clone();
    let grow = canonical_encode(&Input::Player {
        seat: SeatId(7),
        command: ContextCommand::Grow {
            bytes: maximum_padding + 1,
        },
    })
    .unwrap();
    assert!(matches!(
        runtime.replay(&grow, LogicalTime(30), InputIndex(2), &mut rng),
        Err(RuntimeError::LimitExceeded)
    ));
    assert_eq!(runtime.state_hash(), before);
    assert_eq!(runtime.snapshot().unwrap(), snapshot);
    assert_eq!(rng.next_u64(), original_rng.next_u64());
}

#[test]
fn replay_rejects_malformed_unknown_seat_oversized_and_rule_rejected_records_without_mutation() {
    let mut runtime = create(0);
    let snapshot = runtime.snapshot().unwrap();
    let mut unknown_player = canonical_record(vec![]);
    unknown_player[3] = 99;
    let unknown_seat = canonical_encode(&Input::<ContextCommand>::Seat {
        seat: SeatId(99),
        change: SeatChange::Disconnected,
    })
    .unwrap();
    let rejected = canonical_encode(&Input::Player {
        seat: SeatId(7),
        command: ContextCommand::Reject,
    })
    .unwrap();
    let mut trailing = canonical_record(vec![]);
    trailing.push(0);
    let mut noncanonical = canonical_record(vec![]);
    noncanonical.splice(2..3, [0x80, 0]);
    let mut future = canonical_record(vec![]);
    future[0] = 2;
    let oversized_command = canonical_record(vec![0; MAX_RUNTIME_PAYLOAD_BYTES]);
    assert!(oversized_command.len() <= MAX_RUNTIME_INPUT_BYTES);
    for (bytes, class) in [
        (vec![], 0),
        (trailing, 0),
        (noncanonical, 0),
        (future, 0),
        (unknown_player, 1),
        (unknown_seat, 1),
        (oversized_command, 2),
        (vec![0; MAX_RUNTIME_INPUT_BYTES + 1], 2),
        (rejected, 3),
    ] {
        let mut rng = DetRng::for_input(&seed(), InputIndex(9));
        let mut original_rng = rng.clone();
        let result = runtime.replay(&bytes, LogicalTime(50), InputIndex(9), &mut rng);
        assert!(match class {
            0 => matches!(result, Err(RuntimeError::Malformed(_))),
            1 => matches!(result, Err(RuntimeError::UnknownSeat(SeatId(99)))),
            2 => matches!(result, Err(RuntimeError::LimitExceeded)),
            3 => matches!(result, Err(RuntimeError::Rule(_))),
            _ => unreachable!(),
        });
        assert_eq!(runtime.snapshot().unwrap(), snapshot);
        assert_eq!(rng.next_u64(), original_rng.next_u64());
    }
}

#[test]
fn replay_accepts_exact_command_limit_with_canonical_input_wrapper() {
    let payload = vec![0; MAX_RUNTIME_PAYLOAD_BYTES - 5];
    let command = canonical_encode(&ContextCommand::Record {
        payload: payload.clone(),
    })
    .unwrap();
    assert_eq!(command.len(), MAX_RUNTIME_PAYLOAD_BYTES);
    let input = canonical_record(payload);
    assert_eq!(input.len(), MAX_RUNTIME_PAYLOAD_BYTES + 2);
    let mut runtime = create(0);
    let mut rng = DetRng::for_input(&seed(), InputIndex(1));
    assert_eq!(
        runtime
            .replay(&input, LogicalTime(1), InputIndex(1), &mut rng)
            .unwrap()
            .canonical_input,
        input
    );
}

#[test]
#[cfg(feature = "test-support")]
fn approved_native_game_replays_through_generic_snapshot_and_recorded_input_bridge() {
    let fixture = tabula_registry::runtime::test_support::approved_checkmate_fixture();
    let created = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap();
    let identity = created.runtime().identity().clone();
    let (mut live, bound_seed, _, _) = created.into_parts();
    assert_eq!(bound_seed, fixture.seed);
    let (mut replayed, _, _, _) = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap()
        .into_parts();
    let mut records = Vec::new();
    for (ordinal, (seat, payload)) in fixture.commands.into_iter().enumerate() {
        let index = InputIndex(ordinal as u64 * 2 + 1);
        let now = LogicalTime(ordinal as u64 * 10 + 3);
        let mut rng = DetRng::for_input(&fixture.seed, index);
        let transition = live
            .apply(ErasedInput::Player { seat, payload }, now, index, &mut rng)
            .unwrap();
        let snapshot = live.snapshot().unwrap();
        records.push((
            transition,
            now,
            index,
            snapshot,
            live.state_hash(),
            live.project(ClientViewer::Spectator).unwrap(),
        ));
    }
    for (transition, now, index, snapshot, expected_hash, expected_view) in records {
        let mut rng = DetRng::for_input(&fixture.seed, index);
        let replayed_transition = replayed
            .replay(&transition.canonical_input, now, index, &mut rng)
            .unwrap();
        assert_eq!(replayed_transition.events, transition.events);
        assert_eq!(
            canonical_encode(&replayed_transition.effects).unwrap(),
            canonical_encode(&transition.effects).unwrap()
        );
        assert_eq!(replayed.snapshot().unwrap(), snapshot);
        assert_eq!(replayed.state_hash(), expected_hash);
        // Decode/re-encode every live checkpoint, then continue replaying from it.
        replayed = fixture
            .game
            .restore_match(&identity, &fixture.config, &fixture.roster, &snapshot)
            .unwrap();
        assert_eq!(replayed.state_hash(), expected_hash);
        assert_eq!(replayed.creation_config(), fixture.config);
        assert_eq!(
            replayed.project(ClientViewer::Spectator).unwrap(),
            expected_view
        );
    }
    assert_eq!(replayed.state_hash(), live.state_hash());
    assert_eq!(replayed.snapshot().unwrap(), live.snapshot().unwrap());
}

#[test]
#[cfg(feature = "test-support")]
fn approved_long_fixture_exercises_twenty_four_nonterminal_inputs() {
    let fixture = tabula_registry::runtime::test_support::approved_long_fixture();
    assert_eq!(fixture.commands.len(), 24);
    let (mut runtime, _, _, _) = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap()
        .into_parts();
    for (ordinal, (seat, payload)) in fixture.commands.into_iter().enumerate() {
        let index = InputIndex(ordinal as u64 + 1);
        let mut rng = DetRng::for_input(&fixture.seed, index);
        let transition = runtime
            .apply(
                ErasedInput::Player { seat, payload },
                LogicalTime(index.0 * 10),
                index,
                &mut rng,
            )
            .unwrap();
        assert!(
            !transition
                .effects
                .iter()
                .any(|effect| matches!(effect, tabula_game_api::Effect::EndMatch { .. })),
            "fixture unexpectedly ended at input {index:?}"
        );
    }
}
