//! Non-default native harness adapter for ADR-0036. Not a production bootstrap.
//! It validates transport before durable authority; context/self-profile bodies
//! obtain fresh storage-owned leases and release one bounded frame only. Native
//! credential responses follow durable winning-rotation semantics separately.

use std::{
    collections::BTreeMap,
    fmt,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use bytes::Bytes;
use hmac::{Hmac, Mac};
use http_body::{Body as HttpBody, Frame, SizeHint};
use sha2::Sha256;
use tabula_session::{
    CredentialOperation, HttpSessionAuthority, RotateCredential, SessionChannel,
    SessionContextBinding, SessionCredential, SessionError, SessionPublication, SessionSnapshot,
};

use crate::{
    AccountCapabilities, ContextResponse, NativeRefreshResponse, PublicProblem,
    SelfProfileResponse, SessionDisposition, HTTP_CONTRACT_VERSION,
};

pub const SESSION_COOKIE: &str = "__Host-tabula_session";
pub const PREAUTH_COOKIE: &str = "__Host-tabula_preauth";
const JSON_LIMIT: usize = 1024;
const BODY_DEADLINE: Duration = Duration::from_secs(5);
const PREAUTH_LIFETIME: Duration = Duration::from_secs(600);
const PREAUTH_CAPACITY: usize = 256;
const BOOTSTRAP_PER_MINUTE: u16 = 120;

/// Explicit named HTTPS origin and independently random per-process CSRF key.
/// The owner injects authority; construction opens no listener or provider flow.
pub struct IsolatedSessionHttp<A> {
    state: Arc<HttpState<A>>,
}
struct HttpState<A> {
    authority: A,
    origin: String,
    csrf_key: String,
    preauth: Mutex<PreauthState>,
}
struct PreauthState {
    entries: BTreeMap<String, Instant>,
    window: Instant,
    requests: u16,
}
impl<A> Clone for IsolatedSessionHttp<A> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}
impl<A> fmt::Debug for IsolatedSessionHttp<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IsolatedSessionHttp")
            .field("origin", &self.state.origin)
            .finish_non_exhaustive()
    }
}

impl<A: HttpSessionAuthority + 'static> IsolatedSessionHttp<A> {
    pub fn new(authority: A, trusted_origin: &str) -> Result<Self, SessionError> {
        let uri: Uri = trusted_origin
            .parse()
            .map_err(|_| SessionError::InvalidInput)?;
        let authority_text = trusted_origin
            .strip_prefix("https://")
            .ok_or(SessionError::InvalidInput)?;
        if uri.scheme_str() != Some("https")
            || uri.host().is_none()
            || authority_text.is_empty()
            || authority_text
                .chars()
                .any(|c| matches!(c, '/' | '?' | '#' | '@') || c.is_whitespace())
        {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self {
            state: Arc::new(HttpState {
                authority,
                origin: trusted_origin.to_owned(),
                csrf_key: SessionCredential::generate()?.expose_encoded(),
                preauth: Mutex::new(PreauthState {
                    entries: BTreeMap::new(),
                    window: Instant::now(),
                    requests: 0,
                }),
            }),
        })
    }

    /// Exact opt-in isolated routes. No CORS, listener, production startup or WS.
    pub fn router(self) -> Router {
        Router::new()
            .route("/api/v1/auth/context", get(context::<A>))
            .route("/api/v1/me", get(profile::<A>))
            .route("/api/v1/auth/refresh", post(refresh::<A>))
            .route("/api/v1/auth/logout", post(logout::<A>))
            .route("/api/v1/auth/login", post(unavailable))
            .route("/api/v1/auth/register", post(unavailable))
            .route("/api/v1/friends", get(unavailable))
            .fallback(|| async { problem(StatusCode::NOT_FOUND, "not_found") })
            .layer(middleware::from_fn(no_store))
            .with_state(self.state)
    }
}

