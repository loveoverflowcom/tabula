//! Native opt-in friends HTTP and the single shell lobby socket (ADR-0044).
//! Current session and peer policy guards survive through each bounded handoff.

use std::{
    collections::BTreeMap,
    future::{poll_fn, Future},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Request, State,
    },
    http::{header, HeaderValue, StatusCode, Uri},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use futures::Sink;
use serde::Serialize;
use tabula_lobby::{
    social::{
        GuardedSocial, SocialAuthority, SocialClientMessage, SocialError, SocialMutation,
        SocialPresenceCandidate, SocialServerMessage, SocialSession, SocialSnapshot,
        SOCIAL_CONTRACT_VERSION,
    },
    PresenceTracker,
};
use tabula_session::{
    AccountOperationId, AuthSessionId, BoundedSocketFrame, SessionBinding, SessionChannel,
    SessionPublication, SocketFramePublication, MAX_SOCKET_FRAME_BYTES,
};
use tokio::sync::{watch, Notify};

use crate::{
    isolated::{guarded_json, IsolatedSessionHttp},
    PublicProblem,
};

const INBOUND_LIMIT: usize = 32 * 1024;
const OUTBOUND_LIMIT: usize = MAX_SOCKET_FRAME_BYTES;
const SOCKET_CAPACITY: usize = 128;
const USER_SOCKET_CAPACITY: usize = 4;
const PROTOCOL: &str = "tabula-social.v2.json";
const TRANSPORT_FRESHNESS: Duration = Duration::from_secs(5);

struct TransportObservation {
    at: Instant,
    wall_ms: u64,
}
impl TransportObservation {
    fn fresh(&self, now: Instant) -> bool {
        now.checked_duration_since(self.at)
            .is_some_and(|elapsed| elapsed < TRANSPORT_FRESHNESS)
    }
}
struct LiveConnection {
    binding: SessionBinding,
    observation: TransportObservation,
    stale_announced: bool,
}
struct PeerChallenge {
    nonce: String,
    sent: Instant,
}
impl PeerChallenge {
    fn matches(&self, pong: &[u8], now: Instant) -> bool {
        pong == self.nonce.as_bytes()
            && now
                .checked_duration_since(self.sent)
                .is_some_and(|elapsed| elapsed < TRANSPORT_FRESHNESS)
    }
}

