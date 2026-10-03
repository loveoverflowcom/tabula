#![allow(clippy::doc_markdown)] // `@ai.*` schema values must remain bare machine-readable paths.

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
    ///
    /// @ai.role domain-transition
    /// @ai.domain logical-time
    /// @ai.pure true
    /// @ai.invariant time-arithmetic-total
    /// @ai.law non-monotonic-time-clamps-to-zero
    /// @ai.evidence crate::time::verification::logical_time_since_never_wraps
    /// @ai.evidence crate::time::tests::since_is_exact_or_zero
    #[must_use]
    pub fn since(self, earlier: Self) -> Millis {
        Millis(self.0.saturating_sub(earlier.0))
    }

    /// Adds a duration without allowing the logical clock to wrap.
    ///
    /// @ai.role domain-transition
    /// @ai.domain logical-time
    /// @ai.pure true
    /// @ai.invariant time-arithmetic-total
    /// @ai.law time-addition-is-exact-or-saturating
    /// @ai.evidence crate::time::verification::logical_time_plus_is_exact_or_saturates
    /// @ai.evidence crate::time::tests::plus_saturates_at_maximum
    #[must_use]
    pub fn plus(self, d: Millis) -> Self {
        Self(self.0.saturating_add(d.0))
    }
}

impl Millis {
    pub const ZERO: Self = Self(0);

    /// Converts whole seconds to milliseconds, saturating when the result does
    /// not fit in `u64`. Saturation keeps this deterministic conversion total
    /// and consistent with the rest of the logical-time arithmetic.
    ///
    /// @ai.role domain-transition
    /// @ai.domain logical-time
    /// @ai.pure true
    /// @ai.invariant time-arithmetic-total
    /// @ai.law seconds-conversion-is-exact-or-saturating
    /// @ai.evidence crate::time::verification::millis_from_secs_is_exact_or_saturates
    /// @ai.evidence crate::time::tests::from_secs_handles_conversion_boundaries
    #[must_use]
    pub const fn from_secs(s: u64) -> Self {
        Self(s.saturating_mul(1_000))
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalTime, Millis};

    #[test]
    fn from_secs_handles_conversion_boundaries() {
        let largest_exact_seconds = u64::MAX / 1_000;

        assert_eq!(
            Millis::from_secs(largest_exact_seconds),
            Millis(largest_exact_seconds * 1_000)
        );
        assert_eq!(
            Millis::from_secs(largest_exact_seconds + 1),
            Millis(u64::MAX)
        );
        assert_eq!(Millis::from_secs(u64::MAX), Millis(u64::MAX));
    }

    #[test]
    fn plus_saturates_at_maximum() {
        assert_eq!(
            LogicalTime(u64::MAX - 1).plus(Millis(2)),
            LogicalTime(u64::MAX)
        );
        assert_eq!(LogicalTime(41).plus(Millis(1)), LogicalTime(42));
    }

    #[test]
    fn since_is_exact_or_zero() {
        assert_eq!(LogicalTime(42).since(LogicalTime(41)), Millis(1));
        assert_eq!(LogicalTime(41).since(LogicalTime(42)), Millis::ZERO);
    }
}

#[cfg(kani)]
mod verification {
    use super::{LogicalTime, Millis};

    /// Every `u64` seconds value converts exactly when milliseconds fit and
    /// otherwise reaches the documented saturation value. The `checked_mul`
    /// branch is an independent oracle and does not itself wrap.
    #[kani::proof]
    fn millis_from_secs_is_exact_or_saturates() {
        let seconds: u64 = kani::any();
        let converted = Millis::from_secs(seconds);

        match seconds.checked_mul(1_000) {
            Some(expected) => assert!(converted == Millis(expected)),
            None => assert!(converted == Millis(u64::MAX)),
        }
    }

    /// Logical-time addition is exact while it fits and saturates at the
    /// maximum otherwise. In either case the result cannot move backwards.
    #[kani::proof]
    fn logical_time_plus_is_exact_or_saturates() {
        let original = LogicalTime(kani::any());
        let duration = Millis(kani::any());
        let result = original.plus(duration);

        match original.0.checked_add(duration.0) {
            Some(expected) => assert!(result == LogicalTime(expected)),
            None => assert!(result == LogicalTime(u64::MAX)),
        }
        assert!(result >= original);
    }

    /// Subtraction is a forward difference when the pair is ordered and zero
    /// for a non-monotonic pair; it can never become a wrapped huge duration.
    #[kani::proof]
    fn logical_time_since_never_wraps() {
        let now = LogicalTime(kani::any());
        let earlier = LogicalTime(kani::any());
        let result = now.since(earlier);

        if now >= earlier {
            assert!(result == Millis(now.0 - earlier.0));
        } else {
            assert!(result == Millis::ZERO);
        }
        assert!(result.0 <= now.0);
    }
}
