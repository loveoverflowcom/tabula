//! Real service-process/HTTP/PostgreSQL continuity; no Kanidm/browser claim.
//! Fixture accounts are provisioned here, never through application HTTP routes.
#![cfg(all(feature = "local-dev", unix, not(target_arch = "wasm32")))]
#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tabula_core::{MatchId, Occupant, SeatId, UserId};
use tabula_match::durable::{Journal, LoadedMatch, RuntimePortError};
use tabula_match_http::{
    parse_match_id, MatchAdmission, MatchAttachRequest, MatchAttachment, MatchCommandRequest,
    MatchCreateRequest, MatchFrames, MatchGrant, MatchGrantRequest, MatchJoinRequest,
};
use tabula_protocol::{ClientEnvelope, GameCommandFrame, ServerMessage};
use tabula_registry::runtime::test_support::approved_checkmate_fixture;
use tabula_session::{
    AccountEpoch, AccountRecord, AuthSessionId, IssueSession, ProviderIdentityKey,
    SessionAuthority, SessionChannel, SessionContextId, SessionCredential, UnixMillis,
};
use tabula_session_http::{
    isolated::{IsolatedSessionHttp, SESSION_COOKIE},
    ContextResponse,
};
use tabula_storage::{
    local_dev::{LocalDevStorageConfig, LocalDevStore, SchemaPolicy},
    session::PgSessionStore,
};

const ORIGIN: &str = "https://app.localhost:8444";
const IO_BUDGET: Duration = Duration::from_secs(5);

