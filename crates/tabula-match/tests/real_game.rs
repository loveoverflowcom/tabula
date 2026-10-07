//! Real approved game authority exercised through the isolated actor and both
//! envelope codecs. These local ports prove no online authentication, transport,
//! disk durability, recovery, or general game-rule correctness (ADR-0039).

#![cfg(all(feature = "isolated", not(target_arch = "wasm32")))]

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use smallvec::smallvec;
use tabula_core::{
    canonical_encode, GameId, GameVersion, InputIndex, LogicalTime, MatchId, MatchOutcome, Millis,
    Occupant, OutcomeKind, SeatId, SessionId, Standing, StateVersion, TimerId, UserId,
};
use tabula_game_api::Effect;
use tabula_match::{
    ports::Clock,
    runtime::{
        spawn, Binding, Completion, Exit, HostControl, Limits, MatchHandle, Ports, Summary, Ticket,
    },
    runtime_ports::{
        Authority, AuthorityLost, Effects, Journal, JournalRecord, Output, Purpose,
        RuntimePortError,
    },
};
use tabula_protocol::{
    decode_client, decode_server, encode_client, encode_server, ClientEnvelope, Codec, ErrorCode,
    GameCommandFrame, ServerEnvelope, ServerMessage,
};
use tabula_registry::runtime::{
    test_support::{approved_checkmate_fixture, approved_clocked_fixture, RuntimeFixture},
    ClientViewer,
};

const MATCH: MatchId = MatchId(900);
const SPECTATOR: SessionId = SessionId(30);
const MOVES: [(u8, u8, u8); 4] = [(0, 13, 21), (1, 52, 36), (0, 14, 30), (1, 59, 31)];
const CAPTURE_LIMIT: usize = 128;

#[derive(Clone, Debug)]
struct Grant {
    binding: Binding,
    viewer: ClientViewer,
}

/// Resolution and revocation use the same mutex as actual apply/submission.
/// Every identity component and assigned viewer must still be current.
#[derive(Debug, Default)]
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
    Effects(InputIndex),
    Output(SessionId, Option<u64>),
}

type Boundaries = Arc<Mutex<Vec<Boundary>>>;

/// An append records all fields atomically and returns known success only.
#[derive(Debug)]
struct MemoryJournal {
    records: Mutex<Vec<JournalRecord>>,
    boundaries: Boundaries,
}

impl Journal for MemoryJournal {
    async fn append(&self, record: JournalRecord) -> Result<(), RuntimePortError> {
        let mut records = self.records.lock().unwrap();
        assert!(records.len() < CAPTURE_LIMIT, "bounded journal fixture");
        assert_eq!(record.index.0, u64::try_from(records.len()).unwrap());
        assert_eq!(record.version.0, record.index.0);
        self.boundaries
            .lock()
            .unwrap()
            .push(Boundary::Commit(record.index));
        records.push(record);
        Ok(())
    }
}

/// Round-tripped capture is bounded, synchronous, and never retains canonical
/// journal data. It represents submission, not eventual socket delivery.
#[derive(Debug)]
struct CaptureOutput {
    codec: Codec,
    frames: Mutex<Vec<(SessionId, ServerEnvelope)>>,
    boundaries: Boundaries,
}

