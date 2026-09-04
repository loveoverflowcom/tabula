//! Deterministic replay of a full command script, and the ordered-events
//! guarantee (R7) that determinism alone does not exercise on its own.
//! (doc 05 §7, §8)
//!
//! This is deliberately not the same check as
//! [`crate::conformance::determinism::check_apply`]: that check proves two
//! runs agree. This one runs the same script **three** times independently
//! and compares every pair, naming the first diverging step and both
//! hashes — closer to what the nightly replay job actually does (doc 05
//! §7.3) — and compares canonical state bytes, not the hash alone, so a
//! broken constant-hash implementation cannot make this pass by accident.

use super::support;
use super::{GameTestFixture, RulesOf};
use crate::determinism::{self as det, RunTrace};

const RUNS: usize = 3;

pub fn check<F: GameTestFixture>() {
    let scenario = super::scenario::<F>(F::deterministic_script());
    let game = support::game_id::<F>();

    let runs: Vec<RunTrace> = (0..RUNS)
        .map(|i| {
            det::run::<RulesOf<F>>(&scenario).unwrap_or_else(|e| {
                panic!(
                    "{}",
                    support::failure(
                        "deterministic replay",
                        &game,
                        &format!("run {i}: create failed: {e}")
                    )
                )
            })
        })
        .collect();

    let first = &runs[0];
    for (run_no, run) in runs.iter().enumerate().skip(1) {
        if let Some((step, expected, actual)) = first_divergence(first, run) {
            panic!(
                "{}",
                support::failure(
                    "deterministic replay",
                    &game,
                    &format!(
                        "run {run_no} diverged from run 0 at step {step}\n\n\
                         expected hash:\n{}\n\nactual hash:\n{}",
                        support::hex32(&expected),
                        support::hex32(&actual)
                    )
                )
            );
        }

        assert_eq!(
            first.final_state,
            run.final_state,
            "{}",
            support::failure(
                "deterministic replay",
                &game,
                &format!(
                    "run {run_no}'s final canonical state differs from run 0's, though every \
                     checkpoint hash agreed. A hash agreeing while the underlying state \
                     differs means state_hash is not sensitive to the whole state — see the \
                     state-hash-sensitivity check."
                )
            )
        );
        assert_eq!(
            first.events_encoded,
            run.events_encoded,
            "{}",
            support::failure(
                "deterministic replay",
                &game,
                &format!("run {run_no}'s event stream differs from run 0's.")
            )
        );
        assert_eq!(
            first.rejections,
            run.rejections,
            "{}",
            support::failure(
                "deterministic replay",
                &game,
                &format!("run {run_no} accepted or rejected an input differently than run 0 did.")
            )
        );
    }
}

fn first_divergence(a: &RunTrace, b: &RunTrace) -> Option<(u64, [u8; 32], [u8; 32])> {
    a.checkpoints
        .iter()
        .zip(&b.checkpoints)
        .find(|((_, x), (_, y))| x != y)
        .map(|((i, x), (_, y))| (*i, x.0, y.0))
}

/// A script's event stream must be non-empty (otherwise there is nothing to
/// prove ordered) and stable across independent runs. Event order is part
/// of the observable contract (R7): it is stored verbatim and replayed
/// verbatim, so this check never sorts before comparing.
pub fn check_ordered_events<F: GameTestFixture>() {
    let scenario = super::scenario::<F>(F::deterministic_script());
    let game = support::game_id::<F>();

    let trace = det::run::<RulesOf<F>>(&scenario).unwrap_or_else(|e| {
        panic!(
            "{}",
            support::failure("ordered events", &game, &format!("create failed: {e}"))
        )
    });

    assert!(
        !trace.events_encoded.is_empty(),
        "{}",
        support::failure(
            "ordered events",
            &game,
            "GameTestFixture::deterministic_script() produced no events at all; there is \
             no event order to verify. Pick a script where at least one command emits an \
             event."
        )
    );

    let again = det::run::<RulesOf<F>>(&scenario)
        .expect("create succeeded once with this scenario, so it must succeed again");

    assert_eq!(
        trace.events_encoded,
        again.events_encoded,
        "{}",
        support::failure(
            "ordered events",
            &game,
            "the event stream was not identical across two independent runs of the same \
             script. Event order is part of the contract (R7) — never build the event list \
             by iterating an unordered collection."
        )
    );
}
