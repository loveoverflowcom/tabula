//! Real PostgreSQL acceptance of the isolated actor boundary (ADR-0040).
//! These assertions concern durable ordering/recovery, not online auth, socket
//! delivery, game legality, or cross-target determinism. Game inputs stay opaque.
#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sqlx::PgPool;
use tabula_core::{
    canonical_encode, InputIndex, LogicalTime, MatchId, Occupant, SeatId, SessionId, StateVersion,
    UserId,
};
use tabula_game_api::Effect;
use tabula_match::{
    durable::{Journal, JournalRecord, LoadedMatch, RuntimePortError, ScopeState},
    ports::Clock,
    runtime::{
        recover, spawn, Binding, Completion, Exit, HostControl, Limits, MatchHandle, Ports,
        Summary, Ticket,
    },
    runtime_ports::{Authority, AuthorityLost, Effects, Output, Purpose},
};
use tabula_protocol::{
    decode_server, encode_server, ClientEnvelope, Codec, ErrorCode, GameCommandFrame,
    ServerEnvelope, ServerMessage,
};
use tabula_registry::runtime::{
    test_support::{
        approved_checkmate_fixture, approved_clocked_fixture, approved_long_fixture, RuntimeFixture,
    },
    ClientViewer,
};
use tabula_storage::match_postgres::{test_support::Corruption, PgMatchJournal, PgMatchStore};
use tokio::sync::Notify;

const CAPTURE_LIMIT: usize = 512;
const WALL_START: u64 = 1_700_000_000_000;
const WAIT: Duration = Duration::from_secs(10);
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1_000);

fn unique_match() -> MatchId {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    MatchId(now + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed)))
}

async fn database() -> (PgPool, PgMatchStore) {
    let url = std::env::var("TABULA_MATCH_DATABASE_URL")
        .expect("real PostgreSQL acceptance requires TABULA_MATCH_DATABASE_URL; setup never skips");
    let pool = if let Ok(schema) = std::env::var("TABULA_MATCH_TEST_SCHEMA") {
        PgMatchStore::test_pool_in_schema(&url, &schema)
            .await
            .expect("open independent process pool in verified disposable test schema")
    } else {
        PgMatchStore::isolated_test_pool(&url, "actor")
            .await
            .expect("create fresh disposable PostgreSQL acceptance schema")
    };
    let store = PgMatchStore::new(pool.clone());
    let major = store
        .server_version_num()
        .await
        .expect("read actual PostgreSQL version")
        / 10_000;
    assert_eq!(major, 16, "this acceptance is pinned to real PostgreSQL 16");
    PgMatchStore::migrate(&pool)
        .await
        .expect("migrate disposable isolated match schema");
    (pool, store)
}

async fn independent_store(store: &PgMatchStore) -> PgMatchStore {
    let schema = store
        .test_schema()
        .await
        .expect("selected disposable schema");
    let url = std::env::var("TABULA_MATCH_DATABASE_URL").expect("required acceptance URL");
    let pool = PgMatchStore::test_pool_in_schema(&url, &schema)
        .await
        .expect("independent PostgreSQL observer/recovery pool");
    PgMatchStore::new(pool)
}

