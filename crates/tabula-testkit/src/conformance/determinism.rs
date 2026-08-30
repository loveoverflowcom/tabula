//! Deterministic initialization and deterministic command execution.
//! (I-2, doc 05 §7.2)
//!
//! Both checks reuse [`crate::determinism`] — the reference transition
//! loop and its byte-level comparisons already exist and are exercised by
//! `tests/harness_catches_violations.rs`. This module supplies the fixture
//! data that turns that harness into a per-game test.

use tabula_game_api::GameModule;

use super::support;
use super::{GameTestFixture, RulesOf};
use crate::determinism::{self as det, Scenario};

/// Same config + roster + seed, run through `create` independently twice,
/// must produce the same state, hash, and `create` effects.
pub fn check_init<F: GameTestFixture>() {
    let cfg = F::config();
    let roster = F::roster();

    if let Err(err) = F::Module::validate_config(&cfg, &roster) {
        panic!(
            "{}",
            support::failure(
                "deterministic initialization",
                &support::game_id::<F>(),
                &format!(
                    "the fixture's own config/roster was rejected by \
                     GameModule::validate_config: {err}\n\n\
                     Fix the fixture, not this check — every other check in the suite \
                     assumes a config the game itself considers valid."
                )
            )
        );
    }

    let scenario: Scenario<RulesOf<F>> = Scenario {
        config: cfg,
        roster,
        seed: F::seed(),
        inputs: Vec::new(),
    };

    let run = |label: &str| {
        det::run::<RulesOf<F>>(&scenario).unwrap_or_else(|e| {
            panic!(
                "{}",
                support::failure(
                    "deterministic initialization",
                    &support::game_id::<F>(),
                    &format!("run {label}: GameRules::create rejected the fixture's config/roster/seed: {e}")
                )
            )
        })
    };

    let a = run("A");
    let b = run("B");

    assert_eq!(
        a.final_state,
        b.final_state,
        "{}",
        support::failure(
            "deterministic initialization",
            &support::game_id::<F>(),
            "two independent calls to `create` with the same config, roster, and seed \
             produced different canonical state bytes."
        )
    );
    assert_eq!(
        a.final_hash,
        b.final_hash,
        "{}",
        support::failure(
            "deterministic initialization",
            &support::game_id::<F>(),
            &format!(
                "two independent calls to `create` hashed differently.\n\nrun A hash:\n{}\n\nrun B hash:\n{}",
                support::hash_hex(a.final_hash),
                support::hash_hex(b.final_hash)
            )
        )
    );
    assert_eq!(
        a.effects_encoded,
        b.effects_encoded,
        "{}",
        support::failure(
            "deterministic initialization",
            &support::game_id::<F>(),
            "two independent calls to `create` requested different effects (e.g. \
             `SetTimer`) for the same config, roster, and seed."
        )
    );
}

/// The fixture's deterministic script, run independently twice, must
/// produce the same state, events, effects, hash, and rejection pattern
/// (I-2). Delegates entirely to [`det::assert_deterministic`], which
/// compares canonical state bytes rather than the hash alone.
pub fn check_apply<F: GameTestFixture>() {
    let script = F::deterministic_script();
    assert!(
        !script.is_empty(),
        "{}",
        support::failure(
            "deterministic command execution",
            &support::game_id::<F>(),
            "GameTestFixture::deterministic_script() returned an empty script; there is \
             no command execution to check determinism over."
        )
    );

    let scenario = super::scenario::<F>(script);
    det::assert_deterministic::<RulesOf<F>>(&scenario);
}
