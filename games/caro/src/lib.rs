//! # `tabula-game-caro` — the simple-product / SDK-friction benchmark
//!
//! > ## PHASE 3 — DO NOT IMPLEMENT BEFORE PHASE 2 EXITS
//! >
//! > Gate: chess is playable hot-seat on desktop and web from one codebase, and
//! > the `RenderList` command set is locked. (doc 07 Phase 2 exit criteria)
//!
//! Caro (a Gomoku/five-in-a-row family game) is **not tic-tac-toe renamed**.
//! `games/tictactoe` stays the internal SDK smoke test and new-game template
//! (doc 02 §10); it exists to prove the platform works at all, in seconds.
//! Caro exists to prove something tic-tac-toe cannot: that a second,
//! independently-added, real product game costs *only* "implement rules +
//! implement presentation + write tests" (doc 00 §1), on a board large enough
//! that a naive win-check is not free. It is the cheapest **simple** game with
//! perfect information and a large fixed board — the middle rung of the
//! ladder between the tiny SDK example and chess's complex legality. (doc 08 §5.B)
//!
//! ## Scope (doc 08 §5.B)
//!
//! ```text
//! IN:  a fixed, configurable-size square board (a larger board such as 15×15
//!      is the expected default — exact size TBD during implementation),
//!      alternating placement, row/column/diagonal win-line detection,
//!      draw on a full board, local play, future online play
//! OUT: the exact rule variant (freestyle vs. Renju-style restrictions on the
//!      first player) — EXPERIMENT, a future game-design decision, not settled
//!      by this document
//! OUT: tournament opening protocols (swap rules, restricted openings)
//! OUT: an AI engine beyond a trivial/easy bot
//! ```
//!
//! ## Contract sketch (doc 02 §12 style — illustrative, not locked)
//!
//! ```rust,ignore
//! struct State {
//!     board: Grid<Option<Mark>>,   // fixed size, larger than tic-tac-toe's 3x3
//!     turn: SeatId,
//!     status: Status,
//!     moves: u32,
//! }
//! enum Command { Place { at: Coord }, Resign }
//! enum Event { Placed { seat: SeatId, at: Coord, mark: Mark }, Ended { outcome: MatchOutcome } }
//! ```
//!
//! ## The contract lessons this game is here to produce
//!
//! 1. **`View` ≈ `State`.** No hidden information (`hidden_information = false`)
//!    — like chess, but without the complex legality, so any SDK friction found
//!    here is friction, not a symptom of chess's own complexity.
//! 2. **`legal_commands` on a large board.** Unlike tic-tac-toe's nine cells,
//!    a 15×15 board has up to 225 legal placements — still small enough to
//!    fully `Enumerated`, but big enough to be a real test of that path before
//!    tiles forces `Hints` instead (doc 02 §4's `LegalCommands`).
//! 3. **Win-line detection is the interesting algorithm.** Four directions,
//!    checked from the just-placed cell outward, is the cheap approach; this
//!    is the game that proves whether the platform's `apply` budget
//!    (`capabilities.apply_budget`) is comfortable with it at board-game rates.
//! 4. **No RNG, no timers beyond an optional turn clock.** Deliberately boring
//!    on every other axis, so the *only* new variable is "a second game
//!    exists" — this is what makes it an honest SDK-friction measurement
//!    rather than a second correctness benchmark.
//!
//! Caro has `hidden_information = false`; no `SecretModel` (doc 02 §7.3) is required.
//!
//! ## Acceptance (doc 08 §5.B)
//!
//! ```text
//! [ ] win-line detection fully tested in all four directions, incl. edges/corners
//! [ ] draw-on-full-board detection tested
//! [ ] 100k bot self-play: terminates, no determinism failures
//! [ ] legal_commands enumeration matches apply()'s own legality decisions
//! [ ] added with zero changes to tabula-core / tabula-game-api (the SDK-friction claim)
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
