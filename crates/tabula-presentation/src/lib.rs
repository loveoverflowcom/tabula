//! # `tabula-presentation` — renderer-independent presentation
//!
//! > ## PHASE 2
//!
//! This is the crate that makes renderer replacement possible (ADR-010). If games
//! drew directly with Macroquad, the renderer decision would be irreversible and
//! Macroquad's ceiling would become the product's ceiling.
//!
//! **Forbidden here, even behind a feature:** `macroquad`, `miniquad`, `wgpu`,
//! `leptos`, `tokio`, anything that does I/O.
//!
//! ## The pipeline (doc 04 §5.1)
//!
//! ```text
//! View (projection)  ┐
//! Local state        │
//! AnimationSet       ├─→ Presenter ─→ RenderList ─→ Renderer backend ─→ screen
//! Theme (tokens)     │   (per frame)   (flat, sorted, immutable)
//! AssetPack handles  ┘
//!
//! InputEvent ─→ Presenter::on_input ─→ Intent<Command> ─→ MatchClient::send_command
//! ```
//!
//! **Immediate mode and stateless.** The `RenderList` is rebuilt every frame from
//! `(View, Local, Animations, Theme)` and sorted by `(layer, z)` at the end. No
//! retained widget tree, no diffing, no invalidation — because the alternative is
//! a UI framework, and building one is the classic Rust-gamedev death spiral
//! (doc 00 §12).
//!
//! ## The MVP render command set — capped on purpose (doc 04 §5.2, §5.4)
//!
//! ```rust,ignore
//! pub struct RenderList { pub cmds: Vec<RenderCmd>, pub camera: Camera2D }
//!
//! pub enum RenderCmd {
//!     Sprite { asset: AssetHandle, rect: Rect, src: Option<Rect>, tint: Color,
//!              rot: f32, pivot: Vec2, layer: Layer, z: i16 },
//!     Rect   { rect: Rect, radii: Corners, fill: Option<Paint>, border: Option<Border>,
//!              layer: Layer, z: i16 },
//!     Text   { text: TextRef, at: Vec2, style: TextStyleToken, align: Align,
//!              max_width: Option<f32>, color: Color, layer: Layer, z: i16 },
//!     Path   { points: SmallVec<[Vec2; 8]>, stroke: Border, closed: bool,
//!              fill: Option<Paint>, layer: Layer, z: i16 },
//!     PushClip { rect: Rect },      PopClip,
//!     PushTransform { mat: Affine2 }, PopTransform,
//!     PushOpacity { alpha: f32 },   PopOpacity,
//! }
//!
//! pub enum Paint { Solid(Color), LinearGradient { from, to, stops } }
//! pub struct Layer(pub u8);  // Board=0 Pieces=10 Overlay=20 HUD=30 Modal=40 Toast=50
//! ```
//!
//! **Adding a variant requires a shipped-game justification** (doc 04 §5.4). The
//! expected additions, in the expected order, are: `Effect { id, params }` for
//! opt-in shader hooks, `NinePatch` for scalable panels, and a real
//! render-target-backed `PushOpacity`. Anything else needs an argument.
//!
//! ## The `Renderer` trait (doc 04 §6.2)
//!
//! ```rust,ignore
//! pub trait Renderer {
//!     fn begin_frame(&mut self, size: Vec2, dpi: f32) -> FrameCtx;
//!     fn submit(&mut self, list: &RenderList);
//!     fn end_frame(&mut self);
//!     fn measure_text(&self, text: &str, style: TextStyleToken, max_width: Option<f32>) -> TextMetrics;
//!     fn load_pack(&mut self, pack: &LoadedPack) -> Result<PackHandles, RenderError>;
//!     fn drain_input(&mut self) -> impl Iterator<Item = InputEvent>;
//! }
//! ```
//!
//! `measure_text` is the **only** synchronous mid-layout question a presenter
//! asks the backend, and it is cached. Keeping it to one call is what makes a
//! headless backend possible.
//!
//! `renderer-headless` exists **from day one** (doc 04 §6.1): ~200 lines, a
//! `RenderList` recorder plus a `tiny-skia` rasterizer. It is what makes golden
//! `RenderList` tests and image-diff tests possible without a GPU in CI.
//!
//! ## `GamePresentation` — the client-side half of a game (doc 02 §4)
//!
//! ```rust,ignore
//! pub trait GamePresentation: Send + 'static {
//!     type Rules: GameRules;
//!     type Local: Default;   // selection, drag, camera, animations — NEVER canonical
//!
//!     fn asset_pack() -> AssetPackRef;
//!     fn present(view: &View, local: &Local, frame: &FrameCtx) -> RenderList;
//!     fn on_view_event(ev: &ViewEvent, local: &mut Local, frame: &FrameCtx);
//!     fn on_input(input: &InputEvent, view: &View, local: &mut Local) -> Option<Intent<Command>>;
//!     fn a11y(view: &View) -> A11yDescription;
//! }
//! ```
//!
//! ## I-10, stated as a rule you can check
//!
//! **Presentation state never flows back into canonical state.** Animation
//! progress, camera position, hover, and drag-in-flight are client-local and
//! never travel upstream. Concretely:
//!
//! - Animation never affects canonical state, never gates command submission,
//!   and never delays authority.
//! - Motion is driven by **`ViewEvent`s, never by diffing two views** — causality,
//!   not observation (doc 04 §9.3).
//! - An animation whose start is already **>600 ms stale snaps to its end state**.
//! - Opponent actions get **1.15×** duration, so they read as deliberate.
//! - Replay fast-forward scales durations and clamps to instant above **4×**.
//!
//! ## Keyboard navigation is a service here, not per-game
//!
//! A game supplies a **focus graph** (cells + neighbours) derived from its view;
//! this crate handles traversal, focus rendering, and activation. Keyboard play
//! is mandatory for every game (doc 04 §10.3), and it would not survive being
//! reimplemented five times.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/render.rs     RenderList, RenderCmd, Paint, Layer, Camera2D
//! src/renderer.rs   Renderer trait, FrameCtx, TextMetrics
//! src/input.rs      InputEvent (pointer/touch/key/gesture), normalization
//! src/hit.rs        hit-testing, expanded targets, nearest-neighbour disambiguation
//! src/focus.rs      the focus-graph traversal service (keyboard + switch access)
//! src/anim.rs       clocks, springs, AnimationSet, motion-token resolution
//! src/layout.rs     layout primitives, breakpoints, safe-area handling
//! src/audio.rs      AudioSink, AudioCue { asset, gain, pan, priority, cooldown_ms }
//! src/game.rs       GamePresentation, Intent, PendingCommand
//! src/a11y.rs       A11yDescription generation helpers
//! src/widgets/      the ~20 shared widgets (buttons, cards, lists, dialogs, sheets)
//! ```

#![forbid(unsafe_code)]
