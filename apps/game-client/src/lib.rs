//! # `tabula-game-client` — the Macroquad gameplay runtime
//!
//! > ## Phase 2 rendering smoke slice
//!
//! **One codebase, four platforms.** Desktop, Android, iOS, and web-at-`/play/:id`
//! will all run this crate; only the renderer smoke scene is real in this focused first slice.
//! Chess presentation, local hot-seat play, and every network concern remain deliberately deferred.
//!
//! ## I-15, the invariant that defines this crate
//!
//! **`leptos` must never appear in this dependency graph** — native or WASM.
//! Gameplay does not live in a DOM runtime (ADR-011), and it does not live in a
//! `WebView` (ADR-019). `xtask check-deps` enforces it.
//!
//! ## The frame loop (doc 04 §4.1, §5.1)
//!
//! ```text
//! renderer.drain_input()  ─→ Presenter::on_input ─→ Intent<Command>
//!                                                    ─→ MatchClient::send_command
//! MatchClient::poll()     ─→ drained ONCE PER FRAME
//!                            Welcome/Resync → replace view
//!                            ViewEvents     → on_view_event → animations
//!                            Ack/Reject     → resolve or discard the preview
//! theme()                 ─→ resolved once per frame
//! present(view, local, frame) ─→ RenderList (rebuilt fully, sorted by layer,z)
//! renderer.submit(&list)
//! ```
//!
//! Synchronous, single-threaded, **no `async` in the presentation path, no
//! locks**. That is what `poll()` returning an iterator buys.
//!
//! ## Web: two bundles, not one (doc 04 §3.1, ADR-011)
//!
//! ```text
//! /                → app.wasm   (Leptos shell,  target ~1.5–2.5 MB gz)
//! /play/:match_id  → game.wasm  (this crate,    target ~4–6 MB gz)
//! ```
//!
//! `/play/:id` is a **separate document** — a real navigation, not a client-side
//! route into a canvas. Two runtimes fighting over the canvas, the DOM, and the
//! event loop is a problem we decline to have. Two bundles also means two
//! independent caches: a shell deploy does not invalidate the game.
//!
//! Hard size cap: **< 6 MB gzipped** including one game's code, excluding assets.
//! CI fails on a >10% regression (doc 01 §7).
//!
//! ## The handoff (doc 04 §3.4)
//!
//! ```text
//! shell:  POST /matches → { match_id, join_token }
//! shell:  sessionStorage["match.ctx"] = { match_id, join_token, game_id@version, pack }
//! shell:  prefetch game.wasm + pack manifest (link rel=prefetch) DURING the room screen
//! shell:  navigate to /play/:match_id
//! game:   read match.ctx → branded loader with REAL byte-level progress
//! game:   WS Hello + Attach(join_token) → Welcome { view, capabilities }
//!         ... play ...
//! game:   in-canvas result summary + "Rematch" / "Back to lobby"
//! game:   navigate to /matches/:id or /rooms/:id
//! ```
//!
//! Back/forward and deep links must work; re-entering `/play/:id` resumes.
//!
//! **Native has no navigation — it swaps a scene.** The same `MatchContext`
//! struct is passed in-process, so the runtime code is identical on every
//! platform.
//!
//! ## WASM constraints that shape the design (doc 01 §7)
//!
//! Do not rediscover these in Phase 5:
//!
//! - **No threads by default.** Nothing in shared client code may use
//!   `std::thread`.
//! - **No blocking I/O.** All network access is event-driven — hence
//!   `tabula-net-client`'s two backends.
//! - `Instant::now()` works via a `performance.now()` shim, but it is banned in
//!   rules anyway (I-3). Presentation uses the renderer's frame time.
//!
//! ## Current entry point and future layout
//!
//! ```text
//! src/lib.rs        the renderer-neutral visual smoke scene
//! src/main.rs       native/WASM Macroquad entry point
//! src/scene/
//!   mod.rs          the scene stack (native has no navigation — it swaps scenes)
//!   loader.rs       branded loader with real asset progress
//!   match_.rs       the in-match scene: HUD, chat overlay, clocks, result summary
//!   shell.rs        native lobby/catalog screens, drawn with tabula-presentation
//! src/hotseat.rs    deferred: local two-player driver, no server involved
//! src/online.rs     Phase 4: MatchClient wiring, connection-state UI
//! src/context.rs    MatchContext handoff struct + deep-link parsing
//! src/platform/
//!   web.rs          #[cfg(target_arch = "wasm32")] boot from sessionStorage
//!   native.rs       window setup, config dirs
//!   android.rs      Phase 6: cdylib entry, lifecycle events into net-client
//!   ios.rs          Phase 6: staticlib entry
//! ```

#![forbid(unsafe_code)]

use glam::{Affine2, Vec2};
use tabula_design::Theme;
use tabula_presentation::{
    Align, Border, Camera2D, Corners, Layer, Opacity, Paint, Rect, RenderCmd, RenderList,
    RenderListBuilder, RenderListError, TextStyleToken,
};

