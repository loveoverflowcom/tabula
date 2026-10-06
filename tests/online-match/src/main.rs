//! Disposable CI fixture for ADR-0041 actual two-browser acceptance only.
//! Production services stay closed. Fixture identities verify no real provider;
//! all admission, credentials, commands, output and commits use real authority.
//! The private `audit` CLI independently validates the entire durable stream.
#![forbid(unsafe_code)]

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State as HttpState},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tabula_core::{canonical_decode, InputIndex, MatchId, Occupant, OutcomeKind, SeatId, UserId};
use tabula_game_api::{Effect, Input};
use tabula_game_chess::{Command, Config, State as ChessState, Status};
use tabula_match::{
    durable::{Journal, RuntimePortError},
    ports::Clock,
    runtime::{recover, Binding, Completion, Exit, Limits, Ports},
    runtime_ports::{Authority, AuthorityLost, Effects, Output, Purpose},
};
use tabula_match_http::isolated::IsolatedMatchHttp;
use tabula_protocol::ServerEnvelope;
use tabula_session::{
    AccountEpoch, AccountRecord, AuthSessionId, IssueSession, ProviderIdentityKey,
    SessionAuthority, SessionChannel, SessionContextId, SessionCredential, UnixMillis,
};
use tabula_session_http::isolated::{IsolatedSessionHttp, SESSION_COOKIE};
use tabula_storage::{
    match_postgres::PgMatchStore, online_match::PgOnlineMatchStore, session::PgSessionStore,
};

const ORIGIN: &str = "https://localhost:9443";
const ENROLL_HTML: &str = "<!doctype html><html lang=en><meta charset=utf-8><title>Disposable Tabula acceptance</title><h1>Disposable acceptance identity</h1><p>This isolated fixture provisions a synthetic account and uses real durable session authority. It does not test provider login.</p><form action=/__fixture/enroll method=post><button data-testid=fixture-enroll>Start disposable acceptance session</button></form></html>";
type Checked<T> = Result<T, &'static str>;

#[cfg(feature = "continuity-test")]
mod continuity;
#[cfg(feature = "body-publication-test")]
mod publication;

#[derive(Clone)]
struct Fixture {
    sessions: PgSessionStore,
    issued: Arc<AtomicUsize>,
}

fn public_response(status: StatusCode, bytes: impl Into<Body>, kind: &'static str) -> Response {
    let mut response = Response::new(bytes.into());
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn failure(status: StatusCode) -> Response {
    public_response(
        status,
        "Fixture request rejected",
        "text/plain; charset=utf-8",
    )
}

async fn enrollment_page() -> Response {
    let mut response = public_response(StatusCode::OK, ENROLL_HTML, "text/html; charset=utf-8");
    // Fetch's non-CORS form Origin algorithm turns no-referrer into Origin:null.
    // This document-only policy keeps the exact same-origin form authenticated;
    // protected/API responses retain no-referrer and enrollment rejects null.
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    response
}

fn random_id() -> Checked<u128> {
    let random = SessionCredential::generate()
        .map_err(|_| "fixture entropy unavailable")?
        .digest();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&random.as_bytes()[..16]);
    Ok(u128::from_be_bytes(bytes) | 1)
}

async fn issue(fixture: &Fixture) -> Checked<String> {
    if fixture.issued.fetch_add(1, Ordering::SeqCst) >= 32 {
        return Err("disposable identity capacity exhausted");
    }
    let credential = SessionCredential::generate().map_err(|_| "fixture entropy unavailable")?;
    let subject = random_id()?;
    let epoch = AccountEpoch::new(0).map_err(|_| "fixture epoch invalid")?;
    let identity = ProviderIdentityKey::new(
        "https://synthetic-provider.tabula.invalid",
        format!("two-browser-{subject:032x}"),
    )
    .map_err(|_| "fixture provider identity invalid")?;
    let account = AccountRecord::new(
        UserId(subject),
        epoch,
        true,
        UnixMillis::new(0).map_err(|_| "fixture clock invalid")?,
    )
    .map_err(|_| "fixture account invalid")?;
    fixture
        .sessions
        .provision_fixture_identity(identity.clone(), account)
        .await
        .map_err(|_| "real fixture provisioning failed")?;
    fixture
        .sessions
        .issue_session(IssueSession {
            identity,
            expected_epoch: epoch,
            id: AuthSessionId::new(random_id()?).map_err(|_| "fixture record invalid")?,
            channel: SessionChannel::BrowserCookie,
            credential_digest: credential.digest(),
            context_id: SessionContextId::new(random_id()?)
                .map_err(|_| "fixture context invalid")?,
        })
        .await
        .map_err(|_| "real durable session issuance failed")?;
    Ok(credential.expose_encoded())
}