#[derive(Clone)]
struct Grant {
    binding: Binding,
    viewer: ClientViewer,
}
#[derive(Default)]
struct LocalAuthority(Mutex<BTreeMap<SessionId, Grant>>);
impl Authority for LocalAuthority {
    fn with_current<T>(
        &self,
        binding: &Binding,
        purpose: Purpose,
        action: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost> {
        let grants = self.0.lock().map_err(|_| AuthorityLost)?;
        let grant = grants.get(&binding.session()).ok_or(AuthorityLost)?;
        if grant.binding != *binding {
            return Err(AuthorityLost);
        }
        let allowed = match purpose {
            Purpose::Apply(seat) => grant.viewer == ClientViewer::Seat(seat),
            Purpose::Observe(viewer) | Purpose::Receipt(viewer) => grant.viewer == viewer,
        };
        if !allowed {
            return Err(AuthorityLost);
        }
        Ok(action())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Boundary {
    Commit(InputIndex),
    Effect(InputIndex),
    Output(Option<u64>),
}
type Trace = Arc<Mutex<Vec<Boundary>>>;

struct CommitGate {
    entered: Notify,
    release: Notify,
}
struct ObservedJournal {
    inner: Arc<PgMatchJournal>,
    trace: Trace,
    after_commit: Mutex<Option<Arc<CommitGate>>>,
}
impl ObservedJournal {
    fn pause_after_commit(&self) -> Arc<CommitGate> {
        let gate = Arc::new(CommitGate {
            entered: Notify::new(),
            release: Notify::new(),
        });
        *self.after_commit.lock().unwrap() = Some(Arc::clone(&gate));
        gate
    }
}
impl Journal for ObservedJournal {
    async fn append(&self, record: JournalRecord) -> Result<(), RuntimePortError> {
        let index = record.index;
        self.inner.append(record).await?;
        self.trace.lock().unwrap().push(Boundary::Commit(index));
        let gate = self.after_commit.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        Ok(())
    }
    async fn update_ledger(
        &self,
        id: MatchId,
        version: StateVersion,
        observed_ms: u64,
        ledger: Vec<ScopeState>,
    ) -> Result<(), RuntimePortError> {
        self.inner
            .update_ledger(id, version, observed_ms, ledger)
            .await
    }
    async fn load(&self, id: MatchId) -> Result<LoadedMatch, RuntimePortError> {
        self.inner.load(id).await
    }
}

struct CaptureOutput {
    frames: Mutex<Vec<(SessionId, ServerEnvelope)>>,
    trace: Trace,
}
impl Output for CaptureOutput {
    fn submit(&self, binding: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError> {
        let mut frames = self.frames.lock().unwrap();
        if frames.len() >= CAPTURE_LIMIT {
            return Err(RuntimePortError::Busy);
        }
        let encoded = encode_server(Codec::Postcard, &frame).expect("projected frame encodes");
        let decoded = decode_server(Codec::Postcard, &encoded).expect("projected frame decodes");
        assert_eq!(decoded, frame);
        self.trace
            .lock()
            .unwrap()
            .push(Boundary::Output(frame.corr()));
        frames.push((binding.session(), frame));
        Ok(())
    }
}
#[derive(Default)]
struct CaptureEffects {
    requests: Mutex<BTreeMap<(MatchId, InputIndex), Vec<Effect>>>,
    trace: Trace,
}
impl Effects for CaptureEffects {
    async fn execute(
        &self,
        id: MatchId,
        index: InputIndex,
        effects: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        let mut requests = self.requests.lock().unwrap();
        assert!(
            requests.len() < CAPTURE_LIMIT,
            "bounded keyed effect adapter"
        );
        if let Some(previous) = requests.get(&(id, index)) {
            assert!(
                canonical_encode(previous).unwrap() == canonical_encode(&effects).unwrap(),
                "stable effect key must retain exact request"
            );
        } else {
            requests.insert((id, index), effects);
        }
        self.trace.lock().unwrap().push(Boundary::Effect(index));
        Ok(())
    }
}
struct ManualClock {
    unix: AtomicU64,
    monotonic: AtomicU64,
    tick: AtomicU64,
}
impl Clock for ManualClock {
    fn now_unix_ms(&self) -> u64 {
        self.unix.load(Ordering::SeqCst)
    }
    fn monotonic_ms(&self) -> u64 {
        self.monotonic
            .fetch_add(self.tick.load(Ordering::SeqCst), Ordering::SeqCst)
    }
}
impl ManualClock {
    fn new(unix: u64, monotonic: u64) -> Self {
        Self {
            unix: AtomicU64::new(unix),
            monotonic: AtomicU64::new(monotonic),
            tick: AtomicU64::new(0),
        }
    }
    fn set(&self, unix: u64, monotonic: u64) {
        self.unix.store(unix, Ordering::SeqCst);
        self.monotonic.store(monotonic, Ordering::SeqCst);
    }
}

type ActorPorts =
    Ports<LocalAuthority, ObservedJournal, CaptureOutput, CaptureEffects, ManualClock>;
struct Harness {
    id: MatchId,
    fixture: RuntimeFixture,
    players: [Binding; 2],
    spectator: Binding,
    authority: Arc<LocalAuthority>,
    journal: Arc<ObservedJournal>,
    output: Arc<CaptureOutput>,
    effects: Arc<CaptureEffects>,
    clock: Arc<ManualClock>,
    trace: Trace,
    handle: MatchHandle,
    host: HostControl,
    owner: Option<tokio::task::JoinHandle<Summary>>,
    limits: Limits,
}
impl Harness {
    fn adapters(
        id: MatchId,
        fixture: RuntimeFixture,
        journal: PgMatchJournal,
        limits: Limits,
        unix: u64,
        monotonic: u64,
    ) -> (SelfParts, ActorPorts) {
        let players = std::array::from_fn(|at| {
            let seat = SeatId(u8::try_from(at).unwrap());
            let Occupant::Human(subject) = fixture.roster.get(seat).expect("fixture seat").occupant
            else {
                panic!("approved fixture humans");
            };
            Binding::new(
                SessionId(u64::from(seat.0) + 10),
                subject,
                100 + u128::from(seat.0),
                7,
                3,
            )
        });
        let spectator = Binding::new(SessionId(30), UserId(99), 199, 7, 0);
        let authority = Arc::new(LocalAuthority::default());
        for (at, binding) in players.iter().enumerate() {
            authority.0.lock().unwrap().insert(
                binding.session(),
                Grant {
                    binding: binding.clone(),
                    viewer: ClientViewer::Seat(SeatId(u8::try_from(at).unwrap())),
                },
            );
        }
        authority.0.lock().unwrap().insert(
            spectator.session(),
            Grant {
                binding: spectator.clone(),
                viewer: ClientViewer::Spectator,
            },
        );
        let trace = Arc::new(Mutex::new(Vec::new()));
        let journal = Arc::new(ObservedJournal {
            inner: Arc::new(journal),
            trace: Arc::clone(&trace),
            after_commit: Mutex::new(None),
        });
        let output = Arc::new(CaptureOutput {
            frames: Mutex::new(Vec::new()),
            trace: Arc::clone(&trace),
        });
        let effects = Arc::new(CaptureEffects {
            requests: Mutex::new(BTreeMap::new()),
            trace: Arc::clone(&trace),
        });
        let clock = Arc::new(ManualClock::new(unix, monotonic));
        let ports = Ports {
            authority: Arc::clone(&authority),
            journal: Arc::clone(&journal),
            output: Arc::clone(&output),
            effects: Arc::clone(&effects),
            clock: Arc::clone(&clock),
        };
        (
            SelfParts {
                id,
                fixture,
                players,
                spectator,
                authority,
                journal,
                output,
                effects,
                clock,
                trace,
                limits,
            },
            ports,
        )
    }
    async fn start(
        store: &PgMatchStore,
        fixture: RuntimeFixture,
        limits: Limits,
        anchor: u64,
    ) -> Self {
        Self::start_id(store, fixture, limits, anchor, unique_match()).await
    }
    async fn start_id(
        store: &PgMatchStore,
        fixture: RuntimeFixture,
        limits: Limits,
        anchor: u64,
        id: MatchId,
    ) -> Self {
        let journal = store.claim(id).await.expect("claim actual durable owner");
        let (parts, ports) = Self::adapters(id, fixture, journal, limits, WALL_START, anchor);
        let created = parts
            .fixture
            .game
            .create_match(
                &parts.fixture.config,
                &parts.fixture.roster,
                parts.fixture.seed.clone(),
            )
            .expect("approved registry creates canonical match");
        let owner = spawn(id, created, ports, limits).expect("valid bounded owner");
        parts.with_owner(owner)
    }
    async fn reopen(
        store: &PgMatchStore,
        id: MatchId,
        fixture: RuntimeFixture,
        limits: Limits,
        unix: u64,
        monotonic: u64,
    ) -> Self {
        let recovered_store = independent_store(store).await;
        let journal = recovered_store
            .claim(id)
            .await
            .expect("claim replacement actual durable owner");
        let (mut parts, ports) = Self::adapters(id, fixture, journal, limits, unix, monotonic);
        parts.fresh_connections();
        let owner = recover(id, Arc::clone(&parts.fixture.game), ports, limits)
            .await
            .expect("validate complete committed prefix before opening mailbox");
        parts.with_owner(owner)
    }
    async fn attach_all(&self) {
        for (at, binding) in self.players.iter().enumerate() {
            assert_eq!(
                completed(
                    self.handle
                        .attach(
                            binding.clone(),
                            ClientViewer::Seat(SeatId(u8::try_from(at).unwrap()))
                        )
                        .unwrap()
                )
                .await,
                Completion::Submitted
            );
        }
        assert_eq!(
            completed(
                self.handle
                    .attach(self.spectator.clone(), ClientViewer::Spectator)
                    .unwrap()
            )
            .await,
            Completion::Submitted
        );
    }
    fn envelope(&self, seq: u64, corr: u64, payload: Vec<u8>) -> ClientEnvelope {
        ClientEnvelope::new(
            seq,
            corr,
            GameCommandFrame::new(
                self.id,
                self.fixture.game.metadata().id().clone(),
                self.fixture.game.metadata().version().clone(),
                payload,
            )
            .unwrap(),
        )
        .unwrap()
    }
    async fn send(
        &self,
        seat: SeatId,
        seq: u64,
        corr: u64,
        payload: Vec<u8>,
        expected: Result<(), ErrorCode>,
    ) {
        self.send_envelope(seat, self.envelope(seq, corr, payload), expected)
            .await;
    }
    async fn send_envelope(
        &self,
        seat: SeatId,
        envelope: ClientEnvelope,
        expected: Result<(), ErrorCode>,
    ) {
        let corr = envelope.corr();
        let seq = envelope.seq();
        let ticket = self
            .handle
            .command(self.players[usize::from(seat.0)].clone(), envelope)
            .unwrap();
        assert_eq!(completed(ticket).await, Completion::Submitted);
        let frames = self.output.frames.lock().unwrap();
        let caused: Vec<_> = frames
            .iter()
            .filter(|(_, frame)| frame.corr() == Some(corr))
            .collect();
        assert_eq!(caused.len(), 1, "exactly one actual submitted receipt");
        let expected = match expected {
            Ok(()) => ServerMessage::Ack { seq },
            Err(error) => ServerMessage::Reject { seq, error },
        };
        assert_eq!(caused[0].1.body(), &expected);
    }
    async fn command_at(&self, at: usize, seq: u64, corr: u64) {
        let (seat, payload) = &self.fixture.commands[at];
        self.send(*seat, seq, corr, payload.clone(), Ok(())).await;
    }
    async fn load(&self) -> LoadedMatch {
        self.journal
            .load(self.id)
            .await
            .expect("read actual durable committed prefix")
    }
    async fn close(&mut self, expected: u64) {
        assert_eq!(
            completed(self.host.drain().unwrap()).await,
            Completion::Submitted
        );
        let summary = tokio::time::timeout(WAIT, self.owner.take().unwrap())
            .await
            .expect("drained owner terminates")
            .expect("owner survives");
        assert_eq!(summary.exit, Exit::Drained);
        assert_eq!(summary.version, StateVersion(expected));
        assert_eq!(summary.index, InputIndex(expected));
    }
    fn view(&self) -> Vec<u8> {
        self.output
            .frames
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(session, frame)| {
                if *session != self.spectator.session() {
                    return None;
                }
                match frame.body() {
                    ServerMessage::MatchUpdate { view, .. } => Some(view.clone()),
                    _ => None,
                }
            })
            .expect("actual spectator projection")
    }
    fn output_count(&self) -> usize {
        self.output.frames.lock().unwrap().len()
    }
}
struct SelfParts {
    id: MatchId,
    fixture: RuntimeFixture,
    players: [Binding; 2],
    spectator: Binding,
    authority: Arc<LocalAuthority>,
    journal: Arc<ObservedJournal>,
    output: Arc<CaptureOutput>,
    effects: Arc<CaptureEffects>,
    clock: Arc<ManualClock>,
    trace: Trace,
    limits: Limits,
}
impl SelfParts {
    fn fresh_connections(&mut self) {
        let first = NEXT_CONNECTION.fetch_add(3, Ordering::SeqCst);
        let mut grants = self.authority.0.lock().unwrap();
        grants.clear();
        for (at, binding) in self.players.iter_mut().enumerate() {
            *binding = Binding::new(
                SessionId(first + u64::try_from(at).unwrap()),
                binding.subject(),
                binding.record(),
                binding.epoch(),
                binding.generation(),
            );
            grants.insert(
                binding.session(),
                Grant {
                    binding: binding.clone(),
                    viewer: ClientViewer::Seat(SeatId(u8::try_from(at).unwrap())),
                },
            );
        }
        let binding = &self.spectator;
        self.spectator = Binding::new(
            SessionId(first + 2),
            binding.subject(),
            binding.record(),
            binding.epoch(),
            binding.generation(),
        );
        grants.insert(
            self.spectator.session(),
            Grant {
                binding: self.spectator.clone(),
                viewer: ClientViewer::Spectator,
            },
        );
    }
    fn with_owner(
        self,
        (handle, host, owner): (MatchHandle, HostControl, tokio::task::JoinHandle<Summary>),
    ) -> Harness {
        Harness {
            id: self.id,
            fixture: self.fixture,
            players: self.players,
            spectator: self.spectator,
            authority: self.authority,
            journal: self.journal,
            output: self.output,
            effects: self.effects,
            clock: self.clock,
            trace: self.trace,
            handle,
            host,
            owner: Some(owner),
            limits: self.limits,
        }
    }
}
async fn completed(ticket: Ticket) -> Completion {
    tokio::time::timeout(WAIT, ticket.wait())
        .await
        .expect("bounded mailbox operation finishes")
        .expect("receipt submitted or suppressed")
}
fn assert_records_equal(expected: &LoadedMatch, actual: &LoadedMatch) {
    assert_eq!(expected.records.len(), actual.records.len());
    for (left, right) in expected.records.iter().zip(&actual.records) {
        assert!(
            canonical_encode(left).unwrap() == canonical_encode(right).unwrap(),
            "exact canonical record preserved without printing private bytes"
        );
    }
    assert_eq!(expected.version, actual.version);
    assert_eq!(expected.index, actual.index);
}

#[tokio::test]
async fn real_postgres_full_actor_transcript_reopens_terminal_snapshot_and_receipts() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    let mut sequences = [0u64; 2];
    for at in 0..live.fixture.commands.len() {
        let seat = live.fixture.commands[at].0;
        sequences[usize::from(seat.0)] += 1;
        let now = u64::try_from(at + 1).unwrap() * 10;
        live.clock.set(WALL_START + now, 600_000 + now);
        live.command_at(at, sequences[usize::from(seat.0)], 100 + now)
            .await;
        let committed = live.load().await;
        assert_eq!(committed.index, InputIndex(u64::try_from(at + 1).unwrap()));
        assert_eq!(committed.records.last().unwrap().now, LogicalTime(now));
        let trace = live.trace.lock().unwrap();
        let commit = trace
            .iter()
            .position(|boundary| *boundary == Boundary::Commit(committed.index))
            .unwrap();
        let effect = trace
            .iter()
            .position(|boundary| *boundary == Boundary::Effect(committed.index))
            .unwrap();
        let receipt = trace
            .iter()
            .position(|boundary| *boundary == Boundary::Output(Some(100 + now)))
            .unwrap();
        assert!(
            commit < effect && effect < receipt,
            "known DB commit precedes effects and Ack"
        );
    }
    let before = live.load().await;
    assert_eq!(before.records.len(), 5);
    assert!(before.records[0].snapshot.is_some());
    assert!(before.records[4].snapshot.is_some());
    assert!(before.records[4].terminal);
    let final_view = live.view();
    let id = live.id;
    let limits = live.limits;
    live.close(4).await;
    let mut restored = Harness::reopen(
        &store,
        id,
        approved_checkmate_fixture(),
        limits,
        WALL_START + 40,
        9_000_000,
    )
    .await;
    restored.attach_all().await;
    assert_eq!(
        restored.view(),
        final_view,
        "replayed final projection agrees with live projection"
    );
    assert_records_equal(&before, &restored.load().await);
    restored.command_at(3, 2, 201).await;
    assert_records_equal(&before, &restored.load().await);
    restored
        .send(
            SeatId(0),
            3,
            202,
            restored.fixture.commands[0].1.clone(),
            Err(ErrorCode::Terminal),
        )
        .await;
    assert_records_equal(&before, &restored.load().await);
    restored.close(4).await;
}

#[tokio::test]
async fn real_postgres_snapshot_cadence_twenty_retains_exact_tail_and_clock_order() {
    let (_pool, store) = database().await;
    let mut live =
        Harness::start(&store, approved_long_fixture(), Limits::default(), 300_000).await;
    live.attach_all().await;
    let mut seq = [0u64; 2];
    for at in 0..21 {
        let seat = live.fixture.commands[at].0;
        seq[usize::from(seat.0)] += 1;
        let now = u64::try_from(at + 1).unwrap();
        live.clock.set(WALL_START + now, 300_000 + now);
        live.command_at(at, seq[usize::from(seat.0)], 300 + now)
            .await;
    }
    let before = live.load().await;
    assert_eq!(before.records.len(), 22);
    assert_eq!(
        before
            .records
            .iter()
            .filter(|record| record.snapshot.is_some())
            .map(|record| record.index.0)
            .collect::<Vec<_>>(),
        [0, 20]
    );
    assert!(!before.records[21].terminal);
    let view = live.view();
    live.close(21).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_long_fixture(),
        live.limits,
        WALL_START + 21,
        900_000,
    )
    .await;
    restored.attach_all().await;
    assert_eq!(restored.view(), view);
    assert_records_equal(&before, &restored.load().await);
    restored.command_at(21, 11, 400).await;
    let after = restored.load().await;
    assert_eq!(after.index, InputIndex(22));
    assert_eq!(after.records[22].now, LogicalTime(21));
    assert!(after.records[22].snapshot.is_none());
    restored.close(22).await;
}

#[tokio::test]
async fn real_postgres_response_loss_after_commit_stops_then_retry_never_reapplies() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    let output = live.output_count();
    let effects = live.effects.requests.lock().unwrap().len();
    live.journal.inner.lose_next_commit_response();
    let ticket = live
        .handle
        .command(
            live.players[0].clone(),
            live.envelope(1, 501, live.fixture.commands[0].1.clone()),
        )
        .unwrap();
    assert!(
        tokio::time::timeout(WAIT, ticket.wait())
            .await
            .unwrap()
            .is_err(),
        "failed unknown commit produces no successful completion"
    );
    let summary = tokio::time::timeout(WAIT, live.owner.take().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.exit, Exit::JournalFailed);
    assert_eq!(
        live.output_count(),
        output,
        "no Ack or projection escapes an indeterminate response"
    );
    assert_eq!(
        live.effects.requests.lock().unwrap().len(),
        effects,
        "no effect escapes unknown commit receipt"
    );
    let before = live.load().await;
    assert_eq!(
        before.index,
        InputIndex(1),
        "real transaction committed despite response loss"
    );
    assert_eq!(before.records.len(), 2);
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        live.limits,
        WALL_START,
        10_000_000,
    )
    .await;
    restored.attach_all().await;
    restored.command_at(0, 1, 502).await;
    assert_records_equal(&before, &restored.load().await);
    restored
        .send(
            SeatId(0),
            1,
            503,
            restored.fixture.illegal_command.clone(),
            Err(ErrorCode::OperationConflict),
        )
        .await;
    assert_records_equal(&before, &restored.load().await);
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_failed_staged_transaction_rolls_back_and_suppresses_all_output() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        500_000,
    )
    .await;
    live.attach_all().await;
    let before = live.load().await;
    let output = live.output_count();
    let effects = live.effects.requests.lock().unwrap().len();
    live.journal.inner.fail_before_commit();
    let ticket = live
        .handle
        .command(
            live.players[0].clone(),
            live.envelope(1, 601, live.fixture.commands[0].1.clone()),
        )
        .unwrap();
    assert!(tokio::time::timeout(WAIT, ticket.wait())
        .await
        .unwrap()
        .is_err());
    let summary = tokio::time::timeout(WAIT, live.owner.take().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.exit, Exit::JournalFailed);
    assert_eq!(live.output_count(), output);
    assert_eq!(live.effects.requests.lock().unwrap().len(), effects);
    assert_records_equal(&before, &live.load().await);
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        live.limits,
        WALL_START,
        9_000_000,
    )
    .await;
    restored.attach_all().await;
    restored.command_at(0, 1, 602).await;
    assert_eq!(
        restored.load().await.records.len(),
        2,
        "uncommitted command remains admissible exactly once"
    );
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_pending_commit_has_no_ack_projection_or_effect_before_receipt() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_clocked_fixture(),
        Limits::default(),
        800_000,
    )
    .await;
    live.attach_all().await;
    let before = live.load().await;
    let output = live.output_count();
    let effects = live.effects.requests.lock().unwrap().len();
    let observer = independent_store(&store).await;
    let mut pause = live
        .journal
        .inner
        .pause_before_commit()
        .expect("arm staged transaction barrier");
    let ticket = live
        .handle
        .command(
            live.players[0].clone(),
            live.envelope(1, 701, live.fixture.commands[0].1.clone()),
        )
        .unwrap();
    tokio::time::timeout(WAIT, pause.wait_until_entered())
        .await
        .expect("reached actual staged PostgreSQL transaction")
        .expect("staged barrier entered");
    assert_eq!(live.output_count(), output);
    assert_eq!(live.effects.requests.lock().unwrap().len(), effects);
    assert_eq!(
        observer.committed_prefix_for_test(live.id).await.unwrap(),
        (
            before.version,
            before.index,
            u64::try_from(before.records.len()).unwrap()
        ),
        "MVCC reader sees only committed prefix while a transaction is staged"
    );
    pause.release().expect("release staged commit");
    assert_eq!(completed(ticket).await, Completion::Submitted);
    assert_eq!(live.load().await.index, InputIndex(1));
    live.close(1).await;
}