/// Builds the deterministic diagnostic scene used to manually smoke-test the renderer backend.
///
/// It exercises only the renderer-neutral command vocabulary; the executable submits the returned
/// list through `Renderer`, exactly as a future game presenter will.
#[allow(
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::too_many_lines
)]
pub fn smoke_scene(theme: Theme, pointer: Option<Vec2>) -> Result<RenderList, RenderListError> {
    let mut builder = RenderListBuilder::new(Camera2D::default());
    let square = 42.0;
    let board_origin = Vec2::new(36.0, 88.0);
    for rank in 0..8 {
        for file in 0..8 {
            let color = if (rank + file) % 2 == 0 {
                theme.color.surface_container
            } else {
                theme.color.surface_container_high
            };
            builder.push(RenderCmd::Rect {
                rect: Rect::new(
                    board_origin + Vec2::new(file as f32 * square, rank as f32 * square),
                    Vec2::splat(square),
                )?,
                radii: Corners::uniform(0.0)?,
                fill: Some(Paint::Solid(color)),
                border: None,
                layer: Layer::BOARD,
                z: i16::try_from(rank * 8 + file).expect("8 by 8 grid fits in i16"),
            })?;
        }
    }

    builder.push(RenderCmd::Text {
        text: String::from("Macroquad renderer smoke scene"),
        at: Vec2::new(36.0, 42.0),
        style: TextStyleToken::HeadlineSm,
        align: Align::Start,
        max_width: None,
        color: theme.color.on_surface,
        layer: Layer::HUD,
        z: 0,
    })?;
    builder.push(RenderCmd::Rect {
        rect: Rect::new(Vec2::new(396.0, 88.0), Vec2::new(284.0, 150.0))?,
        radii: Corners::uniform(theme.shape.lg.get())?,
        fill: Some(Paint::Solid(theme.color.surface_container)),
        border: Some(Border::new(
            theme.focus.ring_width.get(),
            theme.color.outline,
        )?),
        layer: Layer::HUD,
        z: 1,
    })?;
    builder.push(RenderCmd::Text {
        text: String::from("semantic colours\nrounded border\ntext metrics"),
        at: Vec2::new(420.0, 112.0),
        style: TextStyleToken::BodyLg,
        align: Align::Start,
        max_width: None,
        color: theme.color.on_surface_variant,
        layer: Layer::HUD,
        z: 2,
    })?;

    let scope_layer = Layer::OVERLAY;
    builder.push(RenderCmd::PushClip {
        rect: Rect::new(Vec2::new(396.0, 270.0), Vec2::new(180.0, 120.0))?,
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PushTransform {
        matrix: Affine2::from_scale_angle_translation(
            Vec2::splat(1.2),
            0.28,
            Vec2::new(434.0, 270.0),
        ),
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PushOpacity {
        opacity: Opacity::try_from(0.5).expect("literal opacity is valid"),
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PushOpacity {
        opacity: Opacity::try_from(0.5).expect("literal opacity is valid"),
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::Rect {
        rect: Rect::new(Vec2::ZERO, Vec2::new(160.0, 96.0))?,
        radii: Corners::uniform(theme.shape.md.get())?,
        fill: Some(Paint::Solid(theme.color.primary)),
        border: None,
        layer: Layer::PIECES,
        z: 0,
    })?;
    builder.push(RenderCmd::PopOpacity {
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PopOpacity {
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PopTransform {
        layer: scope_layer,
        z: 0,
    })?;
    builder.push(RenderCmd::PopClip {
        layer: scope_layer,
        z: 0,
    })?;

    let indicator = pointer.unwrap_or(Vec2::new(-12.0, -12.0));
    builder.push(RenderCmd::Rect {
        rect: Rect::new(indicator - Vec2::splat(6.0), Vec2::splat(12.0))?,
        radii: Corners::uniform(6.0)?,
        fill: Some(Paint::Solid(theme.color.danger)),
        border: Some(Border::new(2.0, theme.color.on_danger)?),
        layer: Layer::TOAST,
        z: 0,
    })?;
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_design::ThemeKind;

    #[test]
    fn smoke_scene_is_a_valid_render_list_with_every_state_scope_balanced() {
        let scene =
            smoke_scene(Theme::by_kind(ThemeKind::HighContrastDark), Some(Vec2::ONE)).unwrap();
        assert!(scene
            .commands()
            .iter()
            .any(|command| matches!(command, RenderCmd::Text { .. })));
        assert!(scene
            .commands()
            .iter()
            .any(|command| matches!(command, RenderCmd::PushClip { .. })));
        assert!(scene
            .commands()
            .iter()
            .any(|command| matches!(command, RenderCmd::PushTransform { .. })));
        assert!(scene
            .commands()
            .iter()
            .any(|command| matches!(command, RenderCmd::PushOpacity { .. })));
    }
}
