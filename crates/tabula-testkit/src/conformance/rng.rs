//! Deterministic RNG behavior. (doc 00 §5.2, ADR-026 §4)
//!
//! `DetRng` exposes no draw-count or audit-hash accessor (see
//! `tabula_core::rng`) — there is no additional audit surface to test beyond
//! what every other check already exercises by running scenarios through it.
//! What is not otherwise covered is that an *alternate* seed is
//! independently deterministic too, checked here without ever asserting the
//! two seeds must produce different results (a coincidence is legal; only
//! "same seed implies same result" is a contract).
//!
//! A game with no randomness at all supplies no [`super::RandomnessScenario`]
//! and this check passes trivially: determinism for a script that never
//! draws from `ctx.rng` is already proven by every other check in this
//! suite, and there is nothing seed-specific left to demonstrate.

use super::support;
use super::{GameTestFixture, RulesOf};
use crate::determinism as det;

pub fn check<F: GameTestFixture>() {
    let Some(randomness) = F::randomness() else {
        return;
    };

    let game = support::game_id::<F>();
    let mut alternate = super::scenario::<F>(F::deterministic_script());
    alternate.seed = randomness.alternate_seed;

    let a = det::run::<RulesOf<F>>(&alternate).unwrap_or_else(|e| {
        panic!(
            "{}",
            support::failure(
                "deterministic RNG behavior",
                &game,
                &format!("create failed under the alternate seed: {e}")
            )
        )
    });
    let b = det::run::<RulesOf<F>>(&alternate)
        .expect("create succeeded once under the alternate seed, so it must succeed again");

    assert_eq!(
        a.final_state,
        b.final_state,
        "{}",
        support::failure(
            "deterministic RNG behavior",
            &game,
            "two independent runs under the SAME alternate seed produced different final \
             state. The RNG stream must be a pure function of (seed, input index)."
        )
    );
    assert_eq!(
        a.events_encoded,
        b.events_encoded,
        "{}",
        support::failure(
            "deterministic RNG behavior",
            &game,
            "two independent runs under the SAME alternate seed produced different event \
             streams."
        )
    );
}
