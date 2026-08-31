#![allow(clippy::doc_markdown)]

//! Pure Chess time-control arithmetic.
//!
//! The platform owns wall-clock scheduling. This module owns the meaning of a
//! recorded [`LogicalTime`] and returns timer requests as effects.
//!
//! @ai.role domain-transition
//! @ai.domain chess.clock
//! @ai.pure true
//! @ai.invariant clock-never-underflows
//! @ai.invariant exact-zero-flags
//! @ai.invariant fischer-overflow-is-explicitly-capped
//! @ai.law deterministic-clock-charge
//! @ai.evidence tests::clocks::fischer_exact_zero_flags_before_move
//! @ai.evidence tests::clocks::bronstein_exact_timeout_boundary_flags
//! @ai.evidence tests::clocks::fischer_near_limit_repeated_zero_elapsed_moves_never_wrap

use tabula_core::{LogicalTime, Millis, TimerId};
use tabula_game_api::Effect;

use super::state::{ClockConfig, ClockControl, ClockState, Color};

/// The one timer owned by the Chess rules.
pub(crate) const TIMER_CLOCK: TimerId = TimerId(1);

// `Millis` is a finite wire value. Fischer time is therefore deliberately
// capped at the largest representable value when a legal move's increment
// would exceed it; this keeps every accepted configuration total and makes
// the boundary explicit rather than relying on an accidental integer wrap.
const MAX_CLOCK_MILLIS: u64 = u64::MAX;

/// The result of charging the side that submitted a legal move.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum MoveCharge {
    Disabled,
    Ready(Millis),
    Flagged,
}

/// The result of checking a timer delivery against the current turn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TimerCheck {
    Active(Millis),
    Flagged,
}

/// Returns whether a raw lobby clock config can enter a match.
///
/// A Fischer config must represent the initial remaining time plus its first
/// increment. Later increments use the explicit finite-domain cap in
/// [`charge_completed_move`], so an accepted config cannot produce a panic,
/// wraparound, or rejected move solely because a clock became large.
pub(crate) fn config_is_valid(config: &ClockConfig) -> bool {
    if config.initial.0 == 0 {
        return false;
    }

    match config.control {
        ClockControl::Fischer { increment } => config.initial.0.checked_add(increment.0).is_some(),
        ClockControl::Bronstein { delay } => config.initial.0.checked_add(delay.0).is_some(),
    }
}

/// Creates the canonical starting clock at the time the match is created.
pub(crate) fn initial_state(config: ClockConfig, now: LogicalTime) -> ClockState {
    ClockState {
        remaining: [config.initial; 2],
        last_move_at: now,
        control: config.control,
    }
}

/// Charges elapsed time from a legal move without mutating the state.
///
/// Fischer increments use the explicit finite-domain policy documented by
/// [`MAX_CLOCK_MILLIS`]: a surviving move can cap the remaining time at the
/// maximum representable millisecond value, but it can never wrap below it.
pub(crate) fn charge_completed_move(
    clock: Option<&ClockState>,
    mover: Color,
    now: LogicalTime,
) -> MoveCharge {
    let Some(clock) = clock else {
        return MoveCharge::Disabled;
    };

    let remaining = clock.remaining[color_index(mover)].0;
    let elapsed = now.since(clock.last_move_at).0;
    let charged = match clock.control {
        ClockControl::Fischer { .. } => elapsed,
        ClockControl::Bronstein { delay } => elapsed.saturating_sub(delay.0),
    };

    if charged >= remaining {
        return MoveCharge::Flagged;
    }

    let after_charge = remaining - charged;
    let after_move = match clock.control {
        ClockControl::Fischer { increment } => after_charge
            .checked_add(increment.0)
            .unwrap_or(MAX_CLOCK_MILLIS),
        ClockControl::Bronstein { .. } => after_charge,
    };
    MoveCharge::Ready(Millis(after_move))
}

/// Checks whether a recorded timer delivery is early or at/after the deadline.
pub(crate) fn check_timer(clock: &ClockState, turn: Color, now: LogicalTime) -> TimerCheck {
    let elapsed = now.since(clock.last_move_at).0;
    let timeout_budget = match clock.control {
        ClockControl::Fischer { .. } => clock.remaining[color_index(turn)].0,
        ClockControl::Bronstein { delay } => {
            clock.remaining[color_index(turn)].0.saturating_add(delay.0)
        }
    };

    if elapsed >= timeout_budget {
        TimerCheck::Flagged
    } else {
        TimerCheck::Active(Millis(timeout_budget - elapsed))
    }
}

