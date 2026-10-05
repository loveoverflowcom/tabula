//! Opt-in disposable PR2 first-body-frame publication fixture (ADR-0041).
//!
//! The native witness establishes only that a real authenticated poll captured
//! a nonempty projected queue. The real client must still observe the forwarded
//! inner guard error and zero response-body bytes. This module never supplies
//! session/match authority, changes private bytes, or simulates suppression.
#![cfg(feature = "body-publication-test")]

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::post,
    Router,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tabula_match_http::{isolated::PollCaptureWitness, MatchPollRequest, MAX_REQUEST_BYTES};
use tabula_session::{AuthSessionId, CredentialDigest, SessionCredential};
use tabula_session_http::isolated::IsolatedSessionHttp;
use tabula_storage::session::PgSessionStore;
use tokio::sync::{oneshot, Semaphore};

const ORIGIN: &str = "https://localhost:9443";
const CONTROL_HEADER: &str = "x-tabula-fixture-control";
const MAX_CONTROL_BYTES: usize = 1024;
const BODY_DEADLINE: Duration = Duration::from_secs(2);
const FLOW_LIFETIME: Duration = Duration::from_secs(60);

type FlowSlot = Arc<Mutex<Option<Flow>>>;

struct FixtureState {
    session_http: IsolatedSessionHttp<PgSessionStore>,
    flow: FlowSlot,
    // Bound requests buffered outside the real gateway's own admission layer.
    reads: Arc<Semaphore>,
}

#[derive(Clone)]
struct Target {
    match_id: String,
    attachment_id: String,
    record: AuthSessionId,
}

struct Flow {
    target: Target,
    control: CredentialDigest,
    deadline: Instant,
    phase: Phase,
    capture_in_flight: bool,
    release: Option<oneshot::Sender<()>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Armed,
    Held,
    Failed,
    Released,
    Suppressed,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Armed => "armed",
            Self::Held => "held",
            Self::Failed => "failed",
            Self::Released => "released",
            Self::Suppressed => "suppressed",
        }
    }
}

impl Flow {
    fn expire(&mut self) {
        if Instant::now() >= self.deadline {
            self.cancel();
        }
    }

    fn cancel(&mut self) {
        if matches!(self.phase, Phase::Armed | Phase::Held | Phase::Released) {
            self.phase = Phase::Failed;
        }
        self.capture_in_flight = false;
        // Dropping, rather than sending, closes the body gate without polling
        // the real private body. Cancellation can never establish suppression.
        self.release.take();
    }
}

/// Layer the actual poll body and add bounded fixture controls (ADR-0041).
///
/// The caller compiles this module only with `body-publication-test`. Exactly
/// one flow may be armed in this disposable process; no listener is opened.
pub fn compose(router: Router, session_http: IsolatedSessionHttp<PgSessionStore>) -> Router {
    let state = Arc::new(FixtureState {
        session_http,
        flow: Arc::new(Mutex::new(None)),
        reads: Arc::new(Semaphore::new(4)),
    });
    let controls = Router::new()
        .route("/__fixture/publication/arm", post(arm))
        .route("/__fixture/publication/status", post(status))
        .route("/__fixture/publication/release", post(release))
        .layer(middleware::from_fn(control_no_store))
        .with_state(state.clone());
    router
        .layer(middleware::from_fn_with_state(state, hold_poll))
        .merge(controls)
}

#[derive(Deserialize)]
#[serde(try_from = "RawArm")]
struct Arm {
    match_id: String,
    attachment_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArm {
    version: u16,
    match_id: String,
    attachment_id: String,
}

impl TryFrom<RawArm> for Arm {
    type Error = &'static str;

