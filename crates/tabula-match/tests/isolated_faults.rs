//! Faithful offline ports and deliberately hostile/hidden fake rules.
//! This is not real authentication, storage or transport acceptance.
#![cfg(all(feature = "isolated", not(target_arch = "wasm32")))]

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use tabula_core::{
    canonical_encode, InputIndex, MatchId, MatchSeed, Occupant, RuleError, RuleErrorCode,
    RulesVersion, SeatEntry, SeatId, SeatRoster, SessionId, UserId, Viewer,
};
use tabula_game_api::{
    capabilities::ChatKind, metadata::AssetRef, AsyncTurnPolicy, Budget, Category, ChatChannelSpec,
    ChatPolicy, Complexity, ConfigError, ContentRating, Ctx, Durability, DurationRange, Effect,
    GameCapabilities, GameCapabilitiesSpec, GameId, GameMetadata, GameMetadataSpec, GameModule,
    GameRules, GameVersion, I18nKey, Init, InitError, Input, Outcome, RankedSupport,
    ReconnectPolicy, SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy,
    TurnModel, VoiceRequirement,
};
use tabula_match::{
    ports::Clock,
    runtime::{
        self, AdmissionError, Binding, Completion, Exit, HostControl, Limits, MatchHandle, Summary,
        Ticket,
    },
    runtime_ports::{
        Authority, AuthorityLost, Effects, Journal, JournalRecord, Output, Purpose,
        RuntimePortError,
    },
};
use tabula_protocol::{ClientEnvelope, ErrorCode, GameCommandFrame, ServerEnvelope, ServerMessage};
use tabula_registry::runtime::{ClientViewer, ErasedInput, TypedMatch};
use tokio::sync::Notify;

struct HiddenModule;
struct HiddenRules;
impl GameModule for HiddenModule {
    type Rules = HiddenRules;
    fn metadata() -> &'static GameMetadata {
        &META
    }
    fn capabilities() -> &'static GameCapabilities {
        &CAPS
    }
    fn validate_config(_: &u8, roster: &SeatRoster) -> Result<(), ConfigError> {
        if roster.len() == 2 {
            Ok(())
        } else {
            Err(ConfigError::SeatCount)
        }
    }
}

static META: LazyLock<GameMetadata> = LazyLock::new(|| {
    GameMetadataSpec {
        id: GameId::new("test.runtime.fixture").unwrap(),
        version: GameVersion::new("0.0.1").unwrap(),
        rules_version: RulesVersion(1),
        name_key: I18nKey::new("fixture.name").unwrap(),
        tagline_key: I18nKey::new("fixture.tagline").unwrap(),
        description_key: I18nKey::new("fixture.description").unwrap(),
        categories: vec![Category::Abstract],
        tags: vec![],
        estimated_minutes: DurationRange::new(1, 1).unwrap(),
        complexity: Complexity::Light,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::new("fixture/icon").unwrap(),
        hero: AssetRef::new("fixture/hero").unwrap(),
        rules_url_key: None,
    }
    .into()
});

static CAPS: LazyLock<GameCapabilities> = LazyLock::new(|| {
    GameCapabilitiesSpec {
        seats: SeatSpec::new(SeatCounts::range(2, 2).unwrap(), None, true, false),
        turn_model: TurnModel::Simultaneous,
        hidden_information: true,
        spectators: SpectatorPolicy::Live,
        chat: ChatPolicy::new(
            vec![ChatChannelSpec::new("table", ChatKind::Table).unwrap()],
            false,
        )
        .unwrap(),
        voice: VoiceRequirement::No,
        ranked: RankedSupport::No,
        async_turns: AsyncTurnPolicy::Disabled,
        reconnect: ReconnectPolicy {
            grace: tabula_core::Millis(0),
            notify_rules: true,
        },
        substitution: SubstitutionPolicy::Forbidden,
        pausable: false,
        durability: Durability::AckAfterPersist,
        client_preview: false,
        state_size: StateSizeClass::Small,
        apply_budget: Budget {
            max_apply_micros: 1000,
            max_events_per_input: 4,
        },
        max_match_duration: None,
    }
    .try_into()
    .unwrap()
});

