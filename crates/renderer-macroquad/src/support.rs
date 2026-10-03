//! Pure Macroquad support validation for a complete [`RenderList`].
//!
//! Structural validity belongs to `tabula-presentation`; this module checks the
//! smaller set of commands and effective states that this backend can execute.
//! It deliberately visits the already-interpreted state stream instead of
//! maintaining a second scope stack.

use tabula_assets::AssetDensity;
use tabula_presentation::{
    AssetRef, Dpi, FrameCtx, RenderCmd, RenderCmdKind, RenderError, RenderList,
};

use crate::{
    assets::{ResolvedSprite, SpriteAssetCache, TextureUploader},
    draw, state,
};

/// The primitive families implemented by this backend. (doc 04 §5.2)
///
/// This is a static inventory, not acceptance of every list. Text requires positive uniform
/// scaling, filled paths must be convex, and sprites require a bound, verified, decoded, ready
/// resource in the renderer's cache. Preflight applies those operation-specific conditions.
pub const MACROQUAD_SUPPORTED_COMMANDS: &[RenderCmdKind] = &[
    RenderCmdKind::Sprite,
    RenderCmdKind::Rect,
    RenderCmdKind::RoundedRect,
    RenderCmdKind::Text,
    RenderCmdKind::Path,
    RenderCmdKind::LinearGradient,
];

/// Selects the smallest supported asset density that does not undersample the display, capped
/// at the existing manifest's three density tiers. (doc 04 §12.3)
#[must_use]
pub fn density_for_dpi(dpi: Dpi) -> AssetDensity {
    let density = if dpi.get() <= 1.0 {
        1
    } else if dpi.get() <= 2.0 {
        2
    } else {
        3
    };
    AssetDensity::new(density).expect("selected density is one of the three manifest tiers")
}

/// Rejects unsupported or non-executable commands without a resource cache or drawing context.
/// A sprite's geometry can be valid while its resource is unavailable; this reports that missing
/// ready-resource boundary as an execution error rather than unsupported Sprite capability.
pub(crate) fn preflight(list: &RenderList, frame: &FrameCtx) -> Result<(), RenderError> {
    preflight_with_resolver(list, frame, |asset| {
        Err(RenderError::Execution(format!(
            "renderer-macroquad sprite resource '{}' is not ready: no asset cache supplied",
            asset.as_str(),
        )))
    })
}

/// Resolves every sprite before any primitive can be drawn. Strong handles returned here keep
/// ready textures alive even if the owning pack is released before the queued frame is flushed.
pub(crate) fn prepare<U: TextureUploader>(
    list: &RenderList,
    frame: &FrameCtx,
    cache: &SpriteAssetCache<U>,
) -> Result<Vec<ResolvedSprite<U::Texture>>, RenderError> {
    let mut sprites = Vec::new();
    preflight_with_resolver(list, frame, |asset| {
        let sprite = cache
            .resolve(asset, density_for_dpi(frame.dpi()))
            .map_err(|error| RenderError::Execution(error.to_string()))?;
        // The cache validates decoded bounds before upload. Check the ready descriptor too,
        // using the same normalized source geometry that execution consumes.
        draw::validate_sprite_source(sprite.source(), sprite.width(), sprite.height())?;
        sprites.push(sprite);
        Ok(())
    })?;
    Ok(sprites)
}

