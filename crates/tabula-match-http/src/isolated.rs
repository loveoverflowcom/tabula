//! Native opt-in HTTPS polling composition (ADR-0041; doc03 §4/7/21).
//! Constructors open no listener and perform no migration/provider operation.
use crate::{
    parse_match_id, MatchAdmission, MatchAttachRequest, MatchAttachment, MatchCommandRequest,
    MatchCreateRequest, MatchFrames, MatchGrant, MatchGrantRequest, MatchJoinRequest,
    MatchPollRequest, MAX_REQUEST_BYTES,
};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::post,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tabula_core::{GameId, MatchId, MatchSeed, SessionId};
use tabula_match::{
    durable::Journal,
    runtime::{self, Binding, Completion, Ports},
};
use tabula_registry::{ConfigDraft, ErasedGame};
#[cfg(feature = "acceptance-test-support")]
use tabula_session::AuthSessionId;
use tabula_session::{CredentialOperation, SessionCredential, SessionError};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::{
    match_postgres::PgMatchStore,
    online_match::{OnlineMatchError, OnlineMembership, PgOnlineMatchStore},
    session::PgSessionStore,
};
use tokio::sync::{oneshot, Mutex as AsyncMutex, Semaphore};
#[path = "native_body.rs"]
mod native_body;
#[path = "native_ports.rs"]
mod native_ports;
#[cfg(feature = "acceptance-test-support")]
pub use crate::{AcceptanceFaultHook, AcceptanceFaultPhase, AcceptanceFaultPoint};
use native_body::{private_response, PrivateAttachment};
use native_ports::{
    ActiveRequest, AuthorizedAttachment, ClosedEffects, LiveMatch, NetworkAuthority, NetworkClock,
    NetworkJournal, QueueOutput,
};
const MAX_LIVE_MATCHES: usize = 128;
const REQUEST_DEADLINE: Duration = Duration::from_secs(20);
const GRANT_LIFETIME_MS: u64 = 600_000;
/// Library-only authenticated gameplay adapter; production remains closed.
#[derive(Clone)]
pub struct IsolatedMatchHttp {
    state: Arc<GatewayState>,
}
impl fmt::Debug for IsolatedMatchHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IsolatedMatchHttp").finish_non_exhaustive()
    }
}
/// Non-wire witness for the disposable actual-network acceptance fixture.
/// Created only from a real authenticated poll and its actual private queue.
#[cfg(feature = "acceptance-test-support")]
#[derive(Clone)]
pub struct PollCaptureWitness {
    match_id: String,
    attachment_id: String,
    record: AuthSessionId,
    projected_frames: usize,
}
#[cfg(feature = "acceptance-test-support")]
impl fmt::Debug for PollCaptureWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollCaptureWitness").finish_non_exhaustive()
    }
}
#[cfg(feature = "acceptance-test-support")]
impl PollCaptureWitness {
    pub fn match_id(&self) -> &str {
        &self.match_id
    }
    pub fn attachment_id(&self) -> &str {
        &self.attachment_id
    }
    pub const fn record(&self) -> AuthSessionId {
        self.record
    }
    pub const fn projected_frames(&self) -> usize {
        self.projected_frames
    }
}
struct GatewayState {
    session_http: IsolatedSessionHttp<PgSessionStore>,
    sessions: PgSessionStore,
    online: PgOnlineMatchStore,
    matches: PgMatchStore,
    games: Vec<Arc<dyn ErasedGame>>,
    signing_key: [u8; 32],
    live: AsyncMutex<BTreeMap<MatchId, Arc<LiveMatch>>>,
    request_permits: Arc<Semaphore>,
    work_permits: Arc<Semaphore>,
    limits: std::sync::Mutex<runtime::Limits>,
    #[cfg(feature = "acceptance-test-support")]
    hook: std::sync::Mutex<Option<Arc<dyn AcceptanceFaultHook>>>,
}
impl IsolatedMatchHttp {
    /// Compose reviewed session/admission/commit/frame boundaries at one origin.
    /// Explicit migration and fixture enrollment belong to the caller.
    pub fn new(
        session_http: IsolatedSessionHttp<PgSessionStore>,
        sessions: PgSessionStore,
        online: PgOnlineMatchStore,
        matches: PgMatchStore,
        games: Vec<Arc<dyn ErasedGame>>,
    ) -> Result<Self, SessionError> {
        if games.len() > 64 {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self {
            state: Arc::new(GatewayState {
                session_http,
                sessions,
                online,
                matches,
                games,
                signing_key: random_bytes()?,
                live: AsyncMutex::new(BTreeMap::new()),
                request_permits: Arc::new(Semaphore::new(64)),
                work_permits: Arc::new(Semaphore::new(64)),
                limits: std::sync::Mutex::new(runtime::Limits::default()),
                #[cfg(feature = "acceptance-test-support")]
                hook: std::sync::Mutex::new(None),
            }),
        })
    }
    /// Install only an explicitly opted-in disposable command barrier.
    #[cfg(feature = "acceptance-test-support")]
    #[must_use]
    pub fn with_acceptance_hook(self, hook: Arc<dyn AcceptanceFaultHook>) -> Self {
        *self
            .state
            .hook
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(hook);
        self
    }
    /// Select a bounded receipt policy only for explicit disposable acceptance.
    #[cfg(feature = "acceptance-test-support")]
    #[must_use]
    pub fn with_acceptance_limits(self, limits: runtime::Limits) -> Self {
        *self
            .state
            .limits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = limits;
        self
    }
    /// Pause real SQL only after its next transaction has staged its writes.
    #[cfg(feature = "acceptance-test-support")]
    pub async fn with_acceptance_commit_pause(
        &self,
        id: MatchId,
    ) -> Result<
        tabula_storage::match_postgres::test_support::CommitPause,
        tabula_match::durable::RuntimePortError,
    > {
        let live = self
            .state
            .live
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or(tabula_match::durable::RuntimePortError::Unavailable)?;
        live.journal.journal.pause_before_commit()
    }
    /// Dispose only the selected test owner's actual `PostgreSQL` backend.
    #[cfg(feature = "acceptance-test-support")]
    pub async fn terminate_acceptance_owner_backend(
        &self,
        id: MatchId,
    ) -> Result<(), tabula_match::durable::RuntimePortError> {
        let live = self
            .state
            .live
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or(tabula_match::durable::RuntimePortError::Unavailable)?;
        live.journal
            .journal
            .terminate_owner_backend_for_test()
            .await
    }
    /// Retire only the selected fixture owner; recovery must resolve durable truth.
    #[cfg(feature = "acceptance-test-support")]
    pub async fn retire_acceptance_owner(
        &self,
        id: MatchId,
    ) -> Result<(), tabula_match::durable::RuntimePortError> {
        let live = self
            .state
            .live
            .lock()
            .await
            .remove(&id)
            .ok_or(tabula_match::durable::RuntimePortError::Unavailable)?;
        live.owner_task.abort();
        live.authority.close();
        live.journal.set(None)?;
        Ok(())
    }
    /// Versioned bounded POST polling; merge with the same session HTTP instance.
    pub fn router(self) -> Router {
        Router::new()
            .route("/api/v1/matches", post(create))
            .route("/api/v1/matches/join", post(join))
            .route("/api/v1/matches/:id/grant", post(grant))
            .route("/api/v1/matches/:id/attach", post(attach))
            .route("/api/v1/matches/:id/command", post(command))
            .route("/api/v1/matches/:id/poll", post(poll))
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                bound_requests,
            ))
            .layer(middleware::from_fn(no_store))
            .with_state(self.state)
    }
}
fn random_bytes() -> Result<[u8; 32], SessionError> {
    Ok(*SessionCredential::generate()?.digest().as_bytes())
}
fn random_id() -> Result<u128, SessionError> {
    let bytes = random_bytes()?;
    let mut id = [0; 16];
    id.copy_from_slice(&bytes[..16]);
    let id = u128::from_be_bytes(id);
    if id == 0 {
        Err(SessionError::Unavailable)
    } else {
        Ok(id)
    }
}
fn random_session() -> Result<SessionId, SessionError> {
    let bytes = random_bytes()?;
    let mut id = [0; 8];
    id.copy_from_slice(&bytes[..8]);
    let id = u64::from_be_bytes(id);
    if id == 0 {
        Err(SessionError::Unavailable)
    } else {
        Ok(SessionId(id))
    }
}
fn new_code() -> Result<String, SessionError> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let b = random_bytes()?;
    Ok(b[..12]
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b & 31)]))
        .collect())
}
fn code_digest(code: &str) -> [u8; 32] {
    let mut d = Sha256::new();
    d.update(b"tabula-online-code-v1\0");
    d.update(code.as_bytes());
    d.finalize().into()
}
fn now_ms() -> Result<u64, SessionError> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SessionError::Unavailable)?;
    u64::try_from(d.as_millis()).map_err(|_| SessionError::Unavailable)
}
fn membership_scope(m: &OnlineMembership) -> tabula_core::SeatId {
    m.scope().seat
}
async fn bound_requests(
    State(state): State<Arc<GatewayState>>,
    request: Request,
    next: Next,
) -> Response {
    let Ok(_permit) = state.request_permits.clone().try_acquire_owned() else {
        return problem(StatusCode::TOO_MANY_REQUESTS, "busy");
    };
    match tokio::time::timeout(Duration::from_secs(30), next.run(request)).await {
        Ok(response) => response,
        Err(_) => problem(StatusCode::REQUEST_TIMEOUT, "request_timeout"),
    }
}
async fn no_store(request: Request, next: Next) -> Response {
    let mut response = if request.method() == axum::http::Method::OPTIONS {
        problem(StatusCode::FORBIDDEN, "request_rejected")
    } else {
        next.run(request).await
    };
    for (key, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::PRAGMA, "no-cache"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::REFERRER_POLICY, "no-referrer"),
    ] {
        response
            .headers_mut()
            .insert(key, HeaderValue::from_static(value));
    }
    response
}
fn problem(status: StatusCode, code: &'static str) -> Response {
    let mut r = Response::new(Body::from(format!("{{\"code\":\"{code}\"}}")));
    *r.status_mut() = status;
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    r
}
fn session_problem(e: SessionError) -> Response {
    let (status, code) = match e {
        SessionError::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
        SessionError::Conflict => (StatusCode::CONFLICT, "conflict"),
        SessionError::InvalidInput => (StatusCode::BAD_REQUEST, "request_rejected"),
        SessionError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    problem(status, code)
}
fn online_problem(e: OnlineMatchError) -> Response {
    match e {
        OnlineMatchError::Session(e) => session_problem(e),
        OnlineMatchError::Unavailable => unavailable(),
        OnlineMatchError::Busy | OnlineMatchError::RateLimited => {
            problem(StatusCode::TOO_MANY_REQUESTS, "busy")
        }
        OnlineMatchError::InvalidInput => invalid(),
        OnlineMatchError::JoinUnavailable => problem(StatusCode::FORBIDDEN, "match_unavailable"),
    }
}
fn invalid() -> Response {
    problem(StatusCode::BAD_REQUEST, "request_rejected")
}
fn unavailable() -> Response {
    problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
}
fn admission(
    m: &OnlineMembership,
    code: Option<String>,
) -> Result<MatchAdmission, crate::InvalidMatchHttp> {
    MatchAdmission::new(
        format!("{:032x}", m.match_id().0),
        m.game().as_str().into(),
        m.game_version().as_str().into(),
        m.scope().seat.0,
        code,
        m.ready(),
    )
}
async fn create(State(state): State<Arc<GatewayState>>, request: Request) -> Response {
    let (op, _, body) = match state
        .session_http
        .authenticate_json::<MatchCreateRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(id) = GameId::new(body.game_id()) else {
        return invalid();
    };
    let Some(game) = state.games.iter().find(|g| g.metadata().id() == &id) else {
        return invalid();
    };
    if !game.direct_host_supported() {
        return invalid();
    }
    let mut draft = ConfigDraft::with_defaults(game.form());
    for (k, v) in body.config() {
        draft.set(k, v);
    }
    let Ok(normalized) = game.normalize_direct(body.seats(), &draft) else {
        return invalid();
    };
    let code = match new_code() {
        Ok(v) => v,
        Err(e) => return session_problem(e),
    };
    let match_id = match random_id() {
        Ok(id) => MatchId(id),
        Err(e) => return session_problem(e),
    };
    let m = match state
        .online
        .create(
            op,
            match_id,
            code_digest(&code),
            id,
            game.metadata().version().clone(),
            normalized.canonical_config().to_vec(),
            body.seats(),
        )
        .await
    {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    let Ok(value) = admission(&m, Some(code)) else {
        return unavailable();
    };
    private_response(&state, op, &value, None).await
}
async fn join(State(state): State<Arc<GatewayState>>, request: Request) -> Response {
    let (op, _, body) = match state
        .session_http
        .authenticate_json::<MatchJoinRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    // Every bounded malformed/unknown/full/expired code attempt shares the durable limiter.
    let m = match state.online.join(op, code_digest(body.code())).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    let Ok(value) = admission(&m, None) else {
        return unavailable();
    };
    private_response(&state, op, &value, None).await
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantClaims {
    version: u16,
    issuer: String,
    audience: String,
    purpose: String,
    match_id: u128,
    record: u128,
    subject: u128,
    epoch: u64,
    seat: u8,
    generation: u64,
    not_before: u64,
    expires: u64,
}
impl fmt::Debug for GrantClaims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrantClaims([REDACTED])")
    }
}
fn mint_grant(state: &GatewayState, m: &OnlineMembership) -> Result<String, SessionError> {
    let now = now_ms()?;
    let s = m.scope();
    let expires = now
        .checked_add(GRANT_LIFETIME_MS)
        .ok_or(SessionError::Unavailable)?
        .min(m.snapshot().idle_deadline().get())
        .min(m.snapshot().absolute_deadline().get());
    if expires <= now {
        return Err(SessionError::Unauthenticated);
    }
    let c = GrantClaims {
        version: 1,
        issuer: "tabula-isolated-match".into(),
        audience: "tabula-isolated-poll".into(),
        purpose: "seat-attach".into(),
        match_id: m.match_id().0,
        record: s.record,
        subject: s.subject.0,
        epoch: s.epoch,
        seat: s.seat.0,
        generation: s.generation,
        not_before: now,
        expires,
    };
    let encoded =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&c).map_err(|_| SessionError::Unavailable)?);
    let mut mac = Hmac::<Sha256>::new_from_slice(&state.signing_key)
        .map_err(|_| SessionError::Unavailable)?;
    mac.update(b"tabula-match-grant-v1\0");
    mac.update(encoded.as_bytes());
    Ok(format!(
        "{encoded}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    ))
}
fn verify_grant(
    state: &GatewayState,
    token: &str,
    m: &OnlineMembership,
) -> Result<(), SessionError> {
    let (e, sig) = token.split_once('.').ok_or(SessionError::InvalidInput)?;
    let signature = URL_SAFE_NO_PAD
        .decode(sig)
        .map_err(|_| SessionError::InvalidInput)?;
    if signature.len() != 32 || URL_SAFE_NO_PAD.encode(&signature) != sig {
        return Err(SessionError::InvalidInput);
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(&state.signing_key)
        .map_err(|_| SessionError::Unavailable)?;
    mac.update(b"tabula-match-grant-v1\0");
    mac.update(e.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| SessionError::Unauthenticated)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(e)
        .map_err(|_| SessionError::InvalidInput)?;
    if URL_SAFE_NO_PAD.encode(&bytes) != e {
        return Err(SessionError::InvalidInput);
    }
    let c: GrantClaims = serde_json::from_slice(&bytes).map_err(|_| SessionError::InvalidInput)?;
    let s = m.scope();
    let now = now_ms()?;
    if c.version != 1
        || c.issuer != "tabula-isolated-match"
        || c.audience != "tabula-isolated-poll"
        || c.purpose != "seat-attach"
        || c.match_id != m.match_id().0
        || c.record != s.record
        || c.subject != s.subject.0
        || c.epoch != s.epoch
        || c.seat != s.seat.0
        || c.generation != s.generation
        || now < c.not_before
        || now >= c.expires
        || c.expires > c.not_before.saturating_add(GRANT_LIFETIME_MS)
        || c.expires
            > m.snapshot()
                .idle_deadline()
                .get()
                .min(m.snapshot().absolute_deadline().get())
    {
        return Err(SessionError::Unauthenticated);
    }
    Ok(())
}
async fn grant(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (op, _, _) = match state
        .session_http
        .authenticate_json::<MatchGrantRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(id) = parse_match_id(&id) else {
        return invalid();
    };
    let m = match state.online.resolve(op, id).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    if m.ready() {
        if let Err(r) = ensure_live(&state, op, &m).await {
            return r;
        }
    }
    let token = if m.ready() {
        match mint_grant(&state, &m) {
            Ok(t) => Some(t),
            Err(e) => return session_problem(e),
        }
    } else {
        None
    };
    let Ok(value) = MatchGrant::new(
        m.ready(),
        token,
        m.scope().seat.0,
        m.game().as_str().into(),
        m.game_version().as_str().into(),
    ) else {
        return unavailable();
    };
    private_response(&state, op, &value, None).await
}
async fn ensure_live(
    state: &GatewayState,
    op: CredentialOperation,
    m: &OnlineMembership,
) -> Result<Arc<LiveMatch>, Response> {
    let mut map = state.live.lock().await;
    if let Some(live) = map.get(&m.match_id()) {
        if !live.handle.is_closed() && live.journal.journal.is_owner_active() {
            return Ok(live.clone());
        }
        live.owner_task.abort();
        live.authority.close();
        map.remove(&m.match_id());
    }
    if map.len() >= MAX_LIVE_MATCHES {
        return Err(problem(StatusCode::TOO_MANY_REQUESTS, "busy"));
    }
    let game = state
        .games
        .iter()
        .find(|g| g.metadata().id() == m.game() && g.metadata().version() == m.game_version())
        .cloned()
        .ok_or_else(unavailable)?;
    let roster = m
        .roster()
        .ok_or_else(|| problem(StatusCode::CONFLICT, "waiting_for_players"))?;
    let guard = state
        .online
        .begin_operation(op, m.match_id())
        .await
        .map_err(online_problem)?;
    let started = guard.membership().started();
    let limits = *state.limits.lock().map_err(|_| unavailable())?;
    let (journal, ready) = open_online_journal(state, m.match_id(), guard, started).await?;
    #[cfg(feature = "acceptance-test-support")]
    journal
        .set_hook(state.hook.lock().map_err(|_| unavailable())?.clone())
        .map_err(|_| unavailable())?;
    let output = Arc::new(QueueOutput::default());
    let authority = Arc::new(NetworkAuthority::new(
        m.match_id(),
        state.online.clone(),
        journal.clone(),
        output.clone(),
    ));
    let ports = Ports {
        authority: authority.clone(),
        journal: journal.clone(),
        output: output.clone(),
        effects: Arc::new(ClosedEffects {
            journal: journal.clone(),
        }),
        clock: Arc::new(NetworkClock::new()),
    };
    let (handle, _host, task) = if started {
        runtime::recover_for_admission(m.match_id(), game, m.config(), roster, ports, limits)
            .await
            .map_err(|_| unavailable())?
    } else {
        let created = game
            .create_match(
                m.config(),
                roster,
                MatchSeed::from_bytes(random_bytes().map_err(session_problem)?),
            )
            .map_err(|_| invalid())?;
        if !created.effects().is_empty() {
            return Err(invalid());
        }
        runtime::spawn(m.match_id(), created, ports, limits).map_err(|_| unavailable())?
    };
    let owner_task = task.abort_handle();
    let owner_authority = authority.clone();
    tokio::spawn(async move {
        let _ = task.await;
        owner_authority.close();
    });
    if let Some(receive) = ready {
        if !matches!(
            tokio::time::timeout(REQUEST_DEADLINE, receive).await,
            Ok(Ok(Ok(())))
        ) {
            owner_task.abort();
            authority.close();
            let _ = journal.set(None);
            return Err(unavailable());
        }
    }
    journal.set(None).map_err(|_| unavailable())?;
    let live = Arc::new(LiveMatch {
        handle,
        journal,
        authority,
        output,
        gate: Arc::new(AsyncMutex::new(())),
        owner_task,
    });
    map.insert(m.match_id(), live.clone());
    Ok(live)
}
async fn open_online_journal(
    state: &GatewayState,
    id: MatchId,
    guard: tabula_storage::online_match::PgOnlineOperation,
    started: bool,
) -> Result<
    (
        Arc<NetworkJournal>,
        Option<oneshot::Receiver<Result<(), tabula_match::durable::RuntimePortError>>>,
    ),
    Response,
> {
    let pair = if started {
        guard.cancel().await.map_err(online_problem)?;
        // Dead-owner publication exclusion may outlive its physical backend briefly.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let journal = loop {
            match state.matches.reclaim_online(id).await {
                Ok(journal) => break journal,
                Err(tabula_match::durable::RuntimePortError::Busy)
                    if tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(_) => return Err(unavailable()),
            }
        };
        (Arc::new(NetworkJournal::recovered(journal)), None)
    } else {
        let journal = state
            .matches
            .claim_online(id)
            .await
            .map_err(|_| unavailable())?;
        let (send, receive) = oneshot::channel();
        (
            Arc::new(NetworkJournal::new(journal, guard, send)),
            Some(receive),
        )
    };
    Ok(pair)
}
async fn retire_seat(
    live: &LiveMatch,
    seat: tabula_core::SeatId,
) -> Result<(), tabula_match::runtime_ports::AuthorityLost> {
    for binding in live.authority.for_seat(seat)? {
        if let Ok(ticket) = live.handle.detach(binding.clone()) {
            let _ = ticket.wait().await;
        }
        live.output.remove(&binding);
        live.authority.remove(&binding);
    }
    Ok(())
}
/// A correlation hint only: current authority and the durable scope still gate every retry.
fn operation_scope_hint(id: MatchId, scope: tabula_match::durable::OperationScope) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut digest = Sha256::new();
    digest.update(b"tabula-online-operation-scope-v2\0");
    digest.update(id.0.to_be_bytes());
    digest.update(scope.record.to_be_bytes());
    digest.update(scope.subject.0.to_be_bytes());
    digest.update(scope.epoch.to_be_bytes());
    digest.update([scope.seat.0]);
    digest.update(scope.generation.to_be_bytes());
    let mut hint = String::with_capacity(64);
    for byte in digest.finalize() {
        hint.push(char::from(HEX[usize::from(byte >> 4)]));
        hint.push(char::from(HEX[usize::from(byte & 15)]));
    }
    hint
}
async fn attach(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (op, _, body) = match state
        .session_http
        .authenticate_json::<MatchAttachRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    // An HTTP cancellation cannot replace the sole actor's in-flight journal permit.
    // The bounded owned task retains serialization until durable truth is resolved.
    let Ok(task) = spawn_bounded_work(
        state.work_permits.clone(),
        attach_serialized(state, id, op, body),
    ) else {
        return problem(StatusCode::TOO_MANY_REQUESTS, "busy");
    };
    match task.await {
        Ok(response) => response,
        Err(_) => unavailable(),
    }
}
async fn attach_serialized(
    state: Arc<GatewayState>,
    id: String,
    op: CredentialOperation,
    body: MatchAttachRequest,
) -> Response {
    let Ok(id) = parse_match_id(&id) else {
        return invalid();
    };
    let m = match state.online.resolve(op, id).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    if let Err(e) = verify_grant(&state, body.binding_id(), &m) {
        return grant_problem(e);
    }
    let live = match ensure_live(&state, op, &m).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let Ok(_gate) =
        tokio::time::timeout(Duration::from_secs(5), live.gate.clone().lock_owned()).await
    else {
        return problem(StatusCode::TOO_MANY_REQUESTS, "busy");
    };
    // Every wait invalidates the earlier observation, including membership/epoch.
    let m = match state.online.resolve(op, id).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    if let Err(e) = verify_grant(&state, body.binding_id(), &m) {
        return grant_problem(e);
    }
    if live.handle.is_closed() || !live.journal.journal.is_owner_active() {
        return unavailable();
    }
    let scope = m.scope();
    if retire_seat(&live, scope.seat).await.is_err() {
        return unavailable();
    }
    let session = match random_session() {
        Ok(s) => s,
        Err(e) => return session_problem(e),
    };
    let binding = Binding::new(
        session,
        scope.subject,
        scope.record,
        scope.epoch,
        scope.generation,
    );
    let next_seq = match next_sequence(&live, id, scope).await {
        Ok(seq) => seq,
        Err(response) => return response,
    };
    let guard = match state.online.begin_operation(op, id).await {
        Ok(g) => g,
        Err(e) => return online_problem(e),
    };
    let Ok(active) = ActiveRequest::new(live.journal.clone(), guard) else {
        return unavailable();
    };
    if live
        .authority
        .insert(AuthorizedAttachment {
            binding: binding.clone(),
            credential: op,
            scope,
        })
        .is_err()
        || live.output.insert(binding.clone()).is_err()
    {
        return unavailable();
    }
    let Ok(ticket) = live.handle.attach(
        binding.clone(),
        tabula_registry::ClientViewer::Seat(scope.seat),
    ) else {
        return unavailable();
    };
    let result = wait_for_owner(&live, ticket).await;
    drop(active);
    if let Err(response) = result {
        return response;
    }
    let frames = match live.output.drain(&binding) {
        Ok(f) => f,
        Err(e) => return e.response(),
    };
    let Ok(value) = MatchAttachment::new(
        format!("{:032x}", session.0),
        scope.seat.0,
        next_seq,
        operation_scope_hint(id, scope),
        frames,
    ) else {
        return unavailable();
    };
    publish_attachment(&state, &live, &m, binding, op, &value).await
}
async fn publish_attachment(
    state: &GatewayState,
    live: &LiveMatch,
    member: &OnlineMembership,
    binding: Binding,
    op: CredentialOperation,
    value: &impl Serialize,
) -> Response {
    private_response(
        state,
        op,
        value,
        Some(PrivateAttachment {
            output: live.output.clone(),
            binding,
            journal: live.journal.clone(),
            match_id: member.match_id(),
            scope: member.scope(),
        }),
    )
    .await
}
async fn next_sequence(
    live: &LiveMatch,
    id: MatchId,
    scope: tabula_match::durable::OperationScope,
) -> Result<u64, Response> {
    let loaded = live
        .journal
        .journal
        .load(id)
        .await
        .map_err(|_| unavailable())?;
    loaded
        .ledger
        .iter()
        .find(|s| s.scope == scope)
        .map_or(Some(1), |s| s.highest.checked_add(1))
        .ok_or_else(unavailable)
}
fn attached(
    live: &LiveMatch,
    attachment: &str,
    m: &OnlineMembership,
    op: CredentialOperation,
) -> Result<Binding, StatusCode> {
    let session = SessionId(
        u64::try_from(
            parse_match_id(attachment)
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .0,
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?,
    );
    live.authority
        .refresh(session, m.scope(), op)
        .map_err(|_| StatusCode::CONFLICT)
}
fn grant_problem(error: SessionError) -> Response {
    // Current cookie and membership already passed; a process restart can
    // replace the ephemeral signing key without changing the operation scope.
    if error == SessionError::Unauthenticated {
        problem(StatusCode::CONFLICT, "fresh_grant_required")
    } else {
        session_problem(error)
    }
}
fn attachment_problem(status: StatusCode) -> Response {
    // Current cookie/membership already passed. A retired local transport is
    // recoverable; it cannot turn an uncertain same-scope command into failure.
    let code = if status == StatusCode::CONFLICT {
        "reattach_required"
    } else {
        "request_rejected"
    };
    problem(status, code)
}
async fn command(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (op, _, body) = match state
        .session_http
        .authenticate_json::<MatchCommandRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    // An HTTP cancellation cannot replace the sole actor's in-flight journal permit.
    // The bounded owned task retains serialization until durable truth is resolved.
    let Ok(task) = spawn_bounded_work(
        state.work_permits.clone(),
        command_serialized(state, id, op, body),
    ) else {
        return problem(StatusCode::TOO_MANY_REQUESTS, "busy");
    };
    match task.await {
        Ok(response) => response,
        Err(_) => unavailable(),
    }
}
async fn command_serialized(
    state: Arc<GatewayState>,
    id: String,
    op: CredentialOperation,
    body: MatchCommandRequest,
) -> Response {
    let Ok(id) = parse_match_id(&id) else {
        return invalid();
    };
    if body.command().command().match_id() != id {
        return invalid();
    }
    let Some(live) = state.live.lock().await.get(&id).cloned() else {
        return problem(StatusCode::CONFLICT, "reattach_required");
    };
    let Ok(_gate) =
        tokio::time::timeout(Duration::from_secs(5), live.gate.clone().lock_owned()).await
    else {
        return problem(StatusCode::TOO_MANY_REQUESTS, "busy");
    };
    // Every wait invalidates the earlier observation, including membership/epoch.
    let m = match state.online.resolve(op, id).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    if live.handle.is_closed() || !live.journal.journal.is_owner_active() {
        return unavailable();
    }
    let binding = match attached(&live, body.attachment_id(), &m, op) {
        Ok(b) => b,
        Err(s) => return attachment_problem(s),
    };
    if let Err(e) = live.output.command_rate(&binding, membership_scope(&m)) {
        return e.response();
    }
    #[cfg(feature = "acceptance-test-support")]
    let point = AcceptanceFaultPoint {
        match_id: id,
        record: match AuthSessionId::new(m.scope().record) {
            Ok(record) => record,
            Err(_) => return unavailable(),
        },
        attachment_id: Some(body.attachment_id().to_owned()),
        phase: AcceptanceFaultPhase::BeforeSubmission,
    };
    #[cfg(feature = "acceptance-test-support")]
    {
        let hook = match state.hook.lock() {
            Ok(hook) => hook.clone(),
            Err(_) => return unavailable(),
        };
        if let Some(hook) = hook {
            if hook.reach(point.clone()).await.is_err() {
                return unavailable();
            }
        }
    }
    let guard = match state.online.begin_operation(op, id).await {
        Ok(g) => g,
        Err(e) => return online_problem(e),
    };
    let Ok(active) = ActiveRequest::new(live.journal.clone(), guard) else {
        return unavailable();
    };
    #[cfg(feature = "acceptance-test-support")]
    if active.command(point).is_err() {
        return unavailable();
    }
    let Ok(ticket) = live.handle.command(binding.clone(), body.command().clone()) else {
        return unavailable();
    };
    let result = wait_for_owner(&live, ticket).await;
    drop(active);
    if let Err(response) = result {
        return response;
    }
    let frames = match live.output.drain(&binding) {
        Ok(f) => f,
        Err(e) => return e.response(),
    };
    let Ok(value) = MatchFrames::new(frames) else {
        return unavailable();
    };
    publish_attachment(&state, &live, &m, binding, op, &value).await
}
/// HTTP requester cancellation cannot release capacity still held by journal work.
fn spawn_bounded_work<T, F>(
    permits: Arc<Semaphore>,
    work: F,
) -> Result<tokio::task::JoinHandle<T>, ()>
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let permit = permits.try_acquire_owned().map_err(|_| ())?;
    Ok(tokio::spawn(async move {
        let _permit = permit;
        work.await
    }))
}
async fn wait_for_owner(live: &LiveMatch, ticket: runtime::Ticket) -> Result<(), Response> {
    if matches!(
        tokio::time::timeout(REQUEST_DEADLINE, ticket.wait()).await,
        Ok(Ok(Completion::Submitted))
    ) {
        Ok(())
    } else {
        // A timeout does not imply rollback; only a fresh durable owner can resolve it.
        live.owner_task.abort();
        live.authority.close();
        Err(unavailable())
    }
}
async fn poll(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (op, snapshot, body) = match state
        .session_http
        .authenticate_json::<MatchPollRequest>(request, MAX_REQUEST_BYTES)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(id) = parse_match_id(&id) else {
        return invalid();
    };
    let m = match state.online.resolve(op, id).await {
        Ok(m) => m,
        Err(e) => return online_problem(e),
    };
    let Some(live) = state.live.lock().await.get(&id).cloned() else {
        return problem(StatusCode::CONFLICT, "reattach_required");
    };
    if live.handle.is_closed() || !live.journal.journal.is_owner_active() {
        return unavailable();
    }
    let binding = match attached(&live, body.attachment_id(), &m, op) {
        Ok(b) => b,
        Err(s) => return attachment_problem(s),
    };
    let frames = match live.output.drain(&binding) {
        Ok(f) => f,
        Err(e) => return e.response(),
    };
    #[cfg(feature = "acceptance-test-support")]
    let witness = PollCaptureWitness {
        match_id: format!("{:032x}", id.0),
        attachment_id: body.attachment_id().to_owned(),
        record: snapshot.id(),
        projected_frames: frames
            .iter()
            .filter(|frame| {
                matches!(
                    frame.body(),
                    tabula_protocol::ServerMessage::MatchUpdate { .. }
                )
            })
            .count(),
    };
    #[cfg(not(feature = "acceptance-test-support"))]
    let _ = snapshot;
    let Ok(value) = MatchFrames::new(frames) else {
        return unavailable();
    };
    #[allow(unused_mut)]
    let mut response = publish_attachment(&state, &live, &m, binding, op, &value).await;
    #[cfg(feature = "acceptance-test-support")]
    if response.status().is_success() {
        response.extensions_mut().insert(witness);
    }
    response
}

