//! Single-client Kanidm OIDC adapter, authorization code + S256 PKCE (ADR-0038).
//! No provider password handling, registration, refresh-token authority or token cache.
//! All upstream bodies are bounded; TLS/hostname verification and redirect rejection
//! are mandatory. Only ES256 public keys from the pinned issuer's JWK endpoint verify.

use crate::config::KanidmConfig;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tabula_session::{
    AccountEpoch, BrowserLoginCallback, BrowserLoginProvider, BrowserLoginStart,
    CompletedBrowserLogin, ProviderIdentityKey, SessionAuthority, SessionCredential, SessionError,
};
use url::Url;

type AdmissionFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, SessionError>> + Send + 'a>>;
trait EnrollmentAdmission: Send + Sync {
    fn accounts(
        &self,
        issuer: String,
    ) -> AdmissionFuture<'_, Vec<(ProviderIdentityKey, AccountEpoch)>>;
    fn policy(&self) -> AdmissionFuture<'_, tabula_session::EnrollmentPolicy>;
}
impl<A: tabula_session::EnrollmentAuthority> EnrollmentAdmission for A {
    fn accounts(
        &self,
        issuer: String,
    ) -> AdmissionFuture<'_, Vec<(ProviderIdentityKey, AccountEpoch)>> {
        Box::pin(self.admitted_accounts(issuer))
    }
    fn policy(&self) -> AdmissionFuture<'_, tabula_session::EnrollmentPolicy> {
        Box::pin(self.enrollment_policy())
    }
}
const BODY_LIMIT: usize = 65_536;
const TOKEN_LIMIT: usize = 16_384;
const PENDING_LIMIT: usize = 128;
const FLOW_LIFETIME: Duration = Duration::from_secs(300);
const CLOCK_LEEWAY: u64 = 5;

