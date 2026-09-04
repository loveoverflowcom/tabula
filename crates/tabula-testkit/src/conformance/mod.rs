//! The fixture-driven conformance suite. (doc 02 §11.1, ADR-025, ADR-026)
//!
//! # What this is for
//!
//! Implementing `GameRules` proves a game *compiles* against the contract. It
//! proves nothing about determinism, replay, or safety — those are runtime
//! properties, and ADR-025 exists precisely because "determinism and
//! projection safety cannot be checked by review alone."
//!
//! A game author implements [`GameTestFixture`] once — the data a real match
//! needs (a config, a roster, a seed, a legal command sequence) plus a small
//! number of optional scenarios for capabilities the game has — and
//! [`conformance!`] expands that into the mandatory invariant suite:
//!
//! ```rust,ignore
//! struct ChessFixture;
//!
//! impl GameTestFixture for ChessFixture {
//!     type Module = ChessModule;
//!
//!     fn config() -> Config { Config::default() }
//!     fn roster() -> SeatRoster { /* two seats */ }
//!     fn seed() -> MatchSeed { MatchSeed::from_bytes([42; 32]) }
//!     fn deterministic_script() -> Vec<Input<Command>> { /* a full game */ }
//! }
//!
//! tabula_testkit::conformance!(ChessFixture);
//! ```
//!
//! That one line expands to one `#[test]` per invariant below. A failure
//! names the invariant and the game, not just `assertion failed: left == right`.
//!
//! # The invariants
//!
//! | Test | Checks | Module |
//! |---|---|---|
//! | Stable identity | `GameId` non-empty, reverse-DNS shaped, stable across calls | [`identity`] |
//! | Deterministic initialization | Same config+roster+seed ⇒ same state, hash, and `create` effects | [`determinism`] |
//! | Deterministic command execution | Same script re-run independently ⇒ same state, events, hash (I-2) | [`determinism`] |
//! | Deterministic replay | The full script agrees across 3 independent runs; canonical state compared, not hash alone | [`replay`] |
//! | Ordered events | A script's event stream is stable across runs and never empty | [`replay`] |
//! | Serialization round-trip | `state → canonical bytes → state` preserves semantics and hash (I-8) | [`serialization`] |
//! | Invalid-command safety | A rejected command is a total no-op (R2) and disturbs no later input's RNG (R8) | [`commands`] |
//! | `legal_commands` sanity | Every enumerated command applies cleanly; no duplicates; stable order | [`commands`] |
//! | Terminal-state behavior | A terminal script emits `Effect::EndMatch`; the fixture's post-terminal probe is rejected | [`terminal`] |
//! | State hash sensitivity | Two semantically different states hash differently and never to all-zero | [`hashing`] |
//! | Deterministic RNG behavior | An alternate seed is independently deterministic (without requiring seeds to diverge) | [`rng`] |
//!
//! Every check above is opt-in only where the *capability itself* is
//! optional (invalid commands, terminal states, randomness) — never as a way
//! to make a broken game pass. A fixture that skips a scenario prints why.
//!
//! # Why fixture-driven rather than a single opaque macro body
//!
//! The old shape of this macro accepted a bare `GameModule` path and expanded
//! to nothing meaningful (`assert!(!game_id.as_str().is_empty())` in spirit).
//! That is a green tick that means nothing — see
//! `tests/harness_catches_violations.rs` for why this crate treats "the
//! harness doesn't enforce anything" as a bug class of its own.
//!
//! Real checks need real data: a config the game accepts, a roster it can
//! seat, a script that reaches an interesting state. [`GameTestFixture`] is
//! that data, supplied once per game. The checks themselves are ordinary
//! functions in [`identity`], [`determinism`], [`replay`], [`serialization`],
//! [`commands`], [`terminal`], [`hashing`], and [`rng`] — not macro bodies —
//! so they type-check, navigate, and fail like any other Rust code.
//!
//! # Hidden information is a separate, opt-in suite
//!
//! [`security`] is deliberately **not** part of [`conformance!`]'s
//! expansion. A game either has hidden information or it does not; forcing
//! every game to implement `SecretModel` to satisfy one macro would mean
//! Chess and other perfect-information games carry a fake secret model for no reason. A game with
//! `hidden_information = true` additionally implements
//! [`security::HiddenInformationFixture`] and expands
//! [`crate::projection_security!`] alongside `conformance!`; a
//! perfect-information game does neither and is unaffected.

pub mod commands;
pub mod determinism;
pub mod hashing;
pub mod identity;
pub mod replay;
pub mod rng;
pub mod security;
pub mod serialization;
mod support;
pub mod terminal;

use tabula_core::{MatchSeed, SeatRoster};
use tabula_game_api::{GameModule, GameRules, Input};

use crate::determinism::Scenario;

/// The concrete `GameRules` a fixture exercises.
pub(crate) type RulesOf<F> = <<F as GameTestFixture>::Module as GameModule>::Rules;
/// The concrete `Command` type a fixture's scripts are made of.
pub(crate) type CommandOf<F> = <RulesOf<F> as GameRules>::Command;

/// What a game supplies to receive the full conformance suite.
///
/// Compact by design: four required functions describe one real match
/// (config, roster, seed, a legal script), and three optional scenarios opt
/// into checks for capabilities that are themselves optional — a game with
/// no command rejection, no terminal state, or no randomness simply omits
/// the corresponding scenario rather than faking one.
pub trait GameTestFixture {
    /// The game module under test. Its `Rules` associated type is what every
    /// check below actually exercises; `Module` itself supplies `GameId` and
    /// `validate_config` for the identity and initialization checks.
    type Module: GameModule;