impl GameRules for HiddenRules {
    type State = (u64, u64);
    type Command = u8;
    type Event = (u8, u64);
    type View = Vec<u64>;
    type ViewEvent = (u8, u64);
    type Config = u8;
    const RULES_VERSION: RulesVersion = RulesVersion(1);
    fn create(config: &u8, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: (u64::from(*config), 0),
            events: [].into_iter().collect(),
            effects: [].into_iter().collect(),
        })
    }
    fn apply(
        state: &mut Self::State,
        input: Input<u8>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        let command = match input {
            Input::Player {
                seat: SeatId(0),
                command,
            } => command,
            Input::Player { .. } => return Err(RuleError::code(RuleErrorCode::NotYourTurn)),
            Input::Timer { .. } | Input::Admin(_) => 1,
            Input::Seat { .. } => 5,
        };
        let mut events = smallvec::SmallVec::new();
        match command {
            0 => {
                state.0 += 1;
                events.push((0, state.0));
            }
            1 => {
                state.1 += 1;
                events.push((1, state.1));
            }
            2 => panic!("deliberate rules fixture panic"),
            3 => {
                state.0 = state.0.wrapping_add(ctx.rng.next_u64());
                return Err(RuleError::with_detail(
                    RuleErrorCode::IllegalMove,
                    "private diagnostic never on wire",
                ));
            }
            5 => {}
            6 => {
                state.1 = 999;
            }
            _ => return Err(RuleError::code(RuleErrorCode::UnknownCommand)),
        }
        Ok(Outcome {
            events,
            effects: smallvec::SmallVec::new(),
        })
    }
    fn project(state: &Self::State, viewer: Viewer) -> Self::View {
        if state.1 == 999 {
            return vec![0; 600_000];
        }
        if viewer == Viewer::Seat(SeatId(0)) {
            vec![state.1, state.0]
        } else {
            vec![state.1]
        }
    }
    fn view_event(_: &Self::State, event: &Self::Event, viewer: Viewer) -> Option<Self::ViewEvent> {
        (event.0 == 1 || viewer == Viewer::Seat(SeatId(0))).then_some(*event)
    }
}

#[derive(Default)]
struct Auth {
    grants: Mutex<BTreeMap<SessionId, (Binding, ClientViewer)>>,
    fail_prepare: Mutex<Option<Purpose>>,
    preparations: Mutex<Vec<Purpose>>,
    lose_after_apply: AtomicU64,
    apply_calls: AtomicU64,
}
impl Auth {
    fn grant(&self, binding: &Binding, viewer: ClientViewer) {
        self.grants
            .lock()
            .unwrap()
            .insert(binding.session(), (binding.clone(), viewer));
    }
    fn revoke(&self, binding: &Binding) {
        self.grants.lock().unwrap().remove(&binding.session());
    }
}
impl Authority for Auth {
    async fn prepare(&self, _: &Binding, purpose: Purpose) -> Result<(), AuthorityLost> {
        self.preparations.lock().unwrap().push(purpose);
        if *self.fail_prepare.lock().unwrap() == Some(purpose) {
            Err(AuthorityLost)
        } else {
            Ok(())
        }
    }