struct HubData {
    tracker: PresenceTracker,
    bindings: BTreeMap<u64, LiveConnection>,
    window: Instant,
    requests: u16,
    sessions: BTreeMap<(AuthSessionId, u8), (Instant, u16)>,
    revision: u64,
}
struct PresenceHub {
    data: Mutex<HubData>,
    wake: watch::Sender<u64>,
    next_connection: AtomicU64,
}
impl PresenceHub {
    fn new() -> Arc<Self> {
        let (wake, _) = watch::channel(0);
        Arc::new(Self {
            data: Mutex::new(HubData {
                tracker: PresenceTracker::default(),
                bindings: BTreeMap::new(),
                window: Instant::now(),
                requests: 0,
                sessions: BTreeMap::new(),
                revision: 0,
            }),
            wake,
            next_connection: AtomicU64::new(1),
        })
    }
    fn global_budget(&self) -> bool {
        let Ok(mut data) = self.data.lock() else {
            return false;
        };
        if data.window.elapsed() >= Duration::from_secs(60) {
            data.window = Instant::now();
            data.requests = 0;
        }
        if data.requests >= 240 {
            return false;
        }
        data.requests += 1;
        true
    }
    fn session_budget(&self, session: AuthSessionId, kind: u8, limit: u16) -> bool {
        let Ok(mut data) = self.data.lock() else {
            return false;
        };
        data.sessions
            .retain(|_, (started, _)| started.elapsed() < Duration::from_secs(60));
        if !data.sessions.contains_key(&(session, kind))
            && data.sessions.len() >= SOCKET_CAPACITY * 3
        {
            return false;
        }
        let (_, count) = data
            .sessions
            .entry((session, kind))
            .or_insert((Instant::now(), 0));
        if *count >= limit {
            return false;
        }
        *count += 1;
        true
    }
    fn bindings(&self) -> Option<Vec<SocialPresenceCandidate>> {
        self.data.lock().ok().map(|data| {
            let now = Instant::now();
            data.bindings
                .values()
                .map(|live| SocialPresenceCandidate {
                    binding: live.binding,
                    stale_as_of_ms: (!live.observation.fresh(now))
                        .then_some(live.observation.wall_ms),
                    fresh_until: live
                        .observation
                        .fresh(now)
                        .then_some(live.observation.at + TRANSPORT_FRESHNESS),
                })
                .collect()
        })
    }
    fn fresh(&self, connection: u64) -> bool {
        self.data.lock().ok().is_some_and(|data| {
            data.bindings
                .get(&connection)
                .is_some_and(|live| live.observation.fresh(Instant::now()))
        })
    }
    fn observed_pong(&self, connection: u64) -> bool {
        let Ok(mut data) = self.data.lock() else {
            return false;
        };
        let Some(live) = data.bindings.get_mut(&connection) else {
            return false;
        };
        let now = Instant::now();
        if !live.observation.fresh(now) {
            return false;
        }
        live.observation = TransportObservation {
            at: now,
            wall_ms: now_ms(),
        };
        true
    }
    fn expire(data: &mut HubData) {
        let now = Instant::now();
        let mut changed = false;
        for live in data.bindings.values_mut() {
            if !live.stale_announced && !live.observation.fresh(now) {
                live.stale_announced = true;
                changed = true;
            }
        }
        if changed {
            data.revision = data.revision.saturating_add(1);
        }
    }
    fn attach(&self, binding: SessionBinding) -> Option<u64> {
        let mut data = self.data.lock().ok()?;
        if data.bindings.len() >= SOCKET_CAPACITY
            || data
                .bindings
                .values()
                .filter(|other| other.binding.user_id() == binding.user_id())
                .count()
                >= USER_SOCKET_CAPACITY
        {
            return None;
        }
        let connection = self.next_connection.fetch_add(1, Ordering::Relaxed);
        if connection == 0 {
            return None;
        }
        data.bindings.insert(
            connection,
            LiveConnection {
                binding,
                observation: TransportObservation {
                    at: Instant::now(),
                    wall_ms: now_ms(),
                },
                stale_announced: false,
            },
        );
        data.tracker.attach(connection, binding.user_id());
        data.revision = data.revision.saturating_add(1);
        Some(connection)
    }
    fn detach(&self, connection: u64) {
        if let Ok(mut data) = self.data.lock() {
            if data.bindings.remove(&connection).is_some() {
                data.tracker.detach(connection, now_ms());
                data.revision = data.revision.saturating_add(1);
            }
        }
    }
}

struct SocialState<A> {
    http: IsolatedSessionHttp<A>,
    hub: Arc<PresenceHub>,
    shutdown: Option<watch::Receiver<bool>>,
    lifecycle: Arc<SocketLifecycle>,
}
impl<A> Clone for SocialState<A> {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            hub: self.hub.clone(),
            shutdown: self.shutdown.clone(),
            lifecycle: self.lifecycle.clone(),
        }
    }
}
struct AttachedSocket {
    hub: Arc<PresenceHub>,
    connection: u64,
}
impl Drop for AttachedSocket {
    fn drop(&mut self) {
        self.hub.detach(self.connection);
    }
}

#[derive(Default)]
struct SocketTasks {
    stopping: bool,
    active: usize,
    failed: bool,
}
#[derive(Default)]
struct SocketLifecycle {
    tasks: Mutex<SocketTasks>,
    finished: Notify,
}
impl SocketLifecycle {
    fn register(self: &Arc<Self>) -> Option<SocketTask> {
        let mut tasks = self.tasks.lock().ok()?;
        if tasks.stopping || tasks.active >= SOCKET_CAPACITY {
            return None;
        }
        tasks.active += 1;
        Some(SocketTask {
            lifecycle: self.clone(),
            started: false,
            completed: false,
        })
    }
    fn stop(&self) -> bool {
        let Ok(mut tasks) = self.tasks.lock() else {
            return false;
        };
        tasks.stopping = true;
        true
    }
    async fn wait_idle(&self) -> bool {
        loop {
            // Register the wake before observing the count, so final completion
            // cannot fall between the observation and an unregistered wait.
            let finished = self.finished.notified();
            tokio::pin!(finished);
            finished.as_mut().enable();
            match self.tasks.lock() {
                Ok(tasks) if tasks.active == 0 => return !tasks.failed,
                Ok(_) => {}
                Err(_) => return false,
            }
            finished.await;
        }
    }
}
struct SocketTask {
    lifecycle: Arc<SocketLifecycle>,
    started: bool,
    completed: bool,
}
impl Drop for SocketTask {
    fn drop(&mut self) {
        if let Ok(mut tasks) = self.lifecycle.tasks.lock() {
            tasks.active = tasks.active.saturating_sub(1);
            tasks.failed |= self.started && !self.completed;
            if tasks.active == 0 {
                self.lifecycle.finished.notify_waiters();
            }
        }
    }
}

