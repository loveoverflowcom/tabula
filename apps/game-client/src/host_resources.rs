//! Shared local-host font and resource-failure boundaries. (ADR-0030/0035)
use crate::resolve_display_geometry;
use macroquad::prelude as mq;
use tabula_presentation::Renderer;
use tabula_render_macroquad::MacroquadRenderer;

/// Brand fonts use bounded external aliases on WASM and embedded bytes on native.
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
pub async fn load_builtin_fonts(renderer: &mut MacroquadRenderer) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    let (text, strong, display) = (
        include_bytes!("../../../assets/fonts/OpenSans-Regular.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/OpenSans-Semibold.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/NotoSerif-Bold.ttf").as_slice(),
    );
    #[cfg(target_arch = "wasm32")]
    let (text, strong, display) = {
        let mut fonts = Vec::with_capacity(3);
        for path in [
            "assets/OpenSans-Regular.ttf",
            "assets/OpenSans-Semibold.ttf",
            "assets/NotoSerif-Bold.ttf",
        ] {
            let bytes = mq::load_file(path)
                .await
                .map_err(|error| format!("font load {path}: {error}"))?;
            if bytes.is_empty() || bytes.len() > 256 * 1024 {
                return Err(format!("font load {path}: invalid bounded size"));
            }
            fonts.push(bytes);
        }
        let mut fonts = fonts.into_iter();
        (
            fonts.next().expect("three declared fonts"),
            fonts.next().expect("three declared fonts"),
            fonts.next().expect("three declared fonts"),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let (text, strong, display) = (text.as_slice(), strong.as_slice(), display.as_slice());
    renderer
        .set_builtin_font_bytes(text, strong, display)
        .map_err(|error| format!("built-in fonts: {error:?}"))
}

/// Asset failure never produces a false ready game or an invisible board.
pub async fn show_asset_failure(
    renderer: &mut MacroquadRenderer,
    theme: &tabula_design::Theme,
    error: &str,
) {
    show_asset_failure_impl(renderer, theme, error, false).await;
}

/// Safe recovery for a privacy-shielded local host. Acknowledges only a
/// successfully submitted, ended and flushed public recovery frame.
pub async fn show_asset_failure_with_privacy_ack(
    renderer: &mut MacroquadRenderer,
    theme: &tabula_design::Theme,
    error: &str,
) {
    show_asset_failure_impl(renderer, theme, error, true).await;
}

/// Signals that a role-independent/opaque frame has already been flushed.
/// This bounded virtual file is never a network/cache payload or game event.
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
pub async fn acknowledge_concealed_frame() {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = mq::load_file("tabula-concealed.txt").await;
    }
}

#[allow(clippy::float_arithmetic)] // Screen-space recovery geometry never enters canonical state.
async fn show_asset_failure_impl(
    renderer: &mut MacroquadRenderer,
    theme: &tabula_design::Theme,
    error: &str,
    acknowledge_privacy: bool,
) {
    use tabula_presentation::{
        ActionButton, Align, ButtonInteraction, ButtonTone, Camera2D, FocusGraph, FocusId,
        FocusNode, FocusState, Layer, NavigationAction, Rect, RenderCmd, RenderListBuilder,
        TextStyleToken,
    };
    let mut acknowledgement_needed = acknowledge_privacy;
    let mut interaction = ButtonInteraction::default();
    let mut focus = FocusState::default();
    let id = FocusId::new(0);
    focus.set_keyboard_focus(Some(id));
    for (key, native) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native) {
            interaction.suppress_activation_until_release(key);
        }
    }
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        let _frame = renderer.begin_frame(viewport, dpi, 0, *theme);
        let size = viewport.size();
        let rect = Rect::new(
            glam::Vec2::new(24.0, 220.0),
            glam::Vec2::new((size.x - 48.0).max(44.0), 48.0),
        )
        .expect("bounded recovery target");
        let button = ActionButton::new(id, rect, "Retry assets", theme.density.min_target)
            .expect("44dp recovery")
            .tone(ButtonTone::Filled);
        let graph =
            FocusGraph::new(vec![FocusNode::new(id, rect)]).expect("single recovery action");
        for event in renderer.drain_input() {
            if acknowledge_privacy && matches!(event, tabula_presentation::InputEvent::Focus(false))
            {
                acknowledgement_needed = true;
            }
            if matches!(
                interaction.on_input(&event, &[button], &graph, &mut focus),
                NavigationAction::Activate(_)
            ) {
                let _ = renderer.end_frame();
                mq::next_frame().await;
                return;
            }
        }
        let mut builder = RenderListBuilder::new(Camera2D::default());
        for (text, y, style) in [
            (
                "Local resources could not load",
                48.0,
                TextStyleToken::HeadlineMd,
            ),
            (error, 110.0, TextStyleToken::BodyMd),
        ] {
            let _ = builder.push(RenderCmd::Text {
                text: text.to_owned(),
                at: glam::Vec2::new(24.0, y),
                style,
                align: Align::Start,
                max_width: tabula_design::Positive::new((size.x - 48.0).max(1.0)).ok(),
                color: theme.color.on_surface,
                layer: Layer::HUD,
                z: 0,
            });
        }
        let _ = button.draw(&mut builder, theme, &interaction, &focus, Layer::HUD);
        let render_ok = builder
            .finish()
            .is_ok_and(|scene| renderer.submit(&scene).is_ok());
        let frame_ok = renderer.end_frame().is_ok();
        mq::next_frame().await;
        if acknowledgement_needed && render_ok && frame_ok {
            acknowledgement_needed = false;
            acknowledge_concealed_frame().await;
        }
    }
}
