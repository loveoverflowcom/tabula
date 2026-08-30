//! Logical time. (doc 02 §2, doc 03 §7.2)
//!
//! ## The whole idea
//!
//! Rules never read a clock. The *shell* owns real time: it decides when a timer
//! fires by wall clock, then records that firing as an input at a logical time
//! `T`. Replay reads `T` back out of the log and gets the same answer, forever.
//!
//! Consequences worth internalising:
//!
//! - A game written for 60-second live turns works unchanged for 24-hour
//!   correspondence turns. Nothing in the rules knows the difference.
//!   (doc 02 §12.4, doc 03 §11.3)
//! - Chess decrements a clock by `ctx.now - state.last_move_at` inside `apply`.
//!   It never asks what time it is. On restart the timer is re-derived from
//!   state, not restored from memory. (doc 02 §12.1)
//! - Pausing is subtraction on the shell's side (`paused_for`), invisible to
//!   rules except that logical time advances more slowly.

use serde::{Deserialize, Serialize};

/// Milliseconds since match start, as recorded in the log.
///
/// **The only time rules can see** (I-3). Monotonic non-decreasing: the runtime
/// clamps it so that a clock adjustment on the host can never move a match
/// backwards. (doc 03 §7.2)
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct LogicalTime(pub u64);

/// A duration in milliseconds.
///
/// Deliberately not `core::time::Duration`: that type carries nanosecond
/// precision we never want in canonical state, and it is easy to construct from
/// an `Instant` difference — which is exactly the mistake I-3 forbids.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Millis(pub u64);

impl LogicalTime {
    pub const ZERO: Self = Self(0);

    /// Saturating difference. Saturating rather than wrapping because a
    /// non-monotonic pair is a bug elsewhere, and a huge wrapped duration would
    /// hide it inside a plausible-looking clock value.
    #[must_use]
    pub fn since(self, earlier: Self) -> Millis {
        Millis(self.0.saturating_sub(earlier.0))
    }

    #[must_use]
    pub fn plus(self, d: Millis) -> Self {
        Self(self.0.saturating_add(d.0))
    }
}

impl Millis {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn from_secs(s: u64) -> Self {
        Self(s * 1_000)
    }
}