/// Provider proof owner; authority is injected only to capture invited epochs
/// before browser navigation. HTTP owns durable issuance and cookie publication.
pub struct KanidmOidc<A> {
    inner: Arc<Inner<A>>,
}
struct Inner<A> {
    config: KanidmConfig,
    client: reqwest::Client,
    metadata: Metadata,
    authority: A,
    pending: Mutex<PendingState>,
    enrollment: Option<Arc<dyn EnrollmentAdmission>>,
}
#[derive(Default)]
struct PendingState {
    flows: BTreeMap<String, Pending>,
    reservations: BTreeMap<String, Reservation>,
}
struct Reservation {
    cancelled: Arc<AtomicBool>,
    expires: Instant,
}
struct Pending {
    binding: String,
    nonce: String,
    verifier: String,
    started_seconds: u64,
    expires: Instant,
    epochs: BTreeMap<ProviderIdentityKey, AccountEpoch>,
    enrollment_epoch: Option<AccountEpoch>,
    cancelled: Arc<AtomicBool>,
}
impl<A> Clone for KanidmOidc<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
impl<A> fmt::Debug for KanidmOidc<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KanidmOidc")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Metadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    response_types_supported: Vec<String>,
    response_modes_supported: Vec<String>,
    id_token_signing_alg_values_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    code_challenge_methods_supported: Vec<String>,
}
impl Metadata {
    fn validate(&self, c: &KanidmConfig) -> Result<(), SessionError> {
        if self.issuer != c.issuer
            || self.authorization_endpoint != format!("{}/ui/oauth2", c.provider_origin)
            || self.token_endpoint != format!("{}/oauth2/token", c.provider_origin)
            || self.jwks_uri != format!("{}/public_key.jwk", c.issuer)
            || !self.response_types_supported.iter().any(|x| x == "code")
            || !self.response_modes_supported.iter().any(|x| x == "query")
            || self.id_token_signing_alg_values_supported != ["ES256"]
            || !self
                .token_endpoint_auth_methods_supported
                .iter()
                .any(|x| x == "client_secret_basic")
            || !self
                .code_challenge_methods_supported
                .iter()
                .any(|x| x == "S256")
        {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }
}

impl<A: SessionAuthority> KanidmOidc<A> {
    /// Discover a configured client without provisioning or accepting redirects.
    pub async fn discover(config: KanidmConfig, authority: A) -> Result<Self, SessionError> {
        let mut builder = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .user_agent("Tabula-isolated-OIDC/1");
        if let Some(pem) = &config.root_certificate {
            builder = builder
                .add_root_certificate(reqwest::Certificate::from_pem(pem).map_err(unavailable)?);
        }
        let client = builder.build().map_err(unavailable)?;
        let url = format!("{}/.well-known/openid-configuration", config.issuer);
        let metadata: Metadata =
            bounded_json(client.get(url).send().await.map_err(unavailable)?).await?;
        metadata.validate(&config)?;
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                client,
                metadata,
                authority,
                pending: Mutex::new(PendingState::default()),
                enrollment: None,
            }),
        })
    }
}
fn unavailable<T>(_: T) -> SessionError {
    SessionError::Unavailable
}
fn clock_seconds() -> Result<u64, SessionError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs())
        .map_err(unavailable)
}
fn random_value() -> Result<String, SessionError> {
    Ok(SessionCredential::generate()?.expose_encoded())
}
fn binding_key(raw: &str) -> Result<String, SessionError> {
    Ok(URL_SAFE_NO_PAD.encode(SessionCredential::parse(raw)?.digest().as_bytes()))
}
fn prune(state: &mut PendingState) {
    let now = Instant::now();
    state
        .flows
        .retain(|_, flow| flow.expires > now && !flow.cancelled.load(Ordering::Acquire));
    state.reservations.retain(|_, reservation| {
        reservation.expires > now && !reservation.cancelled.load(Ordering::Acquire)
    });
}
async fn bounded_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, SessionError> {
    if response.status() != reqwest::StatusCode::OK
        || response
            .content_length()
            .is_some_and(|n| n > BODY_LIMIT as u64)
        || response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|x| x.to_str().ok())
            .is_none_or(|x| x.split(';').next() != Some("application/json"))
    {
        return Err(SessionError::Unavailable);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(unavailable)? {
        if body
            .len()
            .checked_add(chunk.len())
            .is_none_or(|n| n > BODY_LIMIT)
        {
            return Err(SessionError::Unavailable);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(unavailable)
}

impl<A: SessionAuthority> BrowserLoginProvider for KanidmOidc<A> {
    fn matches_callback(
        &self,
        preauth_binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        let binding = binding_key(preauth_binding)?;
        let pending = self.inner.pending.lock().map_err(unavailable)?;
        Ok(pending.flows.get(callback.state()).is_some_and(|flow| {
            flow.binding == binding
                && flow.enrollment_epoch.is_none()
                && !flow.epochs.is_empty()
                && flow.expires > Instant::now()
                && !flow.cancelled.load(Ordering::Acquire)
        }))
    }
    async fn begin(&self, preauth_binding: String) -> Result<BrowserLoginStart, SessionError> {
        self.begin_purpose(preauth_binding, None).await
    }

    async fn complete(
        &self,
        preauth_binding: String,
        callback: BrowserLoginCallback,
    ) -> Result<CompletedBrowserLogin, SessionError> {
        let binding = binding_key(&preauth_binding)?;
        let flow = {
            let mut pending = self.inner.pending.lock().map_err(unavailable)?;
            prune(&mut pending);
            let candidate = pending
                .flows
                .get(callback.state())
                .ok_or(SessionError::Unauthenticated)?;
            // Wrong-cookie callbacks cannot consume another browser's pending flow.
            if candidate.binding != binding
                || candidate.enrollment_epoch.is_some()
                || candidate.epochs.is_empty()
            {
                return Err(SessionError::Unauthenticated);
            }
            pending
                .flows
                .remove(callback.state())
                .ok_or(SessionError::Unauthenticated)?
        };
        // Consume before the network await: every outcome, including timeout, is terminal.
        let result = if callback
            .issuer()
            .is_some_and(|issuer| issuer != self.inner.config.issuer)
        {
            Err(SessionError::Unauthenticated)
        } else {
            self.exchange_and_verify(&flow, &callback)
                .await
                .and_then(|identity| {
                    let expected_epoch = *flow
                        .epochs
                        .get(&identity)
                        .ok_or(SessionError::Unauthenticated)?;
                    Ok(CompletedBrowserLogin {
                        identity,
                        expected_epoch,
                    })
                })
        };
        let mut pending = self.inner.pending.lock().map_err(unavailable)?;
        if let Some(reservation) = pending.reservations.get(&binding) {
            if Arc::ptr_eq(&reservation.cancelled, &flow.cancelled) {
                pending.reservations.remove(&binding);
            }
        }
        result
    }
    fn cancel(&self, preauth_binding: &str) {
        let Ok(binding) = binding_key(preauth_binding) else {
            return;
        };
        if let Ok(mut pending) = self.inner.pending.lock() {
            if let Some(reservation) = pending.reservations.remove(&binding) {
                reservation.cancelled.store(true, Ordering::Release);
            }
            pending.flows.retain(|_, flow| flow.binding != binding);
        }
    }
}

impl<A: SessionAuthority> KanidmOidc<A> {
    async fn begin_purpose(
        &self,
        preauth_binding: String,
        enrollment_epoch: Option<AccountEpoch>,
    ) -> Result<BrowserLoginStart, SessionError> {
        let binding = binding_key(&preauth_binding)?;
        let state_value = random_value()?;
        let nonce = random_value()?;
        let verifier = random_value()?;
        let started_seconds = clock_seconds()?;
        let expires = Instant::now() + FLOW_LIFETIME;
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut pending = self.inner.pending.lock().map_err(unavailable)?;
            prune(&mut pending);
            if pending.reservations.len() >= PENDING_LIMIT {
                return Err(SessionError::Unavailable);
            }
            if pending.reservations.contains_key(&binding) {
                return Err(SessionError::Conflict);
            }
            pending.reservations.insert(
                binding.clone(),
                Reservation {
                    cancelled: cancelled.clone(),
                    expires,
                },
            );
            pending.flows.insert(
                state_value.clone(),
                Pending {
                    binding: binding.clone(),
                    nonce: nonce.clone(),
                    verifier: verifier.clone(),
                    started_seconds,
                    expires,
                    epochs: BTreeMap::new(),
                    enrollment_epoch,
                    cancelled: cancelled.clone(),
                },
            );
        }
        let mut epochs = BTreeMap::new();
        for identity in &self.inner.config.admitted {
            let account = match self
                .inner
                .authority
                .account_snapshot(identity.clone())
                .await
            {
                Ok(account) if account.enabled() => account,
                Ok(_) | Err(SessionError::Unauthenticated) => continue,
                Err(_) => {
                    self.cancel(&preauth_binding);
                    return Err(SessionError::Unavailable);
                }
            };
            epochs.insert(identity.clone(), account.authorization_epoch());
        }
        if let Some(admission) = &self.inner.enrollment {
            let Ok(accounts) = admission.accounts(self.inner.config.issuer.clone()).await else {
                self.cancel(&preauth_binding);
                return Err(SessionError::Unavailable);
            };
            for (identity, epoch) in accounts {
                epochs.insert(identity, epoch);
            }
        }
        {
            let mut pending = self.inner.pending.lock().map_err(unavailable)?;
            let flow = pending
                .flows
                .get_mut(&state_value)
                .ok_or(SessionError::Unauthenticated)?;
            if (epochs.is_empty() && enrollment_epoch.is_none())
                || flow.expires <= Instant::now()
                || cancelled.load(Ordering::Acquire)
            {
                drop(pending);
                self.cancel(&preauth_binding);
                return Err(SessionError::Unauthenticated);
            }
            flow.epochs = epochs;
        }
        let mut url =
            Url::parse(&self.inner.metadata.authorization_endpoint).map_err(unavailable)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        url.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", &self.inner.config.client_id),
            ("redirect_uri", &self.inner.config.callback_url),
            ("scope", "openid"),
            ("state", &state_value),
            ("nonce", &nonce),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("prompt", "login"),
            ("max_age", "0"),
            ("response_mode", "query"),
        ]);
        Ok(BrowserLoginStart {
            authorization_url: url.into(),
        })
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
    token_type: String,
    access_token: String,
}
#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<PublicJwk>,
}
#[derive(Deserialize)]
struct PublicJwk {
    kty: String,
    crv: String,
    x: String,
    y: String,
    kid: String,
    #[serde(rename = "use")]
    usage: Option<String>,
    alg: Option<String>,
    key_ops: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictHeader {
    alg: String,
    kid: String,
    typ: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}
#[derive(Deserialize)]
struct IdClaims {
    iss: String,
    sub: String,
    aud: Audience,
    exp: u64,
    iat: u64,
    nonce: String,
    auth_time: u64,
    nbf: Option<u64>,
    azp: Option<String>,
    at_hash: Option<String>,
}

impl<A: SessionAuthority> KanidmOidc<A> {
    async fn exchange_and_verify(
        &self,
        flow: &Pending,
        callback: &BrowserLoginCallback,
    ) -> Result<ProviderIdentityKey, SessionError> {
        if flow.cancelled.load(Ordering::Acquire) || flow.expires <= Instant::now() {
            return Err(SessionError::Unauthenticated);
        }
        // RFC6749 HTTP Basic percent-encodes credentials before base64 transport.
        let encode = |value: &str| {
            url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>()
        };
        let response = self
            .inner
            .client
            .post(&self.inner.metadata.token_endpoint)
            .basic_auth(
                encode(&self.inner.config.client_id),
                Some(encode(&self.inner.config.client_secret)),
            )
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", callback.code()),
                ("redirect_uri", &self.inner.config.callback_url),
                ("code_verifier", &flow.verifier),
            ])
            .send()
            .await
            .map_err(unavailable)?;
        let token: TokenResponse = bounded_json(response).await?;
        if token.id_token.len() > TOKEN_LIMIT
            || token.access_token.len() > TOKEN_LIMIT
            || !token.token_type.eq_ignore_ascii_case("bearer")
        {
            return Err(SessionError::Unauthenticated);
        }
        let jwks: JwkSet = bounded_json(
            self.inner
                .client
                .get(&self.inner.metadata.jwks_uri)
                .send()
                .await
                .map_err(unavailable)?,
        )
        .await?;
        let identity = verify_id_token(&self.inner.config, flow, &token, &jwks, clock_seconds()?)?;
        if flow.cancelled.load(Ordering::Acquire) || flow.expires <= Instant::now() {
            return Err(SessionError::Unauthenticated);
        }
        Ok(identity)
    }
}
fn canonical_segment(segment: &str, max: usize) -> Result<Vec<u8>, SessionError> {
    if segment.is_empty() || segment.len() > max {
        return Err(SessionError::Unauthenticated);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|_| SessionError::Unauthenticated)?;
    if URL_SAFE_NO_PAD.encode(&bytes) != segment {
        return Err(SessionError::Unauthenticated);
    }
    Ok(bytes)
}
fn verify_id_token(
    c: &KanidmConfig,
    flow: &Pending,
    token: &TokenResponse,
    jwks: &JwkSet,
    now: u64,
) -> Result<ProviderIdentityKey, SessionError> {
    let segments: Vec<_> = token.id_token.split('.').collect();
    if segments.len() != 3 || token.id_token.len() > TOKEN_LIMIT {
        return Err(SessionError::Unauthenticated);
    }
    let header: StrictHeader = serde_json::from_slice(&canonical_segment(segments[0], 1024)?)
        .map_err(|_| SessionError::Unauthenticated)?;
    canonical_segment(segments[1], TOKEN_LIMIT)?;
    if canonical_segment(segments[2], 128)?.len() != 64
        || header.alg != "ES256"
        || header.kid.is_empty()
        || header.kid.len() > 256
        || header.typ.as_deref().is_some_and(|x| x != "JWT")
    {
        return Err(SessionError::Unauthenticated);
    }
    if jwks.keys.is_empty() || jwks.keys.len() > 8 {
        return Err(SessionError::Unavailable);
    }
    let keys: Vec<_> = jwks
        .keys
        .iter()
        .filter(|key| key.kid == header.kid)
        .collect();
    if keys.len() != 1 {
        return Err(SessionError::Unauthenticated);
    }
    let key = keys[0];
    if key.kty != "EC"
        || key.crv != "P-256"
        || key.usage.as_deref().is_some_and(|x| x != "sig")
        || key.alg.as_deref().is_some_and(|x| x != "ES256")
        || key
            .key_ops
            .as_ref()
            .is_some_and(|x| x.as_slice() != ["verify"])
        || canonical_segment(&key.x, 43)?.len() != 32
        || canonical_segment(&key.y, 43)?.len() != 32
    {
        return Err(SessionError::Unauthenticated);
    }
    let key = DecodingKey::from_ec_components(&key.x, &key.y)
        .map_err(|_| SessionError::Unauthenticated)?;
    let mut validation = Validation::new(Algorithm::ES256);
    validation.leeway = 0;
    validation.validate_nbf = true;
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.set_issuer(&[&c.issuer]);
    validation.set_audience(&[&c.client_id]);
    let claims = decode::<IdClaims>(&token.id_token, &key, &validation)
        .map_err(|_| SessionError::Unauthenticated)?
        .claims;
    let aud_exact = match &claims.aud {
        Audience::Single(a) => a == &c.client_id,
        Audience::Multiple(a) => a.as_slice() == [c.client_id.as_str()],
    };
    if claims.iss != c.issuer
        || !aud_exact
        || claims.azp.as_deref().is_some_and(|x| x != c.client_id)
        || claims.nonce != flow.nonce
        || claims.exp <= now
        || claims.iat > now.saturating_add(CLOCK_LEEWAY)
        || claims.iat >= claims.exp
        || claims.auth_time > now.saturating_add(CLOCK_LEEWAY)
        || claims.auth_time.saturating_add(CLOCK_LEEWAY) < flow.started_seconds
        || claims.auth_time > claims.iat.saturating_add(CLOCK_LEEWAY)
        || claims.nbf.is_some_and(|x| x > now)
    {
        return Err(SessionError::Unauthenticated);
    }
    if let Some(hash) = claims.at_hash {
        let digest = Sha256::digest(token.access_token.as_bytes());
        if hash != URL_SAFE_NO_PAD.encode(&digest[..16]) {
            return Err(SessionError::Unauthenticated);
        }
    }
    ProviderIdentityKey::new(claims.iss, claims.sub).map_err(|_| SessionError::Unauthenticated)
}