impl Output for CaptureOutput {
    fn submit(&self, binding: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError> {
        let mut frames = self.frames.lock().unwrap();
        if frames.len() == CAPTURE_LIMIT {
            return Err(RuntimePortError::Busy);
        }
        let bytes = encode_server(self.codec, &frame).expect("projected frame encodes");
        let received = decode_server(self.codec, &bytes).expect("projected frame decodes");
        assert_eq!(received, frame);
        self.boundaries
            .lock()
            .unwrap()
            .push(Boundary::Output(binding.session(), received.corr()));
        frames.push((binding.session(), received));
        Ok(())
    }
}

#[derive(Debug)]
struct CaptureEffects {
    requests: Mutex<Vec<(MatchId, InputIndex, Vec<Effect>)>>,
    boundaries: Boundaries,
}

impl Effects for CaptureEffects {
    async fn execute(
        &self,
        match_id: MatchId,
        index: InputIndex,
        effects: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        let mut requests = self.requests.lock().unwrap();
        assert!(requests.len() < CAPTURE_LIMIT, "bounded effects fixture");
        let mut boundaries = self.boundaries.lock().unwrap();
        assert_eq!(boundaries.last(), Some(&Boundary::Commit(index)));
        boundaries.push(Boundary::Effects(index));
        requests.push((match_id, index, effects));
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ManualClock(AtomicU64);

impl Clock for ManualClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    fn monotonic_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Harness {
    codec: Codec,
    fixture: RuntimeFixture,
    players: [Binding; 2],
    spectator: Binding,
    authority: Arc<LocalAuthority>,
    journal: Arc<MemoryJournal>,
    output: Arc<CaptureOutput>,
    effects: Arc<CaptureEffects>,
    clock: Arc<ManualClock>,
    boundaries: Boundaries,
    handle: MatchHandle,
    host: HostControl,
    owner: tokio::task::JoinHandle<Summary>,
}

impl Harness {
    fn new(codec: Codec) -> Self {
        let fixture = approved_checkmate_fixture();
        assert_eq!(fixture.config, [1, 0, 0]);
        Self::with_fixture(codec, fixture, 0)
    }

    #[allow(clippy::too_many_lines)] // Explicit local port wiring stays visible in this fixture.
    fn with_fixture(codec: Codec, fixture: RuntimeFixture, anchor: u64) -> Self {
        // Independent, pinned canonical command values. The fixture is setup,
        // not an oracle for the legality or public outcome of this transcript.
        assert_eq!(fixture.commands.len(), MOVES.len());
        for ((seat, command), &(expected_seat, from, to)) in fixture.commands.iter().zip(&MOVES) {
            assert_eq!(*seat, SeatId(expected_seat));
            assert_eq!(command, &[1, 0, 0, from, to, 0]);
        }
        let players = std::array::from_fn(|index| {
            let seat = SeatId(u8::try_from(index).unwrap());
            let Occupant::Human(subject) = fixture.roster.get(seat).unwrap().occupant else {
                panic!("real fixture must have a resolved human subject")
            };
            Binding::new(
                SessionId(u64::from(seat.0) + 10),
                subject,
                u128::from(seat.0) + 100,
                7,
                3,
            )
        });
        let spectator = Binding::new(SPECTATOR, UserId(99), 199, 7, 0);
        let authority = Arc::new(LocalAuthority::default());
        {
            let mut grants = authority.0.lock().unwrap();
            for (index, binding) in players.iter().enumerate() {
                grants.insert(
                    binding.session(),
                    Grant {
                        binding: binding.clone(),
                        viewer: ClientViewer::Seat(SeatId(u8::try_from(index).unwrap())),
                    },
                );
            }
            grants.insert(
                SPECTATOR,
                Grant {
                    binding: spectator.clone(),
                    viewer: ClientViewer::Spectator,
                },
            );
        }
        let boundaries = Arc::new(Mutex::new(Vec::new()));
        let journal = Arc::new(MemoryJournal {
            records: Mutex::new(Vec::new()),
            boundaries: Arc::clone(&boundaries),
        });
        let output = Arc::new(CaptureOutput {
            codec,
            frames: Mutex::new(Vec::new()),
            boundaries: Arc::clone(&boundaries),
        });
        let effects = Arc::new(CaptureEffects {
            requests: Mutex::new(Vec::new()),
            boundaries: Arc::clone(&boundaries),
        });
        let clock = Arc::new(ManualClock(AtomicU64::new(anchor)));
        let created = fixture
            .game
            .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
            .expect("approved game creates through the generic catalog boundary");
        let (handle, host, owner) = spawn(
            MATCH,
            created,
            Ports {
                authority: Arc::clone(&authority),
                journal: Arc::clone(&journal),
                output: Arc::clone(&output),
                effects: Arc::clone(&effects),
                clock: Arc::clone(&clock),
            },
            Limits::default(),
        )
        .expect("bounded actor starts");
        Self {
            codec,
            fixture,
            players,
            spectator,
            authority,
            journal,
            output,
            effects,
            clock,
            boundaries,
            handle,
            host,
            owner,
        }
    }

    async fn attach(&self) {
        for (index, binding) in self.players.iter().enumerate() {
            assert_eq!(
                completed(
                    self.handle
                        .attach(
                            binding.clone(),
                            ClientViewer::Seat(SeatId(u8::try_from(index).unwrap())),
                        )
                        .unwrap()
                )
                .await,
                Completion::Submitted,
            );
        }
        assert_eq!(
            completed(
                self.handle
                    .attach(self.spectator.clone(), ClientViewer::Spectator)
                    .unwrap()
            )
            .await,
            Completion::Submitted,
        );
        self.assert_spectator(0);
    }

    fn envelope(&self, seq: u64, corr: u64, payload: Vec<u8>) -> ClientEnvelope {
        self.tagged_envelope(
            seq,
            corr,
            MATCH,
            self.fixture.game.metadata().id().clone(),
            self.fixture.game.metadata().version().clone(),
            payload,
        )
    }

    fn tagged_envelope(
        &self,
        seq: u64,
        corr: u64,
        match_id: MatchId,
        game: GameId,
        package: GameVersion,
        payload: Vec<u8>,
    ) -> ClientEnvelope {
        let command = GameCommandFrame::new(match_id, game, package, payload).unwrap();
        let envelope = ClientEnvelope::new(seq, corr, command).unwrap();
        let frame = encode_client(self.codec, &envelope).expect("client envelope encodes");
        let received = decode_client(self.codec, &frame).expect("client envelope decodes");
        assert_eq!(envelope, received);
        received
    }

    async fn receipt(
        &self,
        binding: &Binding,
        envelope: ClientEnvelope,
        expected: Result<(), ErrorCode>,
    ) {
        let seq = envelope.seq();
        let corr = envelope.corr();
        assert_eq!(
            completed(self.handle.command(binding.clone(), envelope).unwrap()).await,
            Completion::Submitted
        );
        let frames = self.output.frames.lock().unwrap();
        let caused: Vec<_> = frames
            .iter()
            .filter(|(session, frame)| *session == binding.session() && frame.corr() == Some(corr))
            .collect();
        assert_eq!(caused.len(), 1, "exactly one receipt for this correlation");
        let expected = match expected {
            Ok(()) => ServerMessage::Ack { seq },
            Err(error) => ServerMessage::Reject { seq, error },
        };
        assert_eq!(caused[0].1.body(), &expected);
        drop(frames);
        if matches!(expected, ServerMessage::Ack { .. }) {
            self.assert_commit_before_receipt(binding.session(), corr);
        }
    }

    fn assert_commit_before_receipt(&self, session: SessionId, corr: u64) {
        let index = self.records().last().unwrap().index;
        let boundaries = self.boundaries.lock().unwrap();
        let commit = boundaries
            .iter()
            .position(|entry| *entry == Boundary::Commit(index))
            .expect("accepted input was atomically committed");
        let output = boundaries
            .iter()
            .position(|entry| *entry == Boundary::Output(session, Some(corr)))
            .expect("receipt was actually submitted");
        assert!(
            commit < output,
            "known-success commit precedes Ack submission"
        );
        assert!(
            commit + 1 < output,
            "all effects follow commit and precede Ack"
        );
    }

    async fn rejected(&self, binding: &Binding, envelope: ClientEnvelope, error: ErrorCode) {
        let before = self.records();
        let updates = self.update_count();
        self.receipt(binding, envelope, Err(error)).await;
        assert_records_equal(&before, &self.records());
        assert_eq!(
            self.update_count(),
            updates,
            "rejection emits no projection update"
        );
    }

    fn records(&self) -> Vec<JournalRecord> {
        self.journal.records.lock().unwrap().clone()
    }

    fn update_count(&self) -> usize {
        self.output
            .frames
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, frame)| matches!(frame.body(), ServerMessage::MatchUpdate { .. }))
            .count()
    }

