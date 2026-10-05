//! Bounded, single-owner isolated actor (doc 03 §6–9, ADR-0039/0040).
//!
//! The sole erased state handle moves into `run`; no client gets a clone.
//! Actual authority guards surround apply and output, with no unrelated await.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchId, MatchSeed, SeatId, SessionId,
    StateVersion, UserId,
};
use tabula_game_api::Effect;
use tabula_protocol::{ClientEnvelope, ErrorCode, ServerEnvelope, ServerMessage};
use tabula_registry::runtime::{
    ClientViewer, CreatedMatch, ErasedInput, ErasedMatch, RuntimeError,
};
use tokio::sync::{mpsc, oneshot};

use crate::{
    durable::{
        LedgerLimits, MatchCreation, MatchIdentity, OperationKey, OperationReceipt, OperationScope,
        ScopeState, JOURNAL_FORMAT,
    },
    ports::Clock,
    runtime_ports::{Authority, Effects, Journal, JournalRecord, Output, Purpose},
};

/// A host-resolved binding. It is intentionally neither serde nor a wire type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    session: SessionId,
    subject: UserId,
    record: u128,
    epoch: u64,
    generation: u64,
}

impl Binding {
    /// Called by the trusted host after resolving current authority.
    #[must_use]
    pub const fn new(
        session: SessionId,
        subject: UserId,
        record: u128,
        epoch: u64,
        generation: u64,
    ) -> Self {
        Self {
            session,
            subject,
            record,
            epoch,
            generation,
        }
    }
    pub const fn session(&self) -> SessionId {
        self.session
    }
    pub const fn subject(&self) -> UserId {
        self.subject
    }
    pub const fn record(&self) -> u128 {
        self.record
    }
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

/// All limits are local acceptance controls, not production capacity claims.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub mailbox: usize,
    pub per_session_inflight: usize,
    pub attachments: usize,
    pub operation_scopes: usize,
    pub receipts_per_scope: usize,
    pub receipt_ttl_ms: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            mailbox: 64,
            per_session_inflight: 4,
            attachments: 32,
            operation_scopes: 64,
            receipts_per_scope: 16,
            receipt_ttl_ms: 120_000,
        }
    }
}

impl Limits {
    fn valid(self) -> bool {
        (2..=1024).contains(&self.mailbox)
            && (1..self.mailbox).contains(&self.per_session_inflight)
            && (1..=256).contains(&self.attachments)
            && (1..=256).contains(&self.operation_scopes)
            && (1..=64).contains(&self.receipts_per_scope)
            && (1..=3_600_000).contains(&self.receipt_ttl_ms)
    }
}

/// Non-secret completion status; actual Ack/Reject travels through Output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Submitted,
    Suppressed,
}

/// Admission failed before enqueue; no sequence was consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    Busy,
    Closed,
    InvalidLimits,
}

/// Terminal owner state. No panic payload or canonical state is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Closed,
    Drained,
    JournalFailed,
    EffectsFailed,
    RulesPanic,
    ProjectionFailed,
    CounterExhausted,
}

/// Host-only known-committed counters, never serialized on the wire.
/// An indeterminate append may have committed beyond these counters. Recovery
/// must validate the authoritative journal; this summary is not a recovery cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub exit: Exit,
    pub version: StateVersion,
    pub index: InputIndex,
}

/// Awaiting/dropping this ticket never directly exposes private game output.
#[derive(Debug)]
pub struct Ticket(oneshot::Receiver<Completion>);
impl Ticket {
    pub async fn wait(self) -> Result<Completion, AdmissionError> {
        self.0.await.map_err(|_| AdmissionError::Closed)
    }
}

#[derive(Debug, Default)]
struct Admission {
    counts: Mutex<BTreeMap<SessionId, usize>>,
}
#[derive(Debug)]
struct Permit {
    admission: Arc<Admission>,
    session: SessionId,
}
impl Drop for Permit {
    fn drop(&mut self) {
        if let Ok(mut counts) = self.admission.counts.lock() {
            if let Some(count) = counts.get_mut(&self.session) {
                *count -= 1;
                if *count == 0 {
                    counts.remove(&self.session);
                }
            }
        }
    }
}