#[tokio::test]
async fn real_postgres_retained_rejected_evicted_and_expired_receipts_survive_restart() {
    let (_pool, store) = database().await;
    let limits = Limits {
        receipts_per_scope: 2,
        receipt_ttl_ms: 50,
        ..Limits::default()
    };
    let mut live = Harness::start(&store, approved_checkmate_fixture(), limits, 600_000).await;
    live.attach_all().await;
    live.command_at(0, 1, 801).await;
    live.send(SeatId(0), 2, 802, vec![1], Err(ErrorCode::Malformed))
        .await;
    live.send(
        SeatId(0),
        3,
        803,
        live.fixture.illegal_command.clone(),
        Err(ErrorCode::RuleRejected),
    )
    .await;
    let before = live.load().await;
    assert_eq!(before.index, InputIndex(1));
    live.close(1).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        limits,
        WALL_START,
        9_000_000,
    )
    .await;
    restored.attach_all().await;
    restored
        .send(
            SeatId(0),
            1,
            804,
            restored.fixture.commands[0].1.clone(),
            Err(ErrorCode::StaleSeq),
        )
        .await;
    restored
        .send(SeatId(0), 2, 805, vec![1], Err(ErrorCode::Malformed))
        .await;
    restored
        .send(
            SeatId(0),
            3,
            806,
            restored.fixture.illegal_command.clone(),
            Err(ErrorCode::RuleRejected),
        )
        .await;
    restored
        .send(
            SeatId(0),
            3,
            807,
            restored.fixture.commands[0].1.clone(),
            Err(ErrorCode::OperationConflict),
        )
        .await;
    assert_records_equal(&before, &restored.load().await);
    restored.close(1).await;
    let mut expired = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        limits,
        WALL_START + 50,
        100_000_000,
    )
    .await;
    expired.attach_all().await;
    expired
        .send(SeatId(0), 2, 808, vec![1], Err(ErrorCode::StaleSeq))
        .await;
    expired
        .send(
            SeatId(0),
            3,
            809,
            expired.fixture.illegal_command.clone(),
            Err(ErrorCode::StaleSeq),
        )
        .await;
    assert_records_equal(&before, &expired.load().await);
    assert_eq!(
        expired
            .load()
            .await
            .ledger
            .iter()
            .find(|scope| scope.scope.seat == SeatId(0))
            .unwrap()
            .highest,
        3
    );
    expired.close(1).await;
}

