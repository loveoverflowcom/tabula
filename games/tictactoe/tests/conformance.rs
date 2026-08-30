//! The mandatory conformance suite. (doc 02 §11)
//!
//! One line. Fifteen tests. **A game may not be registered until it passes.**
//!
//! If you are reading this in a new game crate, this file is complete — do not
//! add to it. Game-specific rule tests go in a sibling file
//! (`tests/rules.rs`), so a conformance failure is never confused with a
//! gameplay-logic failure.

tabula_testkit::conformance!(tabula_game_tictactoe::TicTacToeModule);
