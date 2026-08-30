//! Determinism and transactionality harnesses. (I-2, I-7, I-8, contract R2)
//!
//! ## What "deterministic" means here, precisely
//!
//! ```text
//! same initial state (from the same MatchSeed and MatchConfig)
//! + same ordered input sequence
//! + same rules version
//! ================================================================
//! byte-identical final state, identical event sequence, identical hashes
//! ```
//!
//! And it must hold **across** process restarts, machines, operating systems,
//! architectures (x86-64 and aarch64), native and WASM, debug and release.
//! (doc 00 §5.1)
//!
//! ## Why the R2 checker is separate and expensive
//!
//! `apply` takes `&mut State`, so purity is a contract rather than a type
//! guarantee (doc 02 §3.3). In debug and test builds this module wraps every
//! `apply` with clone-and-compare, so a rejected input that mutated state fails
//! loudly here rather than corrupting a real match and surfacing weeks later as
//! a replay divergence.
//!
//! In release the match actor relies on the invariant instead — the clone cost
//! is real for a large state, and the testkit is where we pay it.

use tabula_core::StateHash;
use tabula_game_api::GameRules;

/// Result of running one input sequence to completion.
#[derive(Debug)]
pub struct RunTrace {
    pub final_hash: StateHash,
    /// Hash after every input, so a divergence report can name the exact index.
    pub checkpoints: Vec<(u64, StateHash)>,
    pub events_encoded: Vec<Vec<u8>>,
    pub rejections: Vec<(u64, tabula_core::RuleErrorCode)>,
}

/// Run a sequence twice and assert byte-identical results. (I-2)
///
/// TODO(phase 0): the comparison must be over the **canonical encoding**, never
/// over `Debug` or `serde_json` — doc 05 §7.1. A `Debug`-based comparison passes
/// while the stored bytes differ, which is exactly the failure mode this test
/// exists to catch.
///
/// # Panics
/// On any divergence, with the first differing input index and both hashes.
pub fn assert_deterministic<R: GameRules>(_inputs: &[()]) {
    todo!("doc 02 §11.1 determinism_same_inputs")
}

/// Snapshot mid-run, restore, continue — must land on the same final hash. (I-8)
///
/// TODO(phase 0): pick the snapshot point randomly per proptest case. A fixed
/// midpoint misses the bugs that only appear when the snapshot lands between two
/// halves of a phase transition.
pub fn assert_deterministic_across_snapshot<R: GameRules>(_inputs: &[()], _at: usize) {
    todo!("doc 02 §11.1 determinism_across_snapshot")
}

/// Every rejected input must leave the state hash unchanged. (contract R2)
///
/// TODO(phase 0): hash before, apply, on `Err` hash again, compare. Report the
/// input that mutated on rejection — that is a correctness bug of the highest
/// severity, because a rejection is supposed to be a no-op.
pub fn assert_transactional_on_error<R: GameRules>(_inputs: &[()]) {
    todo!("doc 02 §11.1 error_is_transactional")
}

/// `state_version` +1 per accepted input, unchanged on rejection. (I-7)
pub fn assert_version_monotonic<R: GameRules>(_inputs: &[()]) {
    todo!("doc 02 §11.1 version_monotonic")
}
