//! The mandatory conformance suite. (doc 02 §11)
//!
//! One fixture, the full suite. Game-specific rule tests live in
//! `tests/rules.rs` and `tests/determinism.rs` so a conformance failure is
//! never confused with a gameplay-logic failure.
//!
//! The hidden-information security suite is **not** here: `SecretModel` is a
//! foreign trait and `TilesRules` a foreign type to this crate, so that impl
//! and its suite live in `src/rules/secret.rs` under `cfg(test)`.

mod support;

use tabula_core::{MatchSeed, SeatRoster};
use tabula_game_api::Input;
use tabula_game_tiles::{Command, Config, Coord, Rotation, TilesModule};
use tabula_testkit::{
    GameTestFixture, InvalidCommandScenario, RandomnessScenario, TerminalScenario,
};

use support::{ALT_SEED, SEED};

struct TilesFixture;

/// Three seats: enough that turn order actually rotates, and enough for the
/// security suite's viewer universe to contain a seat that is not on turn.
const SEATS: u8 = 3;

impl GameTestFixture for TilesFixture {
    type Module = TilesModule;

    fn config() -> Config {
        support::config()
    }

    fn roster() -> SeatRoster {
        support::roster(SEATS)
    }

    fn seed() -> MatchSeed {
        MatchSeed::from_bytes(SEED)
    }

    /// Twelve real placements: four full rounds of three seats, which is deep
    /// enough that the board has branched in every direction and the state is
    /// far from the opening position (what makes the hash-sensitivity and
    /// ordered-events checks non-vacuous).
    fn deterministic_script() -> Vec<Input<Command>> {
        support::drive(&MatchSeed::from_bytes(SEED), SEATS, support::config(), 12).1
    }

    fn invalid_command() -> Option<InvalidCommandScenario<Command>> {
        let (state, setup) =
            support::drive(&MatchSeed::from_bytes(SEED), SEATS, support::config(), 2);
        let probe = support::next_placement(&state).expect("the match is still in progress");
        Some(InvalidCommandScenario {
            setup,
            // Far from the board, so it touches nothing: rejected for a reason
            // that does not depend on which tile the shuffle happened to deal.
            invalid: Input::Player {
                seat: state.turn(),
                command: Command::PlaceTile {
                    at: Coord::new(40, 40).expect("inside the playable space"),
                    rotation: Rotation::R0,
                },
            },
            probe,
        })
    }

    fn terminal() -> Option<TerminalScenario<Command>> {
        let script = support::full_script(&MatchSeed::from_bytes(SEED), SEATS, support::config());
        Some(TerminalScenario {
            // The bag runs dry and the match ends.
            script,
            // Any command after that must be rejected.
            post_terminal: Input::Player {
                seat: tabula_core::SeatId(0),
                command: Command::PlaceTile {
                    at: Coord::ORIGIN,
                    rotation: Rotation::R0,
                },
            },
        })
    }

    /// Tiles draws from `ctx.rng` exactly once, in `create`. This alternate
    /// seed proves determinism does not depend on *which* seed is used.
    fn randomness() -> Option<RandomnessScenario> {
        Some(RandomnessScenario {
            alternate_seed: MatchSeed::from_bytes(ALT_SEED),
        })
    }
}

tabula_testkit::conformance!(TilesFixture);