async fn no_store(request: Request, next: Next) -> Response {
    let mut response = if request.method() == Method::OPTIONS {
        problem(StatusCode::FORBIDDEN, "request_rejected")
    } else {
        next.run(request).await
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn problem(status: StatusCode, code: &str) -> Response {
    let body = PublicProblem {
        version: HTTP_CONTRACT_VERSION,
        status: status.as_u16(),
        title: match status {
            StatusCode::UNAUTHORIZED => "Unauthenticated",
            StatusCode::SERVICE_UNAVAILABLE => "Unavailable",
            _ => "Request rejected",
        }
        .to_owned(),
        code: code.to_owned(),
    };
    json(status, &body, "application/problem+json")
}
fn json(status: StatusCode, value: &impl serde::Serialize, content_type: &'static str) -> Response {
    if let Ok(bytes) = serde_json::to_vec(value) {
        let mut response = Response::new(Body::from(bytes));
        *response.status_mut() = status;
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        response
    } else {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
        response
    }
}
fn session_rejection(error: SessionError) -> Rejection {
    match error {
        SessionError::Unauthenticated => reject(StatusCode::UNAUTHORIZED, "unauthenticated"),
        SessionError::Conflict => reject(StatusCode::CONFLICT, "conflict"),
        SessionError::InvalidInput => reject(StatusCode::FORBIDDEN, "request_rejected"),
        SessionError::Unavailable => reject(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    }
}
fn session_problem(error: SessionError) -> Response {
    session_rejection(error).response()
}
async fn unavailable() -> Response {
    problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
}

struct Rejection {
    status: StatusCode,
    code: &'static str,
}
impl Rejection {
    fn response(self) -> Response {
        problem(self.status, self.code)
    }
}
fn reject(status: StatusCode, code: &'static str) -> Rejection {
    Rejection { status, code }
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, Rejection> {
    let mut values = headers.get_all(name).iter();
    let value = values.next();
    if values.next().is_some() {
        return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
    }
    value
        .map(|value| {
            value
                .to_str()
                .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))
        })
        .transpose()
}

struct Input {
    credential: Option<SessionCredential>,
    channel: SessionChannel,
    preauth_cookie: Option<String>,
}
fn transport<A>(
    state: &HttpState<A>,
    headers: &HeaderMap,
    unsafe_method: bool,
) -> Result<Input, Rejection> {
    let authorization = single(headers, "authorization")?;
    let any_cookie = headers.contains_key(header::COOKIE);
    if authorization.is_some() && any_cookie {
        return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
    }
    let origin = single(headers, "origin")?;
    if origin.is_some_and(|origin| origin != state.origin)
        || (unsafe_method && authorization.is_none() && origin.is_none())
    {
        return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
    }
    if let Some(site) = single(headers, "sec-fetch-site")? {
        if site != "same-origin" && (site != "none" || unsafe_method) {
            return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
        }
    }
    if let Some(authorization) = authorization {
        let encoded = authorization
            .strip_prefix("Bearer ")
            .ok_or_else(|| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
        let credential = SessionCredential::parse(encoded)
            .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
        return Ok(Input {
            credential: Some(credential),
            channel: SessionChannel::NativeBearer,
            preauth_cookie: None,
        });
    }
    let mut session = None;
    let mut preauth = None;
    for value in headers.get_all(header::COOKIE) {
        let cookies = value
            .to_str()
            .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
        for pair in cookies.split(';') {
            let (name, value) = pair
                .trim()
                .split_once('=')
                .ok_or_else(|| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
            if name == SESSION_COOKIE {
                if session.is_some() {
                    return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
                }
                session = Some(
                    SessionCredential::parse(value)
                        .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?,
                );
            }
            if name == PREAUTH_COOKIE {
                if preauth.is_some() {
                    return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
                }
                preauth = Some(value.to_owned());
            }
        }
    }
    Ok(Input {
        credential: session,
        channel: SessionChannel::BrowserCookie,
        preauth_cookie: preauth,
    })
}

fn operation(input: &Input) -> Result<CredentialOperation, Rejection> {
    Ok(CredentialOperation {
        digest: input
            .credential
            .as_ref()
            .ok_or_else(|| reject(StatusCode::UNAUTHORIZED, "unauthenticated"))?
            .digest(),
        channel: input.channel,
        context: None,
    })
}
fn csrf<A>(state: &HttpState<A>, snapshot: &SessionSnapshot) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-isolated-csrf-v1\0");
    mac.update(&snapshot.id().get().to_be_bytes());
    mac.update(&snapshot.context_id().get().to_be_bytes());
    mac.update(&snapshot.user_id().0.to_be_bytes());
    mac.update(&snapshot.authorization_epoch().get().to_be_bytes());
    mac.update(snapshot.channel().as_str().as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}
fn verify_csrf<A>(state: &HttpState<A>, snapshot: &SessionSnapshot, token: &str) -> bool {
    // HMAC's constant-time verifier; canonical size/encoding is checked first.
    let Ok(raw) = URL_SAFE_NO_PAD.decode(token) else {
        return false;
    };
    if raw.len() != 32 || URL_SAFE_NO_PAD.encode(&raw) != token {
        return false;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-isolated-csrf-v1\0");
    mac.update(&snapshot.id().get().to_be_bytes());
    mac.update(&snapshot.context_id().get().to_be_bytes());
    mac.update(&snapshot.user_id().0.to_be_bytes());
    mac.update(&snapshot.authorization_epoch().get().to_be_bytes());
    mac.update(snapshot.channel().as_str().as_bytes());
    mac.verify_slice(&raw).is_ok()
}
fn capabilities(authenticated: bool) -> AccountCapabilities {
    AccountCapabilities {
        login: false,
        register: false,
        friends: false,
        read_self_profile: authenticated,
    }
}
fn public_context(disposition: SessionDisposition) -> ContextResponse {
    ContextResponse {
        version: HTTP_CONTRACT_VERSION,
        disposition,
        account_id: None,
        csrf_token: None,
        capabilities: capabilities(false),
    }
}

async fn context<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    }
    let input = match transport(&state, request.headers(), false) {
        Ok(input) => input,
        Err(error) => return error.response(),
    };
    if input.credential.is_none() {
        return signed_out_context(&state, input.preauth_cookie.as_deref());
    }
    let operation = match operation(&input) {
        Ok(operation) => operation,
        Err(error) => return error.response(),
    };
    match state.authority.begin_publication(operation).await {
        Ok(guard) => {
            let snapshot = guard.snapshot();
            let value = ContextResponse {
                version: HTTP_CONTRACT_VERSION,
                disposition: SessionDisposition::Authenticated,
                account_id: Some(format!("{:032x}", snapshot.user_id().0)),
                csrf_token: (snapshot.channel() == SessionChannel::BrowserCookie)
                    .then(|| csrf(&state, snapshot)),
                capabilities: capabilities(true),
            };
            guarded_json(guard, &value)
        }
        Err(SessionError::Unauthenticated) => json(
            StatusCode::OK,
            &public_context(SessionDisposition::SignedOut),
            "application/json",
        ),
        Err(_) => json(
            StatusCode::SERVICE_UNAVAILABLE,
            &public_context(SessionDisposition::Unavailable),
            "application/json",
        ),
    }
}
fn signed_out_context<A>(state: &HttpState<A>, cookie: Option<&str>) -> Response {
    let now = Instant::now();
    let Ok(mut preauth) = state.preauth.lock() else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if now.duration_since(preauth.window) >= Duration::from_secs(60) {
        preauth.window = now;
        preauth.requests = 0;
    }
    if preauth.requests >= BOOTSTRAP_PER_MINUTE {
        return problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    }
    preauth.requests += 1;
    preauth.entries.retain(|_, expires| *expires > now);
    let retained = cookie.filter(|cookie| preauth.entries.contains_key(*cookie));
    let (cookie, fresh) = if let Some(cookie) = retained {
        (cookie.to_owned(), false)
    } else {
        if preauth.entries.len() >= PREAUTH_CAPACITY {
            return problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
        }
        let cookie = match SessionCredential::generate() {
            Ok(cookie) => cookie.expose_encoded(),
            Err(error) => return session_problem(error),
        };
        preauth
            .entries
            .insert(cookie.clone(), now + PREAUTH_LIFETIME);
        (cookie, true)
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-isolated-preauth-csrf-v1\0");
    mac.update(cookie.as_bytes());
    let mut value = public_context(SessionDisposition::SignedOut);
    value.csrf_token = Some(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()));
    let mut response = json(StatusCode::OK, &value, "application/json");
    if fresh {
        set_cookie(
            &mut response,
            format!("{PREAUTH_COOKIE}={cookie}; Secure; HttpOnly; SameSite=Lax; Path=/"),
        );
    }
    response
}

async fn profile<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    }
    let input = match transport(&state, request.headers(), false) {
        Ok(input) => input,
        Err(error) => return error.response(),
    };
    let operation = match operation(&input) {
        Ok(operation) => operation,
        Err(error) => return error.response(),
    };
    match state.authority.begin_publication(operation).await {
        Ok(guard) => {
            let value = SelfProfileResponse {
                version: HTTP_CONTRACT_VERSION,
                account_id: format!("{:032x}", guard.snapshot().user_id().0),
            };
            guarded_json(guard, &value)
        }
        Err(error) => session_problem(error),
    }
}

async fn unsafe_operation<A: HttpSessionAuthority>(
    state: &HttpState<A>,
    request: Request,
    for_logout: bool,
) -> Result<(Input, CredentialOperation, SessionSnapshot), Rejection> {
    if request.uri().query().is_some() {
        return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
    }
    let input = transport(state, request.headers(), true)?;
    let content_type = single(request.headers(), "content-type")?
        .ok_or_else(|| reject(StatusCode::UNSUPPORTED_MEDIA_TYPE, "request_rejected"))?;
    if !matches!(
        content_type,
        "application/json" | "application/json; charset=utf-8"
    ) {
        return Err(reject(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_rejected",
        ));
    }
    if request.headers().contains_key(header::CONTENT_ENCODING) {
        return Err(reject(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_rejected",
        ));
    }
    let token = single(request.headers(), "x-tabula-csrf")?.map(str::to_owned);
    if input.channel == SessionChannel::BrowserCookie
        && token.as_ref().is_none_or(|token| token.len() != 43)
    {
        return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
    }
    let bytes = tokio::time::timeout(BODY_DEADLINE, to_bytes(request.into_body(), JSON_LIMIT))
        .await
        .map_err(|_| reject(StatusCode::REQUEST_TIMEOUT, "request_rejected"))?
        .map_err(|_| reject(StatusCode::PAYLOAD_TOO_LARGE, "request_rejected"))?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
    if !body.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
    }
    let mut operation = operation(&input)?;
    let snapshot = if for_logout {
        state.authority.read_logout_context(operation).await
    } else {
        state.authority.read_session(operation).await
    }
    .map_err(session_rejection)?;
    if input.channel == SessionChannel::BrowserCookie {
        if !verify_csrf(state, &snapshot, token.as_deref().unwrap_or_default()) {
            return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
        }
        operation.context = Some(SessionContextBinding {
            context_id: snapshot.context_id(),
            authorization_epoch: snapshot.authorization_epoch(),
        });
    }
    Ok((input, operation, snapshot))
}
async fn refresh<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    let (input, credential, snapshot) = match unsafe_operation(&state, request, false).await {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    let replacement = match SessionCredential::generate() {
        Ok(replacement) => replacement,
        Err(error) => return session_problem(error),
    };
    let request = RotateCredential {
        credential,
        expected_generation: snapshot.credential_generation(),
        replacement_digest: replacement.digest(),
    };
    match state.authority.rotate_credential(request).await {
        Ok(_) if input.channel == SessionChannel::BrowserCookie => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::NO_CONTENT;
            set_cookie(
                &mut response,
                format!(
                    "{SESSION_COOKIE}={}; Secure; HttpOnly; SameSite=Lax; Path=/",
                    replacement.expose_encoded()
                ),
            );
            response
        }
        Ok(_) => json(
            StatusCode::OK,
            &NativeRefreshResponse {
                version: HTTP_CONTRACT_VERSION,
                credential: replacement.expose_encoded(),
            },
            "application/json",
        ),
        Err(error) => session_problem(error),
    }
}
async fn logout<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    let (input, credential, _) = match unsafe_operation(&state, request, true).await {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    match state.authority.revoke_credential(credential).await {
        Ok(()) => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::NO_CONTENT;
            if input.channel == SessionChannel::BrowserCookie {
                set_cookie(
                    &mut response,
                    format!("{SESSION_COOKIE}=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
                );
                set_cookie(
                    &mut response,
                    format!("{PREAUTH_COOKIE}=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
                );
            }
            response
        }
        Err(error) => session_problem(error),
    }
}
fn set_cookie(response: &mut Response, cookie: impl AsRef<str>) {
    if let Ok(value) = HeaderValue::from_str(cookie.as_ref()) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// One bounded private frame. Storage's synchronous callback is the logical
/// handoff point; already released Hyper/TCP bytes are outside this boundary.
struct GuardedBody<P> {
    publication: Option<Box<P>>,
    bytes: Option<Bytes>,
}
impl<P: SessionPublication> HttpBody for GuardedBody<P> {
    type Data = Bytes;
    type Error = SessionError;
    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let body = self.get_mut();
        let Some(bytes) = body.bytes.take() else {
            body.publication.take();
            return Poll::Ready(None);
        };
        let Some(publication) = body.publication.as_mut() else {
            return Poll::Ready(Some(Err(SessionError::Unavailable)));
        };
        Poll::Ready(Some(publication.publish(|_| Frame::data(bytes))))
    }
    fn is_end_stream(&self) -> bool {
        self.bytes.is_none()
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
fn guarded_json<P: SessionPublication + 'static>(
    publication: P,
    value: &impl serde::Serialize,
) -> Response {
    match serde_json::to_vec(value) {
        Ok(bytes) => {
            let mut response = Response::new(Body::new(GuardedBody {
                publication: Some(Box::new(publication)),
                bytes: Some(Bytes::from(bytes)),
            }));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
        }
        Err(_) => problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    }
}
