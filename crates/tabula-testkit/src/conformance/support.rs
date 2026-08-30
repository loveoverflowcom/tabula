//! Shared internals for the fixture-driven checks. Not part of the public API.

use tabula_core::{DetRng, InputIndex, LogicalTime, MatchSeed, StateHash};
use tabula_game_api::{Budget, Ctx, GameModule, GameRules, Input, Outcome, RuleError};

use super::GameTestFixture;

/// Apply one input at `index`, deriving `ctx` exactly as the reference runner
/// in [`crate::determinism::run`] does — same seed-to-RNG and index-to-time
/// mapping, so a probe applied here composes correctly with a scenario run
/// through that function.
pub(crate) fn apply_at<R: GameRules>(
    state: &mut R::State,
    input: Input<R::Command>,
    seed: &MatchSeed,
    index: InputIndex,
) -> Result<Outcome<R>, RuleError> {
    let mut rng = DetRng::for_input(seed, index);
    let mut ctx = Ctx {
        now: LogicalTime(index.0 * 1_000),
        index,
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    R::apply(state, input, &mut ctx)
}

pub(crate) fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::with_capacity(64), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

pub(crate) fn hash_hex(hash: StateHash) -> String {
    hex32(&hash.0)
}

/// The `GameId` a fixture's module declares, for diagnostics. Not cached:
/// conformance checks run once each, so recomputing costs nothing and a
/// cache would be one more place for a "which build did this run against"
/// bug to hide.
pub(crate) fn game_id<F: GameTestFixture>() -> String {
    F::Module::metadata().id.0.clone()
}

/// Compose a conformance failure message in the shape the mission specifies:
/// which invariant, which game, then the specific detail.
///
/// `GameRules::Command` carries no `Debug` bound — the contract does not
/// require one — so diagnostics are built only from what the trait does
/// guarantee: ids, step indices, and canonical hashes.
pub(crate) fn failure(invariant: &str, game_id: &str, detail: &str) -> String {
    format!("Tabula conformance failure: {invariant}\n\ngame: {game_id}\n\n{detail}")
}
