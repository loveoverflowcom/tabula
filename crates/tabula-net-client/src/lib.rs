//! # `tabula-net-client` — the client session
//!
//! > ## PHASE 4
//!
//! One API, two transports: `tokio-tungstenite` (native) and browser `WebSocket`
//! via `web-sys` (WASM). The reconnect, sequencing, and idempotency logic is
//! subtle and security-relevant, and it is needed by the Leptos shell, the
//! Macroquad client, the load generator, and the integration tests.
//!
//! **It must exist exactly once.** Two copies of resume logic diverge, and the
//! divergence shows up as a duplicated move in a ranked game.
//!
//! ## The shape (doc 04 §4.1)
//!
//! ```rust,ignore
//! pub struct MatchClient {
//!     conn: Connection,              // native or web backend
//!     codec: Codec,
//!     seq: u32,
//!     pending: VecDeque<Pending>,    // un-acked commands
//!     cursor: StateVersion,
//!     state: ConnState,              // Connecting | Ready | Reconnecting | Resyncing | Failed
//!     events: mpsc::UnboundedSender<ClientEvent>,
//! }
//!
//! pub enum ClientEvent {
//!     Welcome { view: Bytes, capabilities: GameCapabilities, seat: Option<SeatId> },
//!     ViewEvents { to: StateVersion, events: Vec<Bytes> },
//!     Resync { at: StateVersion, view: Bytes },
//!     Ack    { client_seq: u32, at: StateVersion },
//!     Reject { client_seq: u32, code: RuleErrorCode, detail: Option<String> },
//!     Platform(PlatformEvent),
//!     ConnState(ConnState),
//!     Fatal(FatalError),
//! }
//!
//! impl MatchClient {
//!     pub fn send_command<C: Serialize>(&mut self, cmd: &C) -> u32;  // returns client_seq
//!     pub fn poll(&mut self) -> impl Iterator<Item = ClientEvent> + '_;
//! }
//! ```
//!
//! ## `poll()` is the load-bearing API decision
//!
//! It is drained **once per frame**, and it returns an iterator rather than
//! invoking callbacks. That keeps the game loop synchronous, single-threaded,
//! and lock-free — no `async` anywhere in the presentation path.
//!
//! Get this wrong (callbacks, or an async render path) and every presenter has
//! to reason about reentrancy.
//!
//! ## Reconnect (doc 04 §4.4)
//!
//! ```text
//! close/error → ConnState::Reconnecting
//! attempt n:  delay = min(30s, 0.5s * 2^n) * rand(0.5..1.0)      [FULL jitter]
//! close code 4411 (draining) → IMMEDIATE retry, no backoff
//! on connect: Hello → Attach { resume_from: cursor, last_client_seq }
//!             ResumeOk → fold events, replay un-acked commands after acked_through
//!             Resync   → replace view, clear pending, clear animations,
//!                        show a brief "resynced" cue
//! after 6 failed attempts → ConnState::Failed; offer "retry" and "leave match"
//! ```
//!
//! Full jitter, not equal jitter: a deploy disconnects everyone at once, and
//! anything less than full jitter reconnects them in a thundering herd.
//!
//! 4411 is special *because* it means "we are draining, another instance is
//! ready" — backing off there just extends the outage.
//!
//! ## I-12: preview is structurally separate from truth
//!
//! `pending` holds `PendingCommand`s, a **different type** from `View`. The
//! presenter signature is `present(view, local, frame)`, and the preview travels
//! inside `local` — so merging it into the authoritative view is always an
//! explicit act (usually a translucent ghost piece), never an accident.
//!
//! When `capabilities.client_preview == false`, `pending` is display-only
//! ("sending…") and no client-side rules evaluation happens at all.
//!
//! ## Local storage (doc 04 §4.5)
//!
//! ```rust,ignore
//! pub trait KvStore {
//!     fn get(&self, key: &str) -> Option<Vec<u8>>;
//!     fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError>;
//!     fn remove(&self, key: &str);
//! }
//! ```
//!
//! | Data | Key | Backend |
//! |---|---|---|
//! | Session token | `auth.session` | web `localStorage` / native OS keychain |
//! | Preferences | `prefs.v1` | `KvStore`, server-synced |
//! | Catalog | `catalog.v1` | `KvStore`, ETag-revalidated |
//! | Match handoff | `match.ctx` | `sessionStorage` (web) — survives a refresh |
//! | Asset cache | content hash | Cache API/IndexedDB (web), app cache dir (native) |
//!
//! **No game state is ever cached locally as authoritative.**
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/client.rs     MatchClient, send_command, poll
//! src/session.rs    handshake, codec negotiation, auth token attach
//! src/resume.rs     backoff + jitter, resume vs resync, pending replay
//! src/pending.rs    PendingCommand tracking, ack/reject correlation
//! src/transport/
//!   mod.rs          the Connection trait — ONE API
//!   native.rs       #[cfg(feature = "native")] tokio-tungstenite
//!   web.rs          #[cfg(feature = "web")]    web-sys WebSocket
//! src/kv.rs         KvStore trait + per-platform backends
//! ```

#![forbid(unsafe_code)]
