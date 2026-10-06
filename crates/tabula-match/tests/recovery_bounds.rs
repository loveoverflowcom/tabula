//! Exact restart policy using a faithful atomic in-memory journal and approved
//! real module. This is not PostgreSQL/process/online-auth acceptance.
#![cfg(all(feature = "isolated", not(target_arch = "wasm32")))]

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tabula_core::{InputIndex, MatchId, SeatId, SessionId, StateVersion, UserId};
use tabula_game_api::Effect;
use tabula_match::{
    durable::{Journal, JournalRecord, LoadedMatch, RuntimePortError, ScopeState},
    ports::Clock,
    runtime::{self, Binding, Completion, Limits, Ports, RecoveryError},
    runtime_ports::{Authority, AuthorityLost, Effects, Output, Purpose},
};
use tabula_protocol::{ClientEnvelope, ErrorCode, GameCommandFrame, ServerEnvelope, ServerMessage};
use tabula_registry::runtime::{
    test_support::{approved_checkmate_fixture, RuntimeFixture},
    ClientViewer,
};

const ID: MatchId = MatchId(901);

#[derive(Default)]
struct MemoryJournal(Mutex<Option<LoadedMatch>>);
impl Journal for MemoryJournal {
    async fn append(&self, mut record: JournalRecord) -> Result<(), RuntimePortError> {
        let mut head = self.0.lock().unwrap();
        let ledger = std::mem::take(&mut record.ledger);
        if let Some(loaded) = head.as_mut() {
            assert_eq!(record.expected_version, Some(loaded.version));
            assert_eq!(record.version.0, loaded.version.0 + 1);
            loaded.version = record.version;
            loaded.index = record.index;
            loaded.observed_ms = record.now.0;
            loaded.ledger = ledger;
            loaded.records.push(record);
        } else {
            assert!(record.expected_version.is_none());
            *head = Some(LoadedMatch {
                creation: record.creation.clone().unwrap(),
                ledger,
                version: record.version,
                index: record.index,
                observed_ms: record.now.0,
                records: vec![record],
            });
        }
        Ok(())
    }
    async fn update_ledger(
        &self,
        id: MatchId,
        version: StateVersion,
        at: u64,
        ledger: Vec<ScopeState>,
    ) -> Result<(), RuntimePortError> {
        assert_eq!(id, ID);
        let mut head = self.0.lock().unwrap();
        let loaded = head.as_mut().unwrap();
        assert_eq!(loaded.version, version);
        assert!(at >= loaded.observed_ms);
        loaded.observed_ms = at;
        loaded.ledger = ledger;
        Ok(())
    }
    async fn load(&self, id: MatchId) -> Result<LoadedMatch, RuntimePortError> {
        assert_eq!(id, ID);
        self.0
            .lock()
            .unwrap()
            .clone()
            .ok_or(RuntimePortError::Unavailable)
    }
}

