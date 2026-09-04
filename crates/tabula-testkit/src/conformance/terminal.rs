//! Terminal-state behavior.
//!
//! `GameRules` deliberately has no `is_terminal` — terminality is expressed
//! by `Effect::EndMatch`, and a second, independently computed answer to
//! "is this match over" is a divergence source (ADR-026 §1). So this check's
//! generic equivalent of "the terminal script reached a terminal state" is
//! "the terminal script emitted `Effect::EndMatch`", and its equivalent of
//! "post-game commands are rejected" is exactly what the fixture's own game
//! says happens to its own `post_terminal` probe. Nothing here invents a
//! post-game lifecycle; it only checks what the fixture asserts about its
//! own game.

use tabula_core::{canonical_decode, InputIndex};
use tabula_game_api::{Effect, GameRules};

use super::support;
use super::{GameTestFixture, RulesOf};
use crate::determinism as det;

pub fn check<F: GameTestFixture>() {
    let Some(spec) = F::terminal() else {
        eprintln!(
            "tabula conformance: {} supplies no TerminalScenario; skipping terminal-state \
             behavior. This is a legitimate choice only for a game with no terminal state.",
            support::game_id::<F>()
        );
        return;
    };

    let game = support::game_id::<F>();
    let script_len = spec.script.len();
    let scenario = super::scenario::<F>(spec.script);
    let trace = det::run::<RulesOf<F>>(&scenario).unwrap_or_else(|e| {
        panic!(
            "{}",
            support::failure(
                "terminal-state behavior",
                &game,
                &format!("create failed: {e}")
            )
        )
    });

    let ended = trace.effects_encoded.iter().any(|bytes| {
        matches!(
            canonical_decode::<Effect>(bytes),
            Ok(Effect::EndMatch { .. })
        )
    });

    assert!(
        ended,
        "{}",
        support::failure(
            "terminal-state behavior",
            &game,
            "TerminalScenario::script ran to completion without emitting Effect::EndMatch. \
             Terminality is expressed by EndMatch, not a separate is_terminal (ADR-026 §1) — \
             pick a script that actually ends the match."
        )
    );

    let mut state: <RulesOf<F> as GameRules>::State = canonical_decode(&trace.final_state)
        .expect("canonical round trip of a state this crate just encoded");
    let probe_index = InputIndex(script_len as u64 + 1);
    let result =
        support::apply_at::<RulesOf<F>>(&mut state, spec.post_terminal, &F::seed(), probe_index);

    assert!(
        result.is_err(),
        "{}",
        support::failure(
            "terminal-state behavior",
            &game,
            "TerminalScenario::post_terminal was ACCEPTED after Effect::EndMatch had already \
             been emitted. This check enforces exactly what the fixture claims about its own \
             game, not a platform-invented rule — if this game genuinely accepts further \
             commands after ending, supply a `post_terminal` this game actually rejects, or \
             omit the scenario."
        )
    );
}
