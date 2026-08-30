//! `Ctx` — everything nondeterministic that rules are allowed to see.
//! (doc 02 §3.1)
//!
//! Every field here is **recorded in the log**, which is what makes replay total.
//! If a rule needs something that is not on this struct, the answer is almost
//! always "that belongs to the platform, and reaches you as an `Input`".

use tabula_core::{DetRng, InputIndex, LogicalTime};

use crate::capabilities::Budget;

/// The deterministic context for one input application.
///
/// Not `Clone`, not `Send`, borrowed for exactly the duration of one `apply`.
/// That is deliberate: a `Ctx` that outlived its input could draw randomness from
/// the wrong stream position and silently break replay.
#[derive(Debug)]
pub struct Ctx<'a> {
    /// Logical time of **this** input, read from the log. (I-3)
    ///
    /// The only clock rules have. Chess computes elapsed time as
    /// `ctx.now - state.last_move_at`; it never asks what time it is.
    pub now: LogicalTime,

    /// Index of this input in the log; also the RNG domain root.
    pub index: InputIndex,

    /// Deterministic randomness, already domain-separated for this input. (I-4)
    ///
    /// Split further per purpose: `ctx.rng.stream(DOMAIN_SHUFFLE)`. Never derive
    /// randomness from state hashes, time, or player input.
    pub rng: &'a mut DetRng,

    /// A **soft** budget. Observability, not enforcement.
    ///
    /// Exceeding it warns and increments a metric so a slow game cannot quietly
    /// degrade the shared executor. It does not abort the apply, and it is
    /// deliberately not a sandbox limit — doc 02 §9.3 forbids Phase C sandbox
    /// concerns from shaping the Phase A trait signatures.
    pub budget: Budget,
}
