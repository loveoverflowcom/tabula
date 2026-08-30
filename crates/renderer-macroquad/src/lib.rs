//! # `renderer-macroquad` — the first `Renderer` backend
//!
//! > ## PHASE 2
//!
//! Macroquad is **#7 on the value list** (doc 00 §1.1). If someone describes
//! Tabula as "the Macroquad thing", the architecture has been misunderstood.
//! This crate is the designated *replaceable* component: a future
//! `renderer-wgpu` slots in with no changes above it (ADR-010).
//!
//! Forbidden here: any game crate, `tabula-protocol`, `tokio`.
//!
//! ## Responsibilities
//!
//! - Execute a `RenderList` with Macroquad.
//! - Texture/font/atlas management; `load_pack` → `PackHandles`.
//! - Map Macroquad input to `InputEvent` — normalise touch ids, pen pressure,
//!   and (later) gamepad, so nothing above this layer sees platform quirks.
//! - Window/canvas lifecycle and frame pacing.
//! - Implement `AudioSink`.
//!
//! ## What stays Macroquad-specific on purpose (doc 04 §5.3)
//!
//! Anything the command set does not name. The `RenderList` is deliberately
//! minimal; capability that lives only here is capability we can drop when we
//! swap backends.
//!
//! ## The migration triggers — write them down before you need them
//!
//! | Move to | When |
//! |---|---|
//! | `miniquad` (one layer down) | Macroquad blocks needed control: render targets, custom pipelines, text shaping, input edge cases |
//! | `wgpu` | Miniquad blocks us. Not before. |
//!
//! Text shaping is the most likely ceiling (doc 01 §1.3), which is why all text
//! goes through `RenderCmd::Text` with a `TextStyleToken`: swapping to
//! `cosmic-text` becomes a change in this crate, not in every game.
//!
//! Macroquad's practical ceiling is an **EXPERIMENT** to be settled in Phases 2
//! and 6 (doc 09 §3.2). Record what blocks you as you hit it.
//!
//! ## Audio
//!
//! Macroquad's audio for the MVP, behind `AudioSink`. Buses: `sfx`, `ui`,
//! `music`, `voice-duck` — voice active ducks `music` 12 dB and `sfx` 6 dB. If
//! Macroquad cannot do buses, adopt `kira` **inside this crate**; nothing above
//! the trait changes (doc 01 §1.3).
//!
//! ## Build note
//!
//! The `macroquad` dependency is commented out in `Cargo.toml` until Phase 2, so
//! a Phase-0/1 checkout builds without graphics system libraries. Uncomment it
//! together with the first `impl Renderer`.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/renderer.rs  impl Renderer for MacroquadRenderer
//! src/atlas.rs     texture + atlas management, @1x/@2x/@3x selection from FrameCtx.dpi
//! src/text.rs      font loading, measure_text with a metrics cache
//! src/input.rs     Macroquad input -> InputEvent normalization
//! src/audio.rs     impl AudioSink
//! src/window.rs    window/canvas lifecycle, frame pacing, safe-area insets
//! ```

#![forbid(unsafe_code)]