/// Host-admitted client input, using nonblocking bounded enqueue only.
#[derive(Clone, Debug)]
pub struct MatchHandle {
    tx: mpsc::Sender<Envelope>,
    admission: Arc<Admission>,
    limits: Limits,
}

impl MatchHandle {
    fn permit(&self, session: SessionId) -> Result<Permit, AdmissionError> {
        let mut counts = self
            .admission
            .counts
            .lock()
            .map_err(|_| AdmissionError::Closed)?;
        if counts.get(&session).copied().unwrap_or(0) >= self.limits.per_session_inflight {
            return Err(AdmissionError::Busy);
        }
        if !counts.contains_key(&session) && counts.len() >= self.limits.mailbox {
            return Err(AdmissionError::Busy);
        }
        *counts.entry(session).or_default() += 1;
        Ok(Permit {
            admission: Arc::clone(&self.admission),
            session,
        })
    }

    fn enqueue(&self, session: SessionId, work: Work) -> Result<Ticket, AdmissionError> {
        if self.tx.is_closed() {
            return Err(AdmissionError::Closed);
        }
        let permit = self.permit(session)?;
        let (reply, ticket) = oneshot::channel();
        self.tx
            .try_send(Envelope {
                work,
                reply,
                _permit: Some(permit),
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => AdmissionError::Busy,
                mpsc::error::TrySendError::Closed(_) => AdmissionError::Closed,
            })?;
        Ok(Ticket(ticket))
    }

    /// Attach a resolved viewer. Audit/delayed viewers have no construction path.
    pub fn attach(&self, binding: Binding, viewer: ClientViewer) -> Result<Ticket, AdmissionError> {
        self.enqueue(binding.session, Work::Attach { binding, viewer })
    }

    pub fn command(
        &self,
        binding: Binding,
        command: ClientEnvelope,
    ) -> Result<Ticket, AdmissionError> {
        self.enqueue(binding.session, Work::Command { binding, command })
    }

    pub fn detach(&self, binding: Binding) -> Result<Ticket, AdmissionError> {
        self.enqueue(binding.session, Work::Detach { binding })
    }
}

/// Trusted shell inputs cannot be constructed from a client envelope.
#[derive(Debug)]
pub struct HostControl {
    tx: mpsc::Sender<Envelope>,
}
impl HostControl {
    pub fn input(&self, input: ErasedInput) -> Result<Ticket, AdmissionError> {
        self.enqueue(Work::System(input))
    }
    /// Close admission once reached in FIFO order, then drain earlier work.
    pub fn drain(&self) -> Result<Ticket, AdmissionError> {
        self.enqueue(Work::Drain)
    }
    fn enqueue(&self, work: Work) -> Result<Ticket, AdmissionError> {
        let (reply, ticket) = oneshot::channel();
        self.tx
            .try_send(Envelope {
                work,
                reply,
                _permit: None,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => AdmissionError::Busy,
                mpsc::error::TrySendError::Closed(_) => AdmissionError::Closed,
            })?;
        Ok(Ticket(ticket))
    }
}

struct Envelope {
    work: Work,
    reply: oneshot::Sender<Completion>,
    _permit: Option<Permit>,
}
enum Work {
    Attach {
        binding: Binding,
        viewer: ClientViewer,
    },
    Command {
        binding: Binding,
        command: ClientEnvelope,
    },
    Detach {
        binding: Binding,
    },
    System(ErasedInput),
    Drain,
}

type Scope = OperationScope;
type Receipt = OperationReceipt;
fn scope_for(binding: &Binding, seat: SeatId) -> Scope {
    Scope {
        record: binding.record,
        subject: binding.subject,
        epoch: binding.epoch,
        seat,
        generation: binding.generation,
    }
}

#[derive(Debug, Default)]
struct Sequence {
    highest: u64,
    recent: VecDeque<Receipt>,
}
#[derive(Debug)]
struct Attached {
    binding: Binding,
    viewer: ClientViewer,
    view: Vec<u8>,
    frame: u64,
    revision: u64,
}

