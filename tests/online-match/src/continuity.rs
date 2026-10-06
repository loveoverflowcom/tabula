//! CI-only PR3 deterministic command barriers and current-authority faults.
//! Controls never manufacture an Ack, projection, rollback, or successful commit.
//! Tokens remain ephemeral; only fixed phases cross this disposable boundary.
#![cfg(feature = "continuity-test")]

use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tabula_core::MatchId;
use tabula_match::durable::RuntimePortError;
use tabula_match_http::isolated::{
    AcceptanceFaultHook, AcceptanceFaultPhase, AcceptanceFaultPoint, IsolatedMatchHttp,
};
use tabula_session::{
    AuthSessionId, CredentialDigest, IssueSession, ProviderIdentityKey, SessionAuthority,
    SessionChannel, SessionContextId, SessionCredential,
};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::{
    match_postgres::PgMatchStore, online_match::PgOnlineMatchStore, session::PgSessionStore,
};
use tokio::sync::{oneshot, Semaphore};

const LIFETIME: Duration = Duration::from_secs(60);
const ORIGIN: &str = "https://localhost:9443";
const LIMIT: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Armed,
    Held,
    Released,
    Failed,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Armed => "armed",
            Self::Held => "held",
            Self::Released => "released",
            Self::Failed => "failed",
        }
    }
}
struct Flow {
    match_id: MatchId,
    attachment_id: String,
    record: AuthSessionId,
    staged: bool,
    point: AcceptanceFaultPhase,
    token: CredentialDigest,
    deadline: Instant,
    phase: Phase,
    release: Option<oneshot::Sender<()>>,
}
#[derive(Default)]
pub struct ContinuityHook {
    flow: Mutex<Option<Flow>>,
}
impl AcceptanceFaultHook for ContinuityHook {
    fn reach(
        &self,
        point: AcceptanceFaultPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), RuntimePortError>> + Send + '_>> {
        Box::pin(async move {
            let gate = {
                let mut slot = self
                    .flow
                    .lock()
                    .map_err(|_| RuntimePortError::Unavailable)?;
                let Some(flow) = slot.as_mut() else {
                    return Ok(());
                };
                if flow.staged
                    || flow.phase != Phase::Armed
                    || point.match_id != flow.match_id
                    || point.record != flow.record
                    || point.phase != flow.point
                    || point.attachment_id.as_deref() != Some(flow.attachment_id.as_str())
                {
                    return Ok(());
                }
                if Instant::now() >= flow.deadline {
                    flow.phase = Phase::Failed;
                    return Err(RuntimePortError::Unavailable);
                }
                let (tx, rx) = oneshot::channel();
                flow.phase = Phase::Held;
                flow.release = Some(tx);
                (rx, flow.deadline)
            };
            match tokio::time::timeout_at(tokio::time::Instant::from_std(gate.1), gate.0).await {
                Ok(Ok(())) => Ok(()),
                _ => {
                    if let Ok(mut slot) = self.flow.lock() {
                        if let Some(flow) = slot.as_mut() {
                            flow.phase = Phase::Failed;
                            flow.release.take();
                        }
                    }
                    Err(RuntimePortError::Unavailable)
                }
            }
        })
    }
}
struct Controls {
    hook: Arc<ContinuityHook>,
    http: IsolatedSessionHttp<PgSessionStore>,
    sessions: PgSessionStore,
    online: PgOnlineMatchStore,
    matches: PgMatchStore,
    gateway: IsolatedMatchHttp,
    reads: Arc<Semaphore>,
    record_changes: AtomicUsize,
}