struct Current;
impl Authority for Current {
    fn with_current<T>(
        &self,
        _: &Binding,
        _: Purpose,
        action: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost> {
        Ok(action())
    }
}
#[derive(Default)]
struct Frames(Mutex<Vec<ServerEnvelope>>);
impl Output for Frames {
    fn submit(&self, _: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError> {
        self.0.lock().unwrap().push(frame);
        Ok(())
    }
}
#[derive(Default)]
struct KeyedEffects(Mutex<Vec<InputIndex>>);
impl Effects for KeyedEffects {
    async fn execute(
        &self,
        _: MatchId,
        index: InputIndex,
        _: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        let mut keys = self.0.lock().unwrap();
        if !keys.contains(&index) {
            keys.push(index);
        }
        Ok(())
    }
}
struct Time {
    unix: AtomicU64,
    mono: AtomicU64,
}
impl Time {
    fn new(unix: u64, mono: u64) -> Self {
        Self {
            unix: AtomicU64::new(unix),
            mono: AtomicU64::new(mono),
        }
    }
}
impl Clock for Time {
    fn now_unix_ms(&self) -> u64 {
        self.unix.load(Ordering::SeqCst)
    }
    fn monotonic_ms(&self) -> u64 {
        self.mono.load(Ordering::SeqCst)
    }
}
fn ports(
    journal: &Arc<MemoryJournal>,
    frames: &Arc<Frames>,
    effects: &Arc<KeyedEffects>,
    clock: &Arc<Time>,
) -> Ports<Current, MemoryJournal, Frames, KeyedEffects, Time> {
    Ports {
        authority: Arc::new(Current),
        journal: journal.clone(),
        output: frames.clone(),
        effects: effects.clone(),
        clock: clock.clone(),
    }
}
fn binding(connection: u64, seat: u8) -> Binding {
    Binding::new(
        SessionId(connection),
        UserId(u128::from(seat) + 1),
        u128::from(seat) + 11,
        3,
        1,
    )
}
fn command(fixture: &RuntimeFixture, seq: u64, payload: Vec<u8>) -> ClientEnvelope {
    ClientEnvelope::new(
        seq,
        seq,
        GameCommandFrame::new(
            ID,
            fixture.game.metadata().id().clone(),
            fixture.game.metadata().version().clone(),
            payload,
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn restart_restores_original_receipt_time_and_scope_before_fresh_projection() {
    let fixture = approved_checkmate_fixture();
    let journal = Arc::new(MemoryJournal::default());
    let frames = Arc::new(Frames::default());
    let effects = Arc::new(KeyedEffects::default());
    let clock = Arc::new(Time::new(1000, 0));
    let created = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap();
    let (handle, host, task) = runtime::spawn(
        ID,
        created,
        ports(&journal, &frames, &effects, &clock),
        Limits::default(),
    )
    .unwrap();
    let original = binding(1, 0);
    handle
        .attach(original.clone(), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    clock.mono.store(40, Ordering::SeqCst);
    let first = command(&fixture, 1, fixture.commands[0].1.clone());
    assert_eq!(
        handle
            .command(original, first.clone())
            .unwrap()
            .wait()
            .await
            .unwrap(),
        Completion::Submitted
    );
    host.drain().unwrap().wait().await.unwrap();
    assert_eq!(task.await.unwrap().version, StateVersion(1));
    let before = journal.load(ID).await.unwrap();
    assert_eq!(before.records[1].now.0, 40);
    let original_receipt = before.ledger[0].recent[0].clone();
    frames.0.lock().unwrap().clear();
    let clock = Arc::new(Time::new(6000, 9000));
    let (handle, host, task) = runtime::recover_for_admission(
        ID,
        fixture.game.clone(),
        &fixture.config,
        &fixture.roster,
        ports(&journal, &frames, &effects, &clock),
        Limits::default(),
    )
    .await
    .unwrap();
    assert!(
        frames.0.lock().unwrap().is_empty(),
        "recovery never restores old output streams"
    );
    let replacement = binding(99, 0);
    handle
        .attach(replacement.clone(), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(
        matches!(frames.0.lock().unwrap()[0].body(), ServerMessage::MatchUpdate { revision: 0, events, .. } if events.is_empty())
    );
    assert_eq!(frames.0.lock().unwrap()[0].frame(), 1);
    handle
        .command(replacement.clone(), first)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        journal.load(ID).await.unwrap().records.len(),
        2,
        "uncertain committed command is never reapplied under a new connection"
    );
    assert_eq!(
        journal.load(ID).await.unwrap().ledger[0].recent[0],
        original_receipt
    );
    handle
        .command(
            replacement,
            command(&fixture, 1, fixture.illegal_command.clone()),
        )
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(
        frames.0.lock().unwrap().last().unwrap().body(),
        ServerMessage::Reject {
            error: ErrorCode::OperationConflict,
            ..
        }
    ));
    let opponent = binding(100, 1);
    handle
        .attach(opponent.clone(), ClientViewer::Seat(SeatId(1)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    clock.mono.store(9020, Ordering::SeqCst);
    handle
        .command(
            opponent,
            command(&fixture, 1, fixture.commands[1].1.clone()),
        )
        .unwrap()
        .wait()
        .await
        .unwrap();
    host.drain().unwrap().wait().await.unwrap();
    assert_eq!(task.await.unwrap().version, StateVersion(2));
    assert_eq!(
        journal.load(ID).await.unwrap().records[2].now.0,
        5020,
        "recorded match time survives a process-clock reset and outage"
    );
    assert_eq!(
        *effects.0.lock().unwrap(),
        vec![InputIndex(0), InputIndex(1), InputIndex(2)],
        "historical keyed effect replay is idempotent"
    );
}

#[tokio::test]
async fn admission_mismatch_and_malformed_scope_fail_before_any_output_or_effects() {
    let fixture = approved_checkmate_fixture();
    let journal = Arc::new(MemoryJournal::default());
    let frames = Arc::new(Frames::default());
    let effects = Arc::new(KeyedEffects::default());
    let clock = Arc::new(Time::new(1000, 0));
    let created = fixture
        .game
        .create_match(&fixture.config, &fixture.roster, fixture.seed.clone())
        .unwrap();
    let (handle, host, task) = runtime::spawn(
        ID,
        created,
        ports(&journal, &frames, &effects, &clock),
        Limits::default(),
    )
    .unwrap();
    handle
        .attach(binding(1, 0), ClientViewer::Seat(SeatId(0)))
        .unwrap()
        .wait()
        .await
        .unwrap();
    host.drain().unwrap().wait().await.unwrap();
    task.await.unwrap();
    frames.0.lock().unwrap().clear();
    effects.0.lock().unwrap().clear();
    let result = runtime::recover_for_admission(
        ID,
        fixture.game.clone(),
        &[0],
        &fixture.roster,
        ports(&journal, &frames, &effects, &clock),
        Limits::default(),
    )
    .await;
    assert!(matches!(result, Err(RecoveryError::Corrupt)));
    journal.0.lock().unwrap().as_mut().unwrap().ledger[0]
        .scope
        .record = 0;
    let result = runtime::recover(
        ID,
        fixture.game.clone(),
        ports(&journal, &frames, &effects, &clock),
        Limits::default(),
    )
    .await;
    assert!(matches!(result, Err(RecoveryError::Corrupt)));
    assert!(frames.0.lock().unwrap().is_empty());
    assert!(effects.0.lock().unwrap().is_empty());
}
