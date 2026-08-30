use glam::Vec2;
use tabula_design::Theme;

use crate::{InputEvent, RenderList, TextStyleToken};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameCtx {
    pub viewport: Vec2,
    pub dpi: f32,
    pub now_ms: u64,
    pub theme: Theme,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub size: Vec2,
    pub line_count: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError(pub String);

/// Imperative backend port for a renderer-neutral [`RenderList`].
///
/// @ai.role imperative-renderer-port
/// @ai.domain presentation.renderer
/// @ai.pure false
/// @ai.invariant backend-types-do-not-leak
/// @ai.evidence render::tests::builder_orders_draws_within_render_scopes
#[allow(clippy::doc_markdown)]
pub trait Renderer {
    fn begin_frame(&mut self, viewport: Vec2, dpi: f32, now_ms: u64, theme: Theme) -> FrameCtx;
    fn submit(&mut self, list: &RenderList) -> Result<(), RenderError>;
    fn end_frame(&mut self) -> Result<(), RenderError>;
    fn measure_text(
        &self,
        text: &str,
        style: TextStyleToken,
        max_width: Option<f32>,
    ) -> TextMetrics;
    fn drain_input(&mut self) -> Vec<InputEvent>;
}