    fn assert_spectator(&self, applied: usize) {
        let frames = self.output.frames.lock().unwrap();
        let (revision, view, events) = frames
            .iter()
            .rev()
            .find_map(|(session, frame)| {
                if *session != SPECTATOR {
                    return None;
                }
                match frame.body() {
                    ServerMessage::MatchUpdate {
                        revision,
                        view,
                        events,
                    } => Some((*revision, view, events)),
                    _ => None,
                }
            })
            .expect("spectator received a projection");
        assert_eq!(revision, u64::try_from(applied).unwrap());
        assert_eq!(
            view,
            &spectator_view(applied),
            "independently specified public board"
        );
        assert_eq!(events, &expected_events(applied));
    }

    fn assert_journal_and_effects(&self) {
        let records = self.records();
        assert_eq!(records.len(), 5, "creation plus four accepted inputs");
        assert!(records[0].input.is_empty());
        assert!(records[0].events.is_empty());
        for (position, record) in records.iter().enumerate() {
            let at = u64::try_from(position).unwrap();
            assert_eq!(record.match_id, MATCH);
            assert_eq!(record.index, InputIndex(at));
            assert_eq!(record.version, StateVersion(at));
            assert_eq!(record.now, LogicalTime(at * 10));
            if position == 0 {
                continue;
            }
            let (seat, from, to) = MOVES[position - 1];
            assert_eq!(record.input, [1, 0, 0, seat, 0, from, to, 0]);
            assert_eq!(record.events, expected_events(position));
            assert_ne!(record.hash, records[position - 1].hash);
        }
        let requests = self.effects.requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            records.len(),
            "every committed effect batch is delivered"
        );
        for (position, (id, index, effects)) in requests.iter().enumerate() {
            assert_eq!(*id, MATCH);
            assert_eq!(*index, InputIndex(u64::try_from(position).unwrap()));
            if position == 4 {
                assert_eq!(effects.len(), 1);
                assert!(
                    matches!(&effects[0], Effect::EndMatch { outcome } if *outcome == checkmate())
                );
            } else {
                assert!(effects.is_empty());
            }
        }
        let boundaries = self.boundaries.lock().unwrap();
        for at in 0..=4 {
            let commit = boundaries
                .iter()
                .position(|entry| *entry == Boundary::Commit(InputIndex(at)))
                .unwrap();
            assert_eq!(boundaries[commit + 1], Boundary::Effects(InputIndex(at)));
        }
        let frames = self.output.frames.lock().unwrap();
        for binding in self.players.iter().chain(std::iter::once(&self.spectator)) {
            let mut revisions = Vec::new();
            for (expected_frame, (_, frame)) in (1..).zip(
                frames
                    .iter()
                    .filter(|(session, _)| *session == binding.session()),
            ) {
                assert_eq!(
                    frame.frame(),
                    expected_frame,
                    "per-connection frame stream has no gaps"
                );
                if let ServerMessage::MatchUpdate { revision, .. } = frame.body() {
                    revisions.push(*revision);
                }
            }
            assert_eq!(revisions, [0, 1, 2, 3, 4]);
        }
    }

