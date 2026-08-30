//! State serialization round-trip. (I-8, doc 05 §7.1)
//!
//! `state → canonical bytes → state` must preserve semantics and hash — the
//! path production takes into `match_snapshots.payload` and out of a `.tbr`
//! file. Byte-for-byte equality is asserted, not merely semantic equality,
//! because this codebase's canonical encoding (`ENCODING_VERSION` prefix +
//! derived `Serialize` over Postcard, ADR-021/ADR-026) is a fixed,
//! non-self-describing format with exactly one valid encoding per value —
//! the contract this crate ships explicitly promises persistence bytes are
//! stable, unlike a format such as JSON with reordering freedom.
//!
//! Delegates to [`crate::determinism::assert_state_roundtrip`], which
//! performs exactly that comparison against the fixture's own script.

use super::{GameTestFixture, RulesOf};
use crate::determinism as det;

pub fn check<F: GameTestFixture>() {
    let scenario = super::scenario::<F>(F::deterministic_script());
    det::assert_state_roundtrip::<RulesOf<F>>(&scenario);
}
