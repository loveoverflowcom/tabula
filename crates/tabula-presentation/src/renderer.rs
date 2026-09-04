use glam::Vec2;
use tabula_design::{Positive, Theme};

use crate::{InputEvent, RenderCmdKind, RenderList, TextStyleToken};

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
    size: Vec2,
    line_count: u16,
}

impl TextMetrics {
    /// Constructs metrics only for finite non-negative extents and at least one line.
    ///
    /// @ai.role proof-constructor
    /// @ai.domain presentation.text-measurement
    /// @ai.invariant valid-text-metrics
    /// @ai.evidence tests::text_metrics_reject_invalid_output_facts
    #[allow(clippy::doc_markdown)]
    pub fn new(size: Vec2, line_count: u16) -> Result<Self, TextMetricsError> {
        if !size.is_finite() || size.x < 0.0 || size.y < 0.0 {
            return Err(TextMetricsError::InvalidSize);
        }
        if line_count == 0 {
            return Err(TextMetricsError::InvalidLineCount);
        }
        Ok(Self { size, line_count })
    }

    #[must_use]
    pub const fn size(self) -> Vec2 {
        self.size
    }

    #[must_use]
    pub const fn line_count(self) -> u16 {
        self.line_count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMetricsError {
    InvalidSize,
    InvalidLineCount,
}

impl core::fmt::Display for TextMetricsError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSize => "text metrics size must be finite and non-negative",
            Self::InvalidLineCount => "text metrics must contain at least one line",
        })
    }
}

impl std::error::Error for TextMetricsError {}

/// Why a renderer could not accept or finish a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// A frame operation was requested outside the `begin_frame`/`end_frame` lifecycle.
    InvalidLifecycle,
    /// The list is structurally valid, but this backend does not support one of its commands.
    Unsupported(RenderCmdKind),
    /// The backend could not execute an otherwise supported operation.
    Execution(String),
}

/// Imperative backend port for a renderer-neutral [`RenderList`].
///
/// Only normalized presentation values cross this boundary; framework handles and APIs stay in
/// concrete backend crates. `begin_frame` accepts proof-bearing display facts, so a presenter
/// cannot observe an invalid viewport or density.
///
/// @ai.role imperative-renderer-port
/// @ai.domain presentation.renderer
/// @ai.pure false
#[allow(clippy::doc_markdown)]
pub trait Renderer {
    fn begin_frame(&mut self, viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> FrameCtx;
    fn submit(&mut self, list: &RenderList) -> Result<(), RenderError>;
    fn end_frame(&mut self) -> Result<(), RenderError>;
    fn measure_text(
        &self,
        text: &str,
        style: TextStyleToken,
        max_width: Option<Positive>,
    ) -> Result<TextMetrics, RenderError>;
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

    #[test]
    fn text_metrics_reject_invalid_output_facts() {
        assert_eq!(
            TextMetrics::new(Vec2::new(-1.0, 1.0), 1),
            Err(TextMetricsError::InvalidSize)
        );
        assert_eq!(
            TextMetrics::new(Vec2::new(f32::NAN, 1.0), 1),
            Err(TextMetricsError::InvalidSize)
        );
        assert_eq!(
            TextMetrics::new(Vec2::ONE, 0),
            Err(TextMetricsError::InvalidLineCount)
        );
        assert!(TextMetrics::new(Vec2::ZERO, 1).is_ok());
    }
}
