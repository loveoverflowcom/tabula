//! # `tabula-testkit` — the conformance harness
//!
//! **Phase 0.** Real from day one (doc 07 Phase 0, doc 09 §7 step 8).
//!
//! > Determinism and projection safety cannot be checked by review alone.
//! > — ADR-025
//!
//! Every game crate takes this as a dev-dependency and gets the whole invariant
//! suite for free. That is the single biggest developer-experience lever we have:
//! it is what makes the tenth game cost a fraction of the first.
//!
//! ## Usage
//!
//! A game author writes a small [`GameTestFixture`] — the data one real match
//! needs — and gets the mandatory suite for free:
//!
//! ```rust,ignore
//! // games/<slug>/tests/conformance.rs
//! struct TicTacToeFixture;
//!
//! impl GameTestFixture for TicTacToeFixture {
//!     type Module = tabula_game_tictactoe::TicTacToeModule;
//!     // ... config(), roster(), seed(), deterministic_script() ...
//! }
//!
//! tabula_testkit::conformance!(TicTacToeFixture);
//! ```
//!
//! That one line expands to the full suite documented in [`conformance`].
//! **A game may not be registered until it passes them.**
//!
//! ## The highest-value test is the cheapest
//!
//! Bot self-play (doc 02 §11.3). Bots play each other thousands of times with
//! random seeds, and every match is checked for determinism, projection safety,
//! and termination. It finds rule bugs, infinite phases, and projection leaks
//! better than hand-written tests, and it costs a game author nothing beyond
//! implementing `legal_commands`.
//!
//! Nightly it can run at 100k matches per game. Failures carry the base seed,
//! match index, and input index needed to reproduce them; the harness does not
//! mutate the repository or create replay files.
//!
//! ## Module map
//!
//! | Module | What it is |
//! |---|---|
//! | [`conformance`] | The `conformance!` macro and the mandatory test list |
//! | [`determinism`] | Re-run harness, clone-and-compare R2 checker, snapshot round-trip |
//! | [`projection`] | `SecretModel` scanning — the security test category |
//! | [`replay`] | `.tbr` reader/writer and `ReplayRunner` |
//! | [`selfplay`] | Bot-vs-bot driver, the primary fuzzer |
//! | [`strategies`] | `proptest` generators for inputs, rosters, configs |
//! | [`fakes`] | In-memory `EventLog` / `SnapshotStore` / `Clock` |

#![forbid(unsafe_code)]

pub mod conformance;
pub mod determinism;
pub mod fakes;
pub mod projection;
pub mod replay;
pub mod selfplay;
pub mod strategies;

pub use conformance::{
    GameTestFixture, InvalidCommandScenario, RandomnessScenario, TerminalScenario,
};
pub use determinism::{RunTrace, Scenario};
pub use projection::{Secret, SecretModel};
pub use replay::{
    Divergence, DivergenceKind, ReplayDraft, ReplayError, ReplayFrame, ReplayHeader,
    ReplayIdentity, ReplayKind, ReplayRunner, ReplayVerdict, StepResult, ValidatedReplay,
    VerifyReport,
};