/// Returns the remaining delay for re-arming the current turn's timer.
pub(crate) fn arm_effect(clock: &ClockState, turn: Color, now: LogicalTime) -> Effect {
    let delay = match check_timer(clock, turn, now) {
        TimerCheck::Active(delay) => delay,
        // This helper is called only after creation or a surviving move. A
        // zero delay keeps the effect total even if a trusted state is stale.
        TimerCheck::Flagged => Millis::ZERO,
    };
    Effect::SetTimer {
        id: TIMER_CLOCK,
        delay,
    }
}

/// Marks the flagged side's clock at zero and closes its elapsed interval.
pub(crate) fn flag(clock: &mut ClockState, flagged: Color, now: LogicalTime) {
    clock.remaining[color_index(flagged)] = Millis::ZERO;
    clock.last_move_at = now;
}

fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(remaining: u64, control: ClockControl, last_move_at: u64) -> ClockState {
        ClockState {
            remaining: [Millis(remaining); 2],
            last_move_at: LogicalTime(last_move_at),
            control,
        }
    }

    #[test]
    fn fischer_charge_matches_bounded_reference_model() {
        for remaining in 1..=32 {
            for elapsed in 0..=40 {
                for increment in 0..=8 {
                    let clock = state(
                        remaining,
                        ClockControl::Fischer {
                            increment: Millis(increment),
                        },
                        0,
                    );
                    let actual =
                        charge_completed_move(Some(&clock), Color::White, LogicalTime(elapsed));
                    let expected = if elapsed >= remaining {
                        MoveCharge::Flagged
                    } else {
                        MoveCharge::Ready(Millis(remaining - elapsed + increment))
                    };
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn bronstein_charge_matches_bounded_reference_model() {
        for remaining in 1..=32 {
            for elapsed in 0..=40 {
                for delay in 0..=8 {
                    let clock = state(
                        remaining,
                        ClockControl::Bronstein {
                            delay: Millis(delay),
                        },
                        0,
                    );
                    let actual =
                        charge_completed_move(Some(&clock), Color::Black, LogicalTime(elapsed));
                    let charged = elapsed.saturating_sub(delay);
                    let expected = if charged >= remaining {
                        MoveCharge::Flagged
                    } else {
                        MoveCharge::Ready(Millis(remaining - charged))
                    };
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn timer_budget_matches_control_semantics_at_boundaries() {
        for remaining in 1..=16 {
            for allowance in 0..=8 {
                let fischer = state(
                    remaining,
                    ClockControl::Fischer {
                        increment: Millis(0),
                    },
                    10,
                );
                assert_eq!(
                    check_timer(&fischer, Color::White, LogicalTime(10 + remaining - 1)),
                    TimerCheck::Active(Millis(1))
                );
                assert_eq!(
                    check_timer(&fischer, Color::White, LogicalTime(10 + remaining)),
                    TimerCheck::Flagged
                );

                let bronstein = state(
                    remaining,
                    ClockControl::Bronstein {
                        delay: Millis(allowance),
                    },
                    10,
                );
                let budget = remaining + allowance;
                assert_eq!(
                    check_timer(&bronstein, Color::Black, LogicalTime(10 + budget - 1)),
                    TimerCheck::Active(Millis(1))
                );
                assert_eq!(
                    check_timer(&bronstein, Color::Black, LogicalTime(10 + budget)),
                    TimerCheck::Flagged
                );
            }
        }
    }

    #[test]
    fn fischer_increment_caps_at_the_explicit_finite_domain_limit() {
        let increment = 1;
        let near_limit = state(
            u64::MAX - increment,
            ClockControl::Fischer {
                increment: Millis(increment),
            },
            0,
        );
        assert_eq!(
            charge_completed_move(Some(&near_limit), Color::White, LogicalTime::ZERO),
            MoveCharge::Ready(Millis(u64::MAX))
        );

        let at_limit = state(
            u64::MAX,
            ClockControl::Fischer {
                increment: Millis(increment),
            },
            0,
        );
        assert_eq!(
            charge_completed_move(Some(&at_limit), Color::White, LogicalTime::ZERO),
            MoveCharge::Ready(Millis(u64::MAX))
        );
    }

    #[test]
    fn fischer_near_limit_repeated_zero_elapsed_moves_are_deterministic() {
        let mut clock = state(
            u64::MAX - 1,
            ClockControl::Fischer {
                increment: Millis(1),
            },
            0,
        );
        for expected in [u64::MAX, u64::MAX] {
            let actual = charge_completed_move(Some(&clock), Color::White, LogicalTime::ZERO);
            assert_eq!(actual, MoveCharge::Ready(Millis(expected)));
            clock.remaining[0] = match actual {
                MoveCharge::Ready(remaining) => remaining,
                MoveCharge::Disabled | MoveCharge::Flagged => unreachable!(),
            };
        }
    }
}
