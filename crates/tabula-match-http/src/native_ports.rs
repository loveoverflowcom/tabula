//! Storage-backed current actor authority and bounded server-private queues.
use super::{now_ms, problem, unavailable, Response, StatusCode};
use crate::{MAX_BUFFERED_FRAMES, MAX_RESPONSE_BYTES};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tabula_core::{InputIndex, MatchId, SeatId, SessionId};
use tabula_game_api::Effect;
use tabula_match::{
    durable::{Journal, JournalRecord, LoadedMatch, OperationScope, RuntimePortError, ScopeState},
    ports::Clock,
    runtime::{Binding, MatchHandle},
    runtime_ports::{Authority, AuthorityLost, Effects, Output, Purpose},
};
use tabula_protocol::{encode_server, Codec, ServerEnvelope};
use tabula_session::{
    CredentialOperation, HttpSessionAuthority, SessionPublication, SessionSnapshot,
};
use tabula_storage::{
    match_postgres::PgMatchJournal,
    online_match::{PgOnlineMatchStore, PgOnlineOperation},
    session::{PgSessionPublication, PgSessionStore},
};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

pub(super) struct LiveMatch {
    pub handle: MatchHandle,
    pub journal: Arc<NetworkJournal>,
    pub authority: Arc<NetworkAuthority>,
    pub output: Arc<QueueOutput>,
    pub gate: AsyncMutex<()>,
}
pub(super) struct NetworkClock {
    start: Instant,
}
impl NetworkClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}
impl Clock for NetworkClock {
    fn now_unix_ms(&self) -> u64 {
        now_ms().unwrap_or(0)
    }
    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}
