use macroquad::prelude as mq;
use tabula_design::{Positive, Theme};
use tabula_presentation::{
    Dpi, FrameCtx, InputEvent, RenderError, RenderList, Renderer, TextMetrics, TextStyleToken,
    Viewport,
};

use crate::{draw, input::InputState, state, support, text};

/// Replaceable Macroquad backend for a renderer-neutral [`RenderList`].
///
/// The outer Macroquad application owns `next_frame().await`; [`Renderer`] methods remain a
/// synchronous single-frame path, as required by doc 04 §5.1.
///
/// @ai.role imperative-renderer-adapter
/// @ai.domain presentation.backend.macroquad
/// @ai.pure false
/// @ai.invariant macroquad-types-do-not-cross-renderer-port
/// @ai.evidence state::tests::nested_scopes_compose_and_pop_restores_the_exact_parent_state
#[allow(clippy::doc_markdown)]
#[derive(Debug, Default)]
pub struct MacroquadRenderer {
    input: InputState,
    frame: Option<FrameCtx>,
}

impl MacroquadRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Purely checks whether `list` is supported by this backend for `frame`.
    ///
    /// This performs no Macroquad drawing calls. [`Renderer::submit`] runs the
    /// same check before executing its first primitive.
    ///
    /// @ai.role backend-preflight
    /// @ai.domain presentation.backend.macroquad
    /// @ai.pure true
    /// @ai.invariant unsupported-list-rejected-before-draw
    /// @ai.evidence support::tests::sprite_is_rejected_before_execution
    /// @ai.evidence support::tests::supported_nested_scopes_pass_with_their_effective_state
    #[allow(clippy::doc_markdown)]
    pub fn preflight(list: &RenderList, frame: &FrameCtx) -> Result<(), RenderError> {
        support::preflight(list, frame)
    }
}

impl Renderer for MacroquadRenderer {
    fn begin_frame(&mut self, viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> FrameCtx {
        let frame = FrameCtx::new(viewport, dpi, now_ms, theme);
        mq::clear_background(mq::Color::from_rgba(
            theme.color.surface.red(),
            theme.color.surface.green(),
            theme.color.surface.blue(),
            theme.color.surface.alpha(),
        ));
        self.frame = Some(frame);
        frame
    }

    fn submit(&mut self, list: &RenderList) -> Result<(), RenderError> {
        let frame = self.frame.ok_or(RenderError::InvalidLifecycle)?;
        Self::preflight(list, &frame)?;
        let mut result = Ok(());
        state::visit_draws(list, |command, draw_state| {
            if result.is_ok() {
                result = draw::execute(command, draw_state, list.camera(), &frame);
            }
        });
        result
    }

    fn end_frame(&mut self) -> Result<(), RenderError> {
        if self.frame.is_none() {
            return Err(RenderError::InvalidLifecycle);
        }
        mq::set_default_camera();
        self.frame = None;
        Ok(())
    }

    fn measure_text(
        &self,
        value: &str,
        style: TextStyleToken,
        max_width: Option<Positive>,
    ) -> Result<TextMetrics, RenderError> {
        let theme = self.frame.map_or_else(
            || Theme::by_kind(tabula_design::ThemeKind::Light),
            FrameCtx::theme,
        );
        text::measure(value, theme.text_style(style), max_width)
    }

    fn drain_input(&mut self) -> Vec<InputEvent> {
        self.input.drain()
    }
}

#[cfg(test)]
mod tests {
    use tabula_presentation::{Camera2D, RenderError, RenderListBuilder, Renderer};

    use super::MacroquadRenderer;

    #[test]
    fn frame_operations_report_invalid_lifecycle_without_drawing() {
        let list = RenderListBuilder::new(Camera2D::default())
            .finish()
            .expect("empty render list is structurally valid");
        let mut renderer = MacroquadRenderer::new();

        assert_eq!(renderer.submit(&list), Err(RenderError::InvalidLifecycle));
        assert_eq!(renderer.end_frame(), Err(RenderError::InvalidLifecycle));
    }
}
