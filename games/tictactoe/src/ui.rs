//! Presentation. `#[cfg(feature = "presentation")]` — client only.
//!
//! **Phase 2.** The presentation contract does not exist until
//! `tabula-presentation` is real (doc 07 Phase 2).
//!
//! ## The three rules this file must follow
//!
//! 1. **`RenderList` only.** No direct renderer calls, ever. If a game draws
//!    with Macroquad directly, the renderer decision becomes irreversible and
//!    ADR-010 is dead.
//! 2. **No raw colours.** Semantic tokens from `tabula-design` —
//!    `t.color.legal_target`, not `#3E7B5A`. `xtask check-no-raw-colors` greps
//!    for hex literals and `Color::new(` outside `tabula-design`.
//! 3. **All local state lives in `Local`, never in `State`.** Selection, drag,
//!    hover, camera, animation progress. Putting any of it in canonical state
//!    makes presentation authoritative, which is I-10 and also a desync bug
//!    waiting for a slow client.
//!
//! ## The sketch
//!
//! ```rust,ignore
//! use tabula_presentation::{FrameCtx, GamePresentation, InputEvent, Intent, RenderList};
//!
//! #[derive(Default)]
//! pub struct Local {
//!     /// Cell the pointer is over. Client-only, never sent upstream.
//!     hover: Option<u8>,
//!     /// Optimistic preview, kept structurally separate from `View` (I-12).
//!     /// Rendered translucent; replaced by the projection on Ack, discarded on
//!     /// Reject (and the `motion.invalid` token plays — never a modal).
//!     pending: Option<u8>,
//!     anim: tabula_presentation::AnimationSet,
//! }
//!
//! impl GamePresentation for TicTacToePresentation {
//!     type Rules = TicTacToeRules;
//!     type Local = Local;
//!
//!     fn asset_pack() -> AssetPackRef { AssetPackRef::new("tictactoe", "0.1.0") }
//!
//!     fn present(view: &View, local: &Local, frame: &FrameCtx) -> RenderList {
//!         // board grid   -> Layer::Board
//!         // marks        -> Layer::Pieces, motion.token-drop on arrival
//!         // hover/legal  -> Layer::Overlay, t.color.legal_target
//!         // turn + clock -> Layer::HUD
//!     }
//!
//!     fn on_view_event(ev: &ViewEvent, local: &mut Local, frame: &FrameCtx) {
//!         // Animation is driven by EVENTS, never by diffing two views —
//!         // doc 04 §9.3, "motion follows causality".
//!         // An animation whose start is already >600 ms stale snaps to its end.
//!     }
//!
//!     fn on_input(input: &InputEvent, view: &View, local: &mut Local)
//!         -> Option<Intent<Command>>
//!     {
//!         // Both tap-tap and drag-drop must work; tap-tap is the accessible
//!         // default (doc 04 §10.2). Min touch target 44x44 dp.
//!     }
//!
//!     fn a11y(view: &View, _local: &Local) -> A11yDescription { TicTacToeRules::describe(...) }
//! }
//! ```