#[cfg(test)]
mod tests;

impl<A: SessionAuthority + tabula_session::EnrollmentAuthority + Clone + 'static> KanidmOidc<A> {
    /// Explicit enrollment composition; existing invited-only discovery is unchanged.
    pub async fn discover_enrollment(
        config: KanidmConfig,
        authority: A,
    ) -> Result<Self, SessionError> {
        if !config.enrollment_enabled {
            return Err(SessionError::InvalidInput);
        }
        let admissions = Arc::new(authority.clone());
        let mut provider = Self::discover(config, authority).await?;
        Arc::get_mut(&mut provider.inner)
            .ok_or(SessionError::Conflict)?
            .enrollment = Some(admissions);
        Ok(provider)
    }
}
impl<A: SessionAuthority> tabula_session::BrowserEnrollmentProvider for KanidmOidc<A> {
    async fn begin_enrollment(&self, binding: String) -> Result<BrowserLoginStart, SessionError> {
        let policy = self
            .inner
            .enrollment
            .as_ref()
            .ok_or(SessionError::InvalidInput)?
            .policy()
            .await?;
        if !policy.enabled {
            return Err(SessionError::Unauthenticated);
        }
        self.begin_purpose(binding, Some(policy.epoch)).await
    }
    fn matches_enrollment_callback(
        &self,
        binding: &str,
        callback: &BrowserLoginCallback,
    ) -> Result<bool, SessionError> {
        let binding = binding_key(binding)?;
        let pending = self.inner.pending.lock().map_err(unavailable)?;
        Ok(pending.flows.get(callback.state()).is_some_and(|flow| {
            flow.binding == binding
                && flow.enrollment_epoch.is_some()
                && flow.expires > Instant::now()
                && !flow.cancelled.load(Ordering::Acquire)
        }))
    }
    async fn complete_enrollment(
        &self,
        binding: String,
        callback: BrowserLoginCallback,
    ) -> Result<tabula_session::VerifiedEnrollment, SessionError> {
        let key = binding_key(&binding)?;
        let flow = {
            let mut pending = self.inner.pending.lock().map_err(unavailable)?;
            prune(&mut pending);
            let candidate = pending
                .flows
                .get(callback.state())
                .ok_or(SessionError::Unauthenticated)?;
            if candidate.binding != key || candidate.enrollment_epoch.is_none() {
                return Err(SessionError::Unauthenticated);
            }
            pending
                .flows
                .remove(callback.state())
                .ok_or(SessionError::Unauthenticated)?
        };
        let result = if callback
            .issuer()
            .is_some_and(|issuer| issuer != self.inner.config.issuer)
        {
            Err(SessionError::Unauthenticated)
        } else {
            self.exchange_and_verify(&flow, &callback).await
        };
        let mut pending = self.inner.pending.lock().map_err(unavailable)?;
        if pending
            .reservations
            .get(&key)
            .is_some_and(|reservation| Arc::ptr_eq(&reservation.cancelled, &flow.cancelled))
        {
            pending.reservations.remove(&key);
        }
        result.map(|identity| tabula_session::VerifiedEnrollment {
            expected_account_epoch: flow.epochs.get(&identity).copied(),
            identity,
            expected_policy_epoch: flow.enrollment_epoch.expect("checked purpose"),
        })
    }
    fn cancel_enrollment(&self, binding: &str) {
        BrowserLoginProvider::cancel(self, binding);
    }
}
