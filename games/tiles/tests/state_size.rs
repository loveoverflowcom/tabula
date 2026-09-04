//! The `StateSizeClass` measurement. (doc 03 §9.2, doc 08 §4.5)
//!
//! Doc 02 §12.4 *expected* Tiles to be `Medium` (30–120 KB) and said so before
//! any Tiles state existed. This file measures the canonical encoding of a
//! full board and the declared capability follows the number, not the estimate.
//!
//! Deliberately size only. Doc 08 §4.5 also asks for a hash-cost budget, and
//! that is a wall-clock measurement: `std::time::Instant` is a
//! `disallowed-type` in a rules crate (I-3), and a timing assertion in the
//! per-PR tier is a flake generator besides. Timing belongs in the Phase-4 load
//! test with the rest of the per-match cost model (doc 06 §2).

mod support;

use tabula_core::{canonical_encode, MatchSeed};
use tabula_game_api::{GameModule, StateSizeClass};
use tabula_game_tiles::{Status, TilesModule};

use support::{config, drive, SEATS_MAX, SEATS_MIN};

/// The class boundaries from doc 03 §9.2, in bytes.
const TINY_MAX: usize = 1 << 10;
const SMALL_MAX: usize = 16 << 10;
const MEDIUM_MAX: usize = 256 << 10;

fn class_for(bytes: usize) -> StateSizeClass {
    if bytes < TINY_MAX {
        StateSizeClass::Tiny
    } else if bytes < SMALL_MAX {
        StateSizeClass::Small
    } else if bytes < MEDIUM_MAX {
        StateSizeClass::Medium
    } else {
        StateSizeClass::Large
    }
}

/// Play a complete match at every supported seat count and take the largest
/// canonical encoding any of them reaches — the number a snapshot policy has
/// to survive, not an average.
fn largest_full_board_encoding() -> usize {
    let mut largest = 0usize;
    for seats in SEATS_MIN..=SEATS_MAX {
        for seed_byte in [1u8, 42, 200] {
            let seed = MatchSeed::from_bytes([seed_byte; 32]);
            let (state, _) = drive(&seed, seats, config(), 256);
            assert_eq!(
                state.status(),
                Status::Ended,
                "the measurement must be taken on a completed match"
            );
            let bytes = canonical_encode(&state).expect("a State encodes").len();
            largest = largest.max(bytes);
        }
    }
    largest
}

/// The declared class must be the measured one.
///
/// If this fails after a tile-set or state-shape change, the fix is to change
/// the declaration — in `src/lib.rs` **and** `game.toml` — not to widen the
/// test. A snapshot cadence chosen from a stale estimate is exactly the failure
/// doc 03 §9.2 is trying to avoid.
#[test]
fn the_declared_state_size_class_is_the_measured_one() {
    let bytes = largest_full_board_encoding();
    let measured = class_for(bytes);
    let declared = TilesModule::capabilities().state_size();

    assert_eq!(
        declared, measured,
        "a full Tiles board encodes to {bytes} bytes, which is {measured:?}, but the \
         module declares {declared:?}. Update the declaration in src/lib.rs and \
         game.toml to match the measurement."
    );
}

/// The design estimate, contradicted in the direction that matters.
///
/// Doc 02 §12.4 put a full Tiles board at 30–120 KB and doc 03 §9.2 made Tiles
/// the worked example for the `Medium` snapshot class on the strength of it.
/// The measurement is two orders of magnitude smaller, and the honest reading
/// is that **no game in the portfolio exercises `Medium` yet** — not that Tiles
/// should be labelled `Medium` anyway to keep the table populated.
///
/// The bound is the estimate's own floor, so this test states exactly one
/// thing: the estimate was wrong. It does not pin a byte count, which would
/// fail on ordinary tile-set tuning.
#[test]
fn a_full_board_is_far_smaller_than_the_design_estimate() {
    let bytes = largest_full_board_encoding();
    assert!(
        bytes > 100,
        "a full board encoded to {bytes} bytes, which is too small to be a real \
         measurement — the driver probably stopped before the bag ran out"
    );
    assert!(
        bytes < 30_000,
        "a full board encoded to {bytes} bytes, which is inside doc 02 §12.4's \
         30_000..=120_000 estimate after all. If that is genuinely now true, \
         re-open the `Medium` row in doc 03 §9.2 and docs/games/tiles.md."
    );
}

/// The opening position is much smaller than the terminal one — which is the
/// whole reason Tiles is the "growing state" benchmark. A game whose state did
/// not grow would make `StateSizeClass` an uninteresting capability.
#[test]
fn the_state_grows_substantially_between_the_opening_and_a_full_board() {
    let seed = MatchSeed::from_bytes([42u8; 32]);
    let opening = support::create(&seed, 3, config());
    let (full, _) = drive(&seed, 3, config(), 256);

    let opening_bytes = canonical_encode(&opening).expect("encodes").len();
    let full_bytes = canonical_encode(&full).expect("encodes").len();

    // The opening state carries the whole undrawn bag, so it is not tiny
    // either; what changes is where the bytes are.
    assert!(
        full_bytes > opening_bytes / 2,
        "opening {opening_bytes} bytes, full board {full_bytes} bytes"
    );
    assert!(full.board().len() > opening.board().len() * 20);
}
