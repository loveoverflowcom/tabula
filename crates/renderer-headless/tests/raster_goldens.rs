//! Integration tests for CPU rasterizer golden images. (Phase 2, doc 04 §6.1)
//!
//! These tests verify that the headless CPU rasterizer executes its claimed subset
//! (solid square rectangles, borders, logical scissors, transforms, opacity, camera, DPI)
//! accurately against committed PNG goldens without silently dropping unsupported commands.

#![forbid(unsafe_code)]

use glam::{Affine2, Vec2};
use renderer_headless::{compare_raster, HeadlessRenderer, RasterImage, RasterTolerance};
use std::path::Path;
use tabula_design::{Color, Theme, ThemeKind};
use tabula_presentation::{
    Border, Camera2D, Corners, Dpi, FrameCtx, Layer, Opacity, Paint, Rect, RenderCmd,
    RenderListBuilder, Viewport,
};

const GOLDENS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens");

fn build_subset_scene_64x64() -> (tabula_presentation::RenderList, FrameCtx, Color) {
    let frame = FrameCtx::new(
        Viewport::new(Vec2::new(64.0, 64.0)).unwrap(),
        Dpi::new(1.0).unwrap(),
        0,
        Theme::by_kind(ThemeKind::Light),
    );
    let background = Color::rgb(30, 30, 35);

    let mut builder = RenderListBuilder::new(Camera2D::default());

    // 1. Base board background rect with border
    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(4.0, 4.0), Vec2::new(56.0, 56.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(60, 60, 70))),
            border: Some(Border::new(2.0, Color::rgb(100, 100, 120)).unwrap()),
            layer: Layer::BOARD,
            z: 0,
        })
        .unwrap();

    // 2. Scoped clip + transform (clipped orange rect)
    builder
        .push(RenderCmd::PushClip {
            rect: Rect::new(Vec2::new(8.0, 8.0), Vec2::new(24.0, 24.0)).unwrap(),
            layer: Layer::BOARD,
            z: 1,
        })
        .unwrap();
    builder
        .push(RenderCmd::PushTransform {
            matrix: Affine2::from_translation(Vec2::new(4.0, 4.0)),
            layer: Layer::BOARD,
            z: 1,
        })
        .unwrap();
    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::ZERO, Vec2::new(32.0, 32.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(220, 80, 50))),
            border: None,
            layer: Layer::BOARD,
            z: 1,
        })
        .unwrap();
    builder
        .push(RenderCmd::PopTransform {
            layer: Layer::BOARD,
            z: 1,
        })
        .unwrap();
    builder
        .push(RenderCmd::PopClip {
            layer: Layer::BOARD,
            z: 1,
        })
        .unwrap();

    // 3. Scoped inherited opacity (overlapping semi-transparent cyan and yellow rects)
    builder
        .push(RenderCmd::PushOpacity {
            opacity: Opacity::try_from(0.6).unwrap(),
            layer: Layer::PIECES,
            z: 10,
        })
        .unwrap();
    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(36.0, 8.0), Vec2::new(20.0, 20.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(50, 180, 220))),
            border: None,
            layer: Layer::PIECES,
            z: 10,
        })
        .unwrap();
    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(44.0, 16.0), Vec2::new(16.0, 16.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(240, 200, 50))),
            border: None,
            layer: Layer::PIECES,
            z: 11,
        })
        .unwrap();
    builder
        .push(RenderCmd::PopOpacity {
            layer: Layer::PIECES,
            z: 10,
        })
        .unwrap();

    // 4. Scope restoration check (draw after scopes have closed)
    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(8.0, 36.0), Vec2::new(48.0, 20.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(80, 160, 90))),
            border: Some(Border::new(1.0, Color::rgb(255, 255, 255)).unwrap()),
            layer: Layer::OVERLAY,
            z: 5,
        })
        .unwrap();

    let list = builder.finish().unwrap();
    (list, frame, background)
}

fn build_dpi_camera_scene() -> (tabula_presentation::RenderList, FrameCtx, Color) {
    // Logical 32x32 at DPI 2.0 = device 64x64
    let frame = FrameCtx::new(
        Viewport::new(Vec2::new(32.0, 32.0)).unwrap(),
        Dpi::new(2.0).unwrap(),
        0,
        Theme::by_kind(ThemeKind::Light),
    );
    let background = Color::rgb(20, 20, 25);

    // Camera origin at (2.0, 2.0) with zoom 1.0
    let camera = Camera2D::new(Vec2::new(2.0, 2.0), 1.0).unwrap();
    let mut builder = RenderListBuilder::new(camera);

    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(2.0, 2.0), Vec2::new(28.0, 28.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(70, 90, 120))),
            border: Some(Border::new(1.0, Color::rgb(200, 220, 255)).unwrap()),
            layer: Layer::BOARD,
            z: 0,
        })
        .unwrap();

    builder
        .push(RenderCmd::Rect {
            rect: Rect::new(Vec2::new(8.0, 8.0), Vec2::new(16.0, 16.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(Color::rgb(230, 100, 80))),
            border: None,
            layer: Layer::PIECES,
            z: 1,
        })
        .unwrap();

    let list = builder.finish().unwrap();
    (list, frame, background)
}

