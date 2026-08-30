//! # `tabula-game-chess` — the correctness benchmark
//!
//! > ## PHASE 1 — DO NOT IMPLEMENT BEFORE PHASE 0 EXITS
//! >
//! > Gate: `cargo xtask selfplay tictactoe --matches 10000` passes, the
//! > conformance suite is green, and CI demonstrably rejects a deliberately
//! > added forbidden dependency. (doc 09 §7 steps 4–9)
//!
//! Chess is Phase 1 because it is the **simple case that must be perfect**. It
//! has no hidden information and no randomness, so any determinism failure here
//! is a bug in the harness, not in a shuffle. That makes it the control group
//! for everything that follows.
//!
//! It also validates: complex legality, clocks and timers, ranked ratings,
//! spectators, and async (correspondence) turns. (doc 08 §5.A)
//!
//! ## Scope (doc 08 §5.A)
//!
//! ```text
//! IN:  standard rules, all special moves (castling, en passant, promotion),
//!      check/checkmate/stalemate, threefold repetition, 50-move rule,
//!      insufficient material, resign, draw offer/accept/decline, claim draw,
//!      Fischer and Bronstein increments, flag fall, correspondence mode (24h)
//! OUT: variants (Chess960, three-check, atomic) — Phase 9+
//! OUT: opening book, analysis engine, cloud eval — separate optional crate,
//!      NEVER in the rules half
//! OUT: FIDE tournament arbitration rules
//! ```
//!
//! ## Contract sketch (doc 02 §12.1)
//!
//! ```rust,ignore
//! struct State {
//!     board: [Option<Piece>; 64],      // fixed-size, tiny
//!     side: Color,
//!     castling: CastlingRights,
//!     ep: Option<Square>,
//!     halfmove: u8, fullmove: u16,
//!     clocks: [Millis; 2],
//!     last_move_at: LogicalTime,
//!     repetition: SmallVec<[u64; 16]>, // zobrist history for threefold
//!     status: Status,
//!     draw_offer: Option<SeatId>,
//! }
//! enum Command { Move { from: Square, to: Square, promo: Option<PieceKind> },
//!                Resign, OfferDraw, AcceptDraw, DeclineDraw, ClaimDraw(DrawClaim) }
//! enum Event   { Moved { .. }, Captured { .. }, Castled { .. }, Promoted { .. },
//!                ClockUpdated { seat, remaining }, DrawOffered { seat }, Ended { outcome } }
//! ```
//!
//! ## The four contract lessons
//!
//! 1. **`View` ≈ `State`, and is still a separate type.** It omits `repetition`
//!    (an implementation detail) and adds `legal_moves` for the seat on turn.
//! 2. **Clocks are the interesting part.** `apply` decrements the mover's clock
//!    by `ctx.now - state.last_move_at`. It never reads a real clock.
//!    `Effect::SetTimer` is re-armed on every move for the remaining time of the
//!    player now on turn. Restart-safety falls out of the log: on recovery,
//!    timers are re-derived **from state**, not from memory.
//! 3. **Disconnect keeps the clock running.** `notify_rules = true`, and `apply`
//!    for `Input::Seat { Disconnected }` returns `Outcome::empty()` — the clock
//!    burns via the existing timer. That is a rules decision expressed by doing
//!    nothing.
//! 4. **`legal_commands` fully enumerates** (~30 moves), which powers move
//!    highlighting, drag-drop legality, and a free `Trivial` bot.
//!
//! ## The claim this game must prove
//!
//! > The platform contains **zero** lines of clock code. Verified by grep and by
//! > review. — doc 08 §5.A
//!
//! Failure signals: clock code appearing in `tabula-match`; `legal_commands`
//! being called from `apply`; drag state in `State`.
//!
//! ## Acceptance (doc 08 §5.A)
//!
//! ```text
//! [ ] perft depth 1–5 exact for the standard position + 5 published edge cases
//! [ ] all draw conditions reachable in tests
//! [ ] clock invariants hold under property testing
//! [ ] 100k bot self-play: zero determinism failures, zero panics, all terminate
//! [ ] hot-seat playable (Phase 2) and online playable (Phase 4)
//! [ ] replay of 1,000 sampled games byte-exact
//! [ ] keyboard-only game completion
//! [ ] Elo updates correct for win/loss/draw and NEVER on Aborted
//! ```
//!
//! ## File layout when this becomes real
//!
//! ```text
//! src/state.rs   State, Command, Event, View, ViewEvent, Config
//! src/rules.rs   impl GameRules — dispatch to sub-functions per concern
//! src/movegen.rs legal move generation (the bulk of the work; perft tests it)
//! src/clock.rs   Fischer/Bronstein increment arithmetic
//! src/draw.rs    threefold, 50-move, insufficient material
//! src/bot.rs     #[cfg(feature = "bots")]
//! src/ui.rs      #[cfg(feature = "presentation")]  — Phase 2
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