    fn with_current<T>(
        &self,
        binding: &Binding,
        purpose: Purpose,
        action: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost> {
        let guard = self.grants.lock().unwrap();
        let (current, viewer) = guard.get(&binding.session()).ok_or(AuthorityLost)?;
        if current != binding {
            return Err(AuthorityLost);
        }
        let required = match purpose {
            Purpose::Apply(seat) => ClientViewer::Seat(seat),
            Purpose::Observe(viewer) | Purpose::Receipt(viewer) => viewer,
        };
        if *viewer != required {
            return Err(AuthorityLost);
        }
        let result = action();
        if matches!(purpose, Purpose::Apply(_))
            && self.apply_calls.fetch_add(1, Ordering::SeqCst) + 1
                == self.lose_after_apply.load(Ordering::SeqCst)
        {
            return Err(AuthorityLost);
        }
        Ok(result)
    }
}

struct Log {
    records: Mutex<Vec<JournalRecord>>,
    pause: AtomicU64,
    fail: AtomicU64,
    panic: AtomicU64,
    entered: Notify,
    release: Notify,
}
impl Default for Log {
    fn default() -> Self {
        Self {
            records: Mutex::default(),
            pause: AtomicU64::new(u64::MAX),
            fail: AtomicU64::new(u64::MAX),
            panic: AtomicU64::new(u64::MAX),
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}
impl Journal for Log {
    async fn append(&self, record: JournalRecord) -> Result<(), RuntimePortError> {
        assert_ne!(
            record.index.0,
            self.panic.load(Ordering::SeqCst),
            "deliberate journal panic"
        );
        if record.index.0 == self.pause.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        if record.index.0 == self.fail.load(Ordering::SeqCst) {
            return Err(RuntimePortError::Indeterminate);
        }
        let mut records = self.records.lock().unwrap();
        assert_eq!(
            record.index.0,
            u64::try_from(records.len()).unwrap(),
            "atomic contiguous append"
        );
        records.push(record);
        Ok(())
    }
}

#[derive(Default)]
struct Out {
    frames: Mutex<Vec<(SessionId, ServerEnvelope)>>,
    busy: AtomicBool,
}
impl Output for Out {
    fn submit(&self, binding: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError> {
        let mut frames = self.frames.lock().unwrap();
        if self.busy.load(Ordering::SeqCst) || frames.len() >= 256 {
            return Err(RuntimePortError::Busy);
        }
        frames.push((binding.session(), frame));
        Ok(())
    }
}

#[derive(Default)]
struct Fx {
    log: Arc<Log>,
    keys: Mutex<Vec<InputIndex>>,
    fail: AtomicBool,
    callback: Mutex<Option<Arc<HostControl>>>,
    tickets: Mutex<Vec<Ticket>>,
}
impl Effects for Fx {
    async fn execute(
        &self,
        _: MatchId,
        index: InputIndex,
        _: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        assert!(self
            .log
            .records
            .lock()
            .unwrap()
            .iter()
            .any(|record| record.index == index));
        if self.fail.load(Ordering::SeqCst) {
            return Err(RuntimePortError::Unavailable);
        }
        self.keys.lock().unwrap().push(index);
        if index == InputIndex(1) {
            if let Some(host) = self.callback.lock().unwrap().take() {
                self.tickets.lock().unwrap().push(
                    host.input(ErasedInput::Timer {
                        timer: tabula_core::TimerId(0),
                    })
                    .unwrap(),
                );
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct Time(AtomicU64);
impl Clock for Time {
    fn now_unix_ms(&self) -> u64 {
        self.monotonic_ms()
    }
    fn monotonic_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Harness {
    handle: MatchHandle,
    host: Arc<HostControl>,
    join: tokio::task::JoinHandle<Summary>,
    auth: Arc<Auth>,
    log: Arc<Log>,
    out: Arc<Out>,
    fx: Arc<Fx>,
    clock: Arc<Time>,
}
impl Harness {
    fn new(secret: u8, limits: Limits) -> Self {
        Self::starting_at(secret, limits, 0)
    }
    fn starting_at(secret: u8, limits: Limits, clock_start: u64) -> Self {
        let roster = SeatRoster::new(
            (0..2)
                .map(|seat| SeatEntry {
                    seat: SeatId(seat),
                    occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                    team: None,
                })
                .collect(),
        )
        .unwrap();
        let created = TypedMatch::<HiddenModule>::create(
            &canonical_encode(&secret).unwrap(),
            &roster,
            MatchSeed::from_bytes([7; 32]),
        )
        .unwrap();
        let auth = Arc::new(Auth::default());
        let log = Arc::new(Log::default());
        let out = Arc::new(Out::default());
        let fx = Arc::new(Fx {
            log: Arc::clone(&log),
            ..Fx::default()
        });
        let clock = Arc::new(Time(AtomicU64::new(clock_start)));
        let (handle, host, join) = runtime::spawn(
            MatchId(1),
            created,
            runtime::Ports {
                authority: Arc::clone(&auth),
                journal: Arc::clone(&log),
                output: Arc::clone(&out),
                effects: Arc::clone(&fx),
                clock: Arc::clone(&clock),
            },
            limits,
        )
        .unwrap();
        Self {
            handle,
            host: Arc::new(host),
            join,
            auth,
            log,
            out,
            fx,
            clock,
        }
    }
    async fn attach(&self, session: u64, viewer: ClientViewer) -> Binding {
        let binding = Binding::new(
            SessionId(session),
            UserId(u128::from(session)),
            u128::from(session),
            1,
            1,
        );
        self.auth.grant(&binding, viewer);
        assert_eq!(
            self.handle
                .attach(binding.clone(), viewer)
                .unwrap()
                .wait()
                .await
                .unwrap(),
            Completion::Submitted
        );
        binding
    }
    fn command(seq: u64, value: u8) -> ClientEnvelope {
        ClientEnvelope::new(
            seq,
            seq,
            GameCommandFrame::new(
                MatchId(1),
                META.id().clone(),
                META.version().clone(),
                canonical_encode(&value).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    async fn send(&self, binding: &Binding, seq: u64, value: u8) -> Completion {
        self.handle
            .command(binding.clone(), Self::command(seq, value))
            .unwrap()
            .wait()
            .await
            .unwrap()
    }
    async fn end(self) -> Summary {
        self.host.drain().unwrap().wait().await.unwrap();
        self.join.await.unwrap()
    }
    fn frames_for(&self, session: SessionId) -> Vec<ServerEnvelope> {
        self.out
            .frames
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| *target == session)
            .map(|(_, frame)| frame.clone())
            .collect()
    }
}

#[tokio::test]
async fn hidden_actions_have_no_observer_frames_or_version_gaps_and_public_control_is_visible() {
    let mut observer_transcripts = Vec::new();
    let mut authorized_transcripts = Vec::new();
    for secret in [11, 29] {
        let harness = Harness::new(secret, Limits::default());
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        let other = harness.attach(2, ClientViewer::Seat(SeatId(1))).await;
        let spectator = harness.attach(3, ClientViewer::Spectator).await;
        let before = harness.frames_for(other.session());
        for seq in 1..=3 {
            assert_eq!(harness.send(&owner, seq, 0).await, Completion::Submitted);
        }
        assert_eq!(
            harness.frames_for(other.session()),
            before,
            "private event existence must stay hidden"
        );
        assert_eq!(harness.frames_for(spectator.session()).len(), 1);
        assert_eq!(harness.send(&owner, 4, 1).await, Completion::Submitted);
        let observer = harness.frames_for(other.session());
        assert_eq!(observer.len(), 2);
        assert!(matches!(
            observer[1].body(),
            ServerMessage::MatchUpdate { revision: 1, .. }
        ));
        assert_eq!(
            observer[1].frame(),
            2,
            "hidden inputs cannot leak frame gaps"
        );
        observer_transcripts.push(observer);
        authorized_transcripts.push(harness.frames_for(owner.session()));
        {
            let records = harness.log.records.lock().unwrap();
            assert_eq!(records.len(), 5);
            assert_ne!(
                records[0].hash, records[3].hash,
                "genuine secret state changed"
            );
        }
        assert_eq!(harness.end().await.version, tabula_core::StateVersion(4));
    }
    assert_eq!(
        observer_transcripts[0], observer_transcripts[1],
        "secret scrambling preserves unauthorized observable stream"
    );
    assert_ne!(
        authorized_transcripts[0], authorized_transcripts[1],
        "authorized knowledge control is nonvacuous"
    );
}

#[tokio::test]
async fn receipt_eviction_expiry_and_scope_capacity_never_forget_live_highwatermarks() {
    let harness = Harness::new(
        1,
        Limits {
            receipts_per_scope: 1,
            operation_scopes: 1,
            receipt_ttl_ms: 10,
            ..Limits::default()
        },
    );
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.send(&owner, 1, 1).await;
    harness.send(&owner, 2, 1).await;
    harness.send(&owner, 1, 1).await;
    assert!(matches!(
        harness.frames_for(owner.session()).last().unwrap().body(),
        ServerMessage::Reject {
            error: ErrorCode::StaleSeq,
            ..
        }
    ));
    harness.clock.0.store(10, Ordering::SeqCst);
    harness.send(&owner, 2, 1).await;
    assert!(matches!(
        harness.frames_for(owner.session()).last().unwrap().body(),
        ServerMessage::Reject {
            error: ErrorCode::StaleSeq,
            ..
        }
    ));
    harness
        .handle
        .detach(owner.clone())
        .unwrap()
        .wait()
        .await
        .unwrap();
    let replacement = Binding::new(owner.session(), owner.subject(), owner.record(), 2, 2);
    harness
        .auth
        .grant(&replacement, ClientViewer::Seat(SeatId(0)));
    let replacement_attach = harness
        .handle
        .attach(replacement.clone(), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        replacement_attach,
        Completion::Suppressed,
        "full scope capacity refuses new attachment instead of forgetting a watermark"
    );
    assert_eq!(
        harness.send(&replacement, 1, 1).await,
        Completion::Suppressed
    );
    assert_eq!(harness.end().await.version, tabula_core::StateVersion(2));
}

#[tokio::test]
async fn revocation_during_append_suppresses_private_output_and_stale_epoch_and_seat_work() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.log.pause.store(1, Ordering::SeqCst);
    let first = harness
        .handle
        .command(owner.clone(), Harness::command(1, 0))
        .unwrap();
    harness.log.entered.notified().await;
    let queued = harness
        .handle
        .command(owner.clone(), Harness::command(2, 1))
        .unwrap();
    harness.auth.revoke(&owner);
    harness.log.release.notify_one();
    assert_eq!(first.wait().await.unwrap(), Completion::Suppressed);
    assert_eq!(queued.wait().await.unwrap(), Completion::Suppressed);
    assert_eq!(
        harness.frames_for(owner.session()).len(),
        1,
        "no private ack/update after authority loss"
    );
    assert_eq!(
        harness.log.records.lock().unwrap().len(),
        2,
        "already applied input can commit under explicitly offline contract"
    );
    assert_eq!(harness.end().await.version, tabula_core::StateVersion(1));
}

#[tokio::test]
async fn per_sender_backpressure_leaves_room_for_other_senders_and_cancel_before_dequeue_is_noop() {
    let harness = Harness::new(
        1,
        Limits {
            mailbox: 4,
            per_session_inflight: 2,
            ..Limits::default()
        },
    );
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    let other = harness.attach(2, ClientViewer::Seat(SeatId(1))).await;
    harness.log.pause.store(1, Ordering::SeqCst);
    let first = harness
        .handle
        .command(owner.clone(), Harness::command(1, 1))
        .unwrap();
    harness.log.entered.notified().await;
    let cancelled = harness
        .handle
        .command(owner.clone(), Harness::command(2, 1))
        .unwrap();
    assert!(matches!(
        harness
            .handle
            .command(owner.clone(), Harness::command(3, 1)),
        Err(AdmissionError::Busy)
    ));
    let other_ticket = harness
        .handle
        .command(other.clone(), Harness::command(1, 1))
        .unwrap();
    drop(cancelled);
    harness.log.release.notify_one();
    first.wait().await.unwrap();
    other_ticket.wait().await.unwrap();
    assert_eq!(
        harness.send(&owner, 2, 1).await,
        Completion::Submitted,
        "cancelled seq remains reusable"
    );
    assert_eq!(harness.end().await.version, tabula_core::StateVersion(2));
}

#[tokio::test]
async fn indeterminate_append_and_effect_failures_never_ack_success_and_close_all_queued_work() {
    for effects_fail in [false, true] {
        let harness = Harness::new(1, Limits::default());
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        if effects_fail {
            harness.fx.fail.store(true, Ordering::SeqCst);
        } else {
            harness.log.fail.store(1, Ordering::SeqCst);
        }
        let ticket = harness
            .handle
            .command(owner.clone(), Harness::command(1, 1))
            .unwrap();
        assert_eq!(ticket.wait().await.unwrap_err(), AdmissionError::Closed);
        assert_eq!(harness.frames_for(owner.session()).len(), 1);
        let exit = harness.join.await.unwrap().exit;
        assert_eq!(
            exit,
            if effects_fail {
                Exit::EffectsFailed
            } else {
                Exit::JournalFailed
            }
        );
        assert!(matches!(
            harness.handle.command(owner, Harness::command(2, 1)),
            Err(AdmissionError::Closed)
        ));
    }
}

#[tokio::test]
async fn rules_panic_and_projection_bounds_stop_without_success_or_private_diagnostics() {
    for (command, expected) in [(2, Exit::RulesPanic), (6, Exit::ProjectionFailed)] {
        let harness = Harness::new(1, Limits::default());
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        assert_eq!(
            harness
                .handle
                .command(owner.clone(), Harness::command(1, command))
                .unwrap()
                .wait()
                .await
                .unwrap_err(),
            AdmissionError::Closed
        );
        assert_eq!(harness.frames_for(owner.session()).len(), 1);
        assert_eq!(harness.join.await.unwrap().exit, expected);
    }
}

#[tokio::test]
async fn queued_duplicate_rechecks_authority_and_known_success_receipt_survives_output_busy() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.send(&owner, 1, 0).await;
    harness.auth.revoke(&owner);
    assert_eq!(harness.send(&owner, 1, 0).await, Completion::Suppressed);
    assert_eq!(harness.log.records.lock().unwrap().len(), 2);
    harness.auth.grant(&owner, ClientViewer::Seat(SeatId(0)));
    harness
        .handle
        .attach(owner.clone(), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    harness.out.busy.store(true, Ordering::SeqCst);
    assert_eq!(harness.send(&owner, 2, 1).await, Completion::Suppressed);
    harness.out.busy.store(false, Ordering::SeqCst);
    harness
        .handle
        .attach(owner.clone(), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    harness.send(&owner, 2, 1).await;
    assert_eq!(
        harness.log.records.lock().unwrap().len(),
        3,
        "output retry must not reapply"
    );
    harness.end().await;
}

#[tokio::test]
async fn effect_callback_try_enqueue_is_nonreentrant_and_owner_drop_drains_fifo() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    *harness.fx.callback.lock().unwrap() = Some(Arc::clone(&harness.host));
    harness.send(&owner, 1, 1).await;
    let callback = harness.fx.tickets.lock().unwrap().pop().unwrap();
    callback.wait().await.unwrap();
    assert_eq!(
        *harness.fx.keys.lock().unwrap(),
        [InputIndex(0), InputIndex(1), InputIndex(2)]
    );
    drop(harness.handle);
    drop(harness.host);
    assert_eq!(harness.join.await.unwrap().exit, Exit::Closed);
}

#[tokio::test]
async fn unexpected_port_panic_is_observable_as_joinerror_and_mailbox_closes() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.log.panic.store(1, Ordering::SeqCst);
    assert_eq!(
        harness
            .handle
            .command(owner.clone(), Harness::command(1, 1))
            .unwrap()
            .wait()
            .await
            .unwrap_err(),
        AdmissionError::Closed
    );
    assert!(harness.join.await.unwrap_err().is_panic());
    assert!(matches!(
        harness.handle.command(owner, Harness::command(2, 1)),
        Err(AdmissionError::Closed)
    ));
}

#[tokio::test]
async fn rejected_mutating_fake_is_transactional_and_later_legal_probe_matches_clean_baseline() {
    let mut final_hashes = Vec::new();
    for reject_first in [false, true] {
        let harness = Harness::new(1, Limits::default());
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        let seq = if reject_first {
            harness.send(&owner, 1, 3).await;
            2
        } else {
            1
        };
        harness.send(&owner, seq, 1).await;
        final_hashes.push(harness.log.records.lock().unwrap().last().unwrap().hash);
        assert_eq!(harness.end().await.version, tabula_core::StateVersion(1));
    }
    assert_eq!(final_hashes[0], final_hashes[1]);
}

#[tokio::test]
async fn logical_time_is_match_relative_and_never_rewinds_when_the_host_clock_does() {
    let harness = Harness::starting_at(1, Limits::default(), 600_000);
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.clock.0.store(600_010, Ordering::SeqCst);
    harness.send(&owner, 1, 1).await;
    harness.clock.0.store(500_000, Ordering::SeqCst);
    harness.send(&owner, 2, 1).await;
    harness.clock.0.store(600_030, Ordering::SeqCst);
    harness.send(&owner, 3, 1).await;
    let times: Vec<_> = harness
        .log
        .records
        .lock()
        .unwrap()
        .iter()
        .map(|record| record.now.0)
        .collect();
    assert_eq!(times, [0, 10, 10, 30]);
    harness.end().await;
}

#[tokio::test]
async fn all_eighty_one_bounded_retry_conflict_rejection_traces_match_independent_receipt_model() {
    // Three operations, four positions. The model tracks only original
    // sequence/payload/results and successfully applied count, not game state.
    let alphabet = [(1, 1), (1, 0), (2, 3)];
    for encoding in 0..81 {
        let harness = Harness::new(
            1,
            Limits {
                receipts_per_scope: 2,
                ..Limits::default()
            },
        );
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        let mut digits = encoding;
        let mut highest = 0;
        let mut applied = 0;
        let mut receipts: BTreeMap<u64, (u8, Result<(), ErrorCode>)> = BTreeMap::new();
        for _ in 0..4 {
            let (seq, value) = alphabet[digits % alphabet.len()];
            digits /= alphabet.len();
            let expected = if seq <= highest {
                receipts
                    .get(&seq)
                    .map_or(Err(ErrorCode::StaleSeq), |(original, result)| {
                        if *original == value {
                            *result
                        } else {
                            Err(ErrorCode::OperationConflict)
                        }
                    })
            } else {
                highest = seq;
                let result = if value == 3 {
                    Err(ErrorCode::RuleRejected)
                } else {
                    applied += 1;
                    Ok(())
                };
                receipts.insert(seq, (value, result));
                result
            };
            harness.send(&owner, seq, value).await;
            let frames = harness.frames_for(owner.session());
            let response = frames
                .iter()
                .rev()
                .find_map(|frame| match frame.body() {
                    ServerMessage::Ack { seq: received } if *received == seq => Some(Ok(())),
                    ServerMessage::Reject {
                        seq: received,
                        error,
                    } if *received == seq => Some(Err(*error)),
                    _ => None,
                })
                .expect("every admitted operation has a receipt");
            assert_eq!(
                response, expected,
                "trace {encoding}, seq {seq}, payload {value}"
            );
        }
        let summary = harness.end().await;
        assert_eq!(
            summary.version.0, applied,
            "trace {encoding} applies once per novel accepted operation"
        );
    }
}

#[tokio::test]
async fn failed_creation_receipt_never_attaches_or_submits_an_initial_projection() {
    let harness = Harness::new(1, Limits::default());
    harness.log.fail.store(0, Ordering::SeqCst);
    let binding = Binding::new(SessionId(1), UserId(1), 1, 1, 1);
    harness.auth.grant(&binding, ClientViewer::Seat(SeatId(0)));
    let result = harness
        .handle
        .attach(binding, ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await;
    assert_eq!(result.unwrap_err(), AdmissionError::Closed);
    assert!(harness.out.frames.lock().unwrap().is_empty());
    assert_eq!(harness.join.await.unwrap().exit, Exit::JournalFailed);
}

#[tokio::test]
async fn private_commands_cannot_change_another_seats_operation_scope_admission_or_probe() {
    let mut observed = Vec::new();
    for private_command in [false, true] {
        let harness = Harness::new(
            1,
            Limits {
                operation_scopes: 1,
                ..Limits::default()
            },
        );
        let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
        let other = Binding::new(SessionId(2), UserId(2), 2, 1, 1);
        harness.auth.grant(&other, ClientViewer::Seat(SeatId(1)));
        let attachment = harness
            .handle
            .attach(other.clone(), ClientViewer::Seat(SeatId(1)))
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            attachment,
            Completion::Suppressed,
            "capacity is reserved at attach, before any hidden command"
        );
        if private_command {
            harness.send(&owner, 1, 0).await;
        }
        let probe = harness.send(&other, 1, 1).await;
        observed.push((attachment, probe, harness.frames_for(other.session())));
        harness.end().await;
    }
    assert_eq!(
        observed[0], observed[1],
        "active admission/probe cannot infer private command existence"
    );
}

#[tokio::test]
async fn exact_sequence_window_and_extreme_counters_reject_without_consuming_the_next_operation() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    for (seq, expected) in [
        (65, Err(ErrorCode::SeqTooFar)),
        (1, Ok(())),
        (65, Ok(())),
        (130, Err(ErrorCode::SeqTooFar)),
        (u64::MAX, Err(ErrorCode::SeqTooFar)),
        (66, Ok(())),
    ] {
        harness.send(&owner, seq, 5).await;
        let frames = harness.frames_for(owner.session());
        let actual = match frames.last().unwrap().body() {
            ServerMessage::Ack { .. } => Ok(()),
            ServerMessage::Reject { error, .. } => Err(*error),
            ServerMessage::MatchUpdate { .. } => {
                panic!("no-op commands still receive operation receipts")
            }
        };
        assert_eq!(actual, expected, "sequence {seq}");
    }
    assert_eq!(
        harness.end().await.version,
        tabula_core::StateVersion(3),
        "accepted no-ops advance canonical version exactly once"
    );
}

#[tokio::test]
async fn actually_full_mailbox_returns_busy_and_a_later_retry_is_unconsumed() {
    let harness = Harness::new(
        1,
        Limits {
            mailbox: 4,
            per_session_inflight: 2,
            ..Limits::default()
        },
    );
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.log.pause.store(1, Ordering::SeqCst);
    let first = harness
        .handle
        .command(owner.clone(), Harness::command(1, 1))
        .unwrap();
    harness.log.entered.notified().await;
    let queued: Vec<_> = (0..4)
        .map(|_| {
            harness
                .host
                .input(ErasedInput::Timer {
                    timer: tabula_core::TimerId(0),
                })
                .unwrap()
        })
        .collect();
    assert!(matches!(
        harness
            .handle
            .command(owner.clone(), Harness::command(2, 1)),
        Err(AdmissionError::Busy)
    ));
    assert!(matches!(
        harness.host.input(ErasedInput::Timer {
            timer: tabula_core::TimerId(0)
        }),
        Err(AdmissionError::Busy)
    ));
    harness.log.release.notify_one();
    first.wait().await.unwrap();
    for ticket in queued {
        ticket.wait().await.unwrap();
    }
    assert_eq!(harness.send(&owner, 2, 1).await, Completion::Submitted);
    assert_eq!(harness.end().await.version, tabula_core::StateVersion(6));
}

#[tokio::test]
async fn cancellation_after_apply_cannot_undo_commit_and_drain_closes_following_queued_work() {
    let harness = Harness::new(1, Limits::default());
    let owner = harness.attach(1, ClientViewer::Seat(SeatId(0))).await;
    harness.log.pause.store(1, Ordering::SeqCst);
    let started = harness
        .handle
        .command(owner.clone(), Harness::command(1, 1))
        .unwrap();
    harness.log.entered.notified().await;
    let drain = harness.host.drain().unwrap();
    let following = harness
        .handle
        .command(owner.clone(), Harness::command(2, 1))
        .unwrap();
    drop(started);
    harness.log.release.notify_one();
    drain.wait().await.unwrap();
    assert_eq!(following.wait().await.unwrap_err(), AdmissionError::Closed);
    assert!(matches!(
        harness.handle.command(owner, Harness::command(2, 1)),
        Err(AdmissionError::Closed)
    ));
    let summary = harness.join.await.unwrap();
    assert_eq!(summary.exit, Exit::Drained);
    assert_eq!(summary.version.0, 1);
    assert_eq!(
        harness.log.records.lock().unwrap().len(),
        2,
        "started input persisted despite its dropped ticket"
    );
}

#[tokio::test]
async fn failed_fresh_authority_suppresses_initial_projection_and_allows_explicit_retry() {
    let h = Harness::new(1, Limits::default());
    let viewer = ClientViewer::Seat(SeatId(0));
    let binding = Binding::new(SessionId(1), UserId(1), 1, 1, 1);
    h.auth.grant(&binding, viewer);
    *h.auth.fail_prepare.lock().unwrap() = Some(Purpose::Observe(viewer));
    assert_eq!(
        h.handle
            .attach(binding.clone(), viewer)
            .unwrap()
            .wait()
            .await
            .unwrap(),
        Completion::Suppressed
    );
    assert!(h.frames_for(binding.session()).is_empty());
    *h.auth.fail_prepare.lock().unwrap() = None;
    assert_eq!(
        h.handle
            .attach(binding.clone(), viewer)
            .unwrap()
            .wait()
            .await
            .unwrap(),
        Completion::Submitted
    );
    assert_eq!(h.frames_for(binding.session()).len(), 1);
    assert_eq!(
        h.auth.preparations.lock().unwrap().as_slice(),
        &[Purpose::Observe(viewer); 2]
    );
    h.end().await;
}

#[tokio::test]
async fn failed_receipt_preparation_suppresses_receipt_without_losing_other_viewers_update() {
    for command in [1, 250] {
        let h = Harness::new(1, Limits::default());
        let owner = h.attach(1, ClientViewer::Seat(SeatId(0))).await;
        let other = h.attach(2, ClientViewer::Spectator).await;
        *h.auth.fail_prepare.lock().unwrap() =
            Some(Purpose::Receipt(ClientViewer::Seat(SeatId(0))));
        assert_eq!(h.send(&owner, 1, command).await, Completion::Suppressed);
        assert_eq!(h.frames_for(owner.session()).len(), 1);
        let expected = if command == 1 { 2 } else { 1 };
        assert_eq!(h.frames_for(other.session()).len(), expected);
        assert_eq!(h.log.records.lock().unwrap().len(), expected);
        h.end().await;
    }
}

#[tokio::test]
async fn failed_observer_preparation_withholds_post_commit_private_output() {
    let h = Harness::new(1, Limits::default());
    let owner = h.attach(1, ClientViewer::Seat(SeatId(0))).await;
    let other = h.attach(2, ClientViewer::Spectator).await;
    *h.auth.fail_prepare.lock().unwrap() = Some(Purpose::Observe(ClientViewer::Spectator));
    assert_eq!(h.send(&owner, 1, 1).await, Completion::Submitted);
    assert_eq!(h.frames_for(owner.session()).len(), 3);
    assert_eq!(h.frames_for(other.session()).len(), 1);
    assert_eq!(h.log.records.lock().unwrap().len(), 2);
    h.end().await;
}

#[tokio::test]
async fn authority_loss_after_apply_retires_uncommitted_candidate() {
    let h = Harness::new(1, Limits::default());
    let owner = h.attach(1, ClientViewer::Seat(SeatId(0))).await;
    assert!(!h.handle.is_closed());
    // The first Apply callback is an authorization probe; the second mutates.
    h.auth.lose_after_apply.store(2, Ordering::SeqCst);
    let ticket = h
        .handle
        .command(owner.clone(), Harness::command(1, 1))
        .unwrap();
    assert_eq!(ticket.wait().await.unwrap_err(), AdmissionError::Closed);
    assert_eq!(h.auth.apply_calls.load(Ordering::SeqCst), 2);
    assert_eq!(h.frames_for(owner.session()).len(), 1);
    assert_eq!(h.log.records.lock().unwrap().len(), 1);
    assert_eq!(h.fx.keys.lock().unwrap().as_slice(), &[InputIndex(0)]);
    let summary = h.join.await.unwrap();
    assert_eq!(summary.exit, Exit::AuthorityLost);
    assert_eq!(summary.index, InputIndex(0));
    assert!(h.handle.is_closed());
}