async fn enroll(HttpState(fixture): HttpState<Fixture>, request: Request) -> Response {
    // Enrollment exists only in this CI executable, never an app/auth route.
    // Actual form POSTs must carry the exact HTTPS Origin and no existing login.
    if request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        != Some(ORIGIN)
        || request
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|cookies| {
                cookies.split(';').any(|cookie| {
                    cookie
                        .trim()
                        .split_once('=')
                        .is_some_and(|(name, _)| name == SESSION_COOKIE)
                })
            })
        || request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            != Some("application/x-www-form-urlencoded")
    {
        return failure(StatusCode::FORBIDDEN);
    }
    match to_bytes(request.into_body(), 1024).await {
        Ok(bytes) if bytes.is_empty() => {}
        _ => return failure(StatusCode::BAD_REQUEST),
    }
    let Ok(credential) = issue(&fixture).await else {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    };
    let cookie = format!(
        "{SESSION_COOKIE}={credential}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=28800"
    );
    let Ok(cookie) = HeaderValue::from_str(&cookie) else {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut response = public_response(StatusCode::SEE_OTHER, Body::empty(), "text/plain");
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_static("/games/com.tabula.chess"),
    );
    response
}

async fn connect() -> Checked<PgPool> {
    let url = std::env::var("TABULA_ONLINE_MATCH_DATABASE_URL")
        .map_err(|_| "actual disposable PostgreSQL URL is required")?;
    PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&url)
        .await
        .map_err(|_| "actual disposable PostgreSQL connection failed")
}

async fn serve(mode: &str) -> Checked<()> {
    let pool = connect().await?;
    let online = PgOnlineMatchStore::new(pool.clone());
    if PgOnlineMatchStore::migration_versions()
        != [
            202610040001,
            202610050001,
            202610050040,
            202610050041,
            202610060001,
        ]
    {
        return Err("strict migration composition does not match the independently reviewed set");
    }
    PgOnlineMatchStore::migrate(&pool)
        .await
        .map_err(|_| "strict composed migrations failed")?;
    let sessions = PgSessionStore::new(pool.clone());
    let matches = PgMatchStore::new(pool);
    #[cfg(feature = "continuity-test")]
    let hook = Arc::new(continuity::ContinuityHook::default());
    let session_http = IsolatedSessionHttp::new(sessions.clone(), ORIGIN)
        .map_err(|_| "canonical HTTPS session composition failed")?;
    let match_http = IsolatedMatchHttp::new(
        session_http.clone(),
        sessions.clone(),
        online.clone(),
        matches.clone(),
        tabula_registry::registered_games(),
    )
    .map_err(|_| "isolated current-authority match composition failed")?;
    #[cfg(feature = "continuity-test")]
    let match_http = {
        let mut limits = Limits::default();
        match mode {
            "normal" => {}
            "evicted" => limits.receipts_per_scope = 1,
            "expired" => limits.receipt_ttl_ms = 1_000,
            _ => return Err("fixture retention mode invalid"),
        }
        match_http
            .with_acceptance_limits(limits)
            .with_acceptance_hook(hook.clone())
    };
    #[cfg(not(feature = "continuity-test"))]
    if mode != "normal" {
        return Err("fixture continuity feature required");
    }
    let fixture = Router::new()
        .route("/__fixture/enroll", get(enrollment_page).post(enroll))
        .with_state(Fixture {
            sessions: sessions.clone(),
            issued: Arc::new(AtomicUsize::new(0)),
        });
    let router = fixture
        .merge(session_http.clone().router())
        .merge(match_http.clone().router());
    #[cfg(feature = "body-publication-test")]
    let router = publication::compose(router, session_http.clone());
    #[cfg(feature = "continuity-test")]
    let router = continuity::compose(
        router,
        hook,
        session_http,
        sessions,
        online,
        matches,
        match_http,
    );
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 3000))
        .await
        .map_err(|_| "isolated fixture loopback bind failed")?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "isolated fixture listener failed")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditInput {
    match_id: String,
    accounts: Vec<String>,
    #[serde(default = "full_game_inputs")]
    expected_inputs: u8,
    #[serde(default = "opponent_scopes")]
    expected_scopes: u8,
}
fn opponent_scopes() -> u8 {
    2
}
fn full_game_inputs() -> u8 {
    4
}