#[tokio::test]
async fn real_postgres_nonzero_process_clock_restarts_from_durable_logical_anchor() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_clocked_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    live.clock.set(WALL_START + 10, 600_010);
    live.command_at(0, 1, 901).await;
    let before = live.load().await;
    assert_eq!(before.records[0].now, LogicalTime(0));
    assert_eq!(before.records[1].now, LogicalTime(10));
    live.close(1).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_clocked_fixture(),
        live.limits,
        WALL_START + 30,
        90_000_000,
    )
    .await;
    restored.attach_all().await;
    restored.command_at(1, 1, 902).await;
    let after = restored.load().await;
    assert_eq!(
        after.records[2].now,
        LogicalTime(30),
        "restart downtime advances match elapsed time, not process uptime"
    );
    restored.clock.set(WALL_START + 35, 90_000_005);
    restored.command_at(2, 2, 903).await;
    assert_eq!(restored.load().await.records[3].now, LogicalTime(35));
    restored.close(3).await;
}

#[tokio::test]
async fn real_postgres_new_owner_fences_old_actor_and_exact_version_writers() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    let output = live.output_count();
    let replacement = store
        .claim(live.id)
        .await
        .expect("new claim installs durable fence");
    let before = replacement.load(live.id).await.unwrap();
    let ticket = live
        .handle
        .command(
            live.players[0].clone(),
            live.envelope(1, 1001, live.fixture.commands[0].1.clone()),
        )
        .unwrap();
    assert!(tokio::time::timeout(WAIT, ticket.wait())
        .await
        .unwrap()
        .is_err());
    assert_eq!(
        live.owner.take().unwrap().await.unwrap().exit,
        Exit::JournalFailed
    );
    assert_eq!(live.output_count(), output);
    assert!(
        live.journal.load(live.id).await.is_err(),
        "old owner cannot even recover under stale fence"
    );
    assert_records_equal(&before, &replacement.load(live.id).await.unwrap());
    let (parts, ports) = Harness::adapters(
        live.id,
        approved_checkmate_fixture(),
        replacement,
        live.limits,
        WALL_START,
        8_000_000,
    );
    let owner = recover(live.id, Arc::clone(&parts.fixture.game), ports, live.limits)
        .await
        .unwrap();
    let mut fresh = parts.with_owner(owner);
    fresh.attach_all().await;
    let payload = fresh.fixture.commands[0].1.clone();
    let first = fresh
        .handle
        .command(
            fresh.players[0].clone(),
            fresh.envelope(1, 1002, payload.clone()),
        )
        .unwrap();
    let second = fresh
        .handle
        .command(fresh.players[0].clone(), fresh.envelope(1, 1003, payload))
        .unwrap();
    assert_eq!(completed(first).await, Completion::Submitted);
    assert_eq!(completed(second).await, Completion::Submitted);
    assert_eq!(fresh.load().await.index, InputIndex(1));
    assert_eq!(fresh.load().await.records.len(), 2);
    fresh.close(1).await;
}

