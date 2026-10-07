//! Account extension sharing the session adapter's exact transport and CSRF mechanisms.
use super::*;
use crate::accounts::{
    EnrollmentContextResponse, EnrollmentDisposition, EnrollmentFieldPolicy,
    EnrollmentStartResponse, OtherAccountProfileResponse, ProfileUpdateRequest, ProfileVisibility,
    RegistrationDisposition, RegistrationRequest, RegistrationResponse, SelfAccountProfileResponse,
    ACCOUNT_CONTRACT_VERSION,
};
use axum::{extract::Path, routing::patch};
use tabula_session::{
    AccountDisplayName, AccountHandle, AccountOperationId, AccountProfile, AccountProfileAuthority,
    AccountProfileVisibility, BrowserEnrollmentProvider, CredentialDigest, EnrollmentAuthority,
    EnrollmentContext, EnrollmentStatus,
};
pub const ENROLLMENT_COOKIE: &str = "__Host-tabula_enrollment";
pub(super) trait ErasedAccountReadiness: Send + Sync {
    fn ready(&self, snapshot: &SessionSnapshot) -> LoginFuture<'_, bool>;
}
impl<A: EnrollmentAuthority> ErasedAccountReadiness for A {
    fn ready(&self, snapshot: &SessionSnapshot) -> LoginFuture<'_, bool> {
        Box::pin(self.account_profile_ready(snapshot.user_id()))
    }
}
pub(super) type AccountReadiness = Arc<dyn ErasedAccountReadiness>;
pub(super) trait ErasedEnrollment: Send + Sync {
    fn matches(&self, binding: &str, callback: &BrowserLoginCallback)
        -> Result<bool, SessionError>;
    fn complete(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
        digest: CredentialDigest,
        id: AccountOperationId,
    ) -> LoginFuture<'_, EnrollmentContext>;
}
struct EnrollmentService<A, P> {
    authority: A,
    provider: P,
}
impl<A: EnrollmentAuthority, P: BrowserEnrollmentProvider> ErasedEnrollment
    for EnrollmentService<A, P>
{
    fn matches(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        self.provider.matches_enrollment_callback(binding, callback)
    }
    fn complete(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
        digest: CredentialDigest,
        id: AccountOperationId,
    ) -> LoginFuture<'_, EnrollmentContext> {
        Box::pin(async move {
            let verified = self.provider.complete_enrollment(binding, callback).await?;
            self.authority.create_enrollment(verified, digest, id).await
        })
    }
}
pub(super) type EnrollmentProvider = Arc<dyn ErasedEnrollment>;
struct EnrollmentLogin<P>(P);
impl<P: BrowserEnrollmentProvider> BrowserLoginProvider for EnrollmentLogin<P> {
    async fn begin(&self, binding: String) -> Result<BrowserLoginStart, SessionError> {
        self.0.begin_enrollment(binding).await
    }
    async fn complete(
        &self,
        _: String,
        _: BrowserLoginCallback,
    ) -> Result<CompletedBrowserLogin, SessionError> {
        Err(SessionError::InvalidInput)
    }
    fn matches_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        self.0.matches_enrollment_callback(binding, callback)
    }
    fn cancel(&self, binding: &str) {
        self.0.cancel_enrollment(binding);
    }
}
#[derive(Clone)]
struct EnrollmentLoginProvider(LoginProvider);
impl<A: HttpSessionAuthority + EnrollmentAuthority + AccountProfileAuthority + Clone + 'static>
    IsolatedSessionHttp<A>
{
    /// Installs a profile-existence hint before sharing router state. Actual
    /// profile/social requests still obtain their own current permission fences.
    pub fn with_account_readiness(mut self) -> Result<Self, SessionError> {
        let state = Arc::get_mut(&mut self.state).ok_or(SessionError::Conflict)?;
        state.account_readiness = Some(Arc::new(state.authority.clone()));
        Ok(self)
    }
    /// Current-authority profile routes for the gameplay service (ADR-0047).
    /// Provider enrollment and session issuance remain in the auth composition.
    pub fn profile_router(&self) -> Router
    where
        A::ProfilePublication: 'static,
    {
        Router::new()
            .route(
                "/api/v2/profiles/me",
                get(self_profile::<A>).merge(patch(update_profile::<A>)),
            )
            .route(
                "/api/v2/profiles/by-handle/{handle}",
                get(other_profile::<A>),
            )
            .with_state(Arc::clone(&self.state))
            .layer(middleware::from_fn(no_store))
    }
    /// Full explicitly enabled isolated account composition, without a listener.
    pub fn router_with_accounts<
        P: BrowserLoginProvider + BrowserEnrollmentProvider + Clone + 'static,
    >(
        self,
        provider: P,
    ) -> Router
    where
        A::ProfilePublication: 'static,
    {
        let enrollment: EnrollmentProvider = Arc::new(EnrollmentService {
            authority: self.state.authority.clone(),
            provider: provider.clone(),
        });
        let enrollment_login: LoginProvider = Arc::new(EnrollmentLogin(provider.clone()));
        let profiles = self.profile_router();
        let account = Router::new()
            .route("/api/v2/auth/enrollment/start", post(start::<A>))
            .route("/api/v2/auth/enrollment", get(enrollment_context::<A>))
            .route("/api/v2/auth/register", post(register::<A>))
            .with_state(Arc::clone(&self.state));
        account
            .merge(profiles)
            .merge(self.routes(Some(Arc::new(provider))))
            .layer(Extension(enrollment))
            .layer(Extension(EnrollmentLoginProvider(enrollment_login)))
            .layer(middleware::from_fn(no_store))
    }
}
async fn start<A: HttpSessionAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    Extension(provider): Extension<EnrollmentLoginProvider>,
    request: Request,
) -> Response {
    let response = login(State(state), Extension(provider.0), request).await;
    if response.status() != StatusCode::OK {
        return response;
    }
    let Ok(bytes) = to_bytes(response.into_body(), 4096).await else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let Ok(start) = serde_json::from_slice::<LoginStartResponse>(&bytes) else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    json(
        StatusCode::OK,
        &EnrollmentStartResponse {
            version: ACCOUNT_CONTRACT_VERSION,
            authorization_url: start.authorization_url,
        },
        "application/json",
    )
}
pub(super) fn owns_callback(provider: &EnrollmentProvider, request: &Request) -> bool {
    callback_input(request)
        .ok()
        .and_then(|(input, callback)| {
            input
                .preauth_cookie
                .map(|binding| provider.matches(&binding, &callback).unwrap_or(false))
        })
        .unwrap_or(false)
}
pub(super) async fn try_callback<A: HttpSessionAuthority + 'static>(
    state: Arc<HttpState<A>>,
    provider: EnrollmentProvider,
    request: Request,
) -> Response {
    let (input, callback) = match callback_input(&request) {
        Ok(v) => v,
        Err(e) => return e.response(),
    };
    let Some(binding) = input.preauth_cookie.as_deref() else {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    };
    let entry = match lookup_preauth(&state, binding) {
        Ok(e) => e,
        Err(e) => return e.response(),
    };
    let Ok(mut flow) = tokio::time::timeout(LOCK_DEADLINE, entry.gate.lock()).await else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if !entry.live() || !matches!(*flow, PreauthFlow::Pending) {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    }
    match provider.matches(binding, &callback) {
        Ok(true) => {}
        Ok(false) => return problem(StatusCode::FORBIDDEN, "request_rejected"),
        Err(e) => return session_problem(e),
    }
    *flow = PreauthFlow::Consumed(None);
    let cancellation = AttemptCancellation {
        entry: &entry,
        binding,
        armed: true,
    };
    let result = tokio::time::timeout(LOGIN_DEADLINE, async {
        reject_active_session(&state, &input).await?;
        let cookie = SessionCredential::generate()?;
        let id = AccountOperationId::generate()?;
        provider
            .complete(binding.to_owned(), callback, cookie.digest(), id)
            .await?;
        if !entry.live() {
            return Err(SessionError::Unauthenticated);
        }
        reject_active_session(&state, &input).await?;
        Ok(cookie)
    })
    .await;
    let cookie = match result {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return session_problem(e),
        Err(_) => return problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    remove_preauth(&state, binding, &entry);
    drop(cancellation);
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static("/register"));
    set_cookie(
        &mut response,
        format!(
            "{ENROLLMENT_COOKIE}={}; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=300",
            cookie.expose_encoded()
        ),
    );
    clear_preauth_cookie(&mut response);
    response
}
fn enrollment_cookie(headers: &HeaderMap) -> Result<SessionCredential, Rejection> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let raw = value
            .to_str()
            .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
        for pair in raw.split(';') {
            let (name, value) = pair
                .trim()
                .split_once('=')
                .ok_or_else(|| reject(StatusCode::BAD_REQUEST, "request_rejected"))?;
            if name == ENROLLMENT_COOKIE {
                if found.is_some() {
                    return Err(reject(StatusCode::BAD_REQUEST, "request_rejected"));
                }
                found = Some(
                    SessionCredential::parse(value)
                        .map_err(|_| reject(StatusCode::BAD_REQUEST, "request_rejected"))?,
                );
            }
        }
    }
    found.ok_or_else(|| reject(StatusCode::FORBIDDEN, "request_rejected"))
}
fn enrollment_mac<A>(
    state: &HttpState<A>,
    digest: CredentialDigest,
    ctx: EnrollmentContext,
) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.csrf_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"tabula-enrollment-csrf-v2\0");
    mac.update(digest.as_bytes());
    mac.update(&ctx.operation_id.get().to_be_bytes());
    mac.update(&ctx.policy_epoch.get().to_be_bytes());
    mac
}
async fn enrollment_context<A: HttpSessionAuthority + EnrollmentAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    }
    let input = match transport(&state, request.headers(), false) {
        Ok(v) if v.channel == SessionChannel::BrowserCookie => v,
        Ok(_) => return problem(StatusCode::FORBIDDEN, "request_rejected"),
        Err(e) => return e.response(),
    };
    let cookie = match enrollment_cookie(request.headers()) {
        Ok(c) => c,
        Err(e) => return e.response(),
    };
    if let Err(e) = reject_active_session(&state, &input).await {
        return session_problem(e);
    }
    match state.authority.read_enrollment(cookie.digest()).await {
        Ok(ctx) => json(
            StatusCode::OK,
            &EnrollmentContextResponse {
                version: ACCOUNT_CONTRACT_VERSION,
                disposition: match ctx.status {
                    EnrollmentStatus::Ready => EnrollmentDisposition::Ready,
                    EnrollmentStatus::AcceptedWithoutSession => {
                        EnrollmentDisposition::AcceptedWithoutSession
                    }
                    EnrollmentStatus::Rejected => EnrollmentDisposition::Rejected,
                },
                operation_id: if ctx.status == EnrollmentStatus::Ready {
                    Some(ctx.operation_id.encoded())
                } else {
                    None
                },
                csrf_token: if ctx.status == EnrollmentStatus::Ready {
                    Some(
                        URL_SAFE_NO_PAD.encode(
                            enrollment_mac(&state, cookie.digest(), ctx)
                                .finalize()
                                .into_bytes(),
                        ),
                    )
                } else {
                    None
                },
                field_policy: (ctx.status == EnrollmentStatus::Ready)
                    .then_some(EnrollmentFieldPolicy::APPROVED),
                agreement: None,
            },
            "application/json",
        ),
        Err(SessionError::Unauthenticated) => json(
            StatusCode::OK,
            &EnrollmentContextResponse {
                version: ACCOUNT_CONTRACT_VERSION,
                disposition: EnrollmentDisposition::Rejected,
                operation_id: None,
                csrf_token: None,
                field_policy: None,
                agreement: None,
            },
            "application/json",
        ),
        Err(e) => session_problem(e),
    }
}
async fn register<A: HttpSessionAuthority + EnrollmentAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    }
    let input = match transport(&state, request.headers(), true) {
        Ok(v) if v.channel == SessionChannel::BrowserCookie => v,
        Ok(_) => return problem(StatusCode::FORBIDDEN, "request_rejected"),
        Err(e) => return e.response(),
    };
    if let Err(error) = take_login_budget(&state) {
        return error.response();
    }
    let cookie = match enrollment_cookie(request.headers()) {
        Ok(c) => c,
        Err(e) => return e.response(),
    };
    let token = match single(request.headers(), "x-tabula-csrf") {
        Ok(Some(v)) => v.to_owned(),
        _ => return problem(StatusCode::FORBIDDEN, "request_rejected"),
    };
    if !matches!(
        single(request.headers(), "content-type"),
        Ok(Some("application/json" | "application/json; charset=utf-8"))
    ) || request.headers().contains_key(header::CONTENT_ENCODING)
    {
        return problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "request_rejected");
    }
    let Ok(Ok(bytes)) =
        tokio::time::timeout(BODY_DEADLINE, to_bytes(request.into_body(), 1024)).await
    else {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    };
    let Ok(body) = serde_json::from_slice::<RegistrationRequest>(&bytes) else {
        return problem(StatusCode::BAD_REQUEST, "request_rejected");
    };
    let parsed = AccountOperationId::parse(&body.operation_id).and_then(|id| {
        Ok((
            id,
            AccountHandle::new(body.handle)?,
            AccountDisplayName::new(body.display_name)?,
        ))
    });
    let Ok((id, handle, name)) = parsed else {
        return registration_result(EnrollmentStatus::Rejected);
    };
    if let Err(e) = reject_active_session(&state, &input).await {
        return session_problem(e);
    }
    let ctx = match state.authority.read_enrollment(cookie.digest()).await {
        Ok(v) => v,
        Err(e) => return session_problem(e),
    };
    let Ok(raw) = URL_SAFE_NO_PAD.decode(&token) else {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    };
    if raw.len() != 32
        || URL_SAFE_NO_PAD.encode(&raw) != token
        || enrollment_mac(&state, cookie.digest(), ctx)
            .verify_slice(&raw)
            .is_err()
    {
        return problem(StatusCode::FORBIDDEN, "request_rejected");
    }
    match state
        .authority
        .register_account(cookie.digest(), id, handle, name)
        .await
    {
        Ok(status) => registration_result(status),
        Err(SessionError::InvalidInput | SessionError::Unauthenticated) => {
            registration_result(EnrollmentStatus::Rejected)
        }
        Err(e) => session_problem(e),
    }
}
fn registration_result(status: EnrollmentStatus) -> Response {
    json(
        StatusCode::OK,
        &RegistrationResponse {
            version: ACCOUNT_CONTRACT_VERSION,
            disposition: if status == EnrollmentStatus::AcceptedWithoutSession {
                RegistrationDisposition::AcceptedWithoutSession
            } else {
                RegistrationDisposition::Rejected
            },
        },
        "application/json",
    )
}
fn self_dto(profile: &AccountProfile) -> SelfAccountProfileResponse {
    SelfAccountProfileResponse {
        version: ACCOUNT_CONTRACT_VERSION,
        account_id: format!("{:032x}", profile.user_id.0),
        handle: profile.handle.as_str().to_owned(),
        display_name: profile.display_name.as_str().to_owned(),
        visibility: match profile.visibility {
            AccountProfileVisibility::Public => ProfileVisibility::Public,
            AccountProfileVisibility::Friends => ProfileVisibility::Friends,
            AccountProfileVisibility::Private => ProfileVisibility::Private,
        },
        revision: profile.revision,
        as_of_ms: profile.as_of.get(),
    }
}
async fn self_profile<A: AccountProfileAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response
where
    A::ProfilePublication: 'static,
{
    let http = IsolatedSessionHttp {
        state: Arc::clone(&state),
    };
    let (operation, _) = match http.authenticate_read(request).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match state.authority.self_account_profile(operation).await {
        Ok((guard, Some(profile))) => guarded_json(guard, &self_dto(&profile)),
        Ok((guard, None)) => {
            let mut response = guarded_json(
                guard,
                &PublicProblem {
                    version: HTTP_CONTRACT_VERSION,
                    status: StatusCode::NOT_FOUND.as_u16(),
                    title: "Request unavailable".into(),
                    code: "not_available".into(),
                },
            );
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
        Err(e) => session_problem(e),
    }
}
async fn other_profile<A: AccountProfileAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    Path(handle): Path<String>,
    request: Request,
) -> Response
where
    A::ProfilePublication: 'static,
{
    let Ok(handle) = AccountHandle::new(handle) else {
        return problem(StatusCode::NOT_FOUND, "not_available");
    };
    let http = IsolatedSessionHttp {
        state: Arc::clone(&state),
    };
    let (operation, _) = match http.authenticate_read(request).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match state
        .authority
        .other_account_profile(operation, handle)
        .await
    {
        Ok((guard, profile)) => guarded_json(
            guard,
            &OtherAccountProfileResponse {
                version: ACCOUNT_CONTRACT_VERSION,
                account_id: format!("{:032x}", profile.user_id.0),
                handle: profile.handle.as_str().to_owned(),
                display_name: profile.display_name.as_str().to_owned(),
                as_of_ms: profile.as_of.get(),
            },
        ),
        Err(SessionError::Unauthenticated) => problem(StatusCode::NOT_FOUND, "not_available"),
        Err(e) => session_problem(e),
    }
}
async fn update_profile<A: AccountProfileAuthority + 'static>(
    State(state): State<Arc<HttpState<A>>>,
    request: Request,
) -> Response {
    let http = IsolatedSessionHttp {
        state: Arc::clone(&state),
    };
    let (operation, _, body) = match http
        .authenticate_json::<ProfileUpdateRequest>(request, 1024)
        .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = AccountOperationId::parse(&body.operation_id)
        .and_then(|id| Ok((id, AccountDisplayName::new(body.display_name)?)));
    let (id, name) = match input {
        Ok(v) => v,
        Err(e) => return session_problem(e),
    };
    let visibility = match body.visibility {
        ProfileVisibility::Public => AccountProfileVisibility::Public,
        ProfileVisibility::Friends => AccountProfileVisibility::Friends,
        ProfileVisibility::Private => AccountProfileVisibility::Private,
    };
    match state
        .authority
        .update_account_profile(operation, id, body.expected_revision, name, visibility)
        .await
    {
        Ok(()) => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::NO_CONTENT;
            response
        }
        Err(SessionError::Conflict) => problem(StatusCode::CONFLICT, "revision_conflict"),
        Err(e) => session_problem(e),
    }
}