fn checked_id(raw: &str) -> Checked<u128> {
    if raw.len() != 32
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("private audit identity encoding invalid");
    }
    let id = u128::from_str_radix(raw, 16).map_err(|_| "private audit identity invalid")?;
    if id == 0 {
        return Err("private audit identity is zero");
    }
    Ok(id)
}

struct DenyAuthority;
impl Authority for DenyAuthority {
    fn with_current<T>(
        &self,
        _: &Binding,
        _: Purpose,
        _: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost> {
        Err(AuthorityLost)
    }
}
#[derive(Default)]
struct DenyOutput(AtomicUsize);
impl Output for DenyOutput {
    fn submit(&self, _: &Binding, _: ServerEnvelope) -> Result<(), RuntimePortError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(RuntimePortError::Unavailable)
    }
}
#[derive(Default)]
struct AuditEffects(AtomicUsize);
impl Effects for AuditEffects {
    async fn execute(
        &self,
        _: MatchId,
        _: InputIndex,
        effects: Vec<Effect>,
    ) -> Result<(), RuntimePortError> {
        for effect in effects {
            match effect {
                Effect::EndMatch { .. } => {
                    self.0.fetch_add(1, Ordering::SeqCst);
                }
                _ => return Err(RuntimePortError::Unavailable),
            }
        }
        Ok(())
    }
}
struct AuditClock(Instant);
impl Clock for AuditClock {
    fn now_unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

async fn audit(path: &str) -> Checked<()> {
    let bytes = std::fs::read(path).map_err(|_| "private audit input absent")?;
    if bytes.len() > 1024 {
        return Err("private audit input is over limit");
    }
    let input: AuditInput =
        serde_json::from_slice(&bytes).map_err(|_| "private audit input invalid")?;
    if input.accounts.len() != 2 {
        return Err("actual opponent account pair absent");
    }
    if input.expected_inputs > 4 || !(2..=4).contains(&input.expected_scopes) {
        return Err("private audit expected prefix invalid");
    }
    let expected_inputs = u64::from(input.expected_inputs);
    let id = MatchId(checked_id(&input.match_id)?);
    let accounts = [
        UserId(checked_id(&input.accounts[0])?),
        UserId(checked_id(&input.accounts[1])?),
    ];
    if accounts[0] == accounts[1] {
        return Err("actual opponent accounts are equal");
    }
    let pool = connect().await?;
    let store = PgMatchStore::new(pool.clone());
    let journal = Arc::new(
        store
            .claim(id)
            .await
            .map_err(|_| "fresh durable audit fence failed")?,
    );
    let loaded = journal
        .load(id)
        .await
        .map_err(|_| "actual durable prefix load failed")?;
    if u64::try_from(loaded.records.len()).map_err(|_| "audit record count over limit")?
        != expected_inputs + 1
        || loaded.index.0 != expected_inputs
        || loaded.version.0 != expected_inputs
    {
        return Err(
            "actual game did not commit exactly the independently specified public input prefix",
        );
    }
    let game = tabula_registry::registered_games()
        .into_iter()
        .find(|game| game.metadata().id() == &loaded.creation.identity.game)
        .ok_or("exact durable game identity is not registered")?;
    let output = Arc::new(DenyOutput::default());
    let effects = Arc::new(AuditEffects::default());
    let mut limits = Limits::default();
    limits.operation_scopes = usize::from(loaded.creation.limits.scopes);
    limits.receipts_per_scope = usize::from(loaded.creation.limits.receipts_per_scope);
    limits.receipt_ttl_ms = loaded.creation.limits.receipt_ttl_ms;
    let (_handle, host, owner) = recover(
        id,
        game,
        Ports {
            authority: Arc::new(DenyAuthority),
            journal,
            output: output.clone(),
            effects: effects.clone(),
            clock: Arc::new(AuditClock(Instant::now())),
        },
        limits,
    )
    .await
    .map_err(|_| "full committed history did not pass exact runtime recovery")?;
    let completion = tokio::time::timeout(
        Duration::from_secs(10),
        host.drain()
            .map_err(|_| "audit recovery drain admission failed")?
            .wait(),
    )
    .await
    .map_err(|_| "audit recovery drain timed out")?
    .map_err(|_| "audit recovery drain failed")?;
    let summary = tokio::time::timeout(Duration::from_secs(10), owner)
        .await
        .map_err(|_| "audit recovered owner did not terminate")?
        .map_err(|_| "audit recovered owner panicked")?;
    if completion != Completion::Submitted
        || summary.exit != Exit::Drained
        || summary.index.0 != expected_inputs
        || summary.version.0 != expected_inputs
        || output.0.load(Ordering::SeqCst) != 0
        || effects.0.load(Ordering::SeqCst) != usize::from(input.expected_inputs == 4)
    {
        return Err("audit recovery released output or failed terminal effects validation");
    }
    let config: Config = canonical_decode(&loaded.creation.config)
        .map_err(|_| "validated game config decode failed")?;
    if config.clock.is_some() || loaded.creation.roster.len() != 2 {
        return Err("actual immutable match configuration is not the supported roster");
    }
    for (seat, user) in accounts.into_iter().enumerate() {
        let seat = SeatId(u8::try_from(seat).map_err(|_| "audit seat invalid")?);
        if loaded
            .creation
            .roster
            .get(seat)
            .is_none_or(|entry| entry.occupant != Occupant::Human(user))
        {
            return Err("durable immutable roster differs from actual authenticated browsers");
        }
    }
    let expected = [(0, 13, 21), (1, 52, 36), (0, 14, 30), (1, 59, 31)];
    for (record, (seat, from, to)) in loaded.records.iter().skip(1).zip(
        expected
            .into_iter()
            .take(usize::from(input.expected_inputs)),
    ) {
        let command: Input<Command> =
            canonical_decode(&record.input).map_err(|_| "durable command decode failed")?;
        if !matches!(command, Input::Player { seat: got_seat, command: Command::Move {
            from: got_from, to: got_to, promotion: None,
        }} if got_seat == SeatId(seat) && got_from == from && got_to == to)
        {
            return Err(
                "rendered browser actions did not commit the independently specified legal game",
            );
        }
    }
    if expected_inputs == 4 {
        let terminal = loaded
            .records
            .last()
            .ok_or("terminal committed record missing")?;
        if !terminal.terminal || loaded.records.iter().take(4).any(|r| r.terminal) {
            return Err("durable stream does not have exactly one terminal transition");
        }
        let state: ChessState = canonical_decode(
            terminal
                .snapshot
                .as_deref()
                .ok_or("terminal snapshot was not committed atomically")?,
        )
        .map_err(|_| "validated terminal snapshot decode failed")?;
        let Status::Ended { outcome } = state.status else {
            return Err("durable game is not terminal");
        };
        if outcome.kind() != OutcomeKind::Decisive
            || outcome.summary() != "checkmate"
            || outcome
                .standings()
                .iter()
                .find(|s| s.rank == 0)
                .is_none_or(|s| s.seat != SeatId(1))
        {
            return Err("actual durable verdict is not Black checkmate");
        }
        outcome
            .validate_against(&loaded.creation.roster)
            .map_err(|_| "durable outcome roster invalid")?;
    } else if loaded.records.iter().any(|record| record.terminal) {
        return Err("unexpected terminal transition in partial prefix");
    }
    let mut per_seat = [0_u64; 2];
    let mut records = std::collections::BTreeSet::new();
    if loaded.ledger.len() != usize::from(input.expected_scopes)
        || loaded.ledger.iter().any(|scope| {
            let seat = usize::from(scope.scope.seat.0);
            if seat >= 2
                || !records.insert(scope.scope.record)
                || scope.recent.len() > 2
                || scope.recent.iter().any(|receipt| receipt.result.is_err())
            {
                return true;
            }
            let Some(sum) = per_seat[seat].checked_add(scope.highest) else {
                return true;
            };
            per_seat[seat] = sum;
            false
        })
        || per_seat != [expected_inputs.div_ceil(2), expected_inputs / 2]
    {
        return Err("duplicate or unauthorized probe changed durable operation scopes");
    }
    println!(
        "{}",
        serde_json::json!({
            "status": "pass", "actual_postgres": true, "exact_runtime_recovery": true,
            "immutable_actual_opponent_roster": true, "accepted_inputs": expected_inputs,
            "records_including_genesis": expected_inputs + 1, "terminal_transitions": u8::from(expected_inputs == 4),
            "terminal_snapshot_in_atomic_commit": expected_inputs == 4, "verdict": if expected_inputs == 4 { "checkmate" } else { "ongoing" }, "winning_seat": if expected_inputs == 4 { Some(1) } else { None },
            "duplicate_join_and_commands_preserved_stream": true,
            "unauthorized_probes_preserved_stream": true, "audit_client_output": false,
        })
    );
    pool.close().await;
    Ok(())
}

async fn run() -> Checked<()> {
    if std::env::var("TABULA_ONLINE_MATCH_DISPOSABLE").as_deref() != Ok("1")
        || !matches!(std::env::var("CI").as_deref(), Ok("true" | "1"))
    {
        return Err("explicit CI-only disposable fixture opt-in is required");
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode] if mode == "serve" => serve("normal").await,
        [mode] if mode == "serve-evicted" => serve("evicted").await,
        [mode] if mode == "serve-expired" => serve("expired").await,
        [mode, path] if mode == "audit" => audit(path).await,
        _ => Err("serve or private audit input must be specified"),
    }
}

