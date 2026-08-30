//! The `.tbr` replay format and runner. (doc 05 §8)
//!
//! # Replay is a product feature, not a debugging tool
//!
//! It powers spectator catch-up, reconnect, anti-cheat audit, bug reproduction,
//! balance analytics, "watch your last match", and migration validation. It is
//! load-bearing and therefore tested continuously. (doc 00 §5.3)
//!
//! # The model
//!
//! A replay is **the input stream plus enough metadata to re-run it**. Events are
//! not stored — they are derivable — but a checkpoint hash list is included so
//! verification can pinpoint the exact input where a divergence began.
//!
//! # File layout (doc 05 §8.1)
//!
//! ```text
//! ┌ header (postcard, versioned) ─────────────────────────────────────────┐
//! │ magic: b"TBR1"                                                        │
//! │ format_version: u16                                                   │
//! │ match_id, game_id, game_version                                       │
//! │ rules_version, rules_hash: [u8;32]                                    │
//! │ config: canonical bytes                                               │
//! │ roster (occupants pseudonymized unless owner-authorized)              │
//! │ seed: Option<MatchSeed>       ← present ONLY in canonical replays      │
//! │ initial_snapshot: Option<bytes>  ← for partial/projected replays       │
//! │ started_at, duration_ms, outcome                                      │
//! │ kind: Canonical | Projected(Viewer)                                   │
//! ├ input frames (repeated, length-prefixed) ─────────────────────────────┤
//! │ input_index, logical_ms, canonical(Input<Command>)                    │
//! │ checkpoint: Option<[u8;32]>   ← every N inputs                        │
//! ├ trailer ──────────────────────────────────────────────────────────────┤
//! │ input_count, final_state_hash, crc32 of the body                      │
//! └───────────────────────────────────────────────────────────────────────┘
//! zstd-framed. Chess ≈ 3 KB. Werewolf ≈ 15 KB.
//! ```
//!
//! # Two kinds, and why the distinction is a security control
//!
//! | Kind | Contains | Who may hold it |
//! |---|---|---|
//! | **Canonical** | Seed + full inputs; reproduces all secrets | Server-side only, `Viewer::Audit` tooling |
//! | **Projected** | One viewer's `View` + that viewer's `ViewEvent` stream, **no seed** | Downloadable by users |
//!
//! Handing a user a canonical replay of a card game hands them the deck order.
//! The seed field is `Option` for exactly this reason, and the writer must refuse
//! to emit `Projected` with a seed present.

use std::path::Path;

use tabula_core::{RulesVersion, StateVersion};

/// Replays a `.tbr` and verifies it. Used by the nightly job (I-8), by
/// `xtask replay`, by ops tooling, and by the client's replay viewer.
#[derive(Debug)]
pub struct ReplayRunner {
    // TODO(phase 0): header, decoded input frames, a live ErasedMatch (Phase 4)
    // or a directly-typed GameRules run (Phase 0, before the registry exists).
    _private: (),
}

impl ReplayRunner {
    /// # Errors
    /// Bad magic, unsupported format version, or a corrupt trailer CRC.
    pub fn open(_path: &Path) -> Result<Self, ReplayError> {
        todo!("doc 05 §8.1: read header, validate magic + crc32, decode frames")
    }

    /// Decide whether this replay can be trusted before spending time on it.
    #[must_use]
    pub fn check(&self) -> ReplayVerdict {
        todo!("doc 05 §10.2 compatibility matrix")
    }

    /// # Errors
    /// A decode failure, or a rules rejection where the log recorded acceptance
    /// (which is itself a divergence).
    pub fn step(&mut self) -> Result<Option<StepResult>, ReplayError> {
        todo!()
    }

    /// # Errors
    /// As `step`.
    pub fn seek(&mut self, _to: StateVersion) -> Result<(), ReplayError> {
        todo!("restore nearest snapshot <= target, then step forward")
    }

    /// Re-run everything, comparing every checkpoint. The nightly job's entry
    /// point.
    ///
    /// # Errors
    /// As `step`. A checkpoint mismatch is reported in [`VerifyReport`], not as
    /// an `Err` — the caller wants the full divergence picture, not the first
    /// failure.
    pub fn verify(&mut self) -> Result<VerifyReport, ReplayError> {
        todo!("doc 05 §7.3: on mismatch, record (match_id, input_index, expected, actual, rules_hash, build)")
    }
}

/// Can this replay be trusted? (doc 05 §10.2)
///
/// **We never fake a replay.** `Unreplayable` shows the stored outcome and event
/// summary rather than a plausible reconstruction — a wrong replay presented as
/// right destroys the audit value of every replay we hold.
#[derive(Clone, Debug)]
pub enum ReplayVerdict {
    /// `rules_hash` matches a linked build. Replay normally.
    Exact,
    /// `rules_version` is linked but the hash differs — someone changed behaviour
    /// without bumping the version, or the build differs. Replay *with*
    /// verification; a checkpoint mismatch becomes a divergence report.
    CompatibleVersion,
    /// Migrate the initial snapshot, replay from there, mark the result "migrated".
    NeedsMigration {
        from: RulesVersion,
    },
    Unreplayable {
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub input_index: u64,
    pub state_version: StateVersion,
    /// `Some` at checkpoint intervals.
    pub checkpoint_matched: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct VerifyReport {
    pub inputs_replayed: u64,
    pub checkpoints_checked: u64,
    /// Empty means clean. Non-empty pages: `tabula_state_hash_mismatch_total`
    /// must always be 0 (doc 06 §9.2).
    pub divergences: Vec<Divergence>,
}

#[derive(Clone, Debug)]
pub struct Divergence {
    pub input_index: u64,
    pub expected: [u8; 32],
    pub actual: [u8; 32],
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("not a .tbr file (bad magic)")]
    BadMagic,
    #[error("format version {0} is newer than this reader supports")]
    FormatTooNew(u16),
    #[error("corrupt replay: {0}")]
    Corrupt(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