/// The replaceable host boundaries composed for one actor (doc 03 §6.2).
#[derive(Debug)]
pub struct Ports<A, J, O, E, C> {
    pub authority: Arc<A>,
    pub journal: Arc<J>,
    pub output: Arc<O>,
    pub effects: Arc<E>,
    pub clock: Arc<C>,
}

/// Spawn the sole owner. Creation append/effects execute before mailbox work.
pub fn spawn<A, J, O, E, C>(
    id: MatchId,
    created: CreatedMatch,
    ports: Ports<A, J, O, E, C>,
    limits: Limits,
) -> Result<(MatchHandle, HostControl, tokio::task::JoinHandle<Summary>), AdmissionError>
where
    A: Authority,
    J: Journal,
    O: Output,
    E: Effects,
    C: Clock + 'static,
{
    if !limits.valid() {
        return Err(AdmissionError::InvalidLimits);
    }
    let Ports {
        authority,
        journal,
        output,
        effects,
        clock,
    } = ports;
    let anchor = clock.monotonic_ms();
    let (state, seed, events, initial_effects) = created.into_parts();
    let identity = state.identity();
    let creation = MatchCreation {
        format: JOURNAL_FORMAT,
        identity: MatchIdentity {
            game: identity.game.clone(),
            game_version: identity.game_version.clone(),
            rules_version: identity.rules_version,
            rules_hash: identity.rules_hash,
        },
        config: state.creation_config().to_vec(),
        roster: state.roster().clone(),
        seed: *seed.as_bytes(),
        started_at_unix_ms: clock.now_unix_ms(),
        limits: ledger_limits(limits),
    };
    let (tx, rx) = mpsc::channel(limits.mailbox);
    let handle = MatchHandle {
        tx: tx.clone(),
        admission: Arc::default(),
        limits,
    };
    let host = HostControl { tx };
    let actor = Actor {
        id,
        state,
        seed,
        authority,
        journal,
        output,
        effects,
        clock,
        anchor,
        base_time: 0,
        observed_ms: 0,
        creation,
        limits,
        rx,
        attached: BTreeMap::new(),
        scopes: BTreeMap::new(),
        version: StateVersion(0),
        index: InputIndex(0),
        last_time: LogicalTime(0),
        terminal: false,
    };
    Ok((
        handle,
        host,
        tokio::spawn(actor.run(events, initial_effects, None)),
    ))
}

pub(crate) fn ledger_limits(limits: Limits) -> LedgerLimits {
    LedgerLimits {
        scopes: u16::try_from(limits.operation_scopes).expect("validated scope limit"),
        receipts_per_scope: u16::try_from(limits.receipts_per_scope)
            .expect("validated receipt limit"),
        receipt_ttl_ms: limits.receipt_ttl_ms,
    }
}

/// Recovery refuses corrupt/foreign journals before any client or effect output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    Unavailable,
    Corrupt,
    InvalidLimits,
}
impl core::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RecoveryError {}

