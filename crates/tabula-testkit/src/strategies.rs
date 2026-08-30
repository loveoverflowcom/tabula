//! `proptest` strategies. (doc 02 §11.2)
//!
//! The generators are shared so that every game gets the same *quality* of
//! hostile input without each author having to think of the same twenty edge
//! cases: out-of-range seats, commands in the wrong phase, timers that were never
//! set, admin inputs the game does not support.
//!
//! `proptest` over `quickcheck` for shrinking quality — when a 400-input
//! sequence fails, shrinking to the 3 inputs that actually matter is the
//! difference between a fixable bug and a shrug. (doc 01 §1.4)

use proptest::strategy::Strategy;
use tabula_core::SeatRoster;

/// How adversarial the generated sequence should be.
#[derive(Clone, Debug)]
pub struct SeqCfg {
    pub len: usize,
    /// Mostly legal moves (drawn via `legal_commands`), with this fraction of
    /// garbage mixed in. A purely legal sequence never exercises R2 or R3; a
    /// purely hostile one never reaches an interesting state.
    pub hostile_fraction: f32,
    pub include_timers: bool,
    pub include_seat_changes: bool,
    pub include_admin: bool,
}

impl Default for SeqCfg {
    fn default() -> Self {
        Self {
            len: 200,
            hostile_fraction: 0.15,
            include_timers: true,
            include_seat_changes: true,
            include_admin: true,
        }
    }
}

/// Random but *legal-ish* input sequences.
///
/// TODO(phase 0): drive the legal fraction through `legal_commands`, so a game
/// that implements it gets deep sequences that actually reach late-game states.
/// A game that returns `Unknown` falls back to pure fuzz, which is still useful
/// for R3 but will not find phase-transition bugs.
pub fn input_sequence(_cfg: SeqCfg) -> impl Strategy<Value = Vec<()>> {
    // Placeholder so the module type-checks; replace with the real generator.
    proptest::collection::vec(proptest::strategy::Just(()), 0..1)
}

/// Random rosters, including bot occupants, mid-match disconnects, and idle
/// transitions.
///
/// TODO(phase 0): generate rosters that are *legal for the game's `SeatSpec`* —
/// werewolf's role sets are only balanced at particular counts, so an arbitrary
/// count would fail `validate_config` and test nothing.
pub fn roster(_min: u8, _max: u8) -> impl Strategy<Value = SeatRoster> {
    proptest::strategy::Just(SeatRoster {
        seats: smallvec::SmallVec::new(),
    })
}

/// Random but valid `Config` values.
///
/// TODO(phase 5): derive ranges from the `Config` type's declared schema, the
/// same source the generated lobby config form uses (doc 02 §10.3). Until that
/// derive exists, games supply their own strategy.
pub fn config<T: Default + std::fmt::Debug>() -> impl Strategy<Value = T> {
    proptest::strategy::LazyJust::new(T::default)
}
