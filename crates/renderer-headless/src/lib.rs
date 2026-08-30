//! GPU-free implementation of the presentation renderer contract. (doc 04 §6.1)
//!
//! The recorder is the primary test backend. The intentionally small CPU
//! rasterizer covers solid rectangles and borders, enough for deterministic
//! representative image fixtures without pretending to be a software GPU.

#![forbid(unsafe_code)]

use tabula_design::{Color, Theme};
use tabula_presentation::{
    FrameCtx, InputEvent, Paint, Rect, RenderCmd, RenderError, RenderList, Renderer, TextMetrics,
    TextStyleToken,
};

#[derive(Debug, Default)]
pub struct HeadlessRenderer {
    submitted: Vec<RenderList>,
    input: Vec<InputEvent>,
}

impl HeadlessRenderer {
    #[must_use]
    pub fn submitted(&self) -> &[RenderList] {
        &self.submitted
    }
    pub fn queue_input(&mut self, event: InputEvent) {
        self.input.push(event);
    }
    pub fn rasterize(
        &self,
        list: &RenderList,
        width: u32,
        height: u32,
        background: Color,
    ) -> Result<RasterImage, RasterError> {
        let mut pixmap =
            tiny_skia::Pixmap::new(width, height).ok_or(RasterError::InvalidDimensions)?;
        pixmap.fill(to_skia(background));
        for command in list.commands() {
            if let RenderCmd::Rect {
                rect,
                fill: Some(Paint::Solid(color)),
                border,
                ..
            } = command
            {
                fill_rect(&mut pixmap, *rect, *color);
                if let Some(border) = border {
                    stroke_rect(&mut pixmap, *rect, border.width, border.color);
                }
            }
        }
        Ok(RasterImage {
            width,
            height,
            rgba: pixmap.data().to_vec(),
        })
    }
}

impl Renderer for HeadlessRenderer {
    fn begin_frame(
        &mut self,
        viewport: glam::Vec2,
        dpi: f32,
        now_ms: u64,
        theme: Theme,
    ) -> FrameCtx {
        FrameCtx {
            viewport,
            dpi,
            now_ms,
            theme,
        }
    }
    fn submit(&mut self, list: &RenderList) -> Result<(), RenderError> {
        self.submitted.push(list.clone());
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), RenderError> {
        Ok(())
    }
    #[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
    fn measure_text(
        &self,
        text: &str,
        _style: TextStyleToken,
        max_width: Option<f32>,
    ) -> TextMetrics {
        let natural = text.chars().count() as f32 * 8.0;
        let width = max_width.map_or(natural, |max| natural.min(max));
        TextMetrics {
            size: glam::vec2(width, 16.0),
            line_count: 1,
        }
    }
    fn drain_input(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.input)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterError {
    InvalidDimensions,
}
impl RasterImage {
    #[must_use]
    pub fn checksum(&self) -> u64 {
        self.rgba.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
}

fn to_skia(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.red(), color.green(), color.blue(), color.alpha())
}
fn fill_rect(pixmap: &mut tiny_skia::Pixmap, rect: Rect, color: Color) {
    let Some(rect) = tiny_skia::Rect::from_xywh(
        rect.origin().x,
        rect.origin().y,
        rect.size().x,
        rect.size().y,
    ) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(to_skia(color));
    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
}
#[allow(clippy::float_arithmetic)]
fn stroke_rect(pixmap: &mut tiny_skia::Pixmap, rect: Rect, width: f32, color: Color) {
    // tiny-skia does not expose a rectangle stroke helper. Four filled strips
    // are deterministic and sufficient for the contract's simple border.
    let horizontal = width.min(rect.size().y / 2.0);
    let vertical = width.min(rect.size().x / 2.0);
    for (origin, size) in [
        (rect.origin(), glam::vec2(rect.size().x, horizontal)),
        (
            glam::vec2(
                rect.origin().x,
                rect.origin().y + rect.size().y - horizontal,
            ),
            glam::vec2(rect.size().x, horizontal),
        ),
        (rect.origin(), glam::vec2(vertical, rect.size().y)),
        (
            glam::vec2(rect.origin().x + rect.size().x - vertical, rect.origin().y),
            glam::vec2(vertical, rect.size().y),
        ),
    ] {
        fill_rect(
            pixmap,
            Rect::new(origin, size).expect("derived border is valid"),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use tabula_presentation::{Camera2D, Corners, Layer, RenderListBuilder};

    #[test]
    fn recorder_consumes_the_backend_neutral_contract() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::Rect {
                rect: Rect::new(Vec2::ZERO, Vec2::splat(8.0)).unwrap(),
                radii: Corners::uniform(0.0),
                fill: Some(Paint::Solid(Color::rgb(255, 255, 255))),
                border: None,
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        let list = builder.finish().unwrap();
        let mut renderer = HeadlessRenderer::default();
        renderer.submit(&list).unwrap();
        assert_eq!(renderer.submitted(), &[list]);
    }

    #[test]
    fn solid_rect_rasterization_is_deterministic() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::Rect {
                rect: Rect::new(Vec2::splat(1.0), Vec2::splat(2.0)).unwrap(),
                radii: Corners::uniform(0.0),
                fill: Some(Paint::Solid(Color::rgb(255, 0, 0))),
                border: None,
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        let list = builder.finish().unwrap();
        let renderer = HeadlessRenderer::default();
        assert_eq!(
            renderer
                .rasterize(&list, 4, 4, Color::rgb(0, 0, 0))
                .unwrap()
                .checksum(),
            renderer
                .rasterize(&list, 4, 4, Color::rgb(0, 0, 0))
                .unwrap()
                .checksum()
        );
    }

    #[test]
    fn rasterization_rejects_zero_sized_images() {
        let list = RenderListBuilder::new(Camera2D::default())
            .finish()
            .unwrap();
        let renderer = HeadlessRenderer::default();
        for (width, height) in [(0, 1), (1, 0), (0, 0)] {
            assert_eq!(
                renderer.rasterize(&list, width, height, Color::rgb(0, 0, 0)),
                Err(RasterError::InvalidDimensions)
            );
        }
    }
}