    fn try_from(raw: RawArm) -> Result<Self, Self::Error> {
        if raw.version != 1 || !identifier(&raw.match_id) || !identifier(&raw.attachment_id) {
            return Err("fixture request invalid");
        }
        Ok(Self {
            match_id: raw.match_id,
            attachment_id: raw.attachment_id,
        })
    }
}

fn identifier(raw: &str) -> bool {
    raw.len() == 32
        && raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && raw.bytes().any(|byte| byte != b'0')
}

#[derive(Deserialize)]
#[serde(try_from = "RawControl")]
struct Control;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawControl {
    version: u16,
}

impl TryFrom<RawControl> for Control {
    type Error = &'static str;

    fn try_from(raw: RawControl) -> Result<Self, Self::Error> {
        if raw.version != 1 {
            return Err("fixture request invalid");
        }
        Ok(Self)
    }
}

#[derive(Serialize)]
struct Armed {
    version: u16,
    control_token: String,
}

#[derive(Serialize)]
struct Status {
    version: u16,
    phase: &'static str,
}

fn no_store(mut response: Response) -> Response {
    for (key, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::PRAGMA, "no-cache"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ] {
        response
            .headers_mut()
            .insert(key, HeaderValue::from_static(value));
    }
    response
}

async fn control_no_store(request: Request, next: Next) -> Response {
    no_store(next.run(request).await)
}

fn rejected(status: StatusCode) -> Response {
    let mut response = Response::new(Body::from("{\"code\":\"fixture_request_rejected\"}"));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    no_store(response)
}

fn json(value: &impl Serialize) -> Response {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    no_store(response)
}

fn phase_response(phase: Phase) -> Response {
    json(&Status {
        version: 1,
        phase: phase.as_str(),
    })
}

async fn arm(State(state): State<Arc<FixtureState>>, request: Request) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (_, snapshot, body) = match state
        .session_http
        .authenticate_json::<Arm>(request, MAX_CONTROL_BYTES)
        .await
    {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let Ok(control) = SessionCredential::generate() else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let deadline = Instant::now() + FLOW_LIFETIME;
    {
        let Ok(mut slot) = state.flow.lock() else {
            return rejected(StatusCode::SERVICE_UNAVAILABLE);
        };
        if slot.is_some() {
            return rejected(StatusCode::CONFLICT);
        }
        *slot = Some(Flow {
            target: Target {
                match_id: body.match_id,
                attachment_id: body.attachment_id,
                record: snapshot.id(),
            },
            control: control.digest(),
            deadline,
            phase: Phase::Armed,
            capture_in_flight: false,
            release: None,
        });
    }
    // Capacity is one for the process, so this is also at most one cleanup
    // task. It closes an abandoned gate even if no further request arrives.
    let slot = state.flow.clone();
    tokio::spawn(async move {
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
        if let Ok(mut slot) = slot.lock() {
            if let Some(flow) = slot.as_mut() {
                flow.expire();
            }
        };
    });
    json(&Armed {
        version: 1,
        control_token: control.expose_encoded(),
    })
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

fn control_headers(headers: &HeaderMap) -> Result<CredentialDigest, StatusCode> {
    if single(headers, "origin") != Some(ORIGIN)
        || single(headers, "content-type") != Some("application/json")
        || headers.contains_key(header::CONTENT_ENCODING)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    SessionCredential::parse(single(headers, CONTROL_HEADER).ok_or(StatusCode::FORBIDDEN)?)
        .map(|token| token.digest())
        .map_err(|_| StatusCode::FORBIDDEN)
}

fn bounded_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StatusCode> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    serde_json::from_slice(bytes).map_err(|_| StatusCode::BAD_REQUEST)
}

async fn control_request(request: Request) -> Result<CredentialDigest, StatusCode> {
    if request.method() != Method::POST || request.uri().query().is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let control = control_headers(request.headers())?;
    let bytes = tokio::time::timeout(
        BODY_DEADLINE,
        to_bytes(request.into_body(), MAX_CONTROL_BYTES),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
    .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    bounded_json::<Control>(&bytes)?;
    Ok(control)
}

async fn status(State(state): State<Arc<FixtureState>>, request: Request) -> Response {
    control(state, request, false).await
}

async fn release(State(state): State<Arc<FixtureState>>, request: Request) -> Response {
    control(state, request, true).await
}

async fn control(state: Arc<FixtureState>, request: Request, release: bool) -> Response {
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let credential = match control_request(request).await {
        Ok(value) => value,
        Err(status) => return rejected(status),
    };
    let Ok(mut slot) = state.flow.lock() else {
        return rejected(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Some(flow) = slot.as_mut() else {
        return rejected(StatusCode::FORBIDDEN);
    };
    if flow.control != credential {
        return rejected(StatusCode::FORBIDDEN);
    }
    flow.expire();
    if Instant::now() >= flow.deadline {
        return rejected(StatusCode::GONE);
    }
    if release {
        if flow.phase != Phase::Held {
            return rejected(StatusCode::CONFLICT);
        }
        let Some(sender) = flow.release.take() else {
            flow.cancel();
            return rejected(StatusCode::CONFLICT);
        };
        flow.phase = Phase::Released;
        if sender.send(()).is_err() {
            flow.cancel();
            return rejected(StatusCode::CONFLICT);
        }
    }
    phase_response(flow.phase)
}

// A dropped/cancelled actual handler cannot leave a successful capture behind.
struct CaptureCancellation {
    slot: FlowSlot,
}

impl Drop for CaptureCancellation {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(flow) = slot.as_mut() {
                if flow.capture_in_flight {
                    flow.cancel();
                }
            }
        }
    }
}

async fn hold_poll(
    State(state): State<Arc<FixtureState>>,
    request: Request,
    next: Next,
) -> Response {
    let target = {
        let Ok(mut slot) = state.flow.lock() else {
            return rejected(StatusCode::SERVICE_UNAVAILABLE);
        };
        slot.as_mut().and_then(|flow| {
            flow.expire();
            let path = format!("/api/v1/matches/{}/poll", flow.target.match_id);
            (flow.phase == Phase::Armed
                && !flow.capture_in_flight
                && request.method() == Method::POST
                && request.uri().path() == path
                && request.uri().query().is_none())
            .then(|| flow.target.clone())
        })
    };
    let Some(target) = target else {
        return next.run(request).await;
    };
    let Ok(_permit) = state.reads.clone().try_acquire_owned() else {
        return rejected(StatusCode::TOO_MANY_REQUESTS);
    };
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(BODY_DEADLINE, to_bytes(body, MAX_REQUEST_BYTES)).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return rejected(StatusCode::PAYLOAD_TOO_LARGE),
        Err(_) => return rejected(StatusCode::REQUEST_TIMEOUT),
    };
    let attachment_matches = serde_json::from_slice::<MatchPollRequest>(&bytes)
        .is_ok_and(|poll| poll.attachment_id() == target.attachment_id);
    // Reconstitute the exact bytes with the original method, URI and headers.
    // The real handler still performs every transport/authentication check.
    let request = Request::from_parts(parts, Body::from(bytes));
    if !attachment_matches {
        return next.run(request).await;
    }
    let reserved = {
        let Ok(mut slot) = state.flow.lock() else {
            return rejected(StatusCode::SERVICE_UNAVAILABLE);
        };
        slot.as_mut().is_some_and(|flow| {
            flow.expire();
            if flow.phase != Phase::Armed || flow.capture_in_flight {
                return false;
            }
            flow.capture_in_flight = true;
            true
        })
    };
    if !reserved {
        return next.run(request).await;
    }
    let cancellation = CaptureCancellation {
        slot: state.flow.clone(),
    };
    let response = next.run(request).await;
    let captured = response.status() == StatusCode::OK
        && single(response.headers(), "content-type") == Some("application/json")
        && response
            .extensions()
            .get::<PollCaptureWitness>()
            .is_some_and(|witness| {
                witness.match_id() == target.match_id
                    && witness.attachment_id() == target.attachment_id
                    && witness.record() == target.record
                    && witness.projected_frames() > 0
            });
    let receiver = {
        let Ok(mut slot) = state.flow.lock() else {
            return response;
        };
        let Some(flow) = slot.as_mut() else {
            return response;
        };
        flow.expire();
        if flow.phase != Phase::Armed || !flow.capture_in_flight {
            return response;
        }
        flow.capture_in_flight = false;
        if !captured {
            flow.cancel();
            return response;
        }
        let (sender, receiver) = oneshot::channel();
        flow.release = Some(sender);
        flow.phase = Phase::Held;
        receiver
    };
    drop(cancellation);
    let (parts, inner) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(HeldBody::new(inner, receiver, Some(state.flow.clone()))),
    )
}

