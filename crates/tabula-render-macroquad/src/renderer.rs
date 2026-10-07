use macroquad::prelude as mq;
use tabula_design::{Positive, Theme};
use tabula_presentation::{
    Dpi, FrameCtx, InputEvent, RenderError, RenderList, Renderer, TextMetrics, TextStyleToken,
    Viewport,
};

use crate::{
    assets::{ResolvedSprite, SpriteAssetCache, TextureUploader},
    draw,
    input::InputState,
    state, support, text,
};

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
    frame_started: bool,
    assets: SpriteAssetCache,
    fonts: text::BuiltinFonts,
    queued: Vec<(RenderList, Vec<ResolvedSprite<mq::Texture2D>>)>,
    // End-frame queues GPU draws, while the host's next_frame performs the actual flush. Keep
    // strong handles across that await; the following begin_frame is the retirement boundary.
    submitted_sprites: Vec<ResolvedSprite<mq::Texture2D>>,
}

impl MacroquadRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads the host's bounded built-in typefaces once before any frame.
    /// Macroquad fonts never cross the renderer-neutral contract. Replacement
    /// after the first frame begins is rejected to preserve atlas lifetime.
    pub fn set_builtin_font_bytes(
        &mut self,
        text: &[u8],
        strong: &[u8],
        display: &[u8],
    ) -> Result<(), RenderError> {
        if self.frame_started || self.frame.is_some() || !self.queued.is_empty() {
            return Err(RenderError::InvalidLifecycle);
        }
        self.fonts = text::BuiltinFonts::load(text, strong, display)?;
        Ok(())
    }

    /// Checks primitive geometry and support without a graphics context or asset cache.
    ///
    /// Sprites report an explicit missing-ready-resource execution error. Use
    /// [`Self::preflight_with_assets`] to validate Sprite lists against the instance's cache.
    /// Neither method emits drawing calls.
    ///
    /// @ai.role backend-preflight
    /// @ai.domain presentation.backend.macroquad
    /// @ai.pure true
    /// @ai.invariant unsupported-list-rejected-before-draw
    /// @ai.evidence support::tests::sprite_without_ready_resource_fails_explicitly_before_execution
    /// @ai.evidence support::tests::supported_nested_scopes_pass_with_their_effective_state
    #[allow(clippy::doc_markdown)]
    pub fn preflight(list: &RenderList, frame: &FrameCtx) -> Result<(), RenderError> {
        support::preflight(list, frame)
    }

    /// Checks the complete list against this renderer's ready asset cache without drawing,
    /// decoding, or uploading. Missing and invalid resources fail before any primitive is queued.
    pub fn preflight_with_assets(
        &self,
        list: &RenderList,
        frame: &FrameCtx,
    ) -> Result<(), RenderError> {
        Self::preflight_with_cache(list, frame, &self.assets)
    }

    /// Applies the same ready-resource preflight to an explicitly supplied upload seam.
    /// A host can exercise verified-byte, decode, and resource acceptance with a test uploader
    /// without creating a graphics context; this proves acceptance, not rendered pixels.
    pub fn preflight_with_cache<U: TextureUploader>(
        list: &RenderList,
        frame: &FrameCtx,
        cache: &SpriteAssetCache<U>,
    ) -> Result<(), RenderError> {
        support::prepare(list, frame, cache).map(|_| ())
    }

    /// The host-owned asset seam for bound packs and verified image bytes. (doc 04 §12)
    #[must_use]
    pub const fn assets(&self) -> &SpriteAssetCache {
        &self.assets
    }

    /// Loads, rebinds, or releases ready resources. Submitted frames retain their own strong
    /// texture handles until the host has called `next_frame().await` and starts its next frame.
    pub fn assets_mut(&mut self) -> &mut SpriteAssetCache {
        &mut self.assets
    }
}

impl Renderer for MacroquadRenderer {
    fn begin_frame(&mut self, viewport: Viewport, dpi: Dpi, now_ms: u64, theme: Theme) -> FrameCtx {
        // The outer application's next_frame must have flushed the previous frame first.
        self.frame_started = true;
        self.submitted_sprites.clear();
        self.queued.clear();
        self.assets.collect_released();
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
        let sprites = support::prepare(list, &frame, &self.assets)?;
        self.queued.push((list.clone(), sprites));
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), RenderError> {
        let frame = self.frame.ok_or(RenderError::InvalidLifecycle)?;
        // Macroquad's glyph atlas can replace its unmanaged texture when new glyphs appear.
        // Prepare the whole accepted frame before the first primitive references that atlas.
        for (list, _) in &self.queued {
            text::prepare(list, &frame, &self.fonts)?;
        }
        let mut result = Ok(());
        for (list, sprites) in self.queued.drain(..) {
            let mut ready = sprites.iter();
            state::visit_draws(&list, |command, draw_state| {
                if result.is_ok() {
                    let sprite = if matches!(command, tabula_presentation::RenderCmd::Sprite { .. })
                    {
                        ready.next()
                    } else {
                        None
                    };
                    result = draw::execute(
                        command,
                        draw_state,
                        list.camera(),
                        &frame,
                        sprite,
                        &self.fonts,
                    );
                }
            });
            self.submitted_sprites.extend(sprites);
        }
        mq::set_default_camera();
        self.frame = None;
        result
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
        text::measure(value, theme.text_style(style), max_width, &self.fonts)
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

    #[test]
    fn failed_submit_does_not_queue_earlier_primitives() {
        use glam::Vec2;
        use tabula_design::{Theme, ThemeKind};
        use tabula_presentation::{
            AssetRef, Corners, Dpi, FrameCtx, Layer, Paint, Rect, RenderCmd, Viewport,
        };
        let theme = Theme::by_kind(ThemeKind::Light);
        let mut renderer = MacroquadRenderer::new();
        renderer.frame = Some(FrameCtx::new(
            Viewport::new(Vec2::splat(100.0)).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            theme,
        ));
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::Rect {
                rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                radii: Corners::uniform(0.0).unwrap(),
                fill: Some(Paint::Solid(theme.color.primary)),
                border: None,
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::Sprite {
                asset: AssetRef::from_static("missing/tile"),
                rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
                tint: theme.color.on_surface,
                rotation: 0.0,
                pivot: Vec2::ZERO,
                layer: Layer::PIECES,
                z: 0,
            })
            .unwrap();
        assert!(matches!(
            renderer.submit(&builder.finish().unwrap()),
            Err(RenderError::Execution(_))
        ));
        assert!(renderer.queued.is_empty());
        assert!(renderer.submitted_sprites.is_empty());
    }
    #[test]
    fn built_in_font_loading_is_bounded_before_graphics_and_cannot_replace_live_atlases() {
        let mut renderer = MacroquadRenderer::new();
        let oversized = vec![0; 256 * 1024 + 1];
        assert!(matches!(
            renderer.set_builtin_font_bytes(&oversized, &[], &[]),
            Err(tabula_presentation::RenderError::Execution(_))
        ));
        renderer.frame_started = true;
        assert_eq!(
            renderer.set_builtin_font_bytes(&[], &[], &[]),
            Err(tabula_presentation::RenderError::InvalidLifecycle)
        );
    }
}
