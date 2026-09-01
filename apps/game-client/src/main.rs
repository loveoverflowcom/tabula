//! Macroquad platform entry point for the renderer smoke scene, not Chess UI.

use glam::Vec2;
use macroquad::prelude as mq;
use renderer_macroquad::MacroquadRenderer;
use tabula_design::{Theme, ThemeKind};
use tabula_presentation::{Dpi, InputEvent, PointerPhase, Renderer, Viewport};

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: String::from("Tabula renderer smoke"),
        window_width: 720,
        window_height: 460,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut renderer = MacroquadRenderer::new();
    let theme = Theme::by_kind(ThemeKind::Light);
    let mut pointer = None;

    loop {
        let viewport = Viewport::new(Vec2::new(mq::screen_width(), mq::screen_height()))
            .expect("Macroquad supplies a finite non-empty viewport");
        let dpi =
            Dpi::new(mq::screen_dpi_scale()).expect("Macroquad supplies a positive DPI scale");
        let now_ms = presentation_now_ms();
        let _frame = renderer.begin_frame(viewport, dpi, now_ms, theme);
        for event in renderer.drain_input() {
            if let InputEvent::Pointer {
                position,
                phase: PointerPhase::Down | PointerPhase::Move | PointerPhase::Up,
                ..
            } = event
            {
                pointer = Some(position.get());
            }
        }
        let scene = tabula_game_client::smoke_scene(theme, pointer)
            .expect("the static smoke scene satisfies RenderList construction invariants");
        if let Err(error) = renderer.submit(&scene) {
            eprintln!("renderer smoke scene failed: {error:?}");
            break;
        }
        renderer
            .end_frame()
            .expect("Macroquad end_frame is infallible");
        mq::next_frame().await;
    }
}

/// Converts Macroquad's monotonic presentation clock into the renderer frame fact.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn presentation_now_ms() -> u64 {
    (mq::get_time() * 1_000.0) as u64
}