#[tokio::main]
async fn main() {
    if let Err(label) = run().await {
        eprintln!("FAIL: {label}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{checked_id, enrollment_page, failure, AuditInput};

    #[tokio::test]
    async fn enrollment_document_preserves_form_origin_without_changing_api_policy() {
        let document = enrollment_page().await;
        assert_eq!(
            document.headers()[axum::http::header::REFERRER_POLICY],
            "same-origin"
        );
        assert_eq!(
            document.headers()[axum::http::header::CACHE_CONTROL],
            "no-store"
        );
        let protected = failure(axum::http::StatusCode::FORBIDDEN);
        assert_eq!(
            protected.headers()[axum::http::header::REFERRER_POLICY],
            "no-referrer"
        );
    }

    #[test]
    fn private_audit_ids_require_canonical_nonzero_fixed_width_hex() {
        assert_eq!(checked_id("00000000000000000000000000000001"), Ok(1));
        assert_eq!(
            checked_id("ffffffffffffffffffffffffffffffff"),
            Ok(u128::MAX)
        );
        for invalid in [
            "",
            "1",
            "00000000000000000000000000000000",
            "0000000000000000000000000000000A",
            "0000000000000000000000000000000g",
        ] {
            assert!(checked_id(invalid).is_err());
        }
    }

    #[test]
    fn private_audit_input_rejects_unknown_fields_and_absent_roster() {
        assert!(serde_json::from_str::<AuditInput>(
            r#"{"match_id":"00000000000000000000000000000001","accounts":[],"seat":0}"#,
        )
        .is_err());
        assert!(serde_json::from_str::<AuditInput>(
            r#"{"match_id":"00000000000000000000000000000001"}"#,
        )
        .is_err());
    }
}
