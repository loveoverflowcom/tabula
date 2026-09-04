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
use tabula_game_tiles::{Command, Config, Coord, Rotation, TilesModule, TurnPhase};
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

    /// A dozen real inputs — several full turns of three seats, with tiles
    /// placed in every direction and followers claimed along the way — ending
    /// **in the claim step**.
    ///
    /// Ending there is deliberate. `conformance::commands::check_legal` only
    /// inspects `Enumerated` results, and Tiles enumerates only in the claim
    /// step; a script that happened to stop at a turn boundary would leave that
    /// check silently vacuous, which this crate's own docs call out as a bug
    /// class of its own.
    fn deterministic_script() -> Vec<Input<Command>> {
        let (state, script) = support::drive_to_claim_phase(
            &MatchSeed::from_bytes(SEED),
            SEATS,
            support::config(),
            12,
        );
        assert_eq!(
            state.phase(),
            TurnPhase::PlaceMeeple,
            "the fixture script must end in the claim step, or legal_commands              sanity never sees an enumeration"
        );
        assert!(
            !state.features().followers().is_empty(),
            "the fixture script must put at least one follower on the board, or              the security suite scans a View with no follower positions in it"
        );
        script
    }

    fn invalid_command() -> Option<InvalidCommandScenario<Command>> {
        // Stop at a placement step so both the rejected command and the probe
        // are placements: the R8 check applies the probe right after the
        // rejection and needs it to be legal from the same state.
        let (state, setup) =
            support::drive(&MatchSeed::from_bytes(SEED), SEATS, support::config(), 2);
        let probe = support::next_placement(&state).expect("the match is at a placement step");
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