    async fn finish(self, expected_version: u64) -> Vec<JournalRecord> {
        assert_eq!(
            completed(self.host.drain().unwrap()).await,
            Completion::Submitted
        );
        let summary = tokio::time::timeout(std::time::Duration::from_secs(5), self.owner)
            .await
            .expect("owner must terminate")
            .expect("owner task must not panic");
        assert_eq!(summary.exit, Exit::Drained);
        assert_eq!(summary.version, StateVersion(expected_version));
        assert_eq!(summary.index, InputIndex(expected_version));
        self.journal.records.lock().unwrap().clone()
    }
}

async fn completed(ticket: Ticket) -> Completion {
    tokio::time::timeout(std::time::Duration::from_secs(5), ticket.wait())
        .await
        .expect("bounded test operation must finish")
        .expect("owner stays alive")
}

fn checkmate() -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Decisive,
        smallvec![
            Standing {
                seat: SeatId(1),
                rank: 0,
                score: 1
            },
            Standing {
                seat: SeatId(0),
                rank: 1,
                score: 0
            },
        ],
        "checkmate".into(),
        &[SeatId(0), SeatId(1)],
    )
    .unwrap()
}

fn expected_events(applied: usize) -> Vec<Vec<u8>> {
    if applied == 0 {
        return Vec::new();
    }
    let (seat, from, to) = MOVES[applied - 1];
    let mut events = vec![vec![1, 0, 0, seat, from, to, 0, 0]];
    if applied == 4 {
        let mut ended = vec![1, 0, 4];
        ended.extend_from_slice(&canonical_encode(&checkmate()).unwrap()[2..]);
        events.push(ended);
    }
    events
}