pub(super) struct ClosedEffects;
impl Effects for ClosedEffects {
    async fn execute(
        &self,
        _id: MatchId,
        _index: InputIndex,
        effects: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        if effects.iter().all(|e| matches!(e, Effect::EndMatch { .. })) {
            Ok(())
        } else {
            Err(RuntimePortError::Unavailable)
        }
    }
}
pub(super) struct NetworkJournal {
    pub journal: PgMatchJournal,
    active: Mutex<Option<PgOnlineOperation>>,
    initialized: Mutex<Option<oneshot::Sender<Result<(), RuntimePortError>>>>,
}
impl NetworkJournal {
    pub fn new(
        journal: PgMatchJournal,
        guard: PgOnlineOperation,
        ready: oneshot::Sender<Result<(), RuntimePortError>>,
    ) -> Self {
        Self {
            journal,
            active: Mutex::new(Some(guard)),
            initialized: Mutex::new(Some(ready)),
        }
    }
    fn active(&self) -> Result<PgOnlineOperation, RuntimePortError> {
        self.active
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)?
            .clone()
            .ok_or(RuntimePortError::Unavailable)
    }
    pub fn set(&self, guard: Option<PgOnlineOperation>) -> Result<(), RuntimePortError> {
        *self
            .active
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)? = guard;
        Ok(())
    }
}
/// Cancellation retires a pending permit; a begun commit is never blindly retried.
pub(super) struct ActiveRequest {
    journal: Arc<NetworkJournal>,
}
impl ActiveRequest {
    pub fn new(
        journal: Arc<NetworkJournal>,
        guard: PgOnlineOperation,
    ) -> Result<Self, RuntimePortError> {
        journal.set(Some(guard))?;
        Ok(Self { journal })
    }
}
impl Drop for ActiveRequest {
    fn drop(&mut self) {
        let _ = self.journal.set(None);
    }
}
impl Journal for NetworkJournal {
    async fn append(&self, record: JournalRecord) -> Result<(), RuntimePortError> {
        let genesis = record.creation.is_some();
        let result = self
            .journal
            .append_authenticated(record, &self.active()?)
            .await;
        if genesis {
            if let Ok(mut ready) = self.initialized.lock() {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(result);
                }
            }
        }
        result
    }
    async fn update_ledger(
        &self,
        id: MatchId,
        version: tabula_core::StateVersion,
        observed: u64,
        ledger: Vec<ScopeState>,
    ) -> Result<(), RuntimePortError> {
        self.journal
            .update_ledger_authenticated(id, version, observed, ledger, &self.active()?)
            .await
    }
    async fn load(&self, id: MatchId) -> Result<LoadedMatch, RuntimePortError> {
        self.journal.load(id).await
    }
}
#[derive(Clone)]
pub(super) struct AuthorizedAttachment {
    pub binding: Binding,
    pub credential: CredentialOperation,
    pub scope: OperationScope,
}
struct PreparedOutput {
    binding: Binding,
    purpose: Purpose,
    publication: PgSessionPublication,
}
pub(super) struct NetworkAuthority {
    id: MatchId,
    online: PgOnlineMatchStore,
    sessions: PgSessionStore,
    journal: Arc<NetworkJournal>,
    output: Arc<QueueOutput>,
    attachments: Mutex<BTreeMap<SessionId, AuthorizedAttachment>>,
    prepared: Mutex<BTreeMap<SessionId, PreparedOutput>>,
}
impl NetworkAuthority {
    pub fn new(
        id: MatchId,
        online: PgOnlineMatchStore,
        sessions: PgSessionStore,
        journal: Arc<NetworkJournal>,
        output: Arc<QueueOutput>,
    ) -> Self {
        Self {
            id,
            online,
            sessions,
            journal,
            output,
            attachments: Mutex::new(BTreeMap::new()),
            prepared: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn insert(&self, e: AuthorizedAttachment) -> Result<(), AuthorityLost> {
        let mut a = self.attachments.lock().map_err(|_| AuthorityLost)?;
        if a.len() >= 32 {
            return Err(AuthorityLost);
        }
        a.insert(e.binding.session(), e);
        Ok(())
    }
    pub fn for_seat(&self, seat: SeatId) -> Result<Vec<Binding>, AuthorityLost> {
        Ok(self
            .attachments
            .lock()
            .map_err(|_| AuthorityLost)?
            .values()
            .filter(|e| e.scope.seat == seat)
            .map(|e| e.binding.clone())
            .collect())
    }
    fn retire(&self, binding: &Binding) {
        self.remove(binding);
        self.output.remove(binding);
    }
    pub fn close(&self) {
        if let Ok(mut a) = self.attachments.lock() {
            a.clear();
        }
        if let Ok(mut a) = self.prepared.lock() {
            a.clear();
        }
        self.output.close();
    }
    pub fn remove(&self, b: &Binding) {
        if let Ok(mut e) = self.attachments.lock() {
            e.remove(&b.session());
        }
        if let Ok(mut e) = self.prepared.lock() {
            e.remove(&b.session());
        }
    }
    pub fn refresh(
        &self,
        session: SessionId,
        scope: OperationScope,
        credential: CredentialOperation,
    ) -> Result<Binding, AuthorityLost> {
        let mut all = self.attachments.lock().map_err(|_| AuthorityLost)?;
        let e = all
            .get_mut(&session)
            .filter(|e| e.scope == scope)
            .ok_or(AuthorityLost)?;
        e.credential = credential;
        Ok(e.binding.clone())
    }
    fn checked(&self, b: &Binding, p: Purpose) -> Result<AuthorizedAttachment, AuthorityLost> {
        let e = self
            .attachments
            .lock()
            .map_err(|_| AuthorityLost)?
            .get(&b.session())
            .filter(|e| e.binding == *b)
            .cloned()
            .ok_or(AuthorityLost)?;
        let (Purpose::Apply(seat)
        | Purpose::Observe(tabula_registry::ClientViewer::Seat(seat))
        | Purpose::Receipt(tabula_registry::ClientViewer::Seat(seat))) = p
        else {
            return Err(AuthorityLost);
        };
        if e.scope.seat != seat {
            return Err(AuthorityLost);
        }
        Ok(e)
    }
}
impl Authority for NetworkAuthority {
    async fn prepare(&self, b: &Binding, p: Purpose) -> Result<(), AuthorityLost> {
        let result = async {
            let e = self.checked(b, p)?;
            if let Ok(active) = self.journal.active() {
                active.cancel().await.map_err(|_| AuthorityLost)?;
            }
            let m = self
                .online
                .resolve(e.credential, self.id)
                .await
                .map_err(|_| AuthorityLost)?;
            if m.scope() != e.scope {
                return Err(AuthorityLost);
            }
            let publication = self
                .sessions
                .begin_publication(e.credential)
                .await
                .map_err(|_| AuthorityLost)?;
            if !snapshot_matches(publication.snapshot(), b) {
                return Err(AuthorityLost);
            }
            let mut all = self.prepared.lock().map_err(|_| AuthorityLost)?;
            if all.len() >= 32 && !all.contains_key(&b.session()) {
                return Err(AuthorityLost);
            }
            all.insert(
                b.session(),
                PreparedOutput {
                    binding: b.clone(),
                    purpose: p,
                    publication,
                },
            );
            Ok(())
        }
        .await;
        if result.is_err() {
            self.retire(b);
        }
        result
    }
    fn with_current<T>(
        &self,
        b: &Binding,
        p: Purpose,
        action: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost> {
        let result = (|| {
            let e = self.checked(b, p)?;
            if !matches!(p, Purpose::Apply(_)) {
                let prepared = self
                    .prepared
                    .lock()
                    .map_err(|_| AuthorityLost)?
                    .remove(&b.session());
                if let Some(mut permit) = prepared {
                    if permit.binding != *b || permit.purpose != p {
                        return Err(AuthorityLost);
                    }
                    return submit_prepared(&mut permit.publication, &self.output, b, action);
                }
            }
            let guard = self.journal.active().map_err(|_| AuthorityLost)?;
            if guard.scope() != e.scope {
                return Err(AuthorityLost);
            }
            guard.with_current(|_| action()).map_err(|_| AuthorityLost)
        })();
        if result.is_err() {
            self.retire(b);
        }
        result
    }
}
fn submit_prepared<P: SessionPublication, T>(
    publication: &mut P,
    output: &QueueOutput,
    binding: &Binding,
    action: impl FnOnce() -> T,
) -> Result<T, AuthorityLost> {
    let stage = output.begin_stage(binding).map_err(|_| AuthorityLost)?;
    let result = publication
        .publish(|_| action())
        .map_err(|_| AuthorityLost)?;
    stage.commit().map_err(|_| AuthorityLost)?;
    Ok(result)
}
pub(super) fn snapshot_matches(s: &SessionSnapshot, b: &Binding) -> bool {
    s.id().get() == b.record()
        && s.user_id() == b.subject()
        && s.authorization_epoch().get() == b.epoch()
}
struct QueuedAttachment {
    binding: Binding,
    frames: VecDeque<ServerEnvelope>,
    bytes: usize,
    overflow: bool,
    staged: Option<StagedFrames>,
}
#[derive(Default)]
struct StagedFrames {
    frames: VecDeque<ServerEnvelope>,
    bytes: usize,
}
struct QueueStage<'a> {
    output: &'a QueueOutput,
    binding: Binding,
    finished: bool,
}
impl QueueStage<'_> {
    fn commit(mut self) -> Result<(), RuntimePortError> {
        let mut all = self
            .output
            .entries
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)?;
        let entry = all
            .get_mut(&self.binding.session())
            .filter(|e| e.binding == self.binding && !e.overflow)
            .ok_or(RuntimePortError::Unavailable)?;
        let mut staged = entry.staged.take().ok_or(RuntimePortError::Unavailable)?;
        entry.bytes = entry
            .bytes
            .checked_add(staged.bytes)
            .ok_or(RuntimePortError::Unavailable)?;
        entry.frames.append(&mut staged.frames);
        self.finished = true;
        Ok(())
    }
}
impl Drop for QueueStage<'_> {
    fn drop(&mut self) {
        if !self.finished {
            if let Ok(mut entries) = self.output.entries.lock() {
                if let Some(entry) = entries
                    .get_mut(&self.binding.session())
                    .filter(|e| e.binding == self.binding)
                {
                    entry.staged.take();
                }
            }
        }
    }
}
#[derive(Default)]
pub(super) struct QueueOutput {
    entries: Mutex<BTreeMap<SessionId, QueuedAttachment>>,
    seat_rates: Mutex<BTreeMap<SeatId, VecDeque<Instant>>>,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum QueueError {
    Unavailable,
    ReattachRequired,
    Busy,
}
impl QueueError {
    pub fn response(self) -> Response {
        match self {
            Self::Unavailable => unavailable(),
            Self::ReattachRequired => problem(StatusCode::CONFLICT, "reattach_required"),
            Self::Busy => problem(StatusCode::TOO_MANY_REQUESTS, "busy"),
        }
    }
}
impl QueueOutput {
    fn close(&self) {
        if let Ok(mut all) = self.entries.lock() {
            all.clear();
        }
        if let Ok(mut all) = self.seat_rates.lock() {
            all.clear();
        }
    }
    fn begin_stage(&self, binding: &Binding) -> Result<QueueStage<'_>, RuntimePortError> {
        let mut all = self
            .entries
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)?;
        let entry = all
            .get_mut(&binding.session())
            .filter(|e| e.binding == *binding && !e.overflow)
            .ok_or(RuntimePortError::Unavailable)?;
        if entry.staged.is_some() {
            return Err(RuntimePortError::Busy);
        }
        entry.staged = Some(StagedFrames::default());
        Ok(QueueStage {
            output: self,
            binding: binding.clone(),
            finished: false,
        })
    }
    pub fn active(&self, b: &Binding) -> bool {
        self.entries.lock().ok().is_some_and(|e| {
            e.get(&b.session())
                .is_some_and(|e| e.binding == *b && !e.overflow)
        })
    }
    pub fn insert(&self, binding: Binding) -> Result<(), RuntimePortError> {
        let mut e = self
            .entries
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)?;
        if e.len() >= 32 {
            return Err(RuntimePortError::Busy);
        }
        e.insert(
            binding.session(),
            QueuedAttachment {
                binding,
                frames: VecDeque::new(),
                bytes: 0,
                overflow: false,
                staged: None,
            },
        );
        Ok(())
    }
    pub fn remove(&self, b: &Binding) {
        if let Ok(mut e) = self.entries.lock() {
            e.remove(&b.session());
        }
    }
    pub fn drain(&self, b: &Binding) -> Result<Vec<ServerEnvelope>, QueueError> {
        let mut e = self.entries.lock().map_err(|_| QueueError::Unavailable)?;
        let e = e
            .get_mut(&b.session())
            .filter(|e| e.binding == *b && !e.overflow)
            .ok_or(QueueError::ReattachRequired)?;
        e.bytes = 0;
        Ok(e.frames.drain(..).collect())
    }
    pub fn command_rate(&self, b: &Binding, seat: SeatId) -> Result<(), QueueError> {
        if !self.active(b) {
            return Err(QueueError::ReattachRequired);
        }
        let mut rates = self
            .seat_rates
            .lock()
            .map_err(|_| QueueError::Unavailable)?;
        if !rates.contains_key(&seat) && rates.len() >= 8 {
            return Err(QueueError::Busy);
        }
        let commands = rates.entry(seat).or_default();
        let now = Instant::now();
        while commands
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(1))
        {
            commands.pop_front();
        }
        if commands.len() >= 5 {
            return Err(QueueError::Busy);
        }
        commands.push_back(now);
        Ok(())
    }
}
impl Output for QueueOutput {
    fn submit(&self, b: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError> {
        let size = encode_server(Codec::Json, &frame)
            .map_err(|_| RuntimePortError::Unavailable)?
            .len();
        let mut all = self
            .entries
            .lock()
            .map_err(|_| RuntimePortError::Unavailable)?;
        let entry = all
            .get_mut(&b.session())
            .filter(|e| e.binding == *b)
            .ok_or(RuntimePortError::Unavailable)?;
        let staged = entry.staged.as_ref().ok_or(RuntimePortError::Unavailable)?;
        if entry.overflow
            || entry.frames.len() + staged.frames.len() >= MAX_BUFFERED_FRAMES
            || entry
                .bytes
                .checked_add(staged.bytes)
                .and_then(|n| n.checked_add(size))
                .is_none_or(|n| n > MAX_RESPONSE_BYTES - 1024)
        {
            entry.overflow = true;
            entry.frames.clear();
            entry.bytes = 0;
            entry.staged.take();
            return Err(RuntimePortError::Busy);
        }
        let staged = entry.staged.as_mut().ok_or(RuntimePortError::Unavailable)?;
        staged.bytes += size;
        staged.frames.push_back(frame);
        Ok(())
    }
}
