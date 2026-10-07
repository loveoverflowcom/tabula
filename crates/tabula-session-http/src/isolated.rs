//! Non-default native harness adapter for ADR-0036. Not a production bootstrap.
//! It validates transport before durable authority; context/self-profile bodies
//! obtain fresh storage-owned leases and release one bounded frame only. Native
//! credential responses follow durable winning-rotation semantics separately.

use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Extension, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use bytes::Bytes;
use hmac::{Hmac, Mac};
use http_body::{Body as HttpBody, Frame, SizeHint};
use sha2::Sha256;
use tabula_session::{
    AuthSessionId, BrowserLoginCallback, BrowserLoginProvider, BrowserLoginStart,
    CompletedBrowserLogin, CredentialOperation, HttpSessionAuthority, IssueSession,
    RotateCredential, SessionChannel, SessionContextBinding, SessionContextId, SessionCredential,
    SessionError, SessionPublication, SessionSnapshot,
};

use url::Url;

use crate::{
    AccountCapabilities, ContextResponse, LoginStartResponse, NativeRefreshResponse, PublicProblem,
    SelfProfileResponse, SessionDisposition, HTTP_CONTRACT_VERSION,
};

pub const SESSION_COOKIE: &str = "__Host-tabula_session";
pub const PREAUTH_COOKIE: &str = "__Host-tabula_preauth";
const JSON_LIMIT: usize = 1024;
const BODY_DEADLINE: Duration = Duration::from_secs(5);
const PREAUTH_LIFETIME: Duration = Duration::from_secs(600);
const PREAUTH_CAPACITY: usize = 256;
const BOOTSTRAP_PER_MINUTE: u16 = 120;
const LOGIN_PER_MINUTE: u16 = 30;
const LOGIN_DEADLINE: Duration = Duration::from_secs(15);
const LOCK_DEADLINE: Duration = Duration::from_secs(20);
const CALLBACK_URL_LIMIT: usize = 4096;

type LoginFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, SessionError>> + Send + 'a>>;
trait ErasedBrowserLoginProvider: Send + Sync {
    fn begin(&self, binding: String) -> LoginFuture<'_, BrowserLoginStart>;
    fn complete(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
    ) -> LoginFuture<'_, CompletedBrowserLogin>;
    fn matches_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError>;
    fn cancel(&self, binding: &str);
}
impl<P: BrowserLoginProvider> ErasedBrowserLoginProvider for P {
    fn begin(&self, binding: String) -> LoginFuture<'_, BrowserLoginStart> {
        Box::pin(BrowserLoginProvider::begin(self, binding))
    }
    fn complete(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
    ) -> LoginFuture<'_, CompletedBrowserLogin> {
        Box::pin(BrowserLoginProvider::complete(self, binding, callback))
    }
    fn matches_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        BrowserLoginProvider::matches_callback(self, binding, callback)
    }
    fn cancel(&self, binding: &str) {
        BrowserLoginProvider::cancel(self, binding);
    }
}
type LoginProvider = Arc<dyn ErasedBrowserLoginProvider>;

/// One bounded context ordering gate. The map lock never spans async work.
struct PreauthEntry {
    expires: Instant,
    cancelled: AtomicBool,
    gate: tokio::sync::Mutex<PreauthFlow>,
    provider: Mutex<Option<LoginProvider>>,
}
enum PreauthFlow {
    Ready,
    Pending,
    Consumed(Option<CredentialOperation>),
}
impl PreauthEntry {
    fn live(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire) && Instant::now() < self.expires
    }
    fn cancel(&self, binding: &str) {
        self.cancelled.store(true, Ordering::Release);
        let provider = self
            .provider
            .lock()
            .ok()
            .and_then(|provider| provider.clone());
        if let Some(provider) = provider {
            provider.cancel(binding);
        }
        // Retain only the bounded provider handle, never callback secrets or a
        // bearer. A second cancellation after a racing begin future completes
        // must also remove any state created after the first cancellation.
    }
}
/// Handler cancellation/timeout never leaves a reusable in-flight attempt.
struct AttemptCancellation<'a> {
    entry: &'a PreauthEntry,
    binding: &'a str,
    armed: bool,
}
impl Drop for AttemptCancellation<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.entry.cancel(self.binding);
        }
    }
}