enum Gate {
    Waiting(oneshot::Receiver<()>),
    Open,
    Cancelled,
}

struct HeldBody<B: HttpBody> {
    inner: Option<B>,
    original_hint: SizeHint,
    gate: Gate,
    observer: Option<FlowSlot>,
    observed_first: bool,
}

impl<B: HttpBody> HeldBody<B> {
    fn new(inner: B, release: oneshot::Receiver<()>, observer: Option<FlowSlot>) -> Self {
        Self {
            original_hint: inner.size_hint(),
            inner: Some(inner),
            gate: Gate::Waiting(release),
            observer,
            observed_first: false,
        }
    }

    fn observe(&mut self, phase: Phase) {
        if self.observed_first {
            return;
        }
        self.observed_first = true;
        self.record(phase);
    }

    fn record(&mut self, phase: Phase) {
        if let Some(slot) = &self.observer {
            if let Ok(mut slot) = slot.lock() {
                if let Some(flow) = slot.as_mut() {
                    if matches!(flow.phase, Phase::Released | Phase::Suppressed) {
                        flow.phase = phase;
                    } else if matches!(flow.phase, Phase::Armed | Phase::Held) {
                        flow.cancel();
                    }
                }
            }
        }
    }
}

impl<B: HttpBody + Unpin> HttpBody for HeldBody<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let body = self.get_mut();
        if let Gate::Waiting(receiver) = &mut body.gate {
            match Pin::new(receiver).poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => body.gate = Gate::Open,
                Poll::Ready(Err(_)) => {
                    body.gate = Gate::Cancelled;
                    body.inner.take();
                    body.observe(Phase::Failed);
                    return Poll::Ready(None);
                }
            }
        }
        if matches!(body.gate, Gate::Cancelled) {
            return Poll::Ready(None);
        }
        let Some(inner) = body.inner.as_mut() else {
            return Poll::Ready(None);
        };
        let frame = Pin::new(inner).poll_frame(cx);
        match &frame {
            Poll::Ready(Some(Err(_))) => body.observe(Phase::Suppressed),
            Poll::Ready(Some(Ok(_))) => {
                body.observed_first = true;
                body.record(Phase::Failed);
            }
            Poll::Ready(None) => body.observe(Phase::Failed),
            Poll::Pending => {}
        }
        // Forward the original result, including any unexpectedly delivered
        // private data. Client byte assertions must detect a broken real guard.
        frame
    }

    fn is_end_stream(&self) -> bool {
        matches!(self.gate, Gate::Cancelled)
            || self.inner.as_ref().is_none_or(HttpBody::is_end_stream)
    }

    fn size_hint(&self) -> SizeHint {
        self.inner
            .as_ref()
            .map_or_else(|| self.original_hint, HttpBody::size_hint)
    }
}

