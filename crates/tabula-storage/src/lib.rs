//! # `tabula-storage` — the only crate that knows SQL exists
//!
//! > ## PHASE 4
//!
//! Everything above this crate is testable with in-memory fakes, which keeps the
//! test suite fast and the design honest. Building storage *before* the runtime
//! inverts that: the ports end up shaped by SQL, and the fast test suite never
//! materialises. Build ports → fakes → runtime → **then** this crate.
//!
//! **Forbidden: `axum`, game crates, renderers, `tabula-registry`.**
//!
//! ## Compile-time-checked queries, offline
//!
//! `sqlx` with the macros and a **committed `.sqlx/`** directory, so the
//! workspace builds in CI and on a fresh checkout without a live database
//! (doc 01 §1.2). Regenerate with `just sqlx-prepare` after changing a query.
//!
//! If macro compile times become painful, fall back to `sqlx::query` with
//! hand-mapped rows in the hot files — not to an ORM.
//!
//! ## Schema (doc 03 §9.4, §12.2) — 18 tables
//!
//! ```text
//! identity     users, user_identities, sessions
//! catalog      games, game_versions
//! play         rooms, matches, match_players
//! THE LOG      match_inputs, match_events, match_snapshots, pending_effects
//! scheduling   durable_timers
//! social       ratings, replays, chat_messages, presence, queue_entries
//! ```
//!
//! Three columns worth calling out:
//!
//! - `matches.seed` — **encrypted at rest**, never logged, never sent to a
//!   client, redacted even from `Viewer::Audit` debug dumps (doc 03 §19.4, §21).
//!   Leaking it is a total loss of hidden information for that match.
//! - `matches.id` is `UUIDv7` — time-ordered, which is what makes range
//!   partitioning and time-bounded queries cheap later.
//! - `match_events.state_hash` — non-null every N inputs (default 20). This is
//!   the column the nightly replay job compares against, and therefore the only
//!   production detector of determinism drift.
//!
//! ## Migration discipline (doc 06 §11.2) — additive only
//!
//! ```text
//! Add a column       nullable or defaulted; the app tolerates its absence for one release
//! Remove a column    TWO releases: stop using it, then drop it
//! Rename             NEVER. Add new, migrate, drop old.
//! Change a type      new column + backfill + swap
//! Add an index       CREATE INDEX CONCURRENTLY, outside a transaction
//! Long backfills     a job — batched, resumable. NEVER a migration.
//! ```
//!
//! CI gates deploys on an additive-only check (doc 06 §11.1), because a migration
//! that breaks the previous release makes rollback impossible exactly when you
//! need it.
//!
//! ## Write-path performance is the whole design constraint (doc 03 §9.6)
//!
//! Target: **one database round trip per applied input** for
//! `AckAfterPersist`. Appending inputs, appending events, and bumping
//! `matches.state_version` are **one transaction**. Multi-row insert, not a loop.
//!
//! Pool discipline (doc 03 §19.3, doc 06 §6.2):
//!
//! ```text
//! one PgPool per process, max = min(4 x cores, 40)
//! total app connections < 60% of postgres max_connections
//! statement_timeout = 5 s ; idle_in_transaction_session_timeout = 10 s
//! synchronous_commit = on for the match transaction
//! NEVER hold a connection across an unrelated await
//! when total connections > 200: pgbouncer, transaction pooling mode
//! ```
//!
//! ## Partitioning — decide once, and follow §19.2
//!
//! Range-partition `match_inputs` and `match_events` **by `created_at`,
//! monthly** — not by `match_id`. (Doc 03 §9.4's inline SQL comment offers both;
//! §19.2 decides. Follow §19.2.) Introduce it before the log exceeds ~50 GB.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! migrations/            plain SQL, run by `sqlx migrate`, versioned in-repo
//! src/pool.rs            PgPool construction + the discipline above
//! src/event_log.rs       impl EventLog (+ the AckAfterApply batcher)
//! src/snapshots.rs       impl SnapshotStore (zstd, object-storage pointer rows)
//! src/matches.rs         impl MatchRepo
//! src/rooms.rs           impl RoomRepo
//! src/queue.rs           impl QueueStore
//! src/presence.rs        impl PresenceStore
//! src/ratings.rs         impl RatingRepo
//! src/timers.rs          durable_timers poller (FOR UPDATE SKIP LOCKED)
//! src/effects.rs         pending_effects outbox
//! ```

#![forbid(unsafe_code)]
