//! # `tabula-game-client` — the Macroquad gameplay runtime
//!
//! > ## PHASE 2 (hot-seat) → PHASE 4 (online) → PHASE 6 (mobile)
//!
//! **One codebase, four platforms.** Desktop, Android, iOS, and web-at-`/play/:id`
//! all run this crate; only the shell around it differs.
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
//! ## Module layout when this becomes real
//!
//! ```text
//! src/lib.rs        the runtime, shared by every platform entry point
//! src/main.rs       desktop entry point
//! src/scene/
//!   mod.rs          the scene stack (native has no navigation — it swaps scenes)
//!   loader.rs       branded loader with real asset progress
//!   match_.rs       the in-match scene: HUD, chat overlay, clocks, result summary
//!   shell.rs        native lobby/catalog screens, drawn with tabula-presentation
//! src/hotseat.rs    Phase 2: local two-player driver, no server involved
//! src/online.rs     Phase 4: MatchClient wiring, connection-state UI
//! src/context.rs    MatchContext handoff struct + deep-link parsing
//! src/platform/
//!   web.rs          #[cfg(target_arch = "wasm32")] boot from sessionStorage
//!   native.rs       window setup, config dirs
//!   android.rs      Phase 6: cdylib entry, lifecycle events into net-client
//!   ios.rs          Phase 6: staticlib entry
//! ```

#![forbid(unsafe_code)]
