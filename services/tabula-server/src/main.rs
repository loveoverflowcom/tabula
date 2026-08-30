//! # `tabula-server` — THE binary at Stage 0
//!
//! > ## PHASE 4
//!
//! HTTP API + WebSocket gateway + match runtime + lobby, in **one process**
//! composed of library crates that already have the right seams (ADR-015).
//! The crates are the boundary; the process count is a deployment decision.
//!
//! Doc 01 §2.3 rejects the "separate services from day one" outline explicitly:
//! matchmaking, lobby, and the match runtime all need the same room directory,
//! and splitting them at Stage 0 turns that directory into a distributed-consensus
//! problem for zero benefit.
//!
//! The split we will actually want is **gateway ↔ match-worker** (connection
//! fan-out scales differently from CPU-bound match application), and doc 06 §7
//! specifies both it and its measured trigger.
//!
//! ## Build it to unwind
//!
//! ```bash
//! cargo build -p tabula-server --profile release-server   # panic = "unwind"
//! ```
//!
//! The match runtime wraps every `apply()` in `catch_unwind` so a panicking game
//! aborts **that match**, not the process (doc 01 §5.2). A server built with
//! `panic = "abort"` throws that away.
//!
//! ## HTTP surface (doc 03 §2)
//!
//! ```text
//! POST   /api/v1/auth/{register,login,logout,refresh}
//! GET    /api/v1/auth/oidc/:provider  + /callback
//! GET    /api/v1/me
//! GET    /api/v1/games                     catalog, rollout-filtered, i18n keys
//! GET    /api/v1/games/:id                 metadata + capabilities + config schema
//! POST   /api/v1/rooms                     create
//! GET    /api/v1/rooms                     browse (paginated, filtered)
//! POST   /api/v1/rooms/:id/join            reserve a seat → join_token
//! POST   /api/v1/queue   DELETE /api/v1/queue
//! POST   /api/v1/matches                   direct creation (friendly/testing)
//! GET    /api/v1/matches/:id               summary, for resume/spectate discovery
//! GET    /api/v1/matches/:id/replay        signed URL
//! GET    /api/v1/users/:id/matches         history
//! GET    /api/v1/assets/manifest/:pack     long-cache, hashed
//! GET    /healthz  /readyz  /metrics       ops
//! *      /api/v1/admin/*                   separate authz role
//! ```
//!
//! Conventions: `Authorization: Bearer <session>`, `UUIDv7` ids, RFC 9457
//! `problem+json` errors, cursor pagination, `Idempotency-Key` honoured on every
//! resource-creating POST.
//!
//! `/readyz` means **DB reachable, registry loaded, migrations current** — it is
//! what the load balancer gates on during a rolling deploy (doc 06 §11.3).
//!
//! ## WebSocket: exactly one endpoint
//!
//! `GET /ws`, upgraded with `Sec-WebSocket-Protocol: tabula.v1.postcard`
//! (production) or `tabula.v1.json` (dev; refused in production for non-staff).
//!
//! ```text
//! max inbound frame     64 KiB          ping interval       20 s
//! max outbound frame     1 MiB          pong timeout        30 s → Close(4408)
//! outbound queue       256 msgs         → Close(4409, slow_consumer)
//! inbound rate      20 msg/s burst 40 per session; 5 commands/s per seat
//! idle (no attach)      60 s            max sessions/user   4
//! mailbox depth       1024              drain deadline      15 s
//! ```
//!
//! Close codes (doc 03 §3.3):
//!
//! ```text
//! 4400 protocol_version_unsupported   4405 already_attached   4410 match_ended
//! 4401 unauthenticated                4408 heartbeat_timeout  4411 server_draining
//! 4403 unauthorized                   4409 slow_consumer      4429 rate_limited
//! 4404 match_not_found                                        4500 internal
//! ```
//!
//! **4411 means "reconnect immediately, do not back off"** — the client is being
//! moved to a new instance during a deploy, and backing off just extends the gap.
//!
//! ## The session layer knows nothing about games
//!
//! It authenticates, rate-limits, decodes the **envelope** (never the game
//! payload), and forwards. The payload stays opaque bytes until it reaches
//! `ErasedMatch::decode_command` inside the actor. **That is I-9 at the network
//! edge** (doc 03 §4).
//!
//! ## Config
//!
//! `figment`: TOML + env overrides, one typed struct, **validated once at boot,
//! fail fast**. The values that matter at Stage 0 (doc 06 §3.3):
//!
//! ```text
//! tokio worker threads      = vCPUs (default)
//! PgPool max                = min(4 x cores, 40)
//! WS read/write buffer      = 16 KiB each   (set explicitly; do not accept defaults)
//! max connections per user  = 4
//! tcp_nodelay               = true          (turn-based traffic is small and latency-sensitive)
//! WS permessage-deflate     = OFF           (payloads are already compact)
//! statement_timeout         = 5 s
//! ```
//!
//! ## Drain (doc 06 §11.3) — zero lost matches is a Stage 0 exit criterion
//!
//! ```text
//! 1. new instance starts, passes /readyz
//! 2. LB sends new connections to it
//! 3. old instance: SIGTERM → 15 s drain
//!      stop accepting attach → snapshot every live match → flush event batches
//!      → send Draining{retry_after_ms: 250} → Close(4411)
//! 4. clients reconnect immediately and land on the new instance
//! 5. matches rehydrate LAZILY on attach   (startup must be O(1) in match count)
//! ```
//!
//! ## Observability (doc 06 §9)
//!
//! One span per command, `match.command`, with children `decode`, `apply`,
//! `persist`, `redact_project`, `broadcast`, `effects` — because "which of those
//! six is slow" is the only question that matters when a match feels laggy.
//!
//! Sampling: 100% of errors and over-budget spans, 1% of normal commands, 100%
//! for a match under investigation.
//!
//! Three counters **must always be 0** and page when they are not:
//! `tabula_state_hash_mismatch_total`, `tabula_projection_scan_failures_total`,
//! `tabula_actor_panics_total`.
//!
//! **Never log**: seeds, session tokens, join tokens, canonical state, hidden
//! information, chat bodies.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/main.rs        boot: config → tracing → migrations → registry → serve
//! src/config.rs      the typed config struct; validated once, fails fast
//! src/http/          one module per route group above
//! src/ws/
//!   upgrade.rs       subprotocol negotiation, Hello, HelloAck
//!   session.rs       reader/writer tasks, heartbeat, backpressure
//!   limits.rs        token buckets: per session and per seat
//! src/auth/          argon2 passwords, opaque sessions, OIDC, match tokens
//! src/chat.rs        transport + scope enforcement (scoping comes from the game)
//! src/admin/         inspect, cancel, rollout
//! src/telemetry.rs   tracing-subscriber, OTLP, Prometheus metrics
//! src/shutdown.rs    the drain sequence above
//! ```

fn main() {
    eprintln!(
        "tabula-server is a Phase 4 deliverable (docs/architecture/07-phases-and-implementation-roadmap.md).\n\
         Gate: four games pass conformance AND the game contract stopped changing (Phase 3 exit).\n\
         Doc 09 §7: building the server on a moving contract is how protocols get corrupted."
    );
    std::process::exit(1);
}
