//! Desktop entry point.
//!
//! Phase 2 makes this open a Macroquad window and run a hot-seat chess match.
//! Until then it exists so the binary target is real and CI has something to
//! build.
//!
//! ```rust,ignore
//! #[macroquad::main(window_conf)]
//! async fn main() {
//!     let mut renderer = renderer_macroquad::MacroquadRenderer::new();
//!     let mut scenes   = tabula_game_client::scene::Stack::new(Scene::Loader);
//!     loop {
//!         let frame = renderer.begin_frame(screen_size(), dpi());
//!         for ev in renderer.drain_input() { scenes.on_input(&ev); }
//!         scenes.update(&frame);
//!         renderer.submit(&scenes.present(&frame));
//!         renderer.end_frame();
//!         macroquad::window::next_frame().await;
//!     }
//! }
//! ```

fn main() {
    eprintln!(
        "tabula-game-client is a Phase 2 deliverable (docs/architecture/07-phases-and-implementation-roadmap.md).\n\
         Gate: the Phase 1 exit criteria are met (see doc 07)."
    );
    std::process::exit(1);
}