/// Claim a replacement journal outside this function, then validate its entire
/// bounded committed prefix against the exact approved module (ADR-0040).
/// No connection/view cursor is recovered; attachments require fresh authority.
pub async fn recover<A, J, O, E, C>(
    id: MatchId,
    game: Arc<dyn tabula_registry::ErasedGame>,
    ports: Ports<A, J, O, E, C>,
    limits: Limits,
) -> Result<(MatchHandle, HostControl, tokio::task::JoinHandle<Summary>), RecoveryError>
where
    A: Authority,
    J: Journal,
    O: Output,
    E: Effects,
    C: Clock + 'static,
{
    if !limits.valid() {
        return Err(RecoveryError::InvalidLimits);
    }
    let loaded = ports
        .journal
        .load(id)
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    let recovered = catch_unwind(AssertUnwindSafe(|| {
        crate::runtime_recovery::validate(id, game.as_ref(), &loaded, limits)
    }))
    .map_err(|_| RecoveryError::Corrupt)??;
    let Ports {
        authority,
        journal,
        output,
        effects,
        clock,
    } = ports;
    let anchor = clock.monotonic_ms();
    // Outage elapsed time counts in the isolated unpaused match. Clock rollback
    // clamps to the durable observation watermark, never resets to process zero.
    let base_time = clock
        .now_unix_ms()
        .saturating_sub(loaded.creation.started_at_unix_ms)
        .max(loaded.observed_ms);
    let (tx, rx) = mpsc::channel(limits.mailbox);
    let handle = MatchHandle {
        tx: tx.clone(),
        admission: Arc::default(),
        limits,
    };
    let host = HostControl { tx };
    let committed_effects = loaded
        .records
        .iter()
        .map(|record| (record.index, record.effects.clone()))
        .collect();
    let actor = Actor {
        id,
        state: recovered,
        seed: MatchSeed::from_bytes(loaded.creation.seed),
        authority,
        journal,
        output,
        effects,
        clock,
        anchor,
        base_time,
        observed_ms: loaded.observed_ms,
        creation: loaded.creation,
        limits,
        rx,
        attached: BTreeMap::new(),
        scopes: loaded
            .ledger
            .into_iter()
            .map(|state| {
                (
                    state.scope,
                    Sequence {
                        highest: state.highest,
                        recent: state.recent.into(),
                    },
                )
            })
            .collect(),
        version: loaded.version,
        index: loaded.index,
        last_time: loaded.records.last().ok_or(RecoveryError::Corrupt)?.now,
        terminal: loaded
            .records
            .last()
            .ok_or(RecoveryError::Corrupt)?
            .terminal,
    };
    Ok((
        handle,
        host,
        tokio::spawn(actor.run(vec![], vec![], Some(committed_effects))),
    ))
}

struct Actor<A, J, O, E, C> {
    id: MatchId,
    state: Box<dyn ErasedMatch>,
    seed: MatchSeed,
    authority: Arc<A>,
    journal: Arc<J>,
    output: Arc<O>,
    effects: Arc<E>,
    clock: Arc<C>,
    anchor: u64,
    base_time: u64,
    observed_ms: u64,
    creation: MatchCreation,
    limits: Limits,
    rx: mpsc::Receiver<Envelope>,
    attached: BTreeMap<SessionId, Attached>,
    scopes: BTreeMap<Scope, Sequence>,
    version: StateVersion,
    index: InputIndex,
    last_time: LogicalTime,
    terminal: bool,
}