#[cfg(test)]
mod recovery_admission_tests {
    use super::*;
    #[tokio::test]
    async fn cancelled_request_keeps_detached_work_bounded() {
        let permits = Arc::new(Semaphore::new(1));
        let (release, gate) = oneshot::channel::<()>();
        let work = spawn_bounded_work(permits.clone(), async move {
            gate.await.unwrap();
        })
        .unwrap();
        let (waiting, entered) = oneshot::channel();
        let requester = tokio::spawn(async move {
            waiting.send(()).unwrap();
            let _ = work.await;
        });
        entered.await.unwrap();
        requester.abort();
        assert!(requester.await.unwrap_err().is_cancelled());
        assert_eq!(permits.available_permits(), 0);
        assert!(spawn_bounded_work(permits.clone(), async {}).is_err());
        release.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(1), permits.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(permits.available_permits(), 1);
    }
    #[tokio::test]
    async fn stale_local_attachment_is_reattach_required_without_frames() {
        let response = attachment_problem(StatusCode::CONFLICT);
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"{\"code\":\"reattach_required\"}");
        assert_eq!(
            attachment_problem(StatusCode::BAD_REQUEST).status(),
            StatusCode::BAD_REQUEST
        );
    }
    #[tokio::test]
    async fn rejected_grant_is_refreshable_without_weakening_session_denial() {
        let response = grant_problem(SessionError::Unauthenticated);
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"{\"code\":\"fresh_grant_required\"}");
        assert_eq!(
            session_problem(SessionError::Unauthenticated).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            grant_problem(SessionError::InvalidInput).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            grant_problem(SessionError::Unavailable).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    #[test]
    fn operation_scope_hint_binds_authority_not_transport() {
        use tabula_core::{SeatId, UserId};
        let scope = tabula_match::durable::OperationScope {
            record: 1,
            subject: UserId(2),
            epoch: 3,
            seat: SeatId(0),
            generation: 4,
        };
        let original = operation_scope_hint(MatchId(5), scope);
        assert_eq!(original.len(), 64);
        assert_eq!(original, operation_scope_hint(MatchId(5), scope));
        for changed in [
            tabula_match::durable::OperationScope { record: 6, ..scope },
            tabula_match::durable::OperationScope {
                subject: UserId(7),
                ..scope
            },
            tabula_match::durable::OperationScope { epoch: 8, ..scope },
            tabula_match::durable::OperationScope {
                seat: SeatId(1),
                ..scope
            },
            tabula_match::durable::OperationScope {
                generation: 9,
                ..scope
            },
        ] {
            assert_ne!(original, operation_scope_hint(MatchId(5), changed));
        }
        assert_ne!(original, operation_scope_hint(MatchId(6), scope));
    }
}