pub fn compose(
    router: Router,
    hook: Arc<ContinuityHook>,
    http: IsolatedSessionHttp<PgSessionStore>,
    sessions: PgSessionStore,
    online: PgOnlineMatchStore,
    matches: PgMatchStore,
    gateway: IsolatedMatchHttp,
) -> Router {
    let state = Arc::new(Controls {
        hook,
        http,
        sessions,
        online,
        matches,
        gateway,
        reads: Arc::new(Semaphore::new(4)),
        record_changes: AtomicUsize::new(0),
    });
    router.merge(
        Router::new()
            .route("/__fixture/continuity/arm", post(arm))
            .route("/__fixture/continuity/status", post(status))
            .route("/__fixture/continuity/release", post(release))
            .route("/__fixture/continuity/authority", post(authority))
            .route("/__fixture/continuity/oracle", post(oracle))
            .route("/__fixture/continuity/new-record", post(new_record))
            .with_state(state),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arm {
    version: u16,
    match_id: String,
    attachment_id: String,
    point: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    version: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    version: u16,
    match_id: String,
    change: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Oracle {
    version: u16,
    match_id: String,
    expected_inputs: u8,
}
#[derive(Serialize)]
struct PhaseReply {
    version: u16,
    phase: &'static str,
}
fn identifier(raw: &str) -> Option<MatchId> {
    super::checked_id(raw).ok().map(MatchId)
}
fn response(status: StatusCode, value: serde_json::Value) -> Response {
    let Ok(bytes) = serde_json::to_vec(&value) else {
        return super::failure(StatusCode::SERVICE_UNAVAILABLE);
    };
    super::public_response(status, bytes, "application/json")
}
fn rejected(status: StatusCode) -> Response {
    response(
        status,
        serde_json::json!({"code":"fixture_request_rejected"}),
    )
}
fn phase(phase: Phase) -> Response {
    response(
        StatusCode::OK,
        serde_json::json!(PhaseReply {
            version: 1,
            phase: phase.name()
        }),
    )
}
fn single<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    let mut values = headers.get_all(key).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

async fn arm(State(state): State<Arc<Controls>>, request: Request) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (_, snapshot, body) = match state.http.authenticate_json::<Arm>(request, LIMIT).await {
        Ok(value) => value,
        Err(reply) => return reply,
    };
    let Some(match_id) = identifier(&body.match_id) else {
        return rejected(StatusCode::BAD_REQUEST);
    };
    if body.version != 1 || identifier(&body.attachment_id).is_none() {
        return rejected(StatusCode::BAD_REQUEST);
    }
    if !matches!(
        state
            .online
            .has_fixture_membership(snapshot.user_id(), match_id)
            .await,
        Ok(true)
    ) {
        return rejected(StatusCode::FORBIDDEN);
    }
    let point = match body.point.as_str() {
        "before_submission" => AcceptanceFaultPhase::BeforeSubmission,
        "before_commit" | "staged_commit" => AcceptanceFaultPhase::BeforeCommit,
        "after_commit" => AcceptanceFaultPhase::AfterCommit,
        _ => return rejected(StatusCode::BAD_REQUEST),
    };
    let staged = body.point == "staged_commit";
    let pause = if staged {
        match state.gateway.with_acceptance_commit_pause(match_id).await {
            Ok(pause) => Some(pause),
            Err(_) => return rejected(StatusCode::SERVICE_UNAVAILABLE),
        }
    } else {
        None
    };
    let Ok(token) = SessionCredential::generate() else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(mut slot) = state.hook.flow.lock() else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    if slot
        .as_ref()
        .is_some_and(|flow| matches!(flow.phase, Phase::Armed | Phase::Held))
    {
        return rejected(StatusCode::CONFLICT);
    }
    *slot = Some(Flow {
        match_id,
        attachment_id: body.attachment_id,
        record: snapshot.id(),
        staged,
        point,
        token: token.digest(),
        deadline: Instant::now() + LIFETIME,
        phase: Phase::Armed,
        release: None,
    });
    drop(slot);
    if let Some(mut pause) = pause {
        let hook = state.hook.clone();
        tokio::spawn(async move {
            if pause.wait_until_entered().await.is_err() {
                return;
            }
            let gate = {
                let Ok(mut slot) = hook.flow.lock() else {
                    return;
                };
                let Some(flow) = slot.as_mut() else {
                    return;
                };
                if flow.phase != Phase::Armed || !flow.staged {
                    return;
                }
                let (tx, rx) = oneshot::channel();
                flow.release = Some(tx);
                flow.phase = Phase::Held;
                (rx, flow.deadline)
            };
            if matches!(
                tokio::time::timeout_at(tokio::time::Instant::from_std(gate.1), gate.0).await,
                Ok(Ok(()))
            ) {
                let _ = pause.release();
            } else if let Ok(mut slot) = hook.flow.lock() {
                if let Some(flow) = slot.as_mut() {
                    flow.phase = Phase::Failed;
                    flow.release.take();
                }
            }
        });
    }
    response(
        StatusCode::OK,
        serde_json::json!({"version":1,"control_token":token.expose_encoded()}),
    )
}
async fn control_request(request: Request) -> Result<CredentialDigest, StatusCode> {
    if request.uri().query().is_some()
        || single(request.headers(), "origin") != Some(ORIGIN)
        || single(request.headers(), "content-type") != Some("application/json")
        || request.headers().contains_key(header::CONTENT_ENCODING)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let token = SessionCredential::parse(
        single(request.headers(), "x-tabula-fixture-control").ok_or(StatusCode::FORBIDDEN)?,
    )
    .map_err(|_| StatusCode::FORBIDDEN)?
    .digest();
    let bytes = tokio::time::timeout(Duration::from_secs(2), to_bytes(request.into_body(), LIMIT))
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let body: Control = serde_json::from_slice(&bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    if body.version != 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(token)
}
async fn status(State(state): State<Arc<Controls>>, request: Request) -> Response {
    control(state, request, false).await
}
async fn release(State(state): State<Arc<Controls>>, request: Request) -> Response {
    control(state, request, true).await
}
async fn control(state: Arc<Controls>, request: Request, release: bool) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let token = match control_request(request).await {
        Ok(value) => value,
        Err(status) => return rejected(status),
    };
    let Ok(mut slot) = state.hook.flow.lock() else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Some(flow) = slot.as_mut() else {
        return rejected(StatusCode::FORBIDDEN);
    };
    if flow.token != token {
        return rejected(StatusCode::FORBIDDEN);
    }
    if Instant::now() >= flow.deadline {
        flow.phase = Phase::Failed;
        flow.release.take();
        return rejected(StatusCode::GONE);
    }
    if release {
        if flow.phase != Phase::Held {
            return rejected(StatusCode::CONFLICT);
        }
        let Some(tx) = flow.release.take() else {
            return rejected(StatusCode::CONFLICT);
        };
        flow.phase = Phase::Released;
        if tx.send(()).is_err() {
            flow.phase = Phase::Failed;
            return rejected(StatusCode::CONFLICT);
        }
    }
    phase(flow.phase)
}
async fn authority(State(state): State<Arc<Controls>>, request: Request) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (operation, snapshot, body) =
        match state.http.authenticate_json::<Change>(request, LIMIT).await {
            Ok(value) => value,
            Err(reply) => return reply,
        };
    let Some(match_id) = identifier(&body.match_id) else {
        return rejected(StatusCode::BAD_REQUEST);
    };
    if body.version != 1 {
        return rejected(StatusCode::BAD_REQUEST);
    }
    if !matches!(
        state
            .online
            .has_fixture_membership(snapshot.user_id(), match_id)
            .await,
        Ok(true)
    ) {
        return rejected(StatusCode::FORBIDDEN);
    }
    let result = match body.change.as_str() {
        "revoke" => state
            .sessions
            .revoke_session(snapshot.binding())
            .await
            .map_err(|_| ()),
        "expire_soon" => state
            .sessions
            .shorten_credential_lifetime_for_test(operation, 3_000)
            .await
            .map_err(|_| ()),
        "expire" => state
            .sessions
            .expire_credential_for_test(operation)
            .await
            .map_err(|_| ()),
        "epoch" => state
            .sessions
            .invalidate_account_epoch(snapshot.user_id(), snapshot.authorization_epoch())
            .await
            .map(|_| ())
            .map_err(|_| ()),
        "membership" => state
            .online
            .invalidate_membership_for_test(operation, match_id)
            .await
            .map_err(|_| ()),
        "owner_loss" => async {
            state
                .gateway
                .terminate_acceptance_owner_backend(match_id)
                .await?;
            state.gateway.retire_acceptance_owner(match_id).await
        }
        .await
        .map_err(|_| ()),
        _ => return rejected(StatusCode::BAD_REQUEST),
    };
    if result.is_err() {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    }
    response(
        StatusCode::OK,
        serde_json::json!({"version":1,"authority_change_committed":true}),
    )
}
async fn new_record(State(state): State<Arc<Controls>>, request: Request) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (_, snapshot, body) = match state
        .http
        .authenticate_json::<Control>(request, LIMIT)
        .await
    {
        Ok(value) => value,
        Err(reply) => return reply,
    };
    if body.version != 1 {
        return rejected(StatusCode::BAD_REQUEST);
    }
    if state.record_changes.fetch_add(1, Ordering::SeqCst) >= 16 {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    }
    let issued = async {
        state.sessions.revoke_session(snapshot.binding()).await?;
        let credential = SessionCredential::generate()?;
        let identity = ProviderIdentityKey::new(
            "https://synthetic-provider.tabula.invalid",
            format!("two-browser-{:032x}", snapshot.user_id().0),
        )
        .map_err(|_| tabula_session::SessionError::InvalidInput)?;
        state
            .sessions
            .issue_session(IssueSession {
                identity,
                expected_epoch: snapshot.authorization_epoch(),
                id: AuthSessionId::new(
                    super::random_id().map_err(|_| tabula_session::SessionError::Unavailable)?,
                )
                .map_err(|_| tabula_session::SessionError::InvalidInput)?,
                channel: SessionChannel::BrowserCookie,
                credential_digest: credential.digest(),
                context_id: SessionContextId::new(
                    super::random_id().map_err(|_| tabula_session::SessionError::Unavailable)?,
                )
                .map_err(|_| tabula_session::SessionError::InvalidInput)?,
            })
            .await?;
        Ok::<_, tabula_session::SessionError>(credential)
    }
    .await;
    let Ok(credential) = issued else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(cookie) = HeaderValue::from_str(&format!(
        "{}={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=28800",
        tabula_session_http::isolated::SESSION_COOKIE,
        credential.expose_encoded()
    )) else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut reply = response(
        StatusCode::OK,
        serde_json::json!({"version":1,"new_current_record_issued":true}),
    );
    reply.headers_mut().insert(header::SET_COOKIE, cookie);
    reply
}

