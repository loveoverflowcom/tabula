//! The ports. (doc 03 §6.2, doc 01 §3)
//!
//! # Why these exist
//!
//! Everything the match runtime needs from the outside world is a trait defined
//! **here**, and implemented in `tabula-storage` (Postgres) or `tabula-testkit`
//! (in-memory fakes). That is the seam that makes the runtime testable with no
//! database and no HTTP server — and a runtime that needs Postgres to test its
//! ordering logic is a runtime whose ordering logic will not be tested.
//!
//! It is also the seam that makes "add a read replica", "move snapshots to object
//! storage", and "split gateway from match-worker" non-invasive.
//!
//! # ⚠ These signatures are ours to design
//!
//! The architecture docs **name** these ports but never define them. Doc 01 §3
//! lists them; doc 03 §6.2 shows them as `Arc<dyn EventLog>` fields. The
//! signatures below are a first draft derived from the constraints the docs *do*
//! state (§9.6 write path, §19.3 pool discipline, §10 resume). Review them
//! against those sections before locking anything in Phase 4.
//!
//! # Two constraints that must survive review
//!
//! 1. **Take owned data and return.** No port may hold a database connection
//!    across an unrelated await (doc 03 §19.3). A method that borrows and yields
//!    invites exactly that.
//! 2. **One transaction per applied input.** Appending inputs, appending events,
//!    and bumping `matches.state_version` are one transaction (doc 03 §9.6). The
//!    API shape must make it impossible to do them separately.
//!
//! Native `async fn` in traits (AFIT) is used deliberately: it means this module
//! needs **no runtime dependency at all**, so the ports compile in a Phase-0
//! checkout while the actor that uses them does not exist yet.

use tabula_core::{InputIndex, MatchId, RulesVersion, StateHash, StateVersion};

/// One input plus the events it produced, ready to commit atomically.
#[derive(Clone, Debug)]
pub struct LogBatch {
    pub match_id: MatchId,
    pub input_index: InputIndex,
    pub state_version: StateVersion,
    pub kind: InputKind,
    pub seat: Option<u8>,
    pub logical_ms: u64,
    /// `canonical(Input<Command>)` — postcard, always. (doc 05 §7.1)
    pub input_payload: Vec<u8>,
    /// `canonical(Event)` for each event, in order.
    pub events: Vec<Vec<u8>>,
    /// Present every N inputs (default N = 20). This is the value the nightly
    /// replay job compares against. (doc 05 §7.2)
    pub state_hash: Option<StateHash>,
}

/// Matches the `match_inputs.kind` smallint. (doc 03 §9.4)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    Player = 0,
    Timer = 1,
    Seat = 2,
    Admin = 3,
}

/// The append-only log. (doc 03 §9)
///
/// We store **inputs and events**, not just one of them: inputs are what replay
/// needs, events are what resume and audit need without re-running rules.
pub trait EventLog: Send + Sync {
    /// Append one applied input and its events **in a single transaction**,
    /// together with the `matches.state_version` bump.
    ///
    /// For `Durability::AckAfterApply` the implementation may batch (≤64 events
    /// or ≤50 ms) — the caller has already acked. For `AckAfterPersist` the
    /// caller awaits this before acking, so it must not batch.
    ///
    /// # Errors
    /// Any storage failure. The actor's response is to mark the match suspect
    /// and stop accepting inputs, not to retry blindly — a partially applied
    /// state that keeps taking commands is worse than a stopped match.
    fn append(
        &self,
        batch: LogBatch,
    ) -> impl std::future::Future<Output = Result<(), PortError>> + Send;

    /// Events strictly after `after`, for the resume path (`ResumeOk`).
    ///
    /// Only called when `state_version - resume_from <= 200`; beyond that the
    /// actor sends a full `Resync` instead, because replaying 10k events to a
    /// reconnecting client is slower than re-projecting. (doc 03 §10.1)
    fn events_after(
        &self,
        match_id: MatchId,
        after: InputIndex,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<u8>>, PortError>> + Send;

    /// Every input from `from` onward — the replay path.
    fn inputs_from(
        &self,
        match_id: MatchId,
        from: InputIndex,
    ) -> impl std::future::Future<Output = Result<Vec<LogBatch>, PortError>> + Send;
}

/// Periodic canonical state, so replay cost is bounded. (doc 03 §9.1, §9.2)
pub trait SnapshotStore: Send + Sync {
    /// Cadence and encoding are driven by `StateSizeClass`; large states go to
    /// object storage with a pointer row in Postgres.
    fn put(
        &self,
        match_id: MatchId,
        at: StateVersion,
        rules_version: RulesVersion,
        payload: Vec<u8>,
        hash: StateHash,
    ) -> impl std::future::Future<Output = Result<(), PortError>> + Send;