/// Independent finite public-state model for 1.f3 e5 2.g4 Qh4#.
/// Stable canonical schema: 64 optional (color, piece-kind) cells followed by
/// turn, castling, en-passant, clocks, status, viewer and empty spectator hints.
/// No production reducer, projection, or game type constructs this oracle.
fn spectator_view(applied: usize) -> Vec<u8> {
    let mut board = [None; 64];
    let back = [3, 1, 2, 4, 5, 2, 1, 3];
    for (file, kind) in back.into_iter().enumerate() {
        board[file] = Some((0, kind));
        board[file + 8] = Some((0, 0));
        board[file + 48] = Some((1, 0));
        board[file + 56] = Some((1, kind));
    }
    for &(_, from, to) in &MOVES[..applied] {
        board[usize::from(to)] = board[usize::from(from)].take();
    }
    let mut view = vec![1, 0, 64];
    for piece in board {
        if let Some((color, kind)) = piece {
            view.extend([1, color, kind]);
        } else {
            view.push(0);
        }
    }
    view.push(u8::try_from(applied % 2).unwrap());
    view.extend([1, 1, 1, 1]);
    match applied {
        2 => view.extend([1, 44]),
        3 => view.extend([1, 22]),
        _ => view.push(0),
    }
    view.push(u8::from(applied == 4));
    view.push(u8::try_from(1 + applied / 2).unwrap());
    if applied == 4 {
        view.push(1);
        view.extend_from_slice(&canonical_encode(&checkmate()).unwrap()[2..]);
    } else {
        view.push(0);
    }
    view.extend([0, 0, 0, u8::from(applied == 4), 0, 0]);
    view
}

