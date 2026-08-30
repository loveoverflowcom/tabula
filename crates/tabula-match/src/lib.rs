//! # `tabula-match` — the authoritative match runtime
//!
//! > ## PHASE 4 — DO NOT IMPLEMENT BEFORE PHASE 3 EXITS
//! >
//! > This is the hardest, most correctness-critical async code in the product.
//! > It is also where the ordering and idempotency bugs live — doc 09 §6 lists
//! > "Phase 4 ordering/idempotency bugs under load" among the five things most
//! > likely to go wrong.
//!
//! ## The one structural rule: one match, one owner
//!
//! One Tokio task plus one bounded `mpsc` mailbox per live match. No actor
//! framework — frameworks add supervision we do not yet need (ADR-006). Single
//! writer is I-14, and it is expressed by **Rust ownership**: the actor holds
//! `Box<dyn ErasedMatch>`, so there is no second handle to the state anywhere.
//!
//! **No unbounded channels anywhere.** (doc 03 §6.4)
//!
//! ## The command pipeline — exactly these steps, in this order, once (doc 03 §7)
//!
//! ```text
//!  1  WS frame received                                  reader task
//!  2  rate limit: session (20/s burst 40) + seat (5/s)   edge
//!  3  decode ENVELOPE ONLY — payload stays opaque        edge
//!  4  authorize: attached to this match? owns this seat? edge
//!  5  forward to bounded mailbox (1024) + tracing span   edge → actor
//!     └─ mailbox full → Reject{BUSY}
//!  6  idempotency check on (seat, client_seq)            actor
//!     └─ duplicate → RE-SEND STORED ACK, do not re-apply
//!  7  ErasedMatch::decode_command -> typed Command       actor
//!     └─ malformed → Reject{MALFORMED}, count for abuse
//!  8  build Ctx: now = logical_now(), rng = DetRng::for_input(seed, index)
//!  9  apply() inside catch_unwind                        actor
//!     └─ Err → Reject; STATE UNCHANGED (R2); VERSION UNCHANGED (I-7)
//! 10  version += 1; index += 1
//! 11  append canonical events (+ snapshot if due, + state_hash at checkpoints)
//! 12  durability?
//! 13a AckAfterPersist: await commit, THEN ack
//! 13b AckAfterApply:   ack now, commit in a background batch
//! 14  redact: view_events(viewer) per attached viewer
//! 15  broadcast ViewEvents + new state_version
//! 16  execute effects (timers, scopes, bot requests, EndMatch)
//! 17  metrics + span close
//! ```
//!
//! Step 3 is I-9 at the network edge: the game payload stays opaque bytes until
//! it reaches `ErasedMatch::decode_command` **inside the actor**. The session
//! layer knows nothing about games.
//!
//! Step 16 runs **after** persistence, which is why every `Effect` must be
//! idempotent — see `tabula_game_api::effect`.
//!
//! ## Three counters, three different jobs (doc 03 §8.1)
//!
//! | Counter | Scope | Assigned by |
//! |---|---|---|
//! | `client_seq` | per (session, match) | client; monotonic; reset only on a new session |
//! | `state_version` | per match | server; +1 per applied input (I-7) |
//! | `input_index` | per match | the log; equals the row ordinal; the RNG domain root |
//!
//! Conflating any two of these is a subtle, load-dependent bug. They are separate
//! types for that reason.
//!
//! ## Idempotency (doc 03 §8.2)
//!
//! ```text
//! client_seq <= highest AND in `recent`   → re-send the stored result, DO NOT re-apply
//! client_seq <= highest AND evicted       → Reject{STALE_SEQ}
//! client_seq >  highest + 64              → Reject{SEQ_TOO_FAR}
//! ```
//!
//! The cache is **in memory only**. A crash can therefore let a duplicate
//! re-apply; the resume flow is what mitigates it. That is a deliberate,
//! documented trade — persisting it would put a database round trip on the hot
//! path of every command.
//!
//! ## Supervision (doc 03 §6.4)
//!
//! ```text
//! panic inside apply()     caught by catch_unwind; match ends
//!                          Aborted{RulesPanic}; backtrace + input logged; ALERT.
//!                          The PROCESS SURVIVES. Always a Sev-2 bug (violates R3).
//! panic elsewhere          supervisor sees JoinError; rehydrate from snapshot + log;
//!                          attached sessions get Resync.
//! mailbox full (1024)      Reject{BUSY} + counter. Sustained fullness is an alert:
//!                          the actor is CPU-starved or blocked.
//! actor stuck > 5 s        watchdog metric + alert. NOT killed automatically —
//!                          killing loses ordering guarantees. An operator decides.
//! process shutdown         Drain with a 15 s deadline: flush events, write a final
//!                          snapshot, Close(4411), exit.
//! ```
//!
//! This is why the server builds with `panic = "unwind"`
//! (`--profile release-server`) while everything else aborts.
//!
//! ## Timers survive restarts because they are re-derived, not restored
//!
//! On actor start, `rearm_timers_from_state()` reconstructs pending timers from
//! the game state. Nothing timer-related is carried in memory across a restart.
//! That single decision is why a deploy mid-chess-game does not lose the clock.
//! (doc 03 §12.1)
//!
//! Long-horizon timers (async turns, 24 h deadlines) additionally live in a
//! `durable_timers` table polled with `FOR UPDATE SKIP LOCKED`. That poll is
//! **the only place a database poll drives gameplay** (doc 03 §12.2).
//!
//! ## Module layout when this becomes real (doc 03 §4–§12)
//!
//! ```text
//! src/ports.rs        EventLog, SnapshotStore, MatchRepo, Clock, BotRunner, Broadcast
//! src/actor.rs        MatchActor + the run loop            (doc 03 §6.2, §6.3)
//! src/mailbox.rs      Envelope                             (doc 03 §6.2)
//! src/session.rs      SessionId, Session, Attachment       (doc 03 §4)
//! src/router.rs       RoomRouter, MatchHandle              (doc 03 §5)
//! src/supervisor.rs   catch_unwind, restart policy, drain  (doc 03 §6.1, §6.4)
//! src/pipeline.rs     the 17 stages                        (doc 03 §7)
//! src/idempotency.rs  IdempotencyCache, SeatSeqState       (doc 03 §8.2)
//! src/timers.rs       TimerSet, durable timer bridge       (doc 03 §12)
//! src/snapshot.rs     cadence by StateSizeClass            (doc 03 §9.2)
//! src/resume.rs       reconnect; ResumeOk vs Resync at 200 (doc 03 §10)
//! src/spectator.rs    attach, viewer-group fan-out, delay  (doc 03 §11.1)
//! src/effects.rs      effect execution + idempotency keys  (doc 03 §7.1)
//! src/seats.rs        SeatTable, AttachedViewer
//! ```
//!
//! ## Build this in this order
//!
//! Ports → in-memory fakes in `tabula-testkit` → actor + pipeline against the
//! fakes → **then** `tabula-storage`. If storage comes first, the ports get
//! shaped by SQL instead of by the runtime, and the fast test suite never
//! materialises.

#![forbid(unsafe_code)]

pub mod ports;