#[tokio::test]
async fn real_postgres_recovery_fails_closed_for_persisted_corruption_and_partial_heads() {
    let (_pool, store) = database().await;
    // Every mutation is made in the real database through storage-owned test
    // helpers, including rechecksummed semantic corruptions that reach replay.
    for case in 0..20 {
        let mut live = Harness::start(
            &store,
            approved_checkmate_fixture(),
            Limits::default(),
            600_000,
        )
        .await;
        live.attach_all().await;
        live.command_at(0, 1, 1100 + case).await;
        live.close(1).await;
        match case {
            0 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.snapshot.as_mut().unwrap().push(0);
                })
                .await
                .unwrap(),
            1 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.hash.0[0] ^= 1;
                })
                .await
                .unwrap(),
            2 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.events.push(vec![255]);
                })
                .await
                .unwrap(),
            3 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.creation.as_mut().unwrap().identity.rules_hash[0] ^= 1;
                })
                .await
                .unwrap(),
            4 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.creation.as_mut().unwrap().identity.game_version =
                        tabula_core::GameVersion::new("999.0.0").unwrap();
                })
                .await
                .unwrap(),
            5 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.input.push(0);
                })
                .await
                .unwrap(),
            6 => store
                .corrupt_for_test(live.id, Corruption::InputGap)
                .await
                .unwrap(),
            7 => store
                .corrupt_for_test(live.id, Corruption::HeadVersion)
                .await
                .unwrap(),
            8 => store
                .corrupt_for_test(live.id, Corruption::RecordChecksum)
                .await
                .unwrap(),
            9 => store
                .mutate_ledger_for_test(live.id, |ledger| {
                    ledger[0].recent[0].committed_index = Some(InputIndex(99));
                })
                .await
                .unwrap(),
            10 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.creation.as_mut().unwrap().identity.rules_version =
                        tabula_core::RulesVersion(999);
                })
                .await
                .unwrap(),
            11 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.creation.as_mut().unwrap().config.push(0);
                })
                .await
                .unwrap(),
            12 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.events.push(vec![255]);
                })
                .await
                .unwrap(),
            13 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.terminal = true;
                })
                .await
                .unwrap(),
            14 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.effects.push(Effect::SetTimer {
                        id: tabula_core::TimerId(99),
                        delay: tabula_core::Millis(1),
                    });
                })
                .await
                .unwrap(),
            15 => store
                .corrupt_for_test(live.id, Corruption::LedgerChecksum)
                .await
                .unwrap(),
            16 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.creation.as_mut().unwrap().format = 2;
                })
                .await
                .unwrap(),
            17 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.operation.as_mut().unwrap().scope.seat = SeatId(250);
                })
                .await
                .unwrap(),
            18 => store
                .mutate_record_for_test(live.id, InputIndex(1), |record| {
                    record.expected_version = Some(StateVersion(99));
                })
                .await
                .unwrap(),
            19 => store
                .mutate_record_for_test(live.id, InputIndex(0), |record| {
                    record.snapshot = None;
                })
                .await
                .unwrap(),
            _ => unreachable!(),
        }
        let journal = store
            .claim(live.id)
            .await
            .expect("corrupt durable match can be claimed but never admitted");
        let (parts, ports) = Harness::adapters(
            live.id,
            approved_checkmate_fixture(),
            journal,
            live.limits,
            WALL_START,
            8_000_000,
        );
        assert!(
            recover(live.id, Arc::clone(&parts.fixture.game), ports, live.limits)
                .await
                .is_err(),
            "persisted corruption class {case} must fail before exposing a handle"
        );
        assert!(
            parts.output.frames.lock().unwrap().is_empty(),
            "recovery never submits a partial projection"
        );
        assert!(
            parts.effects.requests.lock().unwrap().is_empty(),
            "recovery never executes effects for invalid prefix"
        );
    }
}

struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The subprocess actually holds the actor and PostgreSQL connection. This
/// entrypoint is ignored in normal selection and runs only with explicit parent
/// fixture parameters; missing parameters fail instead of returning success.
#[tokio::test]
#[ignore = "spawned with --exact and explicit parameters by the real process-kill acceptance test"]
async fn recovery_process_child() {
    let mode = std::env::var("TABULA_MATCH_CHILD_MODE").expect("child mode required");
    let id = MatchId(
        std::env::var("TABULA_MATCH_CHILD_ID")
            .expect("child match required")
            .parse()
            .expect("child match ID"),
    );
    let signal =
        PathBuf::from(std::env::var("TABULA_MATCH_CHILD_SIGNAL").expect("child signal required"));
    assert!(
        std::env::var("TABULA_MATCH_TEST_SCHEMA").is_ok(),
        "parent-selected disposable schema required"
    );
    let (_pool, store) = database().await;
    let live = Harness::start_id(
        &store,
        approved_clocked_fixture(),
        Limits::default(),
        600_000,
        id,
    )
    .await;
    live.attach_all().await;
    live.clock.set(WALL_START + 10, 600_010);
    match mode.as_str() {
        "before" => {
            let observer = independent_store(&store).await;
            let mut pause = live
                .journal
                .inner
                .pause_before_commit()
                .expect("real pre-COMMIT gate");
            let _ticket = live
                .handle
                .command(
                    live.players[0].clone(),
                    live.envelope(1, 1201, live.fixture.commands[0].1.clone()),
                )
                .unwrap();
            tokio::time::timeout(WAIT, pause.wait_until_entered())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                observer.committed_prefix_for_test(live.id).await.unwrap(),
                (StateVersion(0), InputIndex(0), 1),
                "ordinary MVCC read sees only genesis while staged transaction holds head lock"
            );
            assert_eq!(
                live.output_count(),
                3,
                "only authorized attach projections exist"
            );
            assert_eq!(
                live.effects.requests.lock().unwrap().len(),
                1,
                "only creation effects exist"
            );
            std::fs::write(&signal, "before-commit\n")
                .expect("signal reached real staged DB boundary");
            // The live pause is retained until SIGKILL; release never runs.
            std::future::pending::<()>().await;
            drop(pause);
        }
        "after" => {
            let pause = live.journal.pause_after_commit();
            let _ticket = live
                .handle
                .command(
                    live.players[0].clone(),
                    live.envelope(1, 1201, live.fixture.commands[0].1.clone()),
                )
                .unwrap();
            tokio::time::timeout(WAIT, pause.entered.notified())
                .await
                .expect("real append committed, actor has not received return");
            assert_eq!(
                live.load().await.records.len(),
                2,
                "actual committed row exists before process death"
            );
            assert_eq!(live.output_count(), 3);
            assert_eq!(live.effects.requests.lock().unwrap().len(), 1);
            std::fs::write(&signal, "after-commit\n")
                .expect("signal reached committed DB boundary");
            std::future::pending::<()>().await;
        }
        _ => panic!("unknown parent child mode"),
    }
}

