//! SQL-free, server-only atomic match journal contract (doc 03 §9, ADR-0040).
//!
//! This module has no registry, runtime, transport, or database dependency.
//! Canonical snapshots, seeds and event records must never become client frames.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use tabula_core::{
    GameId, GameVersion, InputIndex, LogicalTime, MatchId, RulesVersion, SeatId, SeatRoster,
    StateHash, StateVersion, UserId,
};
use tabula_game_api::Effect;
use tabula_protocol::{ErrorCode, GameCommandFrame};

/// Version of this isolated persisted format, independent of the client wire.
pub const JOURNAL_FORMAT: u16 = 1;
/// Hard bound for one canonical state snapshot.
pub const MAX_SNAPSHOT_BYTES: usize = 1_048_576;
/// Hard bound for the complete durable operation ledger.
pub const MAX_LEDGER_BYTES: usize = 4_194_304;
/// Bounded isolated recovery domain; larger matches require a reviewed streaming adapter.
pub const MAX_RECOVERY_RECORDS: usize = 10_001;
/// Maximum total encoded bytes admitted by one isolated recovery.
pub const MAX_RECOVERY_BYTES: usize = 67_108_864;

/// Exact approved package/rules identity; no game-specific dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchIdentity {
    pub game: GameId,
    pub game_version: GameVersion,
    pub rules_version: RulesVersion,
    pub rules_hash: [u8; 32],
}

/// Persisted local bounds, unchanged when an actor is recovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerLimits {
    pub scopes: u16,
    pub receipts_per_scope: u16,
    pub receipt_ttl_ms: u64,
}

/// Immutable initialization facts. Seed/config are private server data (I-16).
#[derive(Clone, Serialize, Deserialize)]
pub struct MatchCreation {
    pub format: u16,
    pub identity: MatchIdentity,
    pub config: Vec<u8>,
    pub roster: SeatRoster,
    pub seed: [u8; 32],
    pub started_at_unix_ms: u64,
    pub limits: LedgerLimits,
}
impl core::fmt::Debug for MatchCreation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MatchCreation")
            .field("format", &self.format)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Match-global identity of a command scope. Connections never reset this key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationScope {
    pub record: u128,
    pub subject: UserId,
    pub epoch: u64,
    pub seat: SeatId,
    pub generation: u64,
}

/// Original public-safe result and exact payload retained for bounded retries.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReceipt {
    pub seq: u64,
    pub command: GameCommandFrame,
    pub result: Result<(), ErrorCode>,
    /// Match logical elapsed time, not process uptime; survives restart.
    pub at: u64,
    pub committed_index: Option<InputIndex>,
}

/// High-watermark is never evicted; expired receipts never authorize reapply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeState {
    pub scope: OperationScope,
    pub highest: u64,
    pub recent: Vec<OperationReceipt>,
}

/// Immutable accepted-command identity retained in the append-only input row.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationKey {
    pub scope: OperationScope,
    pub seq: u64,
    pub command: GameCommandFrame,
}

/// One atomic input/events/version/hash/snapshot/operation-ledger transaction.
#[derive(Clone, Serialize, Deserialize)]
pub struct JournalRecord {
    pub match_id: MatchId,
    pub index: InputIndex,
    pub version: StateVersion,
    pub now: LogicalTime,
    /// Empty only at creation; otherwise canonical typed Input<Command> bytes.
    pub input: Vec<u8>,
    pub events: Vec<Vec<u8>>,
    pub hash: StateHash,
    pub effects: Vec<Effect>,
    pub operation: Option<OperationKey>,
    pub terminal: bool,
    /// Snapshot is part of this commit, never a separate authority.
    pub snapshot: Option<Vec<u8>>,
    /// Present only at index/version zero.
    pub creation: Option<MatchCreation>,
    /// Complete bounded watermark/receipt state after this operation.
    pub ledger: Vec<ScopeState>,
    /// None means creation; otherwise exact last committed canonical version.
    pub expected_version: Option<StateVersion>,
}

/// Consistent committed prefix plus current bounded ledger loaded in one DB snapshot.
#[derive(Clone, Debug)]
pub struct LoadedMatch {
    pub creation: MatchCreation,
    pub records: Vec<JournalRecord>,
    pub ledger: Vec<ScopeState>,
    pub version: StateVersion,
    pub index: InputIndex,
    pub observed_ms: u64,
}

/// SQL-free authoritative journal, with known-success commit receipts only.
///
/// An implementation must fence stale owners and guard `expected_version` inside
/// each transaction. Failure/unknown outcome stops the actor. A replacement
/// claims a new fence and validates recovery before accepting any mailbox work.
pub trait Journal: Send + Sync + 'static {
    fn append(
        &self,
        record: JournalRecord,
    ) -> impl std::future::Future<Output = Result<(), RuntimePortError>> + Send;

    /// Reserve visible admitted scopes and consume rejected commands durably.
    /// Defaults are offline acceptance only; durable implementations override.
    fn update_ledger(
        &self,
        _match_id: MatchId,
        _expected_version: StateVersion,
        _observed_ms: u64,
        _ledger: Vec<ScopeState>,
    ) -> impl std::future::Future<Output = Result<(), RuntimePortError>> + Send {
        async { Ok(()) }
    }

    /// Offline fakes do not advertise recovery. No absent row may mean success.
    fn load(
        &self,
        _match_id: MatchId,
    ) -> impl std::future::Future<Output = Result<LoadedMatch, RuntimePortError>> + Send {
        async { Err(RuntimePortError::Unavailable) }
    }
}

/// Public-safe port failures, never database diagnostics or secret payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePortError {
    Unavailable,
    Busy,
    Indeterminate,
}

impl core::fmt::Debug for OperationReceipt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OperationReceipt")
            .field("seq", &self.seq)
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}
impl core::fmt::Debug for OperationKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OperationKey")
            .field("seq", &self.seq)
            .finish_non_exhaustive()
    }
}
impl core::fmt::Debug for JournalRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JournalRecord")
            .field("match_id", &self.match_id)
            .field("index", &self.index)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