    /// A config this fixture's roster satisfies `GameModule::validate_config` for.
    fn config() -> <RulesOf<Self> as GameRules>::Config;

    /// A roster satisfying the game's `SeatSpec`.
    fn roster() -> SeatRoster;

    /// The match seed used by every deterministic check. Any fixed value —
    /// determinism does not depend on which seed, only on using the same one
    /// consistently, which every check here does on the caller's behalf.
    fn seed() -> MatchSeed;

    /// A legal command sequence exercising a real match from `create`
    /// onward. Must leave the state semantically different from the initial
    /// state — that is what makes the hash-sensitivity check ([`hashing`])
    /// meaningful, and what makes the ordered-events check ([`replay`])
    /// non-vacuous.
    fn deterministic_script() -> Vec<Input<CommandOf<Self>>>;

    /// A command this game is expected to reject, reached via `setup`, plus
    /// a legal `probe` applied immediately after — proving the rejection
    /// disturbed neither state (R2) nor the probe's RNG stream (R8).
    ///
    /// `None` only for a game with no command validation to exercise at all,
    /// which should be rare: almost every game rejects *something* (a
    /// mistimed turn, an out-of-range target).
    fn invalid_command() -> Option<InvalidCommandScenario<CommandOf<Self>>> {
        None
    }

    /// A script that drives the match to a terminal state (one that emits
    /// `Effect::EndMatch`), plus a command to try immediately afterward,
    /// which the fixture's own game is expected to reject.
    ///
    /// `None` for a game with no terminal state in this sense (unusual —
    /// most matches end).
    fn terminal() -> Option<TerminalScenario<CommandOf<Self>>> {
        None
    }

    /// An alternate seed, used only to demonstrate that determinism holds
    /// independently of *which* seed is used. `None` for a game that never
    /// draws from `ctx.rng` at all — Chess is the reference case.
    fn randomness() -> Option<RandomnessScenario> {
        None
    }
}

/// See [`GameTestFixture::invalid_command`].
#[derive(Debug)]
pub struct InvalidCommandScenario<C> {
    /// Legal inputs applied first, to reach the state under test.
    pub setup: Vec<Input<C>>,
    /// The command expected to be rejected from that state.
    pub invalid: Input<C>,
    /// A legal command applied immediately after the rejection, to prove it
    /// consumed no observable randomness (contract R8).
    pub probe: Input<C>,
}

/// See [`GameTestFixture::terminal`].
#[derive(Debug)]
pub struct TerminalScenario<C> {
    /// A script expected to end the match (emit `Effect::EndMatch`).
    pub script: Vec<Input<C>>,
    /// A command tried after the match has ended. The check asserts this is
    /// rejected — the production contract's answer, never one invented here.
    pub post_terminal: Input<C>,
}

/// See [`GameTestFixture::randomness`].
#[derive(Debug)]
pub struct RandomnessScenario {
    /// Any seed different from [`GameTestFixture::seed`].
    pub alternate_seed: MatchSeed,
}

/// Build a [`Scenario`] from a fixture's config/roster/seed plus one script.
pub(crate) fn scenario<F: GameTestFixture>(
    inputs: Vec<Input<CommandOf<F>>>,
) -> Scenario<RulesOf<F>> {
    Scenario {
        config: F::config(),
        roster: F::roster(),
        seed: F::seed(),
        inputs,
    }
}

/// Expand the mandatory conformance suite for a [`GameTestFixture`].
///
/// ```rust,ignore
/// tabula_testkit::conformance!(ChessFixture);
/// ```
///
/// Emits one `#[test]` per row of the table in the module documentation.
/// Each is a thin wrapper around a real function in a sibling module, so a
/// failure has a normal Rust stack frame and the check itself is navigable,
/// type-checked, and reusable outside the macro.
#[macro_export]
macro_rules! conformance {
    ($fixture:ty) => {
        #[test]
        fn tabula_conformance_stable_identity() {
            $crate::conformance::identity::check::<$fixture>();
        }

        #[test]
        fn tabula_conformance_deterministic_initialization() {
            $crate::conformance::determinism::check_init::<$fixture>();
        }

        #[test]
        fn tabula_conformance_deterministic_command_execution() {
            $crate::conformance::determinism::check_apply::<$fixture>();
        }

        #[test]
        fn tabula_conformance_deterministic_replay() {
            $crate::conformance::replay::check::<$fixture>();
        }

        #[test]
        fn tabula_conformance_ordered_events() {
            $crate::conformance::replay::check_ordered_events::<$fixture>();
        }

        #[test]
        fn tabula_conformance_serialization_roundtrip() {
            $crate::conformance::serialization::check::<$fixture>();
        }

        #[test]
        fn tabula_conformance_invalid_command_safety() {
            $crate::conformance::commands::check_invalid::<$fixture>();
        }

        #[test]
        fn tabula_conformance_legal_commands_sanity() {
            $crate::conformance::commands::check_legal::<$fixture>();
        }

        #[test]
        fn tabula_conformance_terminal_behavior() {
            $crate::conformance::terminal::check::<$fixture>();
        }

        #[test]
        fn tabula_conformance_state_hash_sensitivity() {
            $crate::conformance::hashing::check::<$fixture>();
        }

        #[test]
        fn tabula_conformance_deterministic_rng() {
            $crate::conformance::rng::check::<$fixture>();
        }
    };
}