async fn oracle(State(state): State<Arc<Controls>>, request: Request) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (_, snapshot, body) = match state.http.authenticate_json::<Oracle>(request, LIMIT).await {
        Ok(value) => value,
        Err(reply) => return reply,
    };
    let Some(match_id) = identifier(&body.match_id) else {
        return rejected(StatusCode::BAD_REQUEST);
    };
    if body.version != 1 || body.expected_inputs > 4 {
        return rejected(StatusCode::BAD_REQUEST);
    }
    if !matches!(
        state
            .online
            .has_fixture_membership(snapshot.user_id(), match_id)
            .await,
        Ok(true)
    ) {
        return rejected(StatusCode::FORBIDDEN);
    }
    let Ok((version, index, count)) = state.matches.committed_prefix_for_test(match_id).await
    else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let expected = u64::from(body.expected_inputs);
    response(
        StatusCode::OK,
        serde_json::json!({"version":1,"expected_public_transcript_prefix":version.0==expected&&index.0==expected&&count==expected+1}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_reject_unknown_fields_and_unbounded_targets() {
        assert!(serde_json::from_str::<Arm>(
            r#"{"version":1,"match_id":"x","attachment_id":"y","point":"before_commit","seat":0}"#
        )
        .is_err());
        for raw in [
            "",
            "1",
            "00000000000000000000000000000000",
            "fffffffffffffffffffffffffffffffF",
        ] {
            assert!(identifier(raw).is_none());
        }
        assert!(identifier("00000000000000000000000000000001").is_some());
    }
    #[test]
    fn duplicate_control_headers_fail_closed() {
        let mut headers = HeaderMap::new();
        headers.append("x-tabula-fixture-control", HeaderValue::from_static("a"));
        headers.append("x-tabula-fixture-control", HeaderValue::from_static("b"));
        assert!(single(&headers, "x-tabula-fixture-control").is_none());
    }
}