#[test]
fn supported_subset_matches_committed_pixel_golden() {
    let (list, frame, background) = build_subset_scene_64x64();
    let renderer = HeadlessRenderer::default();
    let actual = renderer
        .rasterize(&list, frame, background)
        .expect("rasterization of supported subset succeeds");

    assert_eq!((actual.width, actual.height), (64, 64));

    let golden_path = Path::new(GOLDENS_DIR).join("supported_subset_64x64.png");
    let golden_bytes = std::fs::read(&golden_path).unwrap_or_else(|err| {
        panic!(
            "failed to read golden image at {}: {err}",
            golden_path.display()
        );
    });
    let expected = RasterImage::from_png_bytes(&golden_bytes)
        .expect("decoding committed golden image succeeds");

    let diff = compare_raster(&expected, &actual, RasterTolerance::EXACT)
        .expect("raster output must match committed golden image exactly");
    assert_eq!(diff.different_pixels, 0);
    assert_eq!(diff.max_channel_delta, 0);
}

#[test]
fn dpi_and_camera_scene_matches_committed_pixel_golden() {
    let (list, frame, background) = build_dpi_camera_scene();
    let renderer = HeadlessRenderer::default();
    let actual = renderer
        .rasterize(&list, frame, background)
        .expect("rasterization of DPI camera scene succeeds");

    assert_eq!((actual.width, actual.height), (64, 64));

    let golden_path = Path::new(GOLDENS_DIR).join("dpi_camera_subset_64x64.png");
    let golden_bytes = std::fs::read(&golden_path).unwrap_or_else(|err| {
        panic!(
            "failed to read golden image at {}: {err}",
            golden_path.display()
        );
    });
    let expected = RasterImage::from_png_bytes(&golden_bytes)
        .expect("decoding committed golden image succeeds");

    let diff = compare_raster(&expected, &actual, RasterTolerance::EXACT)
        .expect("dpi camera raster output must match committed golden image exactly");
    assert_eq!(diff.different_pixels, 0);
    assert_eq!(diff.max_channel_delta, 0);
}

#[test]
fn deliberate_pixel_corruption_fails_golden_comparison() {
    let (list, frame, background) = build_subset_scene_64x64();
    let renderer = HeadlessRenderer::default();
    let mut corrupted = renderer
        .rasterize(&list, frame, background)
        .expect("rasterization succeeds");

    // Corrupt one pixel by changing its red channel by 50
    let offset = 0;
    corrupted.rgba[offset] = corrupted.rgba[offset].wrapping_add(50);

    let golden_path = Path::new(GOLDENS_DIR).join("supported_subset_64x64.png");
    let golden_bytes = std::fs::read(&golden_path).expect("golden image exists");
    let expected = RasterImage::from_png_bytes(&golden_bytes).expect("decodes golden");

    assert!(
        compare_raster(&expected, &corrupted, RasterTolerance::EXACT).is_err(),
        "corrupted image must fail exact golden comparison"
    );
}

/// Explicit opt-in regeneration helper for golden image fixtures.
///
/// Run with: `cargo test -p renderer-headless --test raster_goldens -- --ignored regenerate_goldens`
#[test]
#[ignore = "only run explicitly to regenerate committed golden image fixtures"]
fn regenerate_goldens() {
    let dir = Path::new(GOLDENS_DIR);
    std::fs::create_dir_all(dir).expect("create goldens directory");

    let renderer = HeadlessRenderer::default();

    // 1. supported_subset_64x64.png
    let (list, frame, background) = build_subset_scene_64x64();
    let img = renderer.rasterize(&list, frame, background).unwrap();
    let png = img.encode_png().unwrap();
    std::fs::write(dir.join("supported_subset_64x64.png"), png).unwrap();

    // 2. dpi_camera_subset_64x64.png
    let (list, frame, background) = build_dpi_camera_scene();
    let img = renderer.rasterize(&list, frame, background).unwrap();
    let png = img.encode_png().unwrap();
    std::fs::write(dir.join("dpi_camera_subset_64x64.png"), png).unwrap();
}