/// Completion for the existing router's ticker and upgraded socket tasks.
///
/// The caller first sends `true` on its shutdown watch, then waits within the
/// process drain budget. HTTP graceful shutdown alone does not own upgrades.
pub struct SocialShutdown {
    lifecycle: Arc<SocketLifecycle>,
    ticker: tokio::task::JoinHandle<Result<(), ()>>,
}
impl std::fmt::Debug for SocialShutdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SocialShutdown([REDACTED])")
    }
}
impl SocialShutdown {
    /// Rejects new socket admission and waits for actual detach and ticker exit.
    /// Timeout, a poisoned tracker or ticker failure never reports completion.
    pub async fn finish(mut self, budget: Duration) -> bool {
        if !self.lifecycle.stop() {
            self.ticker.abort();
            return false;
        }
        let complete = tokio::time::timeout(budget, async {
            self.lifecycle.wait_idle().await && matches!((&mut self.ticker).await, Ok(Ok(())))
        })
        .await
        .unwrap_or(false);
        if !complete {
            self.ticker.abort();
        }
        complete
    }
}

async fn wait_shutdown(shutdown: &mut Option<watch::Receiver<bool>>) {
    let Some(shutdown) = shutdown else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        if *shutdown.borrow_and_update() || shutdown.changed().await.is_err() {
            return;
        }
    }
}
async fn until_shutdown<T>(
    mut shutdown: Option<watch::Receiver<bool>>,
    work: impl Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        biased;
        () = wait_shutdown(&mut shutdown) => None,
        result = work => Some(result),
    }
}

impl<A> IsolatedSessionHttp<A>
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    /// One explicit isolated router and one coalesced in-process presence owner.
    /// Clone this router for a listener; do not create independent owners for one origin.
    pub fn social_router(&self) -> Router {
        self.build_social_router(None).0
    }
    /// The same single presence owner with explicit process shutdown tracking.
    /// Send `true` before waiting on the returned controller (issue #110 PR01).
    pub fn social_router_with_shutdown(
        &self,
        shutdown: watch::Receiver<bool>,
    ) -> (Router, SocialShutdown) {
        self.build_social_router(Some(shutdown))
    }
    fn build_social_router(
        &self,
        shutdown: Option<watch::Receiver<bool>>,
    ) -> (Router, SocialShutdown) {
        let hub = PresenceHub::new();
        let lifecycle = Arc::new(SocketLifecycle::default());
        let weak = Arc::downgrade(&hub);
        let http = self.clone();
        let ticker_shutdown = shutdown.clone();
        let ticker = tokio::spawn(async move {
            until_shutdown(ticker_shutdown, async move {
                let mut ticker = tokio::time::interval(Duration::from_millis(500));
                let mut published = 0;
                loop {
                    ticker.tick().await;
                    let Some(hub) = weak.upgrade() else {
                        break;
                    };
                    let (revision, offline) = match hub.data.lock() {
                        Ok(mut data) => {
                            PresenceHub::expire(&mut data);
                            (data.revision, data.tracker.drain_offline(now_ms()))
                        }
                        Err(_) => return Err(()),
                    };
                    if revision != published {
                        published = revision;
                        let _ = hub.wake.send(revision);
                    }
                    for (user, at) in offline {
                        // A failed metadata write never creates a live presence fact.
                        if http
                            .authority()
                            .social_record_offline(user, at)
                            .await
                            .is_err()
                        {
                            if let Ok(mut data) = hub.data.lock() {
                                data.tracker.retry_offline(user, at);
                            }
                        }
                    }
                }
                Ok(())
            })
            .await
            .unwrap_or(Ok(()))
        });
        let router = Router::new()
            .route("/api/v2/social", get(snapshot::<A>))
            .route("/api/v2/social/search", get(search::<A>))
            .route("/api/v2/social/mutate", post(mutate::<A>))
            .route("/api/v2/lobby/ws", get(upgrade::<A>))
            .with_state(SocialState {
                http: self.clone(),
                hub,
                shutdown,
                lifecycle: lifecycle.clone(),
            })
            .layer(middleware::from_fn(no_store_layer));
        (router, SocialShutdown { lifecycle, ticker })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .unwrap_or(0)
}
fn random_scope() -> Result<String, SocialError> {
    AccountOperationId::generate()
        .map(|id| format!("{:032x}", id.get()))
        .map_err(|_| SocialError::Unavailable)
}
fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}
async fn no_store_layer(request: Request, next: Next) -> Response {
    no_store(next.run(request).await)
}
fn problem(error: SocialError) -> Response {
    let status = match error {
        SocialError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SocialError::InvalidInput => StatusCode::BAD_REQUEST,
        SocialError::Denied => StatusCode::FORBIDDEN,
        SocialError::Conflict => StatusCode::CONFLICT,
        SocialError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        SocialError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    let value = PublicProblem {
        version: 2,
        status: status.as_u16(),
        title: "Request unavailable".to_owned(),
        code: error.to_string(),
    };
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    no_store(response)
}
fn private_response<T: Serialize, P: SessionPublication + 'static>(
    candidate: GuardedSocial<T, P>,
) -> Response {
    if serde_json::to_vec(&candidate.value).map_or(true, |bytes| bytes.len() > OUTBOUND_LIMIT) {
        return problem(SocialError::Unavailable);
    }
    no_store(guarded_json(candidate.publication, &candidate.value))
}

