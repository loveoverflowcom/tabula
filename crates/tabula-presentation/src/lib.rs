//! Renderer-neutral presentation contracts. (doc 04 §5–§6)
//!
//! This crate accepts only projections, local state, frames, and normalized
//! input. It deliberately has no dependency on a rendering framework.

#![forbid(unsafe_code)]

mod focus;
mod game;
mod input;
mod motion;
mod render;
mod renderer;

pub use focus::{
    handle_navigation, FocusDirection, FocusGraph, FocusGraphError, FocusId, FocusModality,
    FocusNode, FocusState, NavigationAction,
};
pub use game::{AssetPackRef, GamePresentation, Intent};
pub use glam::{Affine2, Vec2};
pub use input::{
    InputEvent, Key, PointerButton, PointerPhase, PointerPosition, PointerPositionError,
};
pub use motion::{
    is_stale_on_arrival, lerp_f32, lerp_vec2, resolve_duration, resolve_motion_start,
    resolve_spring, staggered_start, MotionMode, MotionSample, MotionStart, MotionTimeline,
    STALE_ANIMATION_THRESHOLD_MS,
};
pub use render::{
    Align, Border, Camera2D, Corners, GradientStop, Layer, LinearGradient, Opacity, OpacityError,
    Paint, Rect, RenderCmd, RenderCmdKind, RenderList, RenderListBuilder, RenderListError,
};
pub use renderer::{
    Dpi, FrameCtx, FrameCtxError, RenderError, Renderer, TextMetrics, TextMetricsError, Viewport,
};
pub use tabula_design::{Color, Positive, TextStyleToken};