fn assert_records_equal(expected: &[JournalRecord], actual: &[JournalRecord]) {
    assert_eq!(actual.len(), expected.len());
    for (expected, actual) in expected.iter().zip(actual) {
        assert_eq!(actual.match_id, expected.match_id);
        assert_eq!(actual.index, expected.index);
        assert_eq!(actual.version, expected.version);
        assert_eq!(actual.now, expected.now);
        assert_eq!(actual.input, expected.input);
        assert_eq!(actual.events, expected.events);
        assert_eq!(actual.hash, expected.hash);
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One finite transcript includes all hostile and retry probes.
async fn approved_finite_transcript_preserves_rules_ordering_and_receipts_for_both_codecs() {
    for codec in [Codec::Postcard, Codec::Json] {
        let harness = Harness::new(codec);
        harness.attach().await;
        let white = &harness.players[0];
        let black = &harness.players[1];
        let first = harness.fixture.commands[0].1.clone();

        for corr in [1, 2] {
            harness
                .rejected(
                    white,
                    harness.envelope(1, corr, harness.fixture.illegal_command.clone()),
                    ErrorCode::RuleRejected,
                )
                .await;
        }
        harness
            .rejected(
                white,
                harness.envelope(1, 3, first.clone()),
                ErrorCode::OperationConflict,
            )
            .await;
        for corr in [4, 5] {
            harness
                .rejected(
                    white,
                    harness.envelope(2, corr, vec![1]),
                    ErrorCode::Malformed,
                )
                .await;
        }
        harness
            .rejected(
                white,
                harness.envelope(2, 6, first.clone()),
                ErrorCode::OperationConflict,
            )
            .await;
        // A valid command's meaning and turn legality remain game-owned; the
        // attached black subject cannot claim the white seat through payloads.
        harness
            .rejected(
                black,
                harness.envelope(1, 7, first.clone()),
                ErrorCode::RuleRejected,
            )
            .await;
        let game = harness.fixture.game.metadata().id().clone();
        let package = harness.fixture.game.metadata().version().clone();
        let foreign_game = GameId::new(format!("{}.foreign", game.as_str())).unwrap();
        let foreign_package = GameVersion::new(format!("{}+foreign", package.as_str())).unwrap();
        for (seq, corr, id, tag, version, error) in [
            (
                3,
                8,
                MatchId(901),
                game.clone(),
                package.clone(),
                ErrorCode::WrongMatch,
            ),
            (
                4,
                9,
                MATCH,
                foreign_game,
                package.clone(),
                ErrorCode::WrongGame,
            ),
            (5, 10, MATCH, game, foreign_package, ErrorCode::WrongGame),
        ] {
            harness
                .rejected(
                    white,
                    harness.tagged_envelope(seq, corr, id, tag, version, first.clone()),
                    error,
                )
                .await;
        }
        harness
            .rejected(
                &harness.spectator,
                harness.envelope(1, 11, first.clone()),
                ErrorCode::Unauthorized,
            )
            .await;
        assert_eq!(harness.records().len(), 1);

        for (position, (seat, payload)) in harness.fixture.commands.iter().enumerate() {
            let seq = match position {
                0 => 6,
                1 => 2,
                2 => 7,
                3 => 3,
                _ => unreachable!(),
            };
            let corr = 20 + u64::try_from(position).unwrap();
            harness
                .clock
                .0
                .store(u64::try_from(position + 1).unwrap() * 10, Ordering::SeqCst);
            let binding = &harness.players[usize::from(seat.0)];
            harness
                .receipt(
                    binding,
                    harness.envelope(seq, corr, payload.clone()),
                    Ok(()),
                )
                .await;
            harness.assert_spectator(position + 1);
            let records = harness.records();
            let updates = harness.update_count();
            harness
                .receipt(
                    binding,
                    harness.envelope(seq, corr + 20, payload.clone()),
                    Ok(()),
                )
                .await;
            assert_records_equal(&records, &harness.records());
            assert_eq!(harness.update_count(), updates);
            harness
                .rejected(
                    binding,
                    harness.envelope(seq, corr + 40, vec![1]),
                    ErrorCode::OperationConflict,
                )
                .await;
        }
        for corr in [100, 101] {
            harness
                .rejected(
                    white,
                    harness.envelope(8, corr, first.clone()),
                    ErrorCode::Terminal,
                )
                .await;
        }
        harness.assert_journal_and_effects();
        let dirty = harness.finish(4).await;

        // Self-differential rejection oracle: accepted hashes, input bytes and
        // events agree at every index with a clean run, not just at termination.
        let clean = Harness::new(codec);
        clean.attach().await;
        for (position, (seat, payload)) in clean.fixture.commands.iter().enumerate() {
            clean
                .clock
                .0
                .store(u64::try_from(position + 1).unwrap() * 10, Ordering::SeqCst);
            clean
                .receipt(
                    &clean.players[usize::from(seat.0)],
                    clean.envelope(
                        u64::try_from(position / 2 + 1).unwrap(),
                        200 + u64::try_from(position).unwrap(),
                        payload.clone(),
                    ),
                    Ok(()),
                )
                .await;
        }
        clean.assert_journal_and_effects();
        assert_records_equal(&dirty, &clean.finish(4).await);
    }
}

#[tokio::test]
async fn concurrent_identical_real_commands_apply_once_and_retain_both_correlations() {
    for codec in [Codec::Postcard, Codec::Json] {
        let harness = Harness::new(codec);
        harness.attach().await;
        let binding = &harness.players[0];
        let payload = harness.fixture.commands[0].1.clone();
        let start = Arc::new(tokio::sync::Barrier::new(3));
        let producers: Vec<_> = [301, 302]
            .into_iter()
            .map(|corr| {
                let handle = harness.handle.clone();
                let binding = binding.clone();
                let envelope = harness.envelope(1, corr, payload.clone());
                let start = Arc::clone(&start);
                tokio::spawn(async move {
                    start.wait().await;
                    completed(handle.command(binding, envelope).unwrap()).await
                })
            })
            .collect();
        start.wait().await;
        for producer in producers {
            assert_eq!(producer.await.unwrap(), Completion::Submitted);
        }
        let frames = harness.output.frames.lock().unwrap().clone();
        for corr in [301, 302] {
            let replies: Vec<_> = frames
                .iter()
                .filter(|(_, frame)| frame.corr() == Some(corr))
                .collect();
            assert_eq!(replies.len(), 1);
            assert_eq!(replies[0].1.body(), &ServerMessage::Ack { seq: 1 });
        }
        assert_eq!(harness.records().len(), 2);
        for corr in [301, 302] {
            harness.assert_commit_before_receipt(binding.session(), corr);
        }
        assert_eq!(harness.effects.requests.lock().unwrap().len(), 2);
        assert_eq!(
            harness.update_count(),
            6,
            "three attach projections plus one public transition per viewer"
        );
        harness.assert_spectator(1);
        harness.finish(1).await;
    }
}

#[tokio::test]
async fn real_game_apply_and_output_require_current_resolved_subject_and_seat() {
    for codec in [Codec::Postcard, Codec::Json] {
        let harness = Harness::new(codec);
        let white = &harness.players[0];
        let forged = Binding::new(
            white.session(),
            UserId(999),
            white.record(),
            white.epoch(),
            white.generation(),
        );
        for (binding, viewer) in [
            (forged, ClientViewer::Seat(SeatId(0))),
            (white.clone(), ClientViewer::Seat(SeatId(1))),
        ] {
            assert_eq!(
                completed(harness.handle.attach(binding, viewer).unwrap()).await,
                Completion::Suppressed
            );
        }
        assert!(harness.output.frames.lock().unwrap().is_empty());
        harness.attach().await;
        let before = harness.records();
        let output_before = harness.output.frames.lock().unwrap().clone();
        // Host changes the actual current epoch and seat generation. The stale
        // wire caller cannot restore either using command bytes or correlation.
        harness
            .authority
            .0
            .lock()
            .unwrap()
            .get_mut(&white.session())
            .unwrap()
            .binding = Binding::new(
            white.session(),
            white.subject(),
            white.record(),
            white.epoch() + 1,
            white.generation() + 1,
        );
        assert_eq!(
            completed(
                harness
                    .handle
                    .command(
                        white.clone(),
                        harness.envelope(1, 401, harness.fixture.commands[0].1.clone(),)
                    )
                    .unwrap()
            )
            .await,
            Completion::Suppressed
        );
        assert_records_equal(&before, &harness.records());
        assert_eq!(*harness.output.frames.lock().unwrap(), output_before);
        harness.finish(0).await;
    }
}

#[tokio::test]
async fn clocked_real_game_uses_elapsed_match_time_when_process_clock_is_nonzero() {
    for codec in [Codec::Postcard, Codec::Json] {
        let fixture = approved_clocked_fixture();
        // Canonical Some(clock), initial 1000 ms, Fischer with zero increment.
        assert_eq!(fixture.config, [1, 0, 1, 232, 7, 0, 0]);
        let harness = Harness::with_fixture(codec, fixture, 600_000);
        let white = &harness.players[0];
        for (binding, viewer) in [
            (white.clone(), ClientViewer::Seat(SeatId(0))),
            (harness.spectator.clone(), ClientViewer::Spectator),
        ] {
            assert_eq!(
                completed(harness.handle.attach(binding, viewer).unwrap()).await,
                Completion::Submitted,
            );
        }
        let records = harness.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].now, LogicalTime(0));
        assert_clocked_spectator(&harness, 0);

        harness.clock.0.store(600_010, Ordering::SeqCst);
        harness
            .receipt(
                white,
                harness.envelope(1, 501, harness.fixture.commands[0].1.clone()),
                Ok(()),
            )
            .await;
        let records = harness.records();
        assert_eq!(records.len(), 2);
        let moved = &records[1];
        assert_eq!(
            moved.now,
            LogicalTime(10),
            "process uptime is not match elapsed time"
        );
        assert_eq!(moved.index, InputIndex(1));
        assert_eq!(moved.version, StateVersion(1));
        assert_eq!(moved.input, [1, 0, 0, 0, 0, 13, 21, 0]);
        let mut expected = expected_events(1);
        // White's 1000 ms budget is charged exactly 10 ms, leaving 990.
        expected.push(vec![1, 0, 1, 0, 222, 7]);
        assert_eq!(moved.events, expected);
        assert_ne!(moved.hash, records[0].hash);
        assert_clocked_spectator(&harness, 1);
        {
            let requests = harness.effects.requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            for (position, (id, index, effects)) in requests.iter().enumerate() {
                assert_eq!(*id, MATCH);
                assert_eq!(*index, InputIndex(u64::try_from(position).unwrap()));
                assert_eq!(effects.len(), 1);
                assert!(matches!(
                    effects[0],
                    Effect::SetTimer {
                        id: TimerId(1),
                        delay: Millis(1000)
                    },
                ));
            }
        }
        harness.finish(1).await;
    }
}

fn assert_clocked_spectator(harness: &Harness, applied: usize) {
    assert!(
        applied <= 1,
        "this independent clocked oracle covers creation and first move"
    );
    let mut expected_view = spectator_view(applied);
    let clock_offset = expected_view.len() - 5;
    let clock = if applied == 0 {
        // Some ClockState: [1000,1000], last_move_at=0, Fischer increment0.
        [1, 232, 7, 232, 7, 0, 0, 0]
    } else {
        // Some ClockState: [990,1000], last_move_at=10, Fischer increment0.
        [1, 222, 7, 232, 7, 10, 0, 0]
    };
    expected_view.splice(clock_offset..=clock_offset, clock);
    let mut expected_events = expected_events(applied);
    if applied == 1 {
        expected_events.push(vec![1, 0, 1, 0, 222, 7]);
    }
    let frames = harness.output.frames.lock().unwrap();
    let frame = frames
        .iter()
        .rev()
        .find_map(|(session, frame)| (*session == SPECTATOR).then_some(frame))
        .expect("spectator received the clocked public state");
    assert_eq!(
        frame.body(),
        &ServerMessage::MatchUpdate {
            revision: u64::try_from(applied).unwrap(),
            view: expected_view,
            events: expected_events,
        }
    );
}
