//! # `tabula-game-cards` — Tiến Lên, the hidden-information benchmark
//!
//! > ## PHASE 3 — DO NOT IMPLEMENT BEFORE PHASE 2 EXITS
//! >
//! > Gate: chess is playable hot-seat on desktop and web from one codebase, and
//! > the `RenderList` command set is locked. (doc 07 Phase 2 exit criteria)
//!
//! Chosen over poker **deliberately**: 4 players, hidden hands, trick-taking, no
//! betting economy, and a cultural fit with the first market. Big Two, Tiến Lên
//! Miền Bắc, and simple poker variants all reuse its primitives. (doc 08 §5.B)
//!
//! This is the game that proves the security boundary works.
//!
//! ## Scope (doc 08 §5.B)
//!
//! ```text
//! IN:  52-card deck, deal 13 each, lowest-card start, single/pair/triple/
//!      straight/double-sequence combinations, beat-or-pass trick play,
//!      chop rules (2s and bombs), finishing order → placement scoring,
//!      20s turn timer with auto-pass, deck commitment (hash at start, salt at end)
//! OUT: betting/wagering, currency, tournament ladders
//! OUT: regional rule variants beyond one configurable preset — Phase 9
//! ```
//!
//! ## Contract sketch (doc 02 §12.2)
//!
//! ```rust,ignore
//! struct State {
//!     hands: [SmallVec<[Card; 13]>; 4],     // SECRET, per seat
//!     deck_commit: [u8; 32],                // blake3(shuffled order || salt), public at start
//!     salt: [u8; 16],                       // SECRET until match end
//!     table: Option<Play>,
//!     lead: SeatId, turn: SeatId,
//!     passed: [bool; 4],
//!     finished: SmallVec<[SeatId; 4]>,      // finishing order == standings
//!     scores: [i64; 4],
//! }
//!
//! struct View {
//!     your_hand: SmallVec<[Card; 13]>,      // ONLY ever your own
//!     hand_counts: [u8; 4],                 // public
//!     table: Option<Play>,
//!     turn: SeatId, lead: SeatId,
//!     passed: [bool; 4],
//!     finished: SmallVec<[SeatId; 4]>,
//!     deck_commit: [u8; 32],
//!     you: Option<SeatId>,
//! }
//!
//! enum ViewEvent {
//!     DealtToYou   { cards: SmallVec<[Card; 13]> },
//!     DealtToOther { seat: SeatId, count: u8 },   // DEGRADED, not hidden
//!     Played { seat, cards }, Passed { seat }, TrickWon { seat },
//!     Finished { seat, place }, DeckRevealed { salt }, Ended { outcome },
//! }
//! ```
//!
//! ## The five contract lessons
//!
//! 1. **There is no `Option<Vec<Card>>` anywhere.** `your_hand` is the only hand
//!    present in `View`. An absent field cannot be accidentally filled in by a
//!    careless refactor; a `None` field can. (doc 02 §7.1)
//! 2. **`view_event` degrades rather than hides.** `Dealt { seat, cards }`
//!    becomes `DealtToOther { seat, count }` for other seats — the card-back
//!    animation still plays without leaking anything.
//! 3. **RNG is drawn once, in `create`**, from `ctx.rng.stream(DOMAIN_SHUFFLE)`,
//!    via the pinned Fisher-Yates in `DetRng::shuffle`. A replay in two years
//!    reproduces the same deal.
//! 4. **Spectators are delayed 30 s** so a spectator cannot relay information to
//!    a player in real time. Declared by the capability, enforced by the platform
//!    via buffering. This is what exercises "project from a past snapshot".
//! 5. **The commitment scheme** — `deck_commit` published at start, `salt`
//!    revealed at end — lets any player verify afterwards that the deck was not
//!    manipulated mid-match. **EXPERIMENT** (doc 09 §3.2): build it here because
//!    cards is where players suspect cheating; the fallback is to drop it with a
//!    written note, since the projection remains the real guarantee.
//!
//! ## `SecretModel` (mandatory — `hidden_information = true`)
//!
//! ```text
//! deck order  → nobody, until match end
//! each hand   → its own seat only
//! salt        → nobody, until match end
//! ```
//!
//! Also required: `docs/games/cards.md` with the **information model** — what is
//! intentionally derivable. The token scanner cannot catch derived secrets
//! (deck count + all discards + your hand); only a written model and a human can.
//!
//! ## Acceptance (doc 08 §5.B)
//!
//! ```text
//! [ ] combination validity fully tested, including chops and edge cases
//! [ ] projection scan green for all four seats + live and delayed spectators
//! [ ] a scripted "hostile client" test: repeated resync, attach as spectator
//!     while seated, attempt to attach to another seat — none reveal a hand
//! [ ] shuffle replay exactness over 10k matches
//! [ ] commitment verification test AND a deliberate tampering test that fails
//! [ ] 100k bot self-play: terminates, no determinism failures
//! [ ] delayed spectator sees nothing newer than the window — asserted AT THE SOCKET
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