/// Explicit named HTTPS origin and independently random per-process CSRF key.
/// The owner injects authority; construction opens no listener or provider flow.
pub struct IsolatedSessionHttp<A> {
    state: Arc<HttpState<A>>,
}
struct HttpState<A> {
    authority: A,
    origin: String,
    csrf_key: String,
    enrollment_enabled: bool,
    social_enabled: bool,
    #[cfg(feature = "accounts")]
    account_readiness: Option<account_routes::AccountReadiness>,
    preauth: Mutex<PreauthState>,
}
struct PreauthState {
    entries: BTreeMap<String, Arc<PreauthEntry>>,
    window: Instant,
    requests: u16,
    login_requests: u16,
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
    /// Construct the ADR-0031/0036 adapter with one canonical HTTPS Origin.
    ///
    /// The input must equal its WHATWG ASCII Origin serialization: lowercase
    /// scheme/host, ASCII IDNA, canonical IP literals, no default port, userinfo,
    /// path (even `/`), query or fragment. Noncanonical inputs fail fast rather
    /// than silently changing the allow-list. Domain trailing dots remain
    /// distinct origins. This validates syntax, not DNS, TLS or reachability.
    pub fn new(authority: A, trusted_origin: &str) -> Result<Self, SessionError> {
        let url = Url::parse(trusted_origin).map_err(|_| SessionError::InvalidInput)?;
        let origin = url.origin();
        if url.scheme() != "https"
            || !origin.is_tuple()
            || origin.ascii_serialization() != trusted_origin
        {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self {
            state: Arc::new(HttpState {
                authority,
                origin: trusted_origin.to_owned(),
                csrf_key: SessionCredential::generate()?.expose_encoded(),
                enrollment_enabled: false,
                social_enabled: false,
                #[cfg(feature = "accounts")]
                account_readiness: None,
                preauth: Mutex::new(PreauthState {
                    entries: BTreeMap::new(),
                    window: Instant::now(),
                    requests: 0,
                    login_requests: 0,
                }),
            }),
        })
    }

    /// Exact read transport plus current credential observation. Returned facts
    /// are snapshots; every effect/private output requires another durable fence.
    pub async fn authenticate_read(
        &self,
        request: Request,
    ) -> Result<(CredentialOperation, SessionSnapshot), Response> {
        if request.uri().query().is_some() {
            return Err(problem(StatusCode::BAD_REQUEST, "request_rejected"));
        }
        let input =
            transport(&self.state, request.headers(), false).map_err(Rejection::response)?;
        let operation = operation(&input).map_err(Rejection::response)?;
        let snapshot = self
            .state
            .authority
            .read_session(operation)
            .await
            .map_err(session_problem)?;
        Ok((operation, snapshot))
    }
    /// Configured canonical origin; never derived from request headers.
    pub fn trusted_origin(&self) -> &str {
        &self.state.origin
    }
    /// Injected durable authority for isolated extensions.
    pub fn authority(&self) -> &A {
        &self.state.authority
    }
    /// Enables only explicitly composed isolated account/social capabilities.
    pub fn with_account_capabilities(
        mut self,
        enrollment: bool,
        social: bool,
    ) -> Result<Self, SessionError> {
        let state = Arc::get_mut(&mut self.state).ok_or(SessionError::Conflict)?;
        state.enrollment_enabled = enrollment;
        state.social_enabled = social;
        Ok(self)
    }
    /// Reuse exact Origin/channel/CSRF checks for a bounded authenticated JSON
    /// extension (ADR-0041). Observation grants no later effect/output authority.
    pub async fn authenticate_json<T: serde::de::DeserializeOwned>(
        &self,
        request: Request,
        limit: usize,
    ) -> Result<(CredentialOperation, SessionSnapshot, T), Response> {
        if !(1..=65_536).contains(&limit) || request.uri().query().is_some() {
            return Err(problem(StatusCode::BAD_REQUEST, "request_rejected"));
        }
        let input = transport(&self.state, request.headers(), true).map_err(Rejection::response)?;
        let content_type =
            single(request.headers(), "content-type").map_err(Rejection::response)?;
        if !matches!(
            content_type,
            Some("application/json" | "application/json; charset=utf-8")
        ) || request.headers().contains_key(header::CONTENT_ENCODING)
        {
            return Err(problem(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "request_rejected",
            ));
        }
        let token = single(request.headers(), "x-tabula-csrf")
            .map_err(Rejection::response)?
            .map(str::to_owned);
        if input.channel == SessionChannel::BrowserCookie
            && token.as_ref().is_none_or(|value| value.len() != 43)
        {
            return Err(problem(StatusCode::FORBIDDEN, "request_rejected"));
        }
        let bytes = tokio::time::timeout(BODY_DEADLINE, to_bytes(request.into_body(), limit))
            .await
            .map_err(|_| problem(StatusCode::REQUEST_TIMEOUT, "request_rejected"))?
            .map_err(|_| problem(StatusCode::PAYLOAD_TOO_LARGE, "request_rejected"))?;
        let body = serde_json::from_slice::<T>(&bytes)
            .map_err(|_| problem(StatusCode::BAD_REQUEST, "request_rejected"))?;
        let mut operation = operation(&input).map_err(Rejection::response)?;
        let snapshot = self
            .state
            .authority
            .read_session(operation)
            .await
            .map_err(session_problem)?;
        if input.channel == SessionChannel::BrowserCookie {
            if !verify_csrf(&self.state, &snapshot, token.as_deref().unwrap_or_default()) {
                return Err(problem(StatusCode::FORBIDDEN, "request_rejected"));
            }
            operation.context = Some(SessionContextBinding {
                context_id: snapshot.context_id(),
                authorization_epoch: snapshot.authorization_epoch(),
            });
        }
        Ok((operation, snapshot, body))
    }