struct FixtureSession {
    user: UserId,
    cookie: String,
}
fn random_id() -> u128 {
    let random = SessionCredential::generate().unwrap().digest();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&random.as_bytes()[..16]);
    u128::from_be_bytes(bytes) | 1
}
async fn issue(sessions: &PgSessionStore) -> FixtureSession {
    let credential = SessionCredential::generate().unwrap();
    let user = UserId(random_id());
    let epoch = AccountEpoch::new(0).unwrap();
    let identity = ProviderIdentityKey::new(
        "https://service-process-fixture.invalid",
        format!("fixture-{:032x}", user.0),
    )
    .unwrap();
    sessions
        .provision_fixture_identity(
            identity.clone(),
            AccountRecord::new(user, epoch, true, UnixMillis::new(0).unwrap()).unwrap(),
        )
        .await
        .unwrap();
    sessions
        .issue_session(IssueSession {
            identity,
            expected_epoch: epoch,
            id: AuthSessionId::new(random_id()).unwrap(),
            channel: SessionChannel::BrowserCookie,
            credential_digest: credential.digest(),
            context_id: SessionContextId::new(random_id()).unwrap(),
        })
        .await
        .unwrap();
    FixtureSession {
        user,
        cookie: format!("{SESSION_COOKIE}={}", credential.expose_encoded()),
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("tabula-local-dev-process-{:032x}", random_id()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct ServiceProcess {
    child: Child,
    address: SocketAddr,
    log: PathBuf,
}
impl ServiceProcess {
    fn start(config: &Path, address: SocketAddr, log: PathBuf) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_tabula-server"))
            .args(["serve", "--config"])
            .arg(config)
            .env_clear()
            .stdout(Stdio::null())
            .stderr(Stdio::from(File::create(&log).unwrap()))
            .spawn()
            .unwrap();
        Self {
            child,
            address,
            log,
        }
    }
    async fn ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "service exited before readiness"
            );
            if let Ok(response) = http(self.address, "GET", "/readyz", None, None, ORIGIN, None) {
                if response.status == 200 {
                    return;
                }
            }
            assert!(
                Instant::now() < deadline,
                "service readiness deadline expired"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    async fn terminate(&mut self) {
        assert!(Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .unwrap()
            .success());
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "service did not complete graceful shutdown"
                );
                assert!(fs::read_to_string(&self.log)
                    .unwrap()
                    .contains("local-dev drained"));
                return;
            }
            assert!(
                Instant::now() < deadline,
                "service exceeded its configured drain budget"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}
impl Drop for ServiceProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn write_config(dir: &Path, database: &str, key: &str, address: SocketAddr) -> PathBuf {
    let config = dir.join("server.toml");
    fs::write(&config, format!("mode = \"local-dev\"\nbind = \"{address}\"\nbrowser_origin = \"{ORIGIN}\"\ndatabase_url = {}\ncsrf_key = \"{key}\"\nschema_policy = \"check\"\npool_connections = 8\nlifetime_room_capacity = 16\nrequest_capacity = 8\nwork_capacity = 8\nlive_match_capacity = 8\ndrain_seconds = 5\n", serde_json::to_string(database).unwrap())).unwrap();
    config
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}
impl HttpResponse {
    fn json<T: serde::de::DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).expect("bounded service response has valid JSON")
    }
}
fn http(
    address: SocketAddr,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    origin: &str,
    body: Option<&[u8]>,
) -> std::io::Result<HttpResponse> {
    let mut socket = TcpStream::connect_timeout(&address, IO_BUDGET)?;
    socket.set_read_timeout(Some(IO_BUDGET))?;
    socket.set_write_timeout(Some(IO_BUDGET))?;
    let body = body.unwrap_or_default();
    let mut headers = format!("{method} {path} HTTP/1.1\r\nHost: app.localhost\r\nConnection: close\r\nOrigin: {origin}\r\nSec-Fetch-Site: same-origin\r\nContent-Type: application/json\r\nContent-Length: {}\r\n", body.len());
    if let Some(cookie) = cookie {
        headers.push_str("Cookie: ");
        headers.push_str(cookie);
        headers.push_str("\r\n");
    }
    if let Some(csrf) = csrf {
        headers.push_str("X-Tabula-CSRF: ");
        headers.push_str(csrf);
        headers.push_str("\r\n");
    }
    headers.push_str("\r\n");
    socket.write_all(headers.as_bytes())?;
    socket.write_all(body)?;
    let mut raw = Vec::new();
    socket.take(2_200_000).read_to_end(&mut raw)?;
    let split = raw
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("complete HTTP headers");
    let header = std::str::from_utf8(&raw[..split]).unwrap();
    let status = header
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let body = if header
        .lines()
        .any(|line| line.eq_ignore_ascii_case("transfer-encoding: chunked"))
    {
        decode_chunks(&raw[split + 4..])
    } else {
        raw[split + 4..].to_vec()
    };
    Ok(HttpResponse { status, body })
}
fn decode_chunks(mut raw: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    loop {
        let end = raw
            .windows(2)
            .position(|part| part == b"\r\n")
            .expect("complete chunk size");
        let size = usize::from_str_radix(
            std::str::from_utf8(&raw[..end])
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            16,
        )
        .unwrap();
        raw = &raw[end + 2..];
        if size == 0 {
            assert_eq!(raw, b"\r\n");
            return decoded;
        }
        assert!(
            size <= raw.len().saturating_sub(2),
            "truncated private body"
        );
        decoded.extend_from_slice(&raw[..size]);
        assert_eq!(&raw[size..size + 2], b"\r\n");
        raw = &raw[size + 2..];
    }
}
fn get_context(address: SocketAddr, path: &str, session: &FixtureSession) -> ContextResponse {
    let response = http(
        address,
        "GET",
        path,
        Some(&session.cookie),
        None,
        ORIGIN,
        None,
    )
    .unwrap();
    assert_eq!(response.status, 200);
    response.json()
}
fn post<T: serde::Serialize>(
    address: SocketAddr,
    path: &str,
    session: &FixtureSession,
    csrf: &str,
    value: &T,
) -> HttpResponse {
    http(
        address,
        "POST",
        path,
        Some(&session.cookie),
        Some(csrf),
        ORIGIN,
        Some(&serde_json::to_vec(value).unwrap()),
    )
    .unwrap()
}
fn attach(address: SocketAddr, id: &str, session: &FixtureSession, csrf: &str) -> MatchAttachment {
    let grant = post(
        address,
        &format!("/api/v1/matches/{id}/grant"),
        session,
        csrf,
        &MatchGrantRequest::new().unwrap(),
    );
    assert_eq!(grant.status, 200);
    let grant: MatchGrant = grant.json();
    let attached = post(
        address,
        &format!("/api/v1/matches/{id}/attach"),
        session,
        csrf,
        &MatchAttachRequest::new(grant.binding_id().unwrap().into()).unwrap(),
    );
    assert_eq!(attached.status, 200);
    attached.json()
}
fn assert_ack(response: &HttpResponse, sequence: u64) {
    assert_eq!(response.status, 200);
    let frames: MatchFrames = response.json();
    assert!(
        frames
            .frames()
            .iter()
            .any(|frame| matches!(frame.body(), ServerMessage::Ack { seq } if *seq == sequence)),
        "accepted operation lacked its exact Ack"
    );
}
fn pending_social_socket(address: SocketAddr, session: &FixtureSession) -> TcpStream {
    let mut socket = TcpStream::connect_timeout(&address, IO_BUDGET).unwrap();
    socket.set_read_timeout(Some(IO_BUDGET)).unwrap();
    socket.set_write_timeout(Some(IO_BUDGET)).unwrap();
    let upgrade = format!("GET /api/v2/lobby/ws HTTP/1.1\r\nHost: app.localhost\r\nOrigin: {ORIGIN}\r\nCookie: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: tabula-social.v2.json\r\n\r\n", session.cookie);
    socket.write_all(upgrade.as_bytes()).unwrap();
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        assert!(headers.len() < 8192, "bounded websocket upgrade headers");
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        headers.extend_from_slice(&byte);
    }
    let headers = std::str::from_utf8(&headers).unwrap();
    assert!(
        headers.starts_with("HTTP/1.1 101 "),
        "actual social socket must upgrade before shutdown"
    );
    assert!(headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("sec-websocket-protocol: tabula-social.v2.json")));
    // Deliberately send no Hello: the service owns this pending upgraded task.
    socket
}
fn assert_draining_socket_close(mut socket: TcpStream) {
    let mut header = [0; 2];
    socket.read_exact(&mut header).unwrap();
    assert_eq!(
        header[0], 0x88,
        "pending social socket must receive a final Close frame"
    );
    assert_eq!(header[1] & 0x80, 0, "server frame must be unmasked");
    let size = usize::from(header[1] & 0x7f);
    assert!((2..=125).contains(&size));
    let mut payload = vec![0; size];
    socket.read_exact(&mut payload).unwrap();
    assert_eq!(u16::from_be_bytes([payload[0], payload[1]]), 4411);
}
async fn inspect_committed(store: &LocalDevStore, id: MatchId) -> LoadedMatch {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match store.match_store().reclaim_online(id).await {
            Ok(journal) => return journal.load(id).await.unwrap(),
            Err(RuntimePortError::Busy) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => {
                panic!("gracefully stopped owner did not release durable authority: {error:?}")
            }
        }
    }
}
fn assert_head(head: &LoadedMatch, users: &[FixtureSession; 2], command: &ClientEnvelope) {
    assert_eq!(
        head.records.len(),
        2,
        "duplicate replay appended another accepted input"
    );
    assert_eq!(head.index.0, 1);
    assert_eq!(head.version.0, 1);
    for (seat, session) in [SeatId(0), SeatId(1)].into_iter().zip(users) {
        assert_eq!(
            head.creation.roster.get(seat).unwrap().occupant,
            Occupant::Human(session.user)
        );
    }
    let operation = head.records[1].operation.as_ref().unwrap();
    assert_eq!(operation.seq, 1);
    assert_eq!(operation.command, *command.command());
    assert_eq!(operation.scope.seat, SeatId(0));
    assert_eq!(operation.scope.subject, users[0].user);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an explicitly selected disposable TABULA_LOCAL_DEV_TEST_DATABASE_URL"]
#[allow(clippy::too_many_lines)] // One real-process lifecycle keeps authority and restart assertions together.
async fn real_process_http_sigterm_restart_preserves_seats_and_duplicate_watermark() {
    let database = std::env::var("TABULA_LOCAL_DEV_TEST_DATABASE_URL")
        .expect("select a disposable PostgreSQL database explicitly");
    let store = LocalDevStore::connect(
        &database,
        LocalDevStorageConfig::new(8, 16).unwrap(),
        SchemaPolicy::Apply,
    )
    .await
    .unwrap();
    let users = [
        issue(&store.session_store()).await,
        issue(&store.session_store()).await,
    ];
    let key = SessionCredential::generate().unwrap().expose_encoded();
    let matching = IsolatedSessionHttp::new(store.session_store(), ORIGIN)
        .unwrap()
        .with_csrf_key(&key)
        .unwrap();
    let wrong = IsolatedSessionHttp::new(store.session_store(), ORIGIN).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let auth_address = listener.local_addr().unwrap();
    let (stop_context, context_stopped) = tokio::sync::oneshot::channel();
    let auth_context = tokio::spawn(async move {
        axum::serve(listener, matching.router().nest("/wrong", wrong.router()))
            .with_graceful_shutdown(async {
                let _ = context_stopped.await;
            })
            .await
            .unwrap();
    });
    let scratch = Scratch::new();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let config = write_config(&scratch.0, &database, &key, address);
    let mut process = ServiceProcess::start(&config, address, scratch.0.join("first.log"));
    process.ready().await;
    let fixture = approved_checkmate_fixture();
    let create = MatchCreateRequest::new(
        fixture.game.metadata().id().as_str().into(),
        2,
        BTreeMap::new(),
    )
    .unwrap();
    let contexts = users
        .each_ref()
        .map(|session| get_context(auth_address, "/api/v1/auth/context", session));
    let csrf = contexts
        .each_ref()
        .map(|context| context.csrf_token.as_deref().unwrap());
    let bad_key = get_context(auth_address, "/wrong/api/v1/auth/context", &users[0]);
    assert_eq!(
        post(
            address,
            "/api/v1/matches",
            &users[0],
            bad_key.csrf_token.as_deref().unwrap(),
            &create
        )
        .status,
        403
    );
    assert_eq!(
        post(address, "/api/v1/matches", &users[1], csrf[0], &create).status,
        403
    );
    assert_eq!(
        http(
            address,
            "POST",
            "/api/v1/matches",
            Some(&users[0].cookie),
            Some(csrf[0]),
            "https://evil.localhost:8444",
            Some(&serde_json::to_vec(&create).unwrap())
        )
        .unwrap()
        .status,
        403
    );
    let admission = post(address, "/api/v1/matches", &users[0], csrf[0], &create);
    assert_eq!(
        admission.status, 200,
        "auth-instance CSRF must authorize current server mutation"
    );
    let admission: MatchAdmission = admission.json();
    assert_eq!(admission.seat(), 0);
    let joined = post(
        address,
        "/api/v1/matches/join",
        &users[1],
        csrf[1],
        &MatchJoinRequest::new(admission.join_code().unwrap().into()).unwrap(),
    );
    assert_eq!(joined.status, 200);
    assert_eq!(joined.json::<MatchAdmission>().seat(), 1);
    let id = admission.match_id();
    let attached = [
        attach(address, id, &users[0], csrf[0]),
        attach(address, id, &users[1], csrf[1]),
    ];
    assert_eq!(attached[0].seat(), 0);
    assert_eq!(attached[1].seat(), 1);
    let command = ClientEnvelope::new(
        1,
        700,
        GameCommandFrame::new(
            parse_match_id(id).unwrap(),
            fixture.game.metadata().id().clone(),
            fixture.game.metadata().version().clone(),
            fixture.commands[0].1.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let first =
        MatchCommandRequest::new(attached[0].attachment_id().into(), command.clone()).unwrap();
    assert_ack(
        &post(
            address,
            &format!("/api/v1/matches/{id}/command"),
            &users[0],
            csrf[0],
            &first,
        ),
        1,
    );
    let pending_hello = pending_social_socket(address, &users[0]);
    process.terminate().await;
    assert_draining_socket_close(pending_hello);
    let before = inspect_committed(&store, parse_match_id(id).unwrap()).await;
    assert_head(&before, &users, &command);
    let mut restarted = ServiceProcess::start(&config, address, scratch.0.join("restarted.log"));
    restarted.ready().await;
    let rebound = attach(address, id, &users[0], csrf[0]);
    assert_eq!(rebound.seat(), 0);
    assert_eq!(rebound.next_seq(), 2);
    assert_eq!(rebound.operation_scope(), attached[0].operation_scope());
    let duplicate =
        MatchCommandRequest::new(rebound.attachment_id().into(), command.clone()).unwrap();
    assert_ack(
        &post(
            address,
            &format!("/api/v1/matches/{id}/command"),
            &users[0],
            csrf[0],
            &duplicate,
        ),
        1,
    );
    restarted.terminate().await;
    let after = inspect_committed(&store, parse_match_id(id).unwrap()).await;
    assert_head(&after, &users, &command);
    assert_eq!(before.records[1].hash, after.records[1].hash);
    stop_context.send(()).unwrap();
    auth_context.await.unwrap();
    store.close().await;
}
