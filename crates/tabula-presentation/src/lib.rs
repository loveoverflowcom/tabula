//! Renderer-neutral presentation contracts. (doc 04 §5–§6)
//!
//! This crate accepts only projections, local state, frames, and normalized
//! input. It deliberately has no dependency on a rendering framework.

#![forbid(unsafe_code)]

mod game;
mod input;
mod render;
mod renderer;

pub use game::{AssetPackRef, GamePresentation, Intent};
pub use input::{InputEvent, Key, PointerButton, PointerPhase};
pub use render::{
    Align, Border, Camera2D, Corners, GradientStop, Layer, LinearGradient, Opacity, OpacityError,
    Paint, Rect, RenderCmd, RenderList, RenderListBuilder, RenderListError, TextStyleToken,
};
pub use renderer::{Dpi, FrameCtx, FrameCtxError, RenderError, Renderer, TextMetrics, Viewport};