    /// A fresh bounded publication fence for an extension's actual body handoff.
    pub async fn begin_publication(
        &self,
        operation: CredentialOperation,
    ) -> Result<A::Publication, SessionError> {
        self.state.authority.begin_publication(operation).await
    }

    /// Exact opt-in isolated routes. No CORS, listener, production startup or WS.
    pub fn router(self) -> Router {
        self.routes(None)
    }

    /// Explicit provider-backed isolated composition. Default/production stays closed.
    /// Only callback navigation bypasses ordinary exact-origin transport checks.
    pub fn router_with_login<P: BrowserLoginProvider + 'static>(self, provider: P) -> Router {
        self.routes(Some(Arc::new(provider)))
    }

    fn routes(self, provider: Option<LoginProvider>) -> Router {
        let mut router = Router::new()
            .route("/api/v1/auth/context", get(context::<A>))
            .route("/api/v1/me", get(profile::<A>))
            .route("/api/v1/auth/refresh", post(refresh::<A>))
            .route("/api/v1/auth/logout", post(logout::<A>))
            .route("/api/v1/auth/register", post(unavailable))
            .route("/api/v1/friends", get(unavailable))
            .fallback(|| async { problem(StatusCode::NOT_FOUND, "not_found") });
        router = if let Some(provider) = provider {
            router
                .route("/api/v1/auth/login", post(login::<A>))
                .route("/api/v1/auth/oidc/callback", get(callback::<A>))
                .layer(Extension(provider))
        } else {
            router.route("/api/v1/auth/login", post(unavailable))
        };
        // Apply after all routes, including the explicit OIDC exception.
        router
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
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
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
    browser_cookies(headers)
}