impl<B: HttpBody> Drop for HeldBody<B> {
    fn drop(&mut self) {
        self.observe(Phase::Failed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::{
        collections::VecDeque,
        sync::atomic::{AtomicUsize, Ordering},
        task::Waker,
    };

    struct ProbeBody {
        polls: Arc<AtomicUsize>,
        frames: VecDeque<Result<Frame<Bytes>, &'static str>>,
    }

    impl HttpBody for ProbeBody {
        type Data = Bytes;
        type Error = &'static str;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(self.get_mut().frames.pop_front())
        }

        fn size_hint(&self) -> SizeHint {
            SizeHint::with_exact(17)
        }
    }

    fn probe(
        frames: Vec<Result<Frame<Bytes>, &'static str>>,
    ) -> (HeldBody<ProbeBody>, oneshot::Sender<()>, Arc<AtomicUsize>) {
        let polls = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = oneshot::channel();
        (
            HeldBody::new(
                ProbeBody {
                    polls: polls.clone(),
                    frames: frames.into(),
                },
                receiver,
                None,
            ),
            sender,
            polls,
        )
    }

    #[test]
    fn held_gate_never_polls_inner_before_explicit_release() {
        let (mut body, sender, polls) = probe(vec![Ok(Frame::data(Bytes::from_static(b"real")))]);
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(body.size_hint().exact(), Some(17));
        for _ in 0..3 {
            assert!(matches!(
                Pin::new(&mut body).poll_frame(&mut cx),
                Poll::Pending
            ));
        }
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        sender.send(()).unwrap();
        let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
            panic!("released body did not forward its actual data frame");
        };
        assert_eq!(frame.into_data().unwrap(), Bytes::from_static(b"real"));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn released_gate_forwards_real_data_and_error_unchanged() {
        let (mut body, sender, polls) = probe(vec![
            Ok(Frame::data(Bytes::from_static(b"unexpected private bytes"))),
            Err("actual inner error"),
        ]);
        sender.send(()).unwrap();
        let mut cx = Context::from_waker(Waker::noop());
        let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
            panic!("actual private data was discarded");
        };
        assert_eq!(
            frame.into_data().unwrap(),
            Bytes::from_static(b"unexpected private bytes")
        );
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err("actual inner error")))
        ));
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(None)
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn cancelled_gate_never_polls_or_manufactures_inner_error() {
        let (mut body, sender, polls) = probe(vec![Err("must never be polled")]);
        drop(sender);
        let mut cx = Context::from_waker(Waker::noop());
        for _ in 0..3 {
            assert!(matches!(
                Pin::new(&mut body).poll_frame(&mut cx),
                Poll::Ready(None)
            ));
        }
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert!(body.inner.is_none());
        assert_eq!(body.size_hint().exact(), Some(17));
    }

    fn observed_flow(sender: oneshot::Sender<()>) -> FlowSlot {
        Arc::new(Mutex::new(Some(Flow {
            target: Target {
                match_id: "00000000000000000000000000000001".into(),
                attachment_id: "00000000000000000000000000000002".into(),
                record: AuthSessionId::new(3).unwrap(),
            },
            control: CredentialDigest::from_bytes([0; 32]),
            deadline: Instant::now() + FLOW_LIFETIME,
            phase: Phase::Held,
            capture_in_flight: false,
            release: Some(sender),
        })))
    }

    fn observed_phase(slot: &FlowSlot) -> Phase {
        slot.lock().unwrap().as_ref().unwrap().phase
    }

    fn open_observed_gate(slot: &FlowSlot) {
        let mut slot = slot.lock().unwrap();
        let flow = slot.as_mut().unwrap();
        flow.phase = Phase::Released;
        flow.release.take().unwrap().send(()).unwrap();
    }

    #[test]
    fn suppression_phase_requires_a_real_inner_error_and_cannot_hide_data() {
        let (mut body, sender, polls) = probe(vec![Err("actual guard error")]);
        let slot = observed_flow(sender);
        body.observer = Some(slot.clone());
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Pending
        ));
        assert!(observed_phase(&slot) == Phase::Held);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        open_observed_gate(&slot);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err("actual guard error")))
        ));
        assert!(observed_phase(&slot) == Phase::Suppressed);
        drop(body);
        assert!(observed_phase(&slot) == Phase::Suppressed);

        let (mut body, sender, _) = probe(vec![
            Ok(Frame::data(Bytes::from_static(b"private data"))),
            Err("later error cannot erase delivered data"),
        ]);
        let slot = observed_flow(sender);
        body.observer = Some(slot.clone());
        open_observed_gate(&slot);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Ok(_)))
        ));
        assert!(observed_phase(&slot) == Phase::Failed);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err("later error cannot erase delivered data")))
        ));
        assert!(observed_phase(&slot) == Phase::Failed);
    }

    #[test]
    fn expired_or_dropped_capture_cancels_without_polling_inner() {
        let (mut body, sender, polls) = probe(vec![Err("must not prove suppression")]);
        let slot = observed_flow(sender);
        body.observer = Some(slot.clone());
        {
            let mut slot = slot.lock().unwrap();
            let flow = slot.as_mut().unwrap();
            flow.deadline = Instant::now() - Duration::from_secs(1);
            flow.expire();
        }
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(None)
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert!(observed_phase(&slot) == Phase::Failed);

        let (_, sender, _) = probe(vec![]);
        let slot = observed_flow(sender);
        {
            let mut slot = slot.lock().unwrap();
            let flow = slot.as_mut().unwrap();
            flow.phase = Phase::Armed;
            flow.capture_in_flight = true;
        }
        drop(CaptureCancellation { slot: slot.clone() });
        let slot = slot.lock().unwrap();
        let flow = slot.as_ref().unwrap();
        assert!(flow.phase == Phase::Failed);
        assert!(!flow.capture_in_flight);
        assert!(flow.release.is_none());
    }

    #[test]
    fn control_dtos_require_exact_version_fields_and_nonzero_canonical_ids() {
        assert!(bounded_json::<Control>(br#"{"version":1}"#).is_ok());
        for bytes in [
            br#"{}"#.as_slice(),
            br#"{"version":0}"#,
            br#"{"version":2}"#,
            br#"{"version":1,"version":1}"#,
            br#"{"version":1,"record":"anything"}"#,
            br#"{"version":1} {}"#,
        ] {
            assert!(bounded_json::<Control>(bytes).is_err());
        }
        let id = "0000000000000000000000000000002a";
        let valid = format!("{{\"version\":1,\"match_id\":\"{id}\",\"attachment_id\":\"{id}\"}}");
        assert!(bounded_json::<Arm>(valid.as_bytes()).is_ok());
        for invalid in [
            valid.replace("\"version\":1", "\"version\":2"),
            valid.replace(id, "00000000000000000000000000000000"),
            valid.replace(id, "0000000000000000000000000000002A"),
            valid.replace(id, "2a"),
            valid.replace('}', ",\"seat\":0}"),
        ] {
            assert!(bounded_json::<Arm>(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn control_body_bound_is_inclusive_and_never_silently_truncates() {
        let mut bytes = br#"{"version":1}"#.to_vec();
        bytes.resize(MAX_CONTROL_BYTES, b' ');
        assert!(bounded_json::<Control>(&bytes).is_ok());
        bytes.push(b' ');
        assert_eq!(
            bounded_json::<Control>(&bytes).err(),
            Some(StatusCode::PAYLOAD_TOO_LARGE)
        );
    }

    #[test]
    fn controls_require_one_exact_origin_json_type_and_capability_header() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            CONTROL_HEADER,
            HeaderValue::from_static("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        );
        assert!(control_headers(&headers).is_ok());
        headers.append(CONTROL_HEADER, HeaderValue::from_static("duplicate"));
        assert!(control_headers(&headers).is_err());
        headers.remove(CONTROL_HEADER);
        headers.insert(CONTROL_HEADER, HeaderValue::from_static("malformed"));
        assert!(control_headers(&headers).is_err());
        headers.insert(
            CONTROL_HEADER,
            HeaderValue::from_static("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        );
        headers.append(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        assert!(control_headers(&headers).is_err());
        headers.remove(header::ORIGIN);
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://localhost:9443/"),
        );
        assert!(control_headers(&headers).is_err());
        headers.insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        assert!(control_headers(&headers).is_err());
    }
}