fn preflight_with_resolver(
    list: &RenderList,
    frame: &FrameCtx,
    mut resolve: impl FnMut(&AssetRef) -> Result<(), RenderError>,
) -> Result<(), RenderError> {
    let mut result = Ok(());
    state::visit_draws(list, |command, draw_state| {
        if result.is_ok() {
            result = draw::validate(command, draw_state, list.camera(), frame).and_then(|()| {
                if let RenderCmd::Sprite { asset, .. } = command {
                    resolve(asset)
                } else {
                    Ok(())
                }
            });
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use glam::{Affine2, Vec2};
    use tabula_design::{Color, TextStyleToken, Theme, ThemeKind};
    use tabula_presentation::{
        AssetRef, Border, Camera2D, Corners, Dpi, Layer, Opacity, Paint, Rect, RenderCmd,
        RenderCmdKind, RenderError, RenderList, RenderListBuilder, Viewport,
    };

    use super::{
        density_for_dpi, preflight, preflight_with_resolver, prepare, MACROQUAD_SUPPORTED_COMMANDS,
    };

    fn color() -> Color {
        Theme::by_kind(ThemeKind::Light).color.primary
    }

    fn frame() -> tabula_presentation::FrameCtx {
        tabula_presentation::FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            0,
            Theme::by_kind(ThemeKind::Light),
        )
    }

    fn rect() -> RenderCmd {
        RenderCmd::Rect {
            rect: Rect::new(Vec2::new(10.0, 10.0), Vec2::splat(100.0)).unwrap(),
            radii: Corners::uniform(0.0).unwrap(),
            fill: Some(Paint::Solid(color())),
            border: None,
            layer: Layer::BOARD,
            z: 0,
        }
    }

    fn text() -> RenderCmd {
        RenderCmd::Text {
            text: String::from("supported"),
            at: Vec2::new(10.0, 20.0),
            style: TextStyleToken::BodyMd,
            align: tabula_presentation::Align::Start,
            max_width: None,
            color: color(),
            layer: Layer::HUD,
            z: 0,
        }
    }

    fn list_with(command: RenderCmd) -> RenderList {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(command).unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn ordinary_rect_and_text_lists_pass_macroquad_preflight() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder.push(rect()).unwrap();
        builder.push(text()).unwrap();
        let list = builder.finish().unwrap();

        assert_eq!(preflight(&list, &frame()), Ok(()));
    }

    #[test]
    fn oversized_wrapped_text_is_rejected_before_execution() {
        let mut command = text();
        if let RenderCmd::Text { text: value, .. } = &mut command {
            *value = "x\n".repeat(usize::from(u16::MAX) + 1);
        }
        assert_eq!(
            preflight(&list_with(command), &frame()),
            Err(RenderError::Execution(String::from(
                "backend text has more than u16::MAX lines",
            )))
        );
    }

    #[test]
    fn supported_nested_scopes_pass_with_their_effective_state() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::splat(320.0)).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_scale(Vec2::splat(2.0)),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(rect()).unwrap();
        builder.push(text()).unwrap();
        builder
            .push(RenderCmd::PopOpacity {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();

        assert_eq!(preflight(&builder.finish().unwrap(), &frame()), Ok(()));
    }

    #[test]
    fn sprite_without_ready_resource_fails_explicitly_before_execution() {
        let list = list_with(RenderCmd::Sprite {
            asset: AssetRef::from_static("deferred/asset"),
            rect: Rect::new(Vec2::ZERO, Vec2::ONE).unwrap(),
            tint: color(),
            rotation: 0.0,
            pivot: Vec2::ZERO,
            layer: Layer::PIECES,
            z: 0,
        });

        assert_eq!(
            preflight(&list, &frame()),
            Err(RenderError::Execution(String::from(
                "renderer-macroquad sprite resource 'deferred/asset' is not ready: no asset cache supplied",
            )))
        );
    }

    fn sprite(asset: &'static str, layer: Layer, z: i16) -> RenderCmd {
        RenderCmd::Sprite {
            asset: AssetRef::from_static(asset),
            rect: Rect::new(Vec2::new(10.0, 20.0), Vec2::new(4.0, 2.0)).unwrap(),
            tint: color(),
            rotation: core::f32::consts::FRAC_PI_2,
            pivot: Vec2::new(12.0, 21.0),
            layer,
            z,
        }
    }

    #[test]
    fn support_inventory_declares_sprite_with_resource_specific_preflight() {
        assert!(MACROQUAD_SUPPORTED_COMMANDS.contains(&RenderCmdKind::Sprite));
        assert_eq!(
            preflight_with_resolver(
                &list_with(sprite("ready/tile", Layer::BOARD, 0)),
                &frame(),
                |_| Ok(())
            ),
            Ok(())
        );
        for (dpi, density) in [
            (0.5, 1),
            (1.0, 1),
            (1.01, 2),
            (2.0, 2),
            (2.01, 3),
            (100.0, 3),
        ] {
            assert_eq!(density_for_dpi(Dpi::new(dpi).unwrap()).get(), density);
        }
    }

    #[test]
    fn sprite_preflight_preserves_nested_scope_and_stable_stacking_order() {
        let mut builder = RenderListBuilder::new(Camera2D::new(Vec2::new(2.0, 3.0), 2.0).unwrap());
        builder.push(sprite("root/top", Layer::HUD, 1)).unwrap();
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::splat(200.0)).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_cols(
                    Vec2::new(-2.0, 1.0),
                    Vec2::new(1.0, 3.0),
                    Vec2::new(5.0, 7.0),
                ),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PushOpacity {
                opacity: Opacity::try_from(0.5).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(sprite("group/equal-first", Layer::PIECES, 0))
            .unwrap();
        builder
            .push(sprite("group/equal-second", Layer::PIECES, 0))
            .unwrap();
        builder
            .push(RenderCmd::PopOpacity {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        let list = builder.finish().unwrap();
        let mut visited = Vec::new();
        assert_eq!(
            preflight_with_resolver(&list, &frame(), |asset| {
                visited.push(asset.as_str().to_owned());
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(
            visited,
            ["group/equal-first", "group/equal-second", "root/top"]
        );
    }

    #[test]
    fn sprite_geometry_failure_precedes_resource_resolution_and_empty_clip_is_validated() {
        let mut command = sprite("bad/geometry", Layer::BOARD, 0);
        if let RenderCmd::Sprite { rect, .. } = &mut command {
            *rect = Rect::new(Vec2::splat(f32::MAX / 4.0), Vec2::ONE).unwrap();
        }
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_scale(Vec2::splat(8.0)),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder.push(command).unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        let mut resolutions = 0;
        assert!(
            preflight_with_resolver(&builder.finish().unwrap(), &frame(), |_| {
                resolutions += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(resolutions, 0);

        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushClip {
                rect: Rect::new(Vec2::ZERO, Vec2::ZERO).unwrap(),
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        builder
            .push(sprite("missing/clipped", Layer::BOARD, 0))
            .unwrap();
        builder
            .push(RenderCmd::PopClip {
                layer: Layer::BOARD,
                z: 0,
            })
            .unwrap();
        assert!(
            preflight(&builder.finish().unwrap(), &frame()).is_err(),
            "an empty scissor cannot hide an unavailable resource"
        );
    }

    #[test]
    fn ready_sprite_preflight_reuses_cache_and_leases_texture_until_frame_flush() {
        use crate::assets::{AssetCacheLimits, DecodedRaster, SpriteAssetCache, TextureUploader};
        use image::ImageEncoder;
        use tabula_assets::{AssetPackManifest, UnverifiedAssetBytes};
        #[derive(Debug)]
        struct Uploader;
        impl TextureUploader for Uploader {
            type Texture = (u16, u16);
            fn upload(&mut self, raster: &DecodedRaster) -> Result<Self::Texture, String> {
                Ok((raster.width(), raster.height()))
            }
        }
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&[255; 4 * 4 * 4], 4, 4, image::ColorType::Rgba8)
            .unwrap();
        let manifest = AssetPackManifest::from_toml(&format!(
            r#"pack = "fixture"
version = "1.0.0"
game = "com.example.fixture"
[[files]]
name = "atlas.png"
path = "fixture/1.0.0/atlas.png"
hash = "{}"
bytes = {}
priority = "critical"
[[resources]]
id = "ready/tile"
[[resources.variants]]
file = "atlas.png"
region = {{ x = 1, y = 1, width = 2, height = 2 }}
"#,
            blake3::hash(&png).to_hex(),
            png.len()
        ))
        .unwrap();
        let mut cache = SpriteAssetCache::new(Uploader, AssetCacheLimits::default());
        cache
            .bind_pack(&manifest, manifest.game(), manifest.pack_ref())
            .unwrap();
        let list = list_with(sprite("ready/tile", Layer::BOARD, 0));
        assert!(prepare(&list, &frame(), &cache).is_err());
        cache
            .insert_verified(
                manifest.files()[0]
                    .verify_owned_bytes(UnverifiedAssetBytes::new(png))
                    .unwrap(),
            )
            .unwrap();
        let before = cache.stats();
        let prepared = prepare(&list, &frame(), &cache).unwrap();
        for _ in 0..50 {
            assert_eq!(prepare(&list, &frame(), &cache).unwrap().len(), 1);
        }
        assert_eq!(cache.stats(), before, "preflight never decodes or uploads");
        assert_eq!(
            prepared[0].source(),
            tabula_assets::AssetPixelRegion::new(1, 1, 2, 2).unwrap()
        );
        cache.clear();
        assert_eq!(prepared[0].texture(), &(4, 4));
        assert_eq!(
            cache.stats().resident_textures,
            1,
            "queued frame leases retain released pack textures"
        );
        drop(prepared);
        cache.collect_released();
        assert_eq!(cache.stats().resident_textures, 0);
    }

    #[test]
    fn transformed_text_is_rejected_before_execution() {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        builder
            .push(RenderCmd::PushTransform {
                matrix: Affine2::from_scale(Vec2::new(2.0, 1.0)),
                layer: Layer::HUD,
                z: 0,
            })
            .unwrap();
        builder.push(text()).unwrap();
        builder
            .push(RenderCmd::PopTransform {
                layer: Layer::HUD,
                z: 0,
            })
            .unwrap();

        assert_eq!(
            preflight(&builder.finish().unwrap(), &frame()),
            Err(RenderError::Unsupported(RenderCmdKind::Text))
        );
    }

    #[test]
    fn concave_filled_path_is_rejected_before_execution() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(0.0, 3.0),
        ]
        .into_iter()
        .collect();
        let list = list_with(RenderCmd::Path {
            points,
            stroke: Border::new(1.0, color()).unwrap(),
            closed: true,
            fill: Some(Paint::Solid(color())),
            layer: Layer::BOARD,
            z: 0,
        });

        assert_eq!(
            preflight(&list, &frame()),
            Err(RenderError::Unsupported(RenderCmdKind::Path))
        );
    }
}