#[tokio::test]
async fn real_postgres_sigkill_before_and_after_commit_reopens_actual_durable_prefix() {
    let (_pool, store) = database().await;
    let schema = store.test_schema().await.unwrap();
    for mode in ["before", "after"] {
        let id = unique_match();
        let signal =
            std::env::temp_dir().join(format!("tabula-match-kill-{}-{}", std::process::id(), id.0));
        assert!(!signal.exists(), "fresh subprocess boundary signal");
        let child = Command::new(std::env::current_exe().expect("integration test executable"))
            .args([
                "--exact",
                "recovery_process_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TABULA_MATCH_CHILD_MODE", mode)
            .env("TABULA_MATCH_CHILD_ID", id.0.to_string())
            .env("TABULA_MATCH_CHILD_SIGNAL", &signal)
            .env("TABULA_MATCH_TEST_SCHEMA", &schema)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn actual actor process");
        let mut child = KillOnDrop(child);
        tokio::time::timeout(WAIT, async {
            loop {
                if signal.exists() {
                    break;
                }
                assert!(
                    child.0.try_wait().expect("child status").is_none(),
                    "actor child failed before reaching actual DB boundary"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("child reaches acknowledged real database boundary");
        assert_eq!(
            std::fs::read_to_string(&signal).unwrap(),
            format!("{mode}-commit\n")
        );
        child
            .0
            .kill()
            .expect("kill process without drain or Drop rollback");
        let status = child.0.wait().expect("killed process terminates");
        assert!(
            !status.success(),
            "actual process kill, not graceful actor drain"
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                status.signal(),
                Some(9),
                "POSIX SIGKILL crossed process boundary"
            );
        }
        std::fs::remove_file(&signal).unwrap();
        let mut restored = Harness::reopen(
            &store,
            id,
            approved_clocked_fixture(),
            Limits::default(),
            WALL_START + 10,
            90_000_000,
        )
        .await;
        restored.attach_all().await;
        let prefix = restored.load().await;
        let committed = u64::from(mode == "after");
        assert_eq!(prefix.index, InputIndex(committed));
        assert_eq!(
            prefix.records.len(),
            usize::try_from(committed + 1).unwrap()
        );
        restored.command_at(0, 1, 1301).await;
        let after = restored.load().await;
        assert_eq!(
            after.index,
            InputIndex(1),
            "retry applies precisely when original never committed"
        );
        assert_eq!(
            after.records.len(),
            2,
            "one canonical input after crash/retry"
        );
        if mode == "after" {
            assert_records_equal(&prefix, &after);
        }
        restored.close(1).await;
    }
}

#[tokio::test]
async fn real_postgres_recovered_mailbox_requires_fresh_current_authority() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    live.command_at(0, 1, 1401).await;
    live.close(1).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        live.limits,
        WALL_START,
        9_000_000,
    )
    .await;
    assert_eq!(
        restored.output_count(),
        0,
        "recovery does not restore or emit attachment output"
    );
    restored.attach_all().await;
    let before = restored.load().await;
    let output = restored.output_count();
    let binding = &restored.players[1];
    restored
        .authority
        .0
        .lock()
        .unwrap()
        .get_mut(&binding.session())
        .unwrap()
        .binding = Binding::new(
        binding.session(),
        binding.subject(),
        binding.record(),
        binding.epoch() + 1,
        binding.generation() + 1,
    );
    let ticket = restored
        .handle
        .command(
            binding.clone(),
            restored.envelope(1, 1402, restored.fixture.commands[1].1.clone()),
        )
        .unwrap();
    assert_eq!(completed(ticket).await, Completion::Suppressed);
    assert_records_equal(&before, &restored.load().await);
    assert_eq!(restored.output_count(), output);
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_ticking_clock_persists_receipt_at_actual_applied_time() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_clocked_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    live.clock.tick.store(1, Ordering::SeqCst);
    live.command_at(0, 1, 1501).await;
    let before = live.load().await;
    let record = before.records.last().unwrap();
    assert!(
        record.now.0 > 0,
        "clock actually changes between sequence check and apply"
    );
    let receipt = before
        .ledger
        .iter()
        .find(|scope| scope.scope.seat == SeatId(0))
        .unwrap()
        .recent
        .iter()
        .find(|receipt| receipt.seq == 1)
        .unwrap();
    assert_eq!(
        receipt.at, record.now.0,
        "accepted receipt timestamp is exact apply/log time"
    );
    live.close(1).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_clocked_fixture(),
        live.limits,
        WALL_START,
        90_000_000,
    )
    .await;
    assert_ne!(restored.players[0].session(), live.players[0].session());
    assert_eq!(restored.players[0].record(), live.players[0].record());
    restored.attach_all().await;
    restored.command_at(0, 1, 1502).await;
    assert_records_equal(&before, &restored.load().await);
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_wrong_match_and_game_rejections_retain_original_payload_receipts() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    let payload = live.fixture.commands[0].1.clone();
    let game = live.fixture.game.metadata().id().clone();
    let version = live.fixture.game.metadata().version().clone();
    let wrong_match = ClientEnvelope::new(
        1,
        1601,
        GameCommandFrame::new(
            MatchId(live.id.0 + 1),
            game.clone(),
            version.clone(),
            payload.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let foreign = tabula_core::GameId::new(format!("{}.foreign", game.as_str())).unwrap();
    let wrong_game = ClientEnvelope::new(
        2,
        1602,
        GameCommandFrame::new(live.id, foreign, version, payload).unwrap(),
    )
    .unwrap();
    live.send_envelope(SeatId(0), wrong_match.clone(), Err(ErrorCode::WrongMatch))
        .await;
    live.send_envelope(SeatId(0), wrong_game.clone(), Err(ErrorCode::WrongGame))
        .await;
    let before = live.load().await;
    assert_eq!(before.index, InputIndex(0));
    live.close(0).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        live.limits,
        WALL_START,
        90_000_000,
    )
    .await;
    restored.attach_all().await;
    for (seq, corr, envelope, error) in [
        (1, 1611, wrong_match, ErrorCode::WrongMatch),
        (2, 1612, wrong_game, ErrorCode::WrongGame),
    ] {
        let retry = ClientEnvelope::new(seq, corr, envelope.command().clone()).unwrap();
        restored.send_envelope(SeatId(0), retry, Err(error)).await;
        restored
            .send(
                SeatId(0),
                seq,
                corr + 20,
                restored.fixture.commands[0].1.clone(),
                Err(ErrorCode::OperationConflict),
            )
            .await;
    }
    assert_records_equal(&before, &restored.load().await);
    restored.command_at(0, 3, 1640).await;
    assert_eq!(restored.load().await.index, InputIndex(1));
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_outage_keeps_multiple_expired_scope_watermarks_without_blocking_attach() {
    let (_pool, store) = database().await;
    let limits = Limits {
        receipt_ttl_ms: 50,
        ..Limits::default()
    };
    let mut live = Harness::start(&store, approved_checkmate_fixture(), limits, 600_000).await;
    live.attach_all().await;
    live.clock.set(WALL_START + 10, 600_010);
    live.command_at(0, 1, 1701).await;
    live.clock.set(WALL_START + 20, 600_020);
    live.command_at(1, 1, 1702).await;
    let before = live.load().await;
    live.close(2).await;
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_checkmate_fixture(),
        limits,
        WALL_START + 200,
        90_000_000,
    )
    .await;
    restored.attach_all().await;
    for (at, seat) in [(0, SeatId(0)), (1, SeatId(1))] {
        restored
            .send(
                seat,
                1,
                1710 + u64::from(seat.0),
                restored.fixture.commands[at].1.clone(),
                Err(ErrorCode::StaleSeq),
            )
            .await;
    }
    assert_records_equal(&before, &restored.load().await);
    let loaded = restored.load().await;
    for seat in [SeatId(0), SeatId(1)] {
        assert_eq!(
            loaded
                .ledger
                .iter()
                .find(|scope| scope.scope.seat == seat)
                .unwrap()
                .highest,
            1
        );
    }
    restored.command_at(2, 2, 1720).await;
    assert_eq!(restored.load().await.index, InputIndex(3));
    restored.close(3).await;
}

#[tokio::test]
async fn real_postgres_all_handles_closed_reopens_without_graceful_drain() {
    let (_pool, store) = database().await;
    let live = Harness::start(
        &store,
        approved_checkmate_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    live.command_at(0, 1, 1801).await;
    let before = live.load().await;
    let id = live.id;
    let limits = live.limits;
    let Harness {
        handle,
        host,
        owner,
        ..
    } = live;
    drop(handle);
    drop(host);
    let summary = tokio::time::timeout(WAIT, owner.unwrap())
        .await
        .expect("all senders closed terminates actual actor")
        .unwrap();
    assert_eq!(summary.exit, Exit::Closed);
    assert_eq!(summary.index, InputIndex(1));
    let mut restored = Harness::reopen(
        &store,
        id,
        approved_checkmate_fixture(),
        limits,
        WALL_START,
        900_000,
    )
    .await;
    restored.attach_all().await;
    restored.command_at(0, 1, 1802).await;
    assert_records_equal(&before, &restored.load().await);
    restored.close(1).await;
}

#[tokio::test]
async fn real_postgres_server_rejected_commit_rolls_back_without_consuming_operation() {
    let (_pool, store) = database().await;
    let mut live = Harness::start(
        &store,
        approved_clocked_fixture(),
        Limits::default(),
        600_000,
    )
    .await;
    live.attach_all().await;
    let before = live.load().await;
    let output = live.output_count();
    let effects = live.effects.requests.lock().unwrap().len();
    // This checked disposable-schema constraint is DEFERRABLE INITIALLY
    // DEFERRED. No journal fault switch is set: PostgreSQL itself rejects COMMIT.
    store
        .install_commit_failure_for_test()
        .await
        .expect("install actual deferred server constraint in disposable schema");
    let ticket = live
        .handle
        .command(
            live.players[0].clone(),
            live.envelope(1, 1901, live.fixture.commands[0].1.clone()),
        )
        .unwrap();
    assert!(tokio::time::timeout(WAIT, ticket.wait())
        .await
        .unwrap()
        .is_err());
    let summary = tokio::time::timeout(WAIT, live.owner.take().unwrap())
        .await
        .expect("actual commit failure stops owner")
        .unwrap();
    assert_eq!(summary.exit, Exit::JournalFailed);
    assert_eq!(
        live.output_count(),
        output,
        "server-rejected COMMIT emits no Ack or projection"
    );
    assert_eq!(
        live.effects.requests.lock().unwrap().len(),
        effects,
        "server-rejected COMMIT executes no game effects"
    );
    let observer = independent_store(&store).await;
    assert_eq!(
        observer.committed_prefix_for_test(live.id).await.unwrap(),
        (StateVersion(0), InputIndex(0), 1)
    );
    store
        .clear_commit_failure_for_test()
        .await
        .expect("remove disposable deferred fault constraint");
    let mut restored = Harness::reopen(
        &store,
        live.id,
        approved_clocked_fixture(),
        live.limits,
        WALL_START,
        9_000_000,
    )
    .await;
    restored.attach_all().await;
    let recovered = restored.load().await;
    assert_records_equal(&before, &recovered);
    assert_eq!(
        recovered
            .ledger
            .iter()
            .find(|scope| scope.scope.seat == SeatId(0))
            .unwrap()
            .highest,
        0,
        "rolled-back operation was never consumed"
    );
    restored.command_at(0, 1, 1902).await;
    assert_eq!(restored.load().await.records.len(), 2);
    restored.command_at(0, 1, 1903).await;
    assert_eq!(
        restored.load().await.records.len(),
        2,
        "retry after real rollback commits exactly once"
    );
    restored.close(1).await;
}
