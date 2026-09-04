//! State-hash sensitivity. (doc 05 §7.2)
//!
//! Not a collision-resistance proof — a regression guard against a broken
//! or placeholder hash implementation such as `StateHash::new([0; 32])`,
//! which would otherwise make the entire determinism-drift detection
//! mechanism (doc 05 §7.3, the only thing that catches divergence in
//! production) silently useless.

use super::support;
use super::{GameTestFixture, RulesOf};
use crate::determinism as det;

pub fn check<F: GameTestFixture>() {
    let game = support::game_id::<F>();

    let initial = det::run::<RulesOf<F>>(&super::scenario::<F>(Vec::new())).unwrap_or_else(|e| {
        panic!(
            "{}",
            support::failure(
                "state hash sensitivity",
                &game,
                &format!("create failed: {e}")
            )
        )
    });
    let after = det::run::<RulesOf<F>>(&super::scenario::<F>(F::deterministic_script()))
        .expect("deterministic_script is already verified runnable by earlier checks");

    assert_ne!(
        initial.final_hash,
        after.final_hash,
        "{}",
        support::failure(
            "state hash sensitivity",
            &game,
            &format!(
                "the initial state and the state after `deterministic_script` hashed \
                 identically.\n\nhash:\n{}\n\n\
                 Either the script does not change the state, or state_hash is not \
                 sensitive to the state it is given.",
                support::hash_hex(initial.final_hash)
            )
        )
    );

    for (label, hash) in [
        ("initial", initial.final_hash),
        ("post-script", after.final_hash),
    ] {
        assert_ne!(
            hash.0,
            [0u8; 32],
            "{}",
            support::failure(
                "state hash sensitivity",
                &game,
                &format!(
                    "the {label} state hashed to all-zero bytes — the signature of a \
                     constant or placeholder state_hash implementation."
                )
            )
        );
    }
}