async fn snapshot<A>(State(state): State<SocialState<A>>, request: Request) -> Response
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    if !state.hub.global_budget() {
        return problem(SocialError::RateLimited);
    }
    let (op, _) = match state.http.authenticate_read(request).await {
        Ok(auth) => auth,
        Err(response) => return no_store(response),
    };
    let Some(bindings) = state.hub.bindings() else {
        return problem(SocialError::Unavailable);
    };
    match state
        .http
        .authority()
        .social_snapshot(SocialSession::Credential(op), bindings)
        .await
    {
        Ok(mut candidate) => {
            let scope = match random_scope() {
                Ok(scope) => scope,
                Err(error) => return problem(error),
            };
            candidate.value.scope_id = scope;
            if candidate.value.validate().is_err() {
                return problem(SocialError::Unavailable);
            }
            private_response(candidate)
        }
        Err(error) => problem(error),
    }
}

async fn search<A>(State(state): State<SocialState<A>>, mut request: Request) -> Response
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    if !state.hub.global_budget() {
        return problem(SocialError::RateLimited);
    }
    let Some(raw) = request.uri().query() else {
        return problem(SocialError::InvalidInput);
    };
    if raw.len() > 512 {
        return problem(SocialError::InvalidInput);
    }
    let pairs: Vec<_> = url::form_urlencoded::parse(raw.as_bytes()).collect();
    if pairs.len() != 1 || pairs[0].0 != "q" {
        return problem(SocialError::InvalidInput);
    }
    let query = pairs[0].1.to_string();
    let path = request.uri().path().to_owned();
    *request.uri_mut() = match path.parse::<Uri>() {
        Ok(uri) => uri,
        Err(_) => return problem(SocialError::InvalidInput),
    };
    let (op, session) = match state.http.authenticate_read(request).await {
        Ok(auth) => auth,
        Err(response) => return no_store(response),
    };
    if !state.hub.session_budget(session.id(), 1, 60) {
        return problem(SocialError::RateLimited);
    }
    match state
        .http
        .authority()
        .social_search(SocialSession::Credential(op), query)
        .await
    {
        Ok(candidate) if candidate.value.validate().is_ok() => private_response(candidate),
        Ok(_) => problem(SocialError::Unavailable),
        Err(error) => problem(error),
    }
}

async fn mutate<A>(State(state): State<SocialState<A>>, request: Request) -> Response
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    if !state.hub.global_budget() {
        return problem(SocialError::RateLimited);
    }
    let (op, session, mutation) = match state
        .http
        .authenticate_json::<SocialMutation>(request, INBOUND_LIMIT)
        .await
    {
        Ok(auth) => auth,
        Err(response) => return no_store(response),
    };
    if !state.hub.session_budget(session.id(), 2, 30) {
        return problem(SocialError::RateLimited);
    }
    match state.http.authority().social_mutate(op, mutation).await {
        Ok(candidate) => {
            state
                .hub
                .wake
                .send_modify(|revision| *revision = revision.saturating_add(1));
            private_response(candidate)
        }
        Err(error) => problem(error),
    }
}

