use glam::Vec2;
use tabula_design::Theme;

use crate::{InputEvent, RenderList, TextStyleToken};

/// A finite, non-empty logical viewport supplied by a renderer backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport(Vec2);

impl Viewport {
    pub fn new(size: Vec2) -> Result<Self, FrameCtxError> {
        if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            return Err(FrameCtxError::InvalidViewport);
        }
        Ok(Self(size))
    }

    #[must_use]
    pub const fn size(self) -> Vec2 {
        self.0
    }
}

/// A finite, strictly positive display density supplied by a renderer backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dpi(f32);

impl Dpi {
    pub fn new(value: f32) -> Result<Self, FrameCtxError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(FrameCtxError::InvalidDpi);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// Validated per-frame presentation facts. It is not canonical game state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameCtx {
    viewport: Viewport,
    dpi: Dpi,
    now_ms: u64,
    theme: Theme,
}

impl FrameCtx {
    #[must_use]
    pub const fn new(viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> Self {
        Self {
            viewport,
            dpi,
            now_ms,
            theme,
        }
    }

    #[must_use]
    pub const fn viewport(self) -> Viewport {
        self.viewport
    }

    #[must_use]
    pub const fn dpi(self) -> Dpi {
        self.dpi
    }

    #[must_use]
    pub const fn now_ms(self) -> u64 {
        self.now_ms
    }

    #[must_use]
    pub const fn theme(self) -> Theme {
        self.theme
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameCtxError {
    InvalidViewport,
    InvalidDpi,
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
/// Only normalized presentation values cross this boundary; framework handles and APIs stay in
/// concrete backend crates. `begin_frame` accepts proof-bearing display facts, so a presenter
/// cannot observe an invalid viewport or density.
///
/// @ai.role imperative-renderer-port
/// @ai.domain presentation.renderer
/// @ai.pure false
/// @ai.invariant backend-types-do-not-leak
/// @ai.evidence renderer::tests::frame_context_rejects_invalid_display_facts
#[allow(clippy::doc_markdown)]
pub trait Renderer {
    fn begin_frame(&mut self, viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> FrameCtx;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_context_rejects_invalid_display_facts() {
        assert_eq!(
            Viewport::new(Vec2::new(0.0, 1.0)),
            Err(FrameCtxError::InvalidViewport)
        );
        assert_eq!(
            Viewport::new(Vec2::splat(f32::NAN)),
            Err(FrameCtxError::InvalidViewport)
        );
        assert_eq!(Dpi::new(0.0), Err(FrameCtxError::InvalidDpi));
        assert_eq!(Dpi::new(f32::INFINITY), Err(FrameCtxError::InvalidDpi));
    }
}
