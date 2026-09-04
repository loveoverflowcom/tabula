//! # `tabula-game-tiles` — the state-and-camera benchmark
//!
//! > ## PHASE 3 (rules + presentation) → PHASE 9 (full)
//!
//! Carcassonne-like: draw a tile, place it legally adjacent, optionally place a
//! follower, score completed features. It is here to stress **large dynamic
//! state**, **camera**, and **async turns** — the three things chess, Caro, and
//! Werewolf all leave untested. (doc 08 §4)
//!
//! ## Scope (doc 08 §4)
//!
//! ```text
//! IN:  ~72-tile bag with a fixed distribution, rotation, adjacency legality,
//!      feature graph (roads, cities, fields, monasteries), follower placement
//!      and return, incremental scoring, end-of-game scoring, 2–5 seats,
//!      60s live turns OR 24h async turns
//! OUT: expansions, rivers, custom boards, trading — later
//! ```
//!
//! ## Contract sketch (doc 02 §12.4)
//!
//! ```rust,ignore
//! struct State {
//!     placed: BTreeMap<Coord, PlacedTile>,   // grows to ~72+ entries
//!     bag: SmallVec<[TileKind; 72]>,         // SECRET order, PUBLIC count
//!     drawn: Option<TileKind>,               // public once drawn
//!     meeples: BTreeMap<SeatId, u8>,
//!     features: FeatureGraph,                // incremental union-find for scoring
//!     scores: BTreeMap<SeatId, i64>,
//!     turn: SeatId, phase: TurnPhase,        // Draw | Place | Meeple | Score
//! }
//! enum Command { PlaceTile { at: Coord, rot: Rotation }, SkipMeeple,
//!                PlaceMeeple { on: FeatureSlot }, EndTurn }
//! ```
//!
//! ## The five contract lessons
//!
//! 1. **`state_size = Medium`** changes snapshot policy: every 50 inputs instead
//!    of every 200, stored as compressed blobs (doc 03 §9.2). This is the game
//!    that proves `StateSizeClass` earns its place on `GameCapabilities`.
//! 2. **`legal_commands` returns `Hints`, not `Enumerated`.** Legal
//!    (position × rotation) pairs are too numerous to enumerate; hints give the
//!    client enough to highlight without listing every command.
//! 3. **`FeatureGraph` is an incremental structure** — which is *why* `apply`
//!    takes `&mut State` (doc 02 §3.3). Recomputing scoring from scratch each
//!    turn would be simpler and ~100× slower on a large board. The incremental
//!    structure **must be included in the state hash** so a divergence in it is
//!    caught rather than silently accumulating.
//! 4. **Async turns are the natural mode.** `async_turns.supported = true` with
//!    a 24 h deadline; the match actor hibernates (doc 03 §11.2) and the platform
//!    sends push notifications. **The rules are unchanged between live and async
//!    play — that is the payoff of `LogicalTime`.**
//! 5. **The bag order is secret but its count is public.** Tiles declares
//!    `hidden_information = true`; its `SecretModel` marks the remaining bag
//!    order as authorised to nobody. `View` carries `bag_remaining: u8` and the
//!    drawn tile, never the order. This is a secondary secrecy case; Werewolf
//!    remains the primary benchmark for per-seat knowledge and event existence.
//!
//! Camera pan/zoom/rotate lives entirely in `P::Local`, never in canonical state
//! (I-10). The property test that proves it: identical command sequences issued
//! from different camera positions must produce identical state hashes.
//!
//! ## The hardest a11y case in the product
//!
//! Board Reader with coordinate-relative navigation — "north of the monastery at
//! C4" — plus a legal-placement list. Phase 9, and it is what will shape
//! `A11yRegion` (doc 04 §10.4).
//!
//! ## Acceptance (doc 08 §4)
//!
//! ```text
//! [ ] placement legality fully tested, all rotations and edge adjacency cases
//! [ ] scoring correct for every feature type, incl. end-of-game partial scoring
//! [ ] state hash cost < 200 µs at full board; apply within budget
//! [ ] snapshot size measured and StateSizeClass confirmed
//! [ ] SecretModel + projection scan prove remaining bag order is never exposed
//! [ ] Welcome frame for a full board within the 1 MiB outbound cap, with margin
//! [ ] async match survives 7 real days, 3 deploys, and 2 hibernation cycles
//! [ ] camera never affects state (property test, see above)
//! [ ] Board Reader allows completing a full turn with a screen reader
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