async fn upgrade<A>(
    State(state): State<SocialState<A>>,
    upgrade: WebSocketUpgrade,
    request: Request,
) -> Response
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    if !state.hub.global_budget() {
        return problem(SocialError::RateLimited);
    }
    let origins: Vec<_> = request.headers().get_all(header::ORIGIN).iter().collect();
    let protocols: Vec<_> = request
        .headers()
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .collect();
    if origins.len() != 1
        || origins[0].to_str().ok() != Some(state.http.trusted_origin())
        || protocols.len() != 1
        || protocols[0].to_str().ok() != Some(PROTOCOL)
    {
        return problem(SocialError::Denied);
    }
    let (_, session) = match state.http.authenticate_read(request).await {
        Ok(auth) => auth,
        Err(response) => return no_store(response),
    };
    if session.channel() != SessionChannel::BrowserCookie {
        return problem(SocialError::Denied);
    }
    if !state.hub.session_budget(session.id(), 3, 10) {
        return problem(SocialError::RateLimited);
    }
    let binding = session.binding();
    let Some(task) = state.lifecycle.register() else {
        return problem(SocialError::Unavailable);
    };
    no_store(
        upgrade
            .protocols([PROTOCOL])
            .max_message_size(INBOUND_LIMIT)
            .max_frame_size(INBOUND_LIMIT)
            .on_upgrade(move |socket| tracked_socket(state, socket, binding, task)),
    )
}

async fn tracked_socket<A>(
    state: SocialState<A>,
    mut socket: WebSocket,
    binding: SessionBinding,
    mut task: SocketTask,
) where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    task.started = true;
    let shutdown = state.shutdown.clone();
    if until_shutdown(shutdown, socket_loop(state, &mut socket, binding))
        .await
        .is_none()
    {
        // Canceling the scoped loop drops AttachedSocket through its existing
        // detach path before reporting this upgraded task as complete.
        close(&mut socket, 4411).await;
    }
    task.completed = true;
}

async fn close(socket: &mut WebSocket, code: u16) {
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        socket.send(Message::Close(Some(CloseFrame {
            code,
            reason: "".into(),
        }))),
    )
    .await;
}

/// Wait for transport readiness while the candidate remains unpublished. The
/// storage guard then transfers one bounded owned frame to `start_send` exactly
/// once. A ready transport may write: no post-transfer suppression is claimed.
fn poll_handoff<S, P>(
    sink: &mut S,
    publication: &mut P,
    frame: &mut Option<BoundedSocketFrame>,
    context: &mut std::task::Context<'_>,
) -> std::task::Poll<Result<(), SocialError>>
where
    S: Sink<Message> + Unpin,
    P: SocketFramePublication,
{
    match Pin::new(&mut *sink).poll_ready(context) {
        std::task::Poll::Pending => return std::task::Poll::Pending,
        std::task::Poll::Ready(Err(_)) => {
            return std::task::Poll::Ready(Err(SocialError::Unavailable))
        }
        std::task::Poll::Ready(Ok(())) => {}
    }
    let Some(frame) = frame.take() else {
        return std::task::Poll::Ready(Err(SocialError::Unavailable));
    };
    std::task::Poll::Ready(
        publication
            .handoff(frame, |frame| {
                Pin::new(&mut *sink).start_send(Message::Text(frame.into_text().into()))
            })
            .map_err(|error| {
                if error == tabula_session::SessionError::Unauthenticated {
                    SocialError::Unauthenticated
                } else {
                    SocialError::Unavailable
                }
            })
            .and_then(|result| result.map_err(|_| SocialError::Unavailable)),
    )
}

async fn send_snapshot<P: SocketFramePublication>(
    socket: &mut WebSocket,
    mut candidate: GuardedSocial<SocialSnapshot, P>,
) -> Result<(), SocialError> {
    candidate
        .value
        .validate()
        .map_err(|_| SocialError::Unavailable)?;
    let encoded = serde_json::to_string(&SocialServerMessage::Snapshot {
        snapshot: candidate.value,
    })
    .map_err(|_| SocialError::Unavailable)?;
    let mut frame = Some(BoundedSocketFrame::new(encoded).map_err(|_| SocialError::Unavailable)?);
    let result =
        poll_fn(|context| poll_handoff(socket, &mut candidate.publication, &mut frame, context));
    tokio::time::timeout(Duration::from_secs(2), result)
        .await
        .map_err(|_| SocialError::Unavailable)??;
    tokio::time::timeout(
        Duration::from_secs(2),
        poll_fn(|context| Pin::new(&mut *socket).poll_flush(context)),
    )
    .await
    .map_err(|_| SocialError::Unavailable)?
    .map_err(|_| SocialError::Unavailable)
}