impl<A: Authority, J: Journal, O: Output, E: Effects, C: Clock + 'static> Actor<A, J, O, E, C> {
    async fn run(
        mut self,
        events: Vec<Vec<u8>>,
        effects: Vec<Effect>,
        recovered: Option<Vec<(InputIndex, Vec<Effect>)>>,
    ) -> Summary {
        let start = if let Some(committed) = recovered {
            let mut result = Ok(());
            for (index, effects) in committed {
                if self.effects.execute(self.id, index, effects).await.is_err() {
                    result = Err(Exit::EffectsFailed);
                    break;
                }
            }
            result
        } else {
            let start = self.record(
                self.index,
                self.version,
                self.last_time,
                vec![],
                events,
                &effects,
                None,
                true,
            );
            let start = match start {
                Ok(record) => self.commit(record, effects).await,
                Err(exit) => Err(exit),
            };
            start
        };
        let exit = match start {
            Err(exit) => exit,
            Ok(()) => loop {
                let Some(envelope) = self.rx.recv().await else {
                    break Exit::Closed;
                };
                if envelope.reply.is_closed() {
                    continue;
                }
                let result = match envelope.work {
                    Work::Attach { binding, viewer } => self.attach(binding, viewer).await,
                    Work::Command { binding, command } => self.command(binding, command).await,
                    Work::Detach { binding } => {
                        self.detach(&binding);
                        Ok(Completion::Submitted)
                    }
                    Work::System(input) => self.system(input).await,
                    Work::Drain => {
                        self.rx.close();
                        let _ = envelope.reply.send(Completion::Submitted);
                        break Exit::Drained;
                    }
                };
                match result {
                    Ok(completion) => {
                        let _ = envelope.reply.send(completion);
                    }
                    Err(exit) => {
                        break exit;
                    }
                }
            },
        };
        self.rx.close();
        Summary {
            exit,
            version: self.version,
            index: self.index,
        }
    }

    async fn commit(&mut self, record: JournalRecord, effects: Vec<Effect>) -> Result<(), Exit> {
        let index = record.index;
        let version = record.version;
        let now = record.now;
        let terminal = record.terminal;
        self.journal
            .append(record)
            .await
            .map_err(|_| Exit::JournalFailed)?;
        // Host-visible committed counters advance only after known-success.
        self.index = index;
        self.version = version;
        self.last_time = now;
        self.observed_ms = self.observed_ms.max(now.0);
        self.terminal = terminal;
        self.effects
            .execute(self.id, index, effects)
            .await
            .map_err(|_| Exit::EffectsFailed)
    }

    fn ledger(&self) -> Vec<ScopeState> {
        self.scopes
            .iter()
            .map(|(scope, sequence)| ScopeState {
                scope: *scope,
                highest: sequence.highest,
                recent: sequence.recent.iter().cloned().collect(),
            })
            .collect()
    }

    async fn persist_ledger(&mut self) -> Result<(), Exit> {
        self.observed_ms = self.observed_ms.max(self.logical_now().0);
        let ledger = self.ledger();
        if canonical_encode(&ledger)
            .map_err(|_| Exit::JournalFailed)?
            .len()
            > crate::durable::MAX_LEDGER_BYTES
        {
            return Err(Exit::JournalFailed);
        }
        self.journal
            .update_ledger(self.id, self.version, self.observed_ms, ledger)
            .await
            .map_err(|_| Exit::JournalFailed)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        index: InputIndex,
        version: StateVersion,
        now: LogicalTime,
        input: Vec<u8>,
        events: Vec<Vec<u8>>,
        effects: &[Effect],
        operation: Option<OperationKey>,
        initial: bool,
    ) -> Result<JournalRecord, Exit> {
        let terminal = self.terminal
            || effects
                .iter()
                .any(|effect| matches!(effect, Effect::EndMatch { .. }));
        let snapshot = if initial || index.0 % 20 == 0 || terminal {
            Some(
                catch_unwind(AssertUnwindSafe(|| self.state.snapshot()))
                    .map_err(|_| Exit::RulesPanic)?
                    .map_err(|_| Exit::ProjectionFailed)?,
            )
        } else {
            None
        };
        let ledger = self.ledger();
        if canonical_encode(&ledger)
            .map_err(|_| Exit::JournalFailed)?
            .len()
            > crate::durable::MAX_LEDGER_BYTES
        {
            return Err(Exit::JournalFailed);
        }
        Ok(JournalRecord {
            match_id: self.id,
            index,
            version,
            now,
            input,
            events,
            hash: self.hash()?,
            effects: effects.to_vec(),
            operation,
            terminal,
            snapshot,
            creation: initial.then(|| self.creation.clone()),
            ledger,
            expected_version: if initial { None } else { Some(self.version) },
        })
    }

    fn hash(&self) -> Result<tabula_core::StateHash, Exit> {
        catch_unwind(AssertUnwindSafe(|| self.state.state_hash())).map_err(|_| Exit::RulesPanic)
    }

    fn logical_now(&self) -> LogicalTime {
        LogicalTime(
            self.clock
                .monotonic_ms()
                .saturating_sub(self.anchor)
                .saturating_add(self.base_time)
                .max(self.last_time.0)
                .max(self.observed_ms),
        )
    }

    fn detach(&mut self, binding: &Binding) {
        if self
            .attached
            .get(&binding.session)
            .is_some_and(|attached| attached.binding == *binding)
        {
            self.attached.remove(&binding.session);
        }
    }

    async fn attach(&mut self, binding: Binding, viewer: ClientViewer) -> Result<Completion, Exit> {
        if let ClientViewer::Seat(seat) = viewer {
            if !self.state.roster().iter().any(|entry| entry.seat == seat) {
                return Ok(Completion::Suppressed);
            }
        }
        if self.attached.contains_key(&binding.session)
            || self.attached.len() >= self.limits.attachments
        {
            return Ok(Completion::Suppressed);
        }
        let scope = match viewer {
            ClientViewer::Seat(seat) => Some(scope_for(&binding, seat)),
            ClientViewer::Spectator => None,
        };
        if scope.is_some_and(|scope| !self.scopes.contains_key(&scope))
            && self.scopes.len() >= self.limits.operation_scopes
        {
            return Ok(Completion::Suppressed);
        }
        if self
            .authority
            .with_current(&binding, Purpose::Observe(viewer), || ())
            .is_err()
        {
            return Ok(Completion::Suppressed);
        }
        if let Some(scope) = scope {
            self.scopes.entry(scope).or_default();
            self.persist_ledger().await?;
        }
        let view = catch_unwind(AssertUnwindSafe(|| self.state.project(viewer)))
            .map_err(|_| Exit::RulesPanic)?
            .map_err(|_| Exit::ProjectionFailed)?;
        let frame = ServerEnvelope::new(
            None,
            1,
            ServerMessage::MatchUpdate {
                revision: 0,
                view: view.clone(),
                events: vec![],
            },
        )
        .map_err(|_| Exit::ProjectionFailed)?;
        let submitted = self
            .authority
            .with_current(&binding, Purpose::Observe(viewer), || {
                self.output.submit(&binding, frame)
            });
        if !matches!(submitted, Ok(Ok(()))) {
            return Ok(Completion::Suppressed);
        }
        // Reserve at visible admission, never lazily on a potentially private
        // command. Otherwise another seat can probe Busy to infer that action.
        self.attached.insert(
            binding.session,
            Attached {
                binding,
                viewer,
                view,
                frame: 1,
                revision: 0,
            },
        );
        Ok(Completion::Submitted)
    }

    fn reply(
        &mut self,
        binding: &Binding,
        corr: u64,
        seq: u64,
        result: Result<(), ErrorCode>,
    ) -> Result<Completion, Exit> {
        let Some(attached) = self
            .attached
            .get_mut(&binding.session)
            .filter(|attached| attached.binding == *binding)
        else {
            return Ok(Completion::Suppressed);
        };
        let Some(frame_index) = attached.frame.checked_add(1) else {
            return Err(Exit::CounterExhausted);
        };
        let body = match result {
            Ok(()) => ServerMessage::Ack { seq },
            Err(error) => ServerMessage::Reject { seq, error },
        };
        let frame = ServerEnvelope::new(Some(corr), frame_index, body)
            .map_err(|_| Exit::ProjectionFailed)?;
        let submitted =
            self.authority
                .with_current(binding, Purpose::Receipt(attached.viewer), || {
                    self.output.submit(binding, frame)
                });
        if matches!(submitted, Ok(Ok(()))) {
            attached.frame = frame_index;
            Ok(Completion::Submitted)
        } else {
            self.attached.remove(&binding.session);
            Ok(Completion::Suppressed)
        }
    }

    fn remember(
        &mut self,
        scope: Scope,
        command: &ClientEnvelope,
        result: Result<(), ErrorCode>,
        at: u64,
        committed_index: Option<InputIndex>,
    ) {
        let sequence = self.scopes.entry(scope).or_default();
        sequence.highest = command.seq();
        sequence.recent.push_back(Receipt {
            seq: command.seq(),
            command: command.command().clone(),
            result,
            at,
            committed_index,
        });
        while sequence.recent.len() > self.limits.receipts_per_scope {
            sequence.recent.pop_front();
        }
    }

    fn check_sequence(
        &mut self,
        scope: Scope,
        command: &ClientEnvelope,
        now_ms: u64,
    ) -> Option<Result<(), ErrorCode>> {
        let seq = command.seq();
        if let Some(sequence) = self.scopes.get_mut(&scope) {
            sequence
                .recent
                .retain(|receipt| now_ms.saturating_sub(receipt.at) < self.limits.receipt_ttl_ms);
            if seq <= sequence.highest {
                let result = sequence
                    .recent
                    .iter()
                    .find(|receipt| receipt.seq == seq)
                    .map_or(Err(ErrorCode::StaleSeq), |receipt| {
                        if receipt.command == *command.command() {
                            receipt.result
                        } else {
                            Err(ErrorCode::OperationConflict)
                        }
                    });
                return Some(result);
            }
        } else if self.scopes.len() >= self.limits.operation_scopes {
            return Some(Err(ErrorCode::Busy));
        }
        let highest = self
            .scopes
            .get(&scope)
            .map_or(0, |sequence| sequence.highest);
        if seq.saturating_sub(highest) > 64 {
            return Some(Err(ErrorCode::SeqTooFar));
        }
        None
    }

    fn command_rejection(&self, command: &ClientEnvelope) -> Option<ErrorCode> {
        let identity = self.state.identity();
        if command.command().match_id() != self.id {
            Some(ErrorCode::WrongMatch)
        } else if command.command().game() != &identity.game
            || command.command().game_version() != &identity.game_version
        {
            Some(ErrorCode::WrongGame)
        } else if self.terminal {
            Some(ErrorCode::Terminal)
        } else {
            None
        }
    }

    async fn command(
        &mut self,
        binding: Binding,
        command: ClientEnvelope,
    ) -> Result<Completion, Exit> {
        let corr = command.corr();
        let seq = command.seq();
        let Some(attached) = self
            .attached
            .get(&binding.session)
            .filter(|attached| attached.binding == binding)
        else {
            return Ok(Completion::Suppressed);
        };
        let ClientViewer::Seat(seat) = attached.viewer else {
            return self.reply(&binding, corr, seq, Err(ErrorCode::Unauthorized));
        };
        if self
            .authority
            .with_current(&binding, Purpose::Apply(seat), || ())
            .is_err()
        {
            self.detach(&binding);
            return Ok(Completion::Suppressed);
        }
        let scope = scope_for(&binding, seat);
        let now_ms = self.logical_now().0;
        self.observed_ms = now_ms;
        if let Some(result) = self.check_sequence(scope, &command, now_ms) {
            self.persist_ledger().await?;
            return self.reply(&binding, corr, seq, result);
        }
        let rejection = self.command_rejection(&command);
        if let Some(error) = rejection {
            self.remember(scope, &command, Err(error), now_ms, None);
            self.persist_ledger().await?;
            return self.reply(&binding, corr, seq, Err(error));
        }
        let index = InputIndex(self.index.0.checked_add(1).ok_or(Exit::CounterExhausted)?);
        let version = StateVersion(
            self.version
                .0
                .checked_add(1)
                .ok_or(Exit::CounterExhausted)?,
        );
        let now = self.logical_now();
        let mut rng = DetRng::for_input(&self.seed, index);
        let input = ErasedInput::Player {
            seat,
            payload: command.command().payload().to_vec(),
        };
        let applied = self
            .authority
            .with_current(&binding, Purpose::Apply(seat), || {
                catch_unwind(AssertUnwindSafe(|| {
                    self.state.apply(input, now, index, &mut rng)
                }))
            });
        let transition = match applied {
            Err(_) => {
                self.detach(&binding);
                return Ok(Completion::Suppressed);
            }
            Ok(Err(_)) => return Err(Exit::RulesPanic),
            Ok(Ok(Err(error))) => {
                let error = match error {
                    RuntimeError::Malformed(_) | RuntimeError::LimitExceeded => {
                        ErrorCode::Malformed
                    }
                    RuntimeError::Rule(_) | RuntimeError::UnknownSeat(_) => ErrorCode::RuleRejected,
                    _ => return Err(Exit::ProjectionFailed),
                };
                self.remember(scope, &command, Err(error), now_ms, None);
                self.persist_ledger().await?;
                return self.reply(&binding, corr, seq, Err(error));
            }
            Ok(Ok(Ok(transition))) => transition,
        };
        self.remember(scope, &command, Ok(()), now.0, Some(index));
        let record = self.record(
            index,
            version,
            now,
            transition.canonical_input,
            transition.events.clone(),
            &transition.effects,
            Some(OperationKey {
                scope,
                seq,
                command: command.command().clone(),
            }),
            false,
        )?;
        self.commit(record, transition.effects).await?;
        let updates = self.prepare_updates(&transition.events)?;
        let reply = self.reply(&binding, corr, seq, Ok(()))?;
        self.publish_updates(updates);
        Ok(reply)
    }

    async fn system(&mut self, input: ErasedInput) -> Result<Completion, Exit> {
        if self.terminal {
            return Ok(Completion::Suppressed);
        }
        let index = InputIndex(self.index.0.checked_add(1).ok_or(Exit::CounterExhausted)?);
        let version = StateVersion(
            self.version
                .0
                .checked_add(1)
                .ok_or(Exit::CounterExhausted)?,
        );
        let now = self.logical_now();
        let mut rng = DetRng::for_input(&self.seed, index);
        let applied = catch_unwind(AssertUnwindSafe(|| {
            self.state.apply(input, now, index, &mut rng)
        }))
        .map_err(|_| Exit::RulesPanic)?;
        let Ok(transition) = applied else {
            return Ok(Completion::Suppressed);
        };
        let record = self.record(
            index,
            version,
            now,
            transition.canonical_input,
            transition.events.clone(),
            &transition.effects,
            None,
            false,
        )?;
        self.commit(record, transition.effects).await?;
        let updates = self.prepare_updates(&transition.events)?;
        self.publish_updates(updates);
        Ok(Completion::Submitted)
    }

    fn prepare_updates(&self, events: &[Vec<u8>]) -> Result<Vec<PendingUpdate>, Exit> {
        let mut updates = Vec::new();
        for (session, attached) in &self.attached {
            let (view, redacted) = catch_unwind(AssertUnwindSafe(|| {
                Ok::<_, RuntimeError>((
                    self.state.project(attached.viewer)?,
                    self.state.view_events(events, attached.viewer)?,
                ))
            }))
            .map_err(|_| Exit::RulesPanic)?
            .map_err(|_| Exit::ProjectionFailed)?;
            if view == attached.view && redacted.is_empty() {
                continue;
            }
            let frame_index = attached
                .frame
                .checked_add(1)
                .ok_or(Exit::CounterExhausted)?;
            let revision = attached
                .revision
                .checked_add(1)
                .ok_or(Exit::CounterExhausted)?;
            let frame = ServerEnvelope::new(
                None,
                frame_index,
                ServerMessage::MatchUpdate {
                    revision,
                    view: view.clone(),
                    events: redacted,
                },
            )
            .map_err(|_| Exit::ProjectionFailed)?;
            updates.push(PendingUpdate {
                session: *session,
                view,
                frame_index,
                revision,
                frame,
            });
        }
        Ok(updates)
    }

    fn publish_updates(&mut self, updates: Vec<PendingUpdate>) {
        let mut lost = Vec::new();
        for update in updates {
            let Some(attached) = self.attached.get_mut(&update.session) else {
                continue;
            };
            // A command receipt may just have advanced this connection's frame.
            let Some(frame_index) = attached.frame.checked_add(1) else {
                lost.push(update.session);
                continue;
            };
            let frame = if frame_index == update.frame_index {
                update.frame
            } else {
                let Ok(frame) = ServerEnvelope::new(None, frame_index, update.frame.body().clone())
                else {
                    lost.push(update.session);
                    continue;
                };
                frame
            };
            let submitted = self.authority.with_current(
                &attached.binding,
                Purpose::Observe(attached.viewer),
                || self.output.submit(&attached.binding, frame),
            );
            if matches!(submitted, Ok(Ok(()))) {
                attached.frame = frame_index;
                attached.revision = update.revision;
                attached.view = update.view;
            } else {
                lost.push(update.session);
            }
        }
        for session in lost {
            self.attached.remove(&session);
        }
    }
}

struct PendingUpdate {
    session: SessionId,
    view: Vec<u8>,
    frame_index: u64,
    revision: u64,
    frame: ServerEnvelope,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AdmissionError {}
