//! Invalid-command safety (contract R2/R8) and `legal_commands` sanity.
//! (doc 02 §3.2, §11.1)

use std::collections::BTreeSet;

use tabula_core::{canonical_decode, canonical_encode, InputIndex, MatchSeed, SeatId};
use tabula_game_api::{GameRules, Input, LegalCommands};

use super::support;
use super::{CommandOf, GameTestFixture, RulesOf};
use crate::determinism::{self as det, Scenario};

/// A command the game is expected to reject must leave state byte-identical
/// (R2) and must not disturb a legal command applied right after it (R8).
/// Delegates to [`det::assert_transactional_on_error`] and
/// [`det::assert_rejection_does_not_disturb_rng`], which already implement
/// both comparisons against canonical bytes.
pub fn check_invalid<F: GameTestFixture>() {
    let Some(spec) = F::invalid_command() else {
        eprintln!(
            "tabula conformance: {} supplies no InvalidCommandScenario; skipping \
             invalid-command safety. This is a legitimate choice only for a game with no \
             command rejection to exercise at all.",
            support::game_id::<F>()
        );
        return;
    };

    let mut inputs = spec.setup.clone();
    inputs.push(spec.invalid.clone());
    let with_invalid: Scenario<RulesOf<F>> = super::scenario::<F>(inputs);
    det::assert_transactional_on_error::<RulesOf<F>>(&with_invalid);

    let setup_only: Scenario<RulesOf<F>> = super::scenario::<F>(spec.setup);
    det::assert_rejection_does_not_disturb_rng::<RulesOf<F>>(
        &setup_only,
        &spec.invalid,
        &spec.probe,
    );
}

/// Every command `legal_commands` enumerates must actually apply; the
/// enumeration must contain no duplicates and must be returned in the same
/// order on a repeated call from the same state. Checked at two points — the
/// initial state and the state after the fixture's deterministic script —
/// deliberately not the whole game tree (doc 02 §3's default is `Unknown`
/// precisely because full enumeration is not always cheap or meaningful).
pub fn check_legal<F: GameTestFixture>() {
    let game = support::game_id::<F>();

    let initial_trace =
        det::run::<RulesOf<F>>(&super::scenario::<F>(Vec::new())).unwrap_or_else(|e| {
            panic!(
                "{}",
                support::failure(
                    "legal_commands sanity",
                    &game,
                    &format!("create failed: {e}")
                )
            )
        });
    let initial_state: <RulesOf<F> as GameRules>::State =
        canonical_decode(&initial_trace.final_state)
            .expect("canonical round trip of a state this crate just encoded");

    let script = F::deterministic_script();
    let script_len = script.len() as u64;
    let final_trace = det::run::<RulesOf<F>>(&super::scenario::<F>(script))
        .expect("deterministic_script is already verified runnable by earlier checks");
    let final_state: <RulesOf<F> as GameRules>::State = canonical_decode(&final_trace.final_state)
        .expect("canonical round trip of a state this crate just encoded");

    let seed = F::seed();
    for seat in F::roster().iter().map(|entry| entry.seat) {
        check_one::<F>("initial state", &initial_state, seat, &seed, 0, &game);
        check_one::<F>(
            "post-script state",
            &final_state,
            seat,
            &seed,
            script_len + 1,
            &game,
        );
    }
}

fn check_one<F: GameTestFixture>(
    label: &str,
    state: &<RulesOf<F> as GameRules>::State,
    seat: SeatId,
    seed: &MatchSeed,
    probe_index_base: u64,
    game: &str,
) {
    let LegalCommands::Enumerated(commands) = RulesOf::<F>::legal_commands(state, seat) else {
        // Hints/Unknown/None: nothing structural to check. `legal_commands`
        // is never required to enumerate the whole game tree (doc 02 §3).
        return;
    };

    let LegalCommands::Enumerated(commands_again) = RulesOf::<F>::legal_commands(state, seat)
    else {
        panic!(
            "{}",
            support::failure(
                "legal_commands sanity",
                game,
                &format!(
                    "legal_commands({label}, seat {seat:?}) returned `Enumerated` once and a \
                     different variant on a second call from the identical state."
                )
            )
        );
    };

    let encode_all = |cmds: &[CommandOf<F>]| -> Vec<Vec<u8>> {
        cmds.iter()
            .map(|c| canonical_encode(c).expect("a legal command must be canonically encodable"))
            .collect()
    };
    let encoded = encode_all(&commands);
    let encoded_again = encode_all(&commands_again);

    assert_eq!(
        encoded,
        encoded_again,
        "{}",
        support::failure(
            "legal_commands sanity",
            game,
            &format!(
                "legal_commands({label}, seat {seat:?}) returned its commands in a different \
                 order on a second call from the same state; ordering must be deterministic \
                 when it is observable."
            )
        )
    );

    let mut encountered = BTreeSet::new();
    for bytes in &encoded {
        assert!(
            encountered.insert(bytes.clone()),
            "{}",
            support::failure(
                "legal_commands sanity",
                game,
                &format!("legal_commands({label}, seat {seat:?}) returned the same command twice.")
            )
        );
    }

    for (i, command) in commands.into_iter().enumerate() {
        let mut probe_state = state.clone();
        let index = InputIndex(probe_index_base + 1_000_000 + i as u64);
        let input = Input::Player { seat, command };
        if let Err(err) = support::apply_at::<RulesOf<F>>(&mut probe_state, input, seed, index) {
            panic!(
                "{}",
                support::failure(
                    "legal_commands sanity",
                    game,
                    &format!(
                        "legal_commands({label}, seat {seat:?}) enumerated a command at index \
                         {i} that `apply` rejected with {:?}. Every command legal_commands \
                         enumerates must be applicable to that state (doc 02 §3).",
                        err.code
                    )
                )
            );
        }
    }
}