// One loop owns the scoped stream, input budget and fail-closed lifetime.
#[allow(clippy::too_many_lines)]
async fn socket_loop<A>(state: SocialState<A>, socket: &mut WebSocket, binding: SessionBinding)
where
    A: SocialAuthority + Send + Sync + 'static,
    A::SocialPublication: 'static,
{
    let hello = tokio::time::timeout(Duration::from_secs(5), socket.recv()).await;
    let valid = match hello {
        Ok(Some(Ok(Message::Text(text)))) if text.len() <= INBOUND_LIMIT => matches!(
            serde_json::from_str::<SocialClientMessage>(&text),
            Ok(SocialClientMessage::Hello {
                version: SOCIAL_CONTRACT_VERSION
            })
        ),
        _ => false,
    };
    if !valid {
        close(socket, 4400).await;
        return;
    }
    // Revalidate the historical upgrade binding before counting an attachment.
    if let Err(error) = state.http.authority().observe_binding(binding).await {
        close(
            socket,
            if error == tabula_session::SessionError::Unauthenticated {
                4401
            } else {
                1011
            },
        )
        .await;
        return;
    }
    let Some(connection) = state.hub.attach(binding) else {
        close(socket, 4429).await;
        return;
    };
    let _attached = AttachedSocket {
        hub: state.hub.clone(),
        connection,
    };
    let mut wake = state.hub.wake.subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    heartbeat.tick().await;
    let Ok(mut scope) = random_scope() else {
        close(socket, 1011).await;
        return;
    };
    let mut revision = 0_u64;
    let mut incoming_window = Instant::now();
    let mut incoming = 0_u16;
    let mut challenge: Option<PeerChallenge> = None;
    loop {
        if !state.hub.fresh(connection) {
            close(socket, 1001).await;
            return;
        }
        if challenge.is_none() {
            let Ok(nonce) = random_scope() else {
                close(socket, 1011).await;
                return;
            };
            challenge = Some(PeerChallenge {
                nonce,
                sent: Instant::now(),
            });
        }
        let challenge_bytes = challenge
            .as_ref()
            .expect("challenge was installed")
            .nonce
            .as_bytes()
            .to_vec();
        if !matches!(
            tokio::time::timeout(
                Duration::from_secs(1),
                socket.send(Message::Ping(challenge_bytes.into()))
            )
            .await,
            Ok(Ok(()))
        ) {
            close(socket, 1011).await;
            return;
        }
        let Some(bindings) = state.hub.bindings() else {
            close(socket, 1011).await;
            return;
        };
        let mut candidate = match state
            .http
            .authority()
            .social_snapshot(SocialSession::Connection(binding), bindings)
            .await
        {
            Ok(candidate) => candidate,
            Err(SocialError::Unauthenticated) => {
                close(socket, 4401).await;
                return;
            }
            Err(_) => {
                close(socket, 1011).await;
                return;
            }
        };
        let Some(next_revision) = revision.checked_add(1) else {
            close(socket, 1011).await;
            return;
        };
        revision = next_revision;
        candidate.value.scope_id.clone_from(&scope);
        candidate.value.revision = revision;
        if let Err(error) = send_snapshot(socket, candidate).await {
            close(
                socket,
                if error == SocialError::Unauthenticated {
                    4401
                } else {
                    1011
                },
            )
            .await;
            return;
        }
        loop {
            tokio::select! {
                _ = heartbeat.tick() => break,
                changed = wake.changed() => { if changed.is_err() { close(socket,1011).await; return; } break; },
                message = socket.recv() => {
                    if incoming_window.elapsed() >= Duration::from_secs(60) { incoming_window = Instant::now(); incoming = 0; }
                    incoming = incoming.saturating_add(1);
                    if incoming > 120 { close(socket,4429).await; return; }
                    match message {
                        Some(Ok(Message::Text(text))) if text.len() <= INBOUND_LIMIT => {
                            let Ok(SocialClientMessage::Resync) = serde_json::from_str::<SocialClientMessage>(&text) else { close(socket,4400).await; return; };
                            let Ok(new_scope) = random_scope() else { close(socket,1011).await; return; };
                            scope = new_scope; revision = 0; break;
                        },
                        Some(Ok(Message::Pong(pong))) => {
                            if challenge.as_ref().is_some_and(|challenge| challenge.matches(&pong, Instant::now())) && state.hub.observed_pong(connection) { challenge = None; }
                        },
                        Some(Ok(Message::Ping(_))) => {},
                        Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                        _ => { close(socket,4400).await; return; },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        convert::Infallible,
        task::{Context, Poll},
    };

    #[tokio::test]
    async fn shutdown_cancels_the_scoped_socket_work_and_detaches_before_completion() {
        let hub = PresenceHub::new();
        let binding = Publication::new().snapshot.binding();
        let connection = hub.attach(binding).unwrap();
        let attached = AttachedSocket {
            hub: hub.clone(),
            connection,
        };
        let lifecycle = Arc::new(SocketLifecycle::default());
        let mut task = lifecycle.register().unwrap();
        let (stop, shutdown) = watch::channel(false);
        let ticker_shutdown = shutdown.clone();
        let ticker = tokio::spawn(async move {
            until_shutdown(Some(ticker_shutdown), std::future::pending::<()>()).await;
            Ok(())
        });
        let socket = tokio::spawn(async move {
            task.started = true;
            let work = async move {
                let _attached = attached;
                std::future::pending::<()>().await;
            };
            assert!(until_shutdown(Some(shutdown), work).await.is_none());
            task.completed = true;
            drop(task);
        });
        assert_eq!(hub.bindings().unwrap().len(), 1);
        stop.send(true).unwrap();
        assert!(
            SocialShutdown {
                lifecycle: lifecycle.clone(),
                ticker,
            }
            .finish(Duration::from_secs(1))
            .await
        );
        socket.await.unwrap();
        assert!(hub.bindings().unwrap().is_empty());
        assert!(
            lifecycle.register().is_none(),
            "drain closes upgraded admission"
        );
    }

    #[tokio::test]
    async fn shutdown_does_not_report_success_for_lost_work_or_failed_ticker() {
        let lifecycle = Arc::new(SocketLifecycle::default());
        let mut task = lifecycle.register().unwrap();
        task.started = true;
        drop(task); // A started callback disappearing before its normal exit.
        let ticker = tokio::spawn(async { Ok(()) });
        assert!(
            !SocialShutdown { lifecycle, ticker }
                .finish(Duration::from_secs(1))
                .await
        );

        let lifecycle = Arc::new(SocketLifecycle::default());
        let ticker = tokio::spawn(async { Err(()) });
        assert!(
            !SocialShutdown { lifecycle, ticker }
                .finish(Duration::from_secs(1))
                .await
        );

        let lifecycle = Arc::new(SocketLifecycle::default());
        let task = lifecycle.register().unwrap();
        let ticker = tokio::spawn(async { Ok(()) });
        assert!(
            !SocialShutdown { lifecycle, ticker }
                .finish(Duration::from_millis(1))
                .await
        );
        drop(task);
    }

    #[tokio::test]
    async fn shutdown_is_observed_before_work_and_channel_loss_also_stops() {
        let (stop, shutdown) = watch::channel(true);
        let ran = std::sync::atomic::AtomicBool::new(false);
        assert!(until_shutdown(Some(shutdown), async {
            ran.store(true, Ordering::Release);
        })
        .await
        .is_none());
        assert!(!ran.load(Ordering::Acquire));
        drop(stop);

        let (stop, shutdown) = watch::channel(false);
        drop(stop);
        assert!(until_shutdown(Some(shutdown), std::future::pending::<()>())
            .await
            .is_none());
    }

    #[test]
    fn transport_presence_expires_without_exact_fresh_pong() {
        let now = Instant::now();
        let observation = TransportObservation {
            at: now,
            wall_ms: 1_000,
        };
        let challenge = PeerChallenge {
            nonce: "current-connection-challenge".into(),
            sent: now,
        };
        assert!(observation.fresh(now));
        assert!(observation.fresh(now + Duration::from_millis(4_999)));
        assert!(!observation.fresh(now + Duration::from_secs(5)));
        assert!(!observation.fresh(now + Duration::from_secs(60)));
        assert!(!challenge.matches(b"old-connection-challenge", now + Duration::from_secs(1)));
        assert!(!challenge.matches(b"", now + Duration::from_secs(1)));
        assert!(challenge.matches(
            b"current-connection-challenge",
            now + Duration::from_millis(4_999)
        ));
        assert!(!challenge.matches(
            b"current-connection-challenge",
            now + Duration::from_secs(5)
        ));
        // Sending heartbeats or receiving an unrelated reply never renews the
        // last actual positive observation; stale transport is not Online.
        assert!(!observation.fresh(now + Duration::from_secs(6)));
    }
    use tabula_core::UserId;
    use tabula_session::{
        AccountEpoch, AuthSessionId, CredentialDigest, SessionContextId, SessionError,
        SessionRecord, SessionSnapshot, UnixMillis,
    };

    struct ReadySink {
        ready: bool,
        sent: usize,
    }
    impl Sink<Message> for ReadySink {
        type Error = Infallible;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.ready {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
        fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
            self.get_mut().sent += 1;
            Ok(())
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }
    struct Publication {
        live: bool,
        used: bool,
        snapshot: SessionSnapshot,
    }
    impl Publication {
        fn new() -> Self {
            Self {
                live: true,
                used: false,
                snapshot: SessionRecord::issue(
                    AuthSessionId::new(1).unwrap(),
                    UserId(1),
                    AccountEpoch::new(0).unwrap(),
                    SessionChannel::BrowserCookie,
                    CredentialDigest::from_bytes([1; 32]),
                    SessionContextId::new(1).unwrap(),
                    UnixMillis::new(100).unwrap(),
                )
                .unwrap()
                .snapshot(),
            }
        }
    }
    impl SessionPublication for Publication {
        fn snapshot(&self) -> &SessionSnapshot {
            &self.snapshot
        }
        fn publish<R>(
            &mut self,
            build: impl FnOnce(&SessionSnapshot) -> R,
        ) -> Result<R, SessionError> {
            if !self.live || self.used {
                return Err(SessionError::Unauthenticated);
            }
            self.used = true;
            Ok(build(&self.snapshot))
        }
    }
    impl SocketFramePublication for Publication {
        fn handoff<R>(
            &mut self,
            frame: BoundedSocketFrame,
            transport: impl FnOnce(BoundedSocketFrame) -> R,
        ) -> Result<R, SessionError> {
            if !self.live || self.used {
                return Err(SessionError::Unauthenticated);
            }
            self.used = true;
            Ok(transport(frame))
        }
    }

    #[test]
    fn backpressure_retains_candidate_and_expiry_never_calls_transport() {
        let mut sink = ReadySink {
            ready: false,
            sent: 0,
        };
        let mut publication = Publication::new();
        let mut frame = Some(BoundedSocketFrame::new("{}".into()).unwrap());
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(poll_handoff(&mut sink, &mut publication, &mut frame, &mut context).is_pending());
        assert!(frame.is_some());
        assert!(!publication.used);
        assert_eq!(sink.sent, 0);
        publication.live = false;
        sink.ready = true;
        assert_eq!(
            poll_handoff(&mut sink, &mut publication, &mut frame, &mut context),
            Poll::Ready(Err(SocialError::Unauthenticated))
        );
        assert_eq!(sink.sent, 0);
        assert!(!publication.used);
    }

    #[test]
    fn ready_transport_receives_one_owned_frame_only() {
        let mut sink = ReadySink {
            ready: true,
            sent: 0,
        };
        let mut publication = Publication::new();
        let mut frame = Some(BoundedSocketFrame::new("{}".into()).unwrap());
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert_eq!(
            poll_handoff(&mut sink, &mut publication, &mut frame, &mut context),
            Poll::Ready(Ok(()))
        );
        assert_eq!(sink.sent, 1);
        assert!(frame.is_none());
        assert!(publication.used);
        frame = Some(BoundedSocketFrame::new("{}".into()).unwrap());
        assert_eq!(
            poll_handoff(&mut sink, &mut publication, &mut frame, &mut context),
            Poll::Ready(Err(SocialError::Unauthenticated))
        );
        assert_eq!(sink.sent, 1);
    }

    #[test]
    fn flat_mutations_accept_only_explicit_resource_fields() {
        let request = r#"{"operation_id":"00000000000000000000000000000001","action":"send","target_user_id":"00000000000000000000000000000002"}"#;
        let mutation: SocialMutation =
            serde_json::from_str(request).expect("documented Send must decode");
        mutation.validate().unwrap();
        let forged = r#"{"operation_id":"00000000000000000000000000000001","action":"send","target_user_id":"00000000000000000000000000000002","actor_id":"00000000000000000000000000000003"}"#;
        assert!(serde_json::from_str::<SocialMutation>(forged).is_err());
        let accept = r#"{"operation_id":"00000000000000000000000000000001","action":"accept","request_id":"00000000000000000000000000000002","expected_revision":1}"#;
        serde_json::from_str::<SocialMutation>(accept)
            .unwrap()
            .validate()
            .unwrap();
    }
}