    /// The **newest snapshot at or before** `at` — not the nearest, and not the
    /// latest. Replay always moves forward from a known-good point.
    fn load_nearest(
        &self,
        match_id: MatchId,
        at: StateVersion,
    ) -> impl std::future::Future<Output = Result<Option<Snapshot>, PortError>> + Send;
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub state_version: StateVersion,
    pub rules_version: RulesVersion,
    pub payload: Vec<u8>,
    pub state_hash: StateHash,
}

/// The `matches` and `match_players` aggregate. (doc 03 §9.4)
pub trait MatchRepo: Send + Sync {
    fn load(
        &self,
        id: MatchId,
    ) -> impl std::future::Future<Output = Result<Option<MatchRecord>, PortError>> + Send;

    /// Record the outcome. **Must be guarded by `ended_at IS NULL` in a single
    /// UPDATE**, which is what makes `Effect::EndMatch` idempotent under crash
    /// recovery (doc 03 §7.1).
    fn end_match(
        &self,
        id: MatchId,
        outcome_json: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<bool, PortError>> + Send;

    /// Seat ownership, for pipeline step 4. Sessions must never be able to
    /// self-assign a seat (doc 03 §21).
    fn seat_owner(
        &self,
        id: MatchId,
        seat: u8,
    ) -> impl std::future::Future<Output = Result<Option<u128>, PortError>> + Send;

    /// On startup, mark matches orphaned by a crash as `hibernating` so they
    /// rehydrate lazily on attach. Startup must be O(1) in live-match count —
    /// eagerly rehydrating every match makes restarts scale with popularity
    /// (doc 03 §13.1).
    fn reap_orphans(
        &self,
        node_id: &str,
    ) -> impl std::future::Future<Output = Result<u64, PortError>> + Send;
}

#[derive(Clone, Debug)]
pub struct MatchRecord {
    pub id: MatchId,
    pub state_version: StateVersion,
    pub rules_version: RulesVersion,
    /// The match seed. **Encrypted at rest, never logged, never sent to a
    /// client, and redacted even from `Viewer::Audit` debug dumps**
    /// (doc 03 §19.4, §21).
    pub seed: [u8; 32],
    pub started_at_unix_ms: u64,
    pub paused_for_ms: u64,
    pub last_logical_ms: u64,
}

/// Real time, injectable. (doc 01 §3)
///
/// The whole point is the fake: a manually advanced clock turns timer, grace
/// period, and hibernation tests from minutes of sleeping into microseconds of
/// determinism.
pub trait Clock: Send + Sync {
    fn now_unix_ms(&self) -> u64;
    /// Monotonic, for measuring elapsed time within one process.
    fn monotonic_ms(&self) -> u64;
}

/// Bot move requests. (doc 02 §6)
///
/// A **handle**, not an `Arc<dyn>`: it owns a task plus `spawn_blocking` for
/// heavier search, and it needs to be cloneable into each actor.
///
/// The reply comes back as `Envelope::BotMove` and enters the pipeline as an
/// ordinary `Input::Player` — a bot has no privileged path.
pub trait BotRunner: Send + Sync {
    /// Idempotency key is `(match_id, seat, state_version)`. A duplicate request
    /// for an already-advanced version is dropped, which is what makes
    /// `Effect::RequestBotMove` safe to re-run after a crash.
    fn request(
        &self,
        match_id: MatchId,
        seat: u8,
        at: StateVersion,
        deadline_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), PortError>> + Send;
}

/// Fan-out to attached viewers. (doc 03 §20 seam 4)
///
/// # Group by viewer, not by connection
///
/// **Emit per-viewer-*group* byte streams, not per-viewer.** All spectators share
/// one group; each seat is its own group. Cost is therefore O(seats + 1)
/// redactions per input rather than O(attached sessions) — which is the
/// difference between a 5,000-spectator match working and melting.
///
/// Implement the grouping at Stage 0, even though in-process fan-out would work
/// without it. Retrofitting it later means rewriting the redaction path.
pub trait Broadcast: Send + Sync {
    fn send_group(
        &self,
        match_id: MatchId,
        group: ViewerGroup,
        at: StateVersion,
        frames: Vec<Vec<u8>>,
    ) -> impl std::future::Future<Output = Result<(), PortError>> + Send;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ViewerGroup {
    Seat(u8),
    SpectatorsLive,
    /// Buffered by the actor; one group per distinct delay window.
    SpectatorsDelayed,
}

#[derive(Debug, thiserror::Error)]
pub enum PortError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("not found")]
    NotFound,
    #[error("encode/decode: {0}")]
    Codec(String),
}
