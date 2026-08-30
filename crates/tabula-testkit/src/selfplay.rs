//! Bot self-play — the primary fuzzer. (doc 02 §11.3)
//!
//! # Why this is the best test in the suite
//!
//! It is cheap and it finds the bugs that matter. Bots play each other thousands
//! of times with random seeds; every match is checked for determinism,
//! projection safety, and termination. It finds rule bugs, infinite phases, and
//! projection leaks better than hand-written tests, and it costs a game author
//! nothing beyond implementing `legal_commands` — after which a `Trivial` bot is
//! free.
//!
//! # Cadence
//!
//! - Per PR: 1000 matches per game (`bot_self_play_terminates`).
//! - Nightly: 100k matches per game.
//! - Any failing seed is **auto-committed** to
//!   `tests/replays/<game>/regressions/` so the bug can never silently return.
//!
//! # It is also the acceptance demo
//!
//! Doc 09 §7 step 9: `xtask selfplay tictactoe --matches 10000` must pass before
//! Phase 1 begins. That is the gate on the whole Phase 0 contract.

use tabula_game_api::GameModule;

#[derive(Clone, Debug)]
pub struct SelfPlayConfig {
    pub matches: u32,
    /// Base seed. Match *n* uses a seed derived from it, so a failing run is
    /// reproducible from `(base_seed, match_index)` alone.
    pub base_seed: [u8; 32],
    /// Fraction of inputs that are deliberately hostile — garbage commands,
    /// out-of-range seats, wrong-phase actions, timers that do not exist.
    /// Exercises contract R3 alongside the happy path.
    pub hostile_fraction: f32,
    /// Fail a match that has not terminated after this many inputs. Catches
    /// infinite phase loops, which are otherwise a hang rather than a failure.
    pub max_inputs: u32,
    pub check_projections: bool,
}

impl Default for SelfPlayConfig {
    fn default() -> Self {
        Self {
            matches: 1_000,
            base_seed: [0u8; 32],
            hostile_fraction: 0.05,
            max_inputs: 10_000,
            check_projections: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SelfPlayReport {
    pub matches_run: u32,
    pub terminated: u32,
    /// Each entry is a `(match_index, reason)` that must be turned into a
    /// committed regression replay.
    pub failures: Vec<(u32, String)>,
    pub inputs_total: u64,
    pub p99_apply_micros: u32,
}

/// Run bots against each other and check every invariant on every match.
///
/// TODO(phase 0): implement for tictactoe first (doc 09 §7 step 9). The order
/// that makes debugging easiest:
///   1. run one match, assert it terminates
///   2. add determinism re-run
///   3. add hostile input injection
///   4. add projection scanning
///   5. add the regression auto-commit path
///
/// Do not parallelise across matches until step 5 works — a nondeterministic
/// test harness debugging a determinism bug is not a good afternoon.
pub fn run<M: GameModule>(_cfg: &SelfPlayConfig) -> SelfPlayReport {
    todo!("doc 02 §11.3")
}