fn browser_cookies(headers: &HeaderMap) -> Result<Input, Rejection> {
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
                SessionCredential::parse(value)
                    .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
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
    provider: Option<Extension<LoginProvider>>,
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
        return signed_out_context(&state, input.preauth_cookie.as_deref(), provider.is_some());
    }
    let operation = match operation(&input) {
        Ok(operation) => operation,
        Err(error) => return error.response(),
    };
    match state.authority.begin_publication(operation).await {
        Ok(guard) => {
            let snapshot = guard.snapshot();
            #[cfg(feature = "accounts")]
            let profile_ready = match &state.account_readiness {
                Some(readiness) => match readiness.ready(snapshot).await {
                    Ok(ready) => ready,
                    Err(_) => return session_problem(SessionError::Unavailable),
                },
                None => true,
            };
            #[cfg(not(feature = "accounts"))]
            let profile_ready = true;
            let value = ContextResponse {
                version: HTTP_CONTRACT_VERSION,
                disposition: SessionDisposition::Authenticated,
                account_id: Some(format!("{:032x}", snapshot.user_id().0)),
                csrf_token: (snapshot.channel() == SessionChannel::BrowserCookie)
                    .then(|| csrf(&state, snapshot)),
                capabilities: AccountCapabilities {
                    friends: state.social_enabled && profile_ready,
                    ..capabilities(true)
                },
            };
            guarded_json(guard, &value)
        }
        Err(SessionError::Unauthenticated) if input.channel == SessionChannel::BrowserCookie => {
            // A terminal/stale HttpOnly cookie must not strand the browser. A
            // fresh non-authorizing context does not clear or overwrite it.
            signed_out_context(&state, input.preauth_cookie.as_deref(), provider.is_some())
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
fn signed_out_context<A>(
    state: &HttpState<A>,
    cookie: Option<&str>,
    login_enabled: bool,
) -> Response {
    let now = Instant::now();
    let Ok(mut preauth) = state.preauth.lock() else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if now.duration_since(preauth.window) >= Duration::from_secs(60) {
        preauth.window = now;
        preauth.requests = 0;
        preauth.login_requests = 0;
    }
    if preauth.requests >= BOOTSTRAP_PER_MINUTE {
        return problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    }
    preauth.requests += 1;
    preauth.entries.retain(|binding, entry| {
        if entry.live() {
            true
        } else {
            entry.cancel(binding);
            false
        }
    });
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
        preauth.entries.insert(
            cookie.clone(),
            Arc::new(PreauthEntry {
                expires: now + PREAUTH_LIFETIME,
                cancelled: AtomicBool::new(false),
                gate: tokio::sync::Mutex::new(PreauthFlow::Ready),
                provider: Mutex::new(None),
            }),
        );
        (cookie, true)
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-isolated-preauth-csrf-v1\0");
    mac.update(cookie.as_bytes());
    let mut value = public_context(SessionDisposition::SignedOut);
    value.capabilities.login = login_enabled;
    value.capabilities.register = state.enrollment_enabled;
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

fn preauth_operation<A>(
    state: &HttpState<A>,
    input: &Input,
    token: Option<&str>,
) -> Result<(String, Arc<PreauthEntry>), Rejection> {
    if input.channel != SessionChannel::BrowserCookie {
        return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
    }
    let binding = input
        .preauth_cookie
        .as_ref()
        .ok_or_else(|| reject(StatusCode::FORBIDDEN, "request_rejected"))?;
    let token = token.ok_or_else(|| reject(StatusCode::FORBIDDEN, "request_rejected"))?;
    let raw = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| reject(StatusCode::FORBIDDEN, "request_rejected"))?;
    if raw.len() != 32 || URL_SAFE_NO_PAD.encode(&raw) != token {
        return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-isolated-preauth-csrf-v1\0");
    mac.update(binding.as_bytes());
    mac.verify_slice(&raw)
        .map_err(|_| reject(StatusCode::FORBIDDEN, "request_rejected"))?;
    let entry = lookup_preauth(state, binding)?;
    Ok((binding.clone(), entry))
}
fn lookup_preauth<A>(state: &HttpState<A>, binding: &str) -> Result<Arc<PreauthEntry>, Rejection> {
    let preauth = state
        .preauth
        .lock()
        .map_err(|_| reject(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
    let entry = preauth
        .entries
        .get(binding)
        .ok_or_else(|| reject(StatusCode::FORBIDDEN, "request_rejected"))?;
    if !entry.live() {
        entry.cancel(binding);
        return Err(reject(StatusCode::FORBIDDEN, "request_rejected"));
    }
    Ok(Arc::clone(entry))
}
fn remove_preauth<A>(state: &HttpState<A>, binding: &str, entry: &Arc<PreauthEntry>) {
    if let Ok(mut preauth) = state.preauth.lock() {
        if preauth
            .entries
            .get(binding)
            .is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            preauth.entries.remove(binding);
        }
    }
}
async fn reject_active_session<A: HttpSessionAuthority>(
    state: &HttpState<A>,
    input: &Input,
) -> Result<(), SessionError> {
    if input.credential.is_none() {
        return Ok(());
    }
    let request = operation(input).map_err(|_| SessionError::InvalidInput)?;
    match state.authority.read_session(request).await {
        Ok(_) => Err(SessionError::Conflict),
        Err(SessionError::Unauthenticated) => Ok(()),
        Err(error) => Err(error),
    }
}
fn take_login_budget<A>(state: &HttpState<A>) -> Result<(), Rejection> {
    let mut preauth = state
        .preauth
        .lock()
        .map_err(|_| reject(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
    if preauth.window.elapsed() >= Duration::from_secs(60) {
        preauth.window = Instant::now();
        preauth.requests = 0;
        preauth.login_requests = 0;
    }
    if preauth.login_requests >= LOGIN_PER_MINUTE {
        return Err(reject(StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    }
    preauth.login_requests += 1;
    Ok(())
}
async fn login<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    Extension(provider): Extension<LoginProvider>,
    request: Request,
) -> Response {
    let (input, token) = match unsafe_json(&state, request).await {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    let (binding, entry) = match preauth_operation(&state, &input, token.as_deref()) {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    if let Err(error) = take_login_budget(&state) {
        return error.response();
    }
    let Ok(mut flow) = tokio::time::timeout(LOCK_DEADLINE, entry.gate.lock()).await else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if !entry.live() {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    }
    if !matches!(*flow, PreauthFlow::Ready) {
        return problem(StatusCode::CONFLICT, "conflict");
    }
    {
        let Ok(mut stored) = entry.provider.lock() else {
            return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
        };
        *stored = Some(Arc::clone(&provider));
    }
    let mut cancellation = AttemptCancellation {
        entry: &entry,
        binding: &binding,
        armed: true,
    };
    *flow = PreauthFlow::Consumed(None);
    let result = tokio::time::timeout(LOGIN_DEADLINE, async {
        reject_active_session(&state, &input).await?;
        provider.begin(binding.clone()).await
    })
    .await;
    let start = match result {
        Ok(Ok(start)) => start,
        Ok(Err(error)) => return session_problem(error),
        Err(_) => return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    if !entry.live() {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    }
    let value = LoginStartResponse {
        version: HTTP_CONTRACT_VERSION,
        authorization_url: start.authorization_url,
    };
    let parsed = Url::parse(&value.authorization_url);
    if value.validate().is_err()
        || !parsed.is_ok_and(|url| {
            url.scheme() == "https"
                && url.has_host()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        })
    {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    }
    *flow = PreauthFlow::Pending;
    cancellation.armed = false;
    json(StatusCode::OK, &value, "application/json")
}
fn callback_input(request: &Request) -> Result<(Input, BrowserLoginCallback), Rejection> {
    let rejected = || reject(StatusCode::BAD_REQUEST, "request_rejected");
    if request.method() != Method::GET
        || request.uri().to_string().len() > CALLBACK_URL_LIMIT
        || request.headers().contains_key(header::TRANSFER_ENCODING)
        || request.headers().contains_key(header::CONTENT_ENCODING)
    {
        return Err(rejected());
    }
    if single(request.headers(), "authorization")?.is_some() {
        return Err(rejected());
    }
    // This exception is exclusively a top-level GET provider navigation. It
    // cannot authorize any ordinary unsafe route or cross-origin private read.
    if single(request.headers(), "sec-fetch-mode")?.is_some_and(|mode| mode != "navigate")
        || single(request.headers(), "sec-fetch-dest")?.is_some_and(|dest| dest != "document")
        || single(request.headers(), "content-length")?.is_some_and(|length| length != "0")
    {
        return Err(rejected());
    }
    single(request.headers(), "origin")?;
    single(request.headers(), "sec-fetch-site")?;
    let input = browser_cookies(request.headers())?;
    let query = request.uri().query().ok_or_else(rejected)?;
    if query
        .split('&')
        .any(|pair| pair.is_empty() || !pair.contains('='))
    {
        return Err(rejected());
    }
    let bytes = query.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return Err(rejected());
            }
            index += 2;
        }
        index += 1;
    }
    let mut state = None;
    let mut code = None;
    let mut issuer = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        let slot = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "iss" => &mut issuer,
            _ => return Err(rejected()),
        };
        if slot.is_some() {
            return Err(rejected());
        }
        *slot = Some(value.into_owned());
    }
    let callback = BrowserLoginCallback::new(
        state.ok_or_else(rejected)?,
        code.ok_or_else(rejected)?,
        issuer,
    )
    .map_err(|_| rejected())?;
    Ok((input, callback))
}
fn random_id() -> Result<u128, SessionError> {
    // Independent OS draws for record ID, context ID and bearer. Public IDs
    // are never bearer entropy, UUID generation or a deterministic game RNG.
    let random = SessionCredential::generate()?;
    let bytes = URL_SAFE_NO_PAD
        .decode(random.expose_encoded())
        .map_err(|_| SessionError::Unavailable)?;
    let raw: [u8; 16] = bytes[..16]
        .try_into()
        .map_err(|_| SessionError::Unavailable)?;
    let value = u128::from_be_bytes(raw);
    if value == 0 {
        return Err(SessionError::Unavailable);
    }
    Ok(value)
}
async fn callback<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    Extension(provider): Extension<LoginProvider>,
    #[cfg(feature = "accounts")] enrollment: Option<Extension<account_routes::EnrollmentProvider>>,
    request: Request,
) -> Response {
    #[cfg(feature = "accounts")]
    if let Some(Extension(enrollment)) =
        enrollment.filter(|Extension(provider)| account_routes::owns_callback(provider, &request))
    {
        return account_routes::try_callback(state, enrollment, request).await;
    }
    let (input, callback) = match callback_input(&request) {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    let Some(binding) = input.preauth_cookie.as_deref() else {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    };
    let entry = match lookup_preauth(&state, binding) {
        Ok(entry) => entry,
        Err(error) => return error.response(),
    };
    let Ok(mut flow) = tokio::time::timeout(LOCK_DEADLINE, entry.gate.lock()).await else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if !entry.live() || !matches!(*flow, PreauthFlow::Pending) {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    }
    // Top-level cross-site navigation carries SameSite=Lax cookies. A guessed
    // callback state must not consume/cancel a real flow just by carrying its cookie.
    match provider.matches_callback(binding, &callback) {
        Ok(true) => {}
        Ok(false) => return problem(StatusCode::FORBIDDEN, "request_rejected"),
        Err(error) => return session_problem(error),
    }
    *flow = PreauthFlow::Consumed(None);
    let cancellation = AttemptCancellation {
        entry: &entry,
        binding,
        armed: true,
    };
    let result = tokio::time::timeout(LOGIN_DEADLINE, async {
        reject_active_session(&state, &input).await?;
        let completed = provider.complete(binding.to_owned(), callback).await?;
        if !entry.live() {
            return Err(SessionError::InvalidInput);
        }
        // Recheck after provider I/O; never switch an already active cookie.
        reject_active_session(&state, &input).await?;
        let credential = SessionCredential::generate()?;
        let request = IssueSession {
            identity: completed.identity,
            expected_epoch: completed.expected_epoch,
            id: AuthSessionId::new(random_id()?)?,
            channel: SessionChannel::BrowserCookie,
            credential_digest: credential.digest(),
            context_id: SessionContextId::new(random_id()?)?,
        };
        let snapshot = state.authority.issue_session(request).await?;
        let operation = CredentialOperation {
            digest: credential.digest(),
            channel: SessionChannel::BrowserCookie,
            context: Some(SessionContextBinding {
                context_id: snapshot.context_id(),
                authorization_epoch: snapshot.authorization_epoch(),
            }),
        };
        *flow = PreauthFlow::Consumed(Some(operation));
        if !entry.live() {
            // This check is the completion decision under the context gate.
            // A cancellation ordered later can revoke the known verifier when
            // the gate releases; already handed-off response bytes are external.
            // Logout/expiry won while durable issuance was awaiting. A known
            // commit is revoked if possible; never expose its bearer/cookie.
            state.authority.revoke_credential(operation).await?;
            return Err(SessionError::InvalidInput);
        }
        Ok(credential)
    })
    .await;
    let credential = match result {
        Ok(Ok(credential)) => credential,
        Ok(Err(error)) => {
            entry.cancel(binding);
            remove_preauth(&state, binding, &entry);
            return session_problem(error);
        }
        Err(_) => {
            entry.cancel(binding);
            remove_preauth(&state, binding, &entry);
            return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
        }
    };
    remove_preauth(&state, binding, &entry);
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static("/account"));
    set_cookie(
        &mut response,
        format!(
            "{SESSION_COOKIE}={}; Secure; HttpOnly; SameSite=Lax; Path=/",
            credential.expose_encoded()
        ),
    );
    clear_preauth_cookie(&mut response);
    drop(cancellation);
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

async fn unsafe_json<A>(
    state: &HttpState<A>,
    request: Request,
) -> Result<(Input, Option<String>), Rejection> {
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
    Ok((input, token))
}

async fn unsafe_operation<A: HttpSessionAuthority>(
    state: &HttpState<A>,
    request: Request,
    for_logout: bool,
) -> Result<(Input, CredentialOperation, SessionSnapshot), Rejection> {
    let (input, token) = unsafe_json(state, request).await?;
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
    let (input, token) = match unsafe_json(&state, request).await {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    if input.credential.is_none() {
        return cancel_preauth_logout(&state, &input, token.as_deref()).await;
    }
    if input.channel == SessionChannel::BrowserCookie
        && preauth_operation(&state, &input, token.as_deref()).is_ok()
    {
        // A terminal cookie may accompany the signed-out context. The preauth
        // token can cancel that flow only when durable authority confirms no
        // currently active session; it cannot silently log out/switch an account.
        match reject_active_session(&state, &input).await {
            Ok(()) => return cancel_preauth_logout(&state, &input, token.as_deref()).await,
            Err(error) => return session_problem(error),
        }
    }
    let mut credential = match operation(&input) {
        Ok(operation) => operation,
        Err(error) => return error.response(),
    };
    if input.channel == SessionChannel::BrowserCookie {
        let snapshot = match state.authority.read_logout_context(credential).await {
            Ok(snapshot) => snapshot,
            Err(error) => return session_problem(error),
        };
        if !verify_csrf(&state, &snapshot, token.as_deref().unwrap_or_default()) {
            return problem(StatusCode::FORBIDDEN, "request_rejected");
        }
        credential.context = Some(SessionContextBinding {
            context_id: snapshot.context_id(),
            authorization_epoch: snapshot.authorization_epoch(),
        });
    }
    let cancelled_entry = if let Some(binding) = input.preauth_cookie.as_deref() {
        let Ok(mut preauth) = state.preauth.lock() else {
            return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
        };
        let entry = preauth.entries.remove(binding);
        if let Some(entry) = &entry {
            entry.cancel(binding);
        }
        entry
    } else {
        None
    };
    match state.authority.revoke_credential(credential).await {
        Ok(()) => {
            // G5: cancellation was published before revocation awaited.
            // If issuance had already committed, also revoke its exact known
            // verifier after the context gate releases, without retaining a bearer.
            if let Some(entry) = cancelled_entry {
                if let Err(error) = revoke_completed_preauth(&state, &entry).await {
                    return session_problem(error);
                }
            }
            logout_response(input.channel == SessionChannel::BrowserCookie)
        }
        Err(error) => session_problem(error),
    }
}
async fn cancel_preauth_logout<A: HttpSessionAuthority>(
    state: &HttpState<A>,
    input: &Input,
    token: Option<&str>,
) -> Response {
    let (binding, entry) = match preauth_operation(state, input, token) {
        Ok(values) => values,
        Err(error) => return error.response(),
    };
    entry.cancel(&binding);
    remove_preauth(state, &binding, &entry);
    if let Err(error) = revoke_completed_preauth(state, &entry).await {
        return session_problem(error);
    }
    // Preauth-CSRF proves only this flow. Do not clear an unrelated session
    // cookie that may have appeared while this request was in flight.
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    clear_preauth_cookie(&mut response);
    response
}
async fn revoke_completed_preauth<A: HttpSessionAuthority>(
    state: &HttpState<A>,
    entry: &PreauthEntry,
) -> Result<(), SessionError> {
    let flow = tokio::time::timeout(LOCK_DEADLINE, entry.gate.lock())
        .await
        .map_err(|_| SessionError::Unavailable)?;
    if let PreauthFlow::Consumed(Some(credential)) = &*flow {
        tokio::time::timeout(
            LOGIN_DEADLINE,
            state.authority.revoke_credential(*credential),
        )
        .await
        .map_err(|_| SessionError::Unavailable)??;
    }
    Ok(())
}
fn logout_response(browser: bool) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    if browser {
        set_cookie(
            &mut response,
            format!("{SESSION_COOKIE}=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
        );
        clear_preauth_cookie(&mut response);
    }
    response
}
fn clear_preauth_cookie(response: &mut Response) {
    set_cookie(
        response,
        format!("{PREAUTH_COOKIE}=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
    );
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
pub(crate) fn guarded_json<P: SessionPublication + 'static>(
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

#[cfg(test)]
mod login_expiry_tests {
    use super::*;
    use crate::{
        test_authority::TestAuthority,
        test_wire::{WireServer, TRUSTED_ORIGIN},
    };
    use tabula_session::{AccountEpoch, ProviderIdentityKey};
    struct Provider;
    impl BrowserLoginProvider for Provider {
        async fn begin(&self, _: String) -> Result<BrowserLoginStart, SessionError> {
            Ok(BrowserLoginStart {
                authorization_url: "https://kanidm.invalid/oauth2/authorise?state=state".to_owned(),
            })
        }
        async fn complete(
            &self,
            _: String,
            _: BrowserLoginCallback,
        ) -> Result<CompletedBrowserLogin, SessionError> {
            Ok(CompletedBrowserLogin {
                identity: ProviderIdentityKey::new("https://kanidm.invalid", "synthetic-subject")?,
                expected_epoch: AccountEpoch::new(0)?,
            })
        }
        fn matches_callback(
            &self,
            _: &str,
            callback: &BrowserLoginCallback,
        ) -> Result<bool, SessionError> {
            Ok(callback.state() == "state")
        }
        fn cancel(&self, _: &str) {}
    }
    #[tokio::test]
    async fn actual_tcp_expired_preauth_cannot_start_complete_or_cancel_and_gets_new_context() {
        for after_begin in [false, true] {
            let authority = TestAuthority::new();
            authority.enable_browser_login();
            let adapter = IsolatedSessionHttp::new(authority.clone(), TRUSTED_ORIGIN).unwrap();
            let state = Arc::clone(&adapter.state);
            let server = WireServer::start(adapter.router_with_login(Provider)).await;
            let context = server.request("GET", "/api/v1/auth/context", &[], "").await;
            let cookie = context.cookie(PREAUTH_COOKIE).unwrap();
            let csrf = context.json()["csrf_token"].as_str().unwrap().to_owned();
            let headers = [
                ("Origin", TRUSTED_ORIGIN),
                ("Cookie", cookie.as_str()),
                ("Content-Type", "application/json"),
                ("X-Tabula-CSRF", csrf.as_str()),
            ];
            if after_begin {
                assert_eq!(
                    server
                        .request("POST", "/api/v1/auth/login", &headers, "{}")
                        .await
                        .status,
                    200
                );
            }
            {
                let mut preauth = state.preauth.lock().unwrap();
                let binding = cookie.strip_prefix(&format!("{PREAUTH_COOKIE}=")).unwrap();
                Arc::get_mut(preauth.entries.get_mut(binding).unwrap())
                    .expect("no in-flight request owns this context")
                    .expires = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
            }
            for (method, path, body) in [
                ("POST", "/api/v1/auth/login", "{}"),
                (
                    "GET",
                    "/api/v1/auth/oidc/callback?state=state&code=code",
                    "",
                ),
                ("POST", "/api/v1/auth/logout", "{}"),
            ] {
                let response = server.request(method, path, &headers, body).await;
                assert_eq!(response.status, 403, "{response:?}");
                response.assert_no_cookie();
                response.assert_no_store();
            }
            assert_eq!(authority.issuance_attempts(), 0);
            let fresh = server
                .request("GET", "/api/v1/auth/context", &[("Cookie", &cookie)], "")
                .await;
            assert_eq!(fresh.status, 200);
            assert_ne!(
                fresh.cookie(PREAUTH_COOKIE).as_deref(),
                Some(cookie.as_str())
            );
        }
    }
}

#[cfg(feature = "accounts")]
mod account_routes;
