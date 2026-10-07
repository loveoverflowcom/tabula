//! Shared identity in the native setup's ordinary renderer/resource pipeline (doc 04 §5, §12).
//!
//! The small embedded white masks are exported from the canonical SVG outlines.
//! Their validated metadata extends the game's setup group. The combined manifest stays
//! bound across setup/gameplay transitions; gameplay retains its original lazy group.

use tabula_assets::AssetPackRef;
use tabula_core::GameId;
use tabula_presentation::AssetRef;

use crate::fixture_assets::LocalSpriteResources;

const MANIFEST_FRAGMENT: &str =
    include_str!("../../../assets/brand/generated/native-setup.pack.fragment.toml");
const MARK_MASK: &[u8] = include_bytes!("../../../assets/brand/generated/native-mark-mask.png");
const WORDMARK_MASK: &[u8] =
    include_bytes!("../../../assets/brand/generated/native-wordmark-mask.png");
const MASKS: &[(&str, &[u8])] = &[
    ("brand-mark-mask.png", MARK_MASK),
    ("brand-wordmark-mask.png", WORDMARK_MASK),
];

/// Small embedded shared identity needed before game packs (assets/README.md).
/// Miniquad owns the native window/launcher mask; these are unmasked RGBA exports.
#[cfg(not(target_arch = "wasm32"))]
pub fn brand_icon() -> macroquad::miniquad::conf::Icon {
    macroquad::miniquad::conf::Icon {
        small: *include_bytes!("../../../assets/brand/generated/native-icon-16.rgba"),
        medium: *include_bytes!("../../../assets/brand/generated/native-icon-32.rgba"),
        big: *include_bytes!("../../../assets/brand/generated/native-icon-64.rgba"),
    }
}

/// Canonical outlined T Portal mark, tinted by the presenter's semantic brand role.
#[must_use]
pub fn mark_asset() -> AssetRef {
    AssetRef::new("brand/mark").expect("the fixed brand mark resource is canonical")
}

/// Canonical outlined lowercase wordmark; never rendered with an installed font.
#[must_use]
pub fn wordmark_asset() -> AssetRef {
    AssetRef::new("brand/wordmark").expect("the fixed brand wordmark resource is canonical")
}

/// Adds shared identity only to setup while preserving the original gameplay group.
///
/// The existing one-pack cache keeps the same validated manifest for the whole host
/// lifetime. Ordinary scene selection never preloads masks for a gameplay-only entry.
/// Only small shared masks use the documented embedded-brand exception.
pub fn local_resources(
    manifest_text: &str,
    game: &GameId,
    pack: &AssetPackRef,
    mut setup: Vec<AssetRef>,
    gameplay: Vec<AssetRef>,
) -> Result<LocalSpriteResources, String> {
    setup.extend([mark_asset(), wordmark_asset()]);
    LocalSpriteResources::new(
        &format!("{manifest_text}\n{MANIFEST_FRAGMENT}"),
        game,
        pack,
        setup,
        gameplay,
    )
    .map(|resources| resources.with_shared_images(MASKS))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::fixture_assets::LocalAssetScene;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use tabula_assets::{load_verified, AssetDensity, AssetPackManifest, MemoryAssetSource};
    #[rustfmt::skip]
    use tabula_game_chess::presentation::assets; // xtask-allow-game-id: existing native fixture regression, not platform dispatch.
    use tabula_render_macroquad::assets::{
        decode_verified_raster, AssetCacheLimits, AssetLoadState, DecodedRaster, SpriteAssetCache,
        TextureUploader,
    };

    #[derive(Debug)]
    struct FixtureUploader;

    impl TextureUploader for FixtureUploader {
        type Texture = ();
        fn upload(&mut self, _: &DecodedRaster) -> Result<Self::Texture, String> {
            Ok(())
        }
    }

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("embedded setup validation must complete without I/O"),
        }
    }

    fn declarations() -> (LocalSpriteResources, LocalSpriteResources) {
        let game = GameId::new("com.tabula.chess").unwrap(); // xtask-allow-game-id: existing native fixture regression, not platform dispatch.
        let original = LocalSpriteResources::new(
            assets::MANIFEST,
            &game,
            &assets::asset_pack(),
            assets::setup_resources(),
            assets::gameplay_resources(),
        )
        .unwrap()
        .with_embedded_images(assets::ALL_IMAGES);
        let combined = local_resources(
            assets::MANIFEST,
            &game,
            &assets::asset_pack(),
            assets::setup_resources(),
            assets::gameplay_resources(),
        )
        .unwrap()
        .with_embedded_images(assets::ALL_IMAGES);
        (original, combined)
    }

    fn combined_manifest() -> AssetPackManifest {
        AssetPackManifest::from_toml(&format!("{}\n{MANIFEST_FRAGMENT}", assets::MANIFEST)).unwrap()
    }

    #[test]
    fn setup_gameplay_setup_gameplay_keeps_textures_warm_and_original_gameplay_group() {
        let (original, combined) = declarations();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        for target in [1, 2, 3, 1] {
            let density = AssetDensity::new(target).unwrap();
            assert_eq!(
                combined
                    .selected_files(LocalAssetScene::Gameplay, density)
                    .unwrap(),
                original
                    .selected_files(LocalAssetScene::Gameplay, density)
                    .unwrap(),
            );
            for scene in [LocalAssetScene::Setup, LocalAssetScene::Gameplay] {
                ready(combined.prepare_with_source(&mut cache, scene, density, &combined)).unwrap();
            }
            let warm = cache.stats();
            for scene in [LocalAssetScene::Setup, LocalAssetScene::Gameplay] {
                ready(combined.prepare_with_source(&mut cache, scene, density, &combined)).unwrap();
            }
            assert_eq!(cache.stats().uploads, warm.uploads);
            assert_eq!(cache.stats().decodes, warm.decodes);
            assert_eq!(cache.stats().releases, 0);
            for resource in [mark_asset(), wordmark_asset(), assets::cover_asset()] {
                assert_eq!(cache.state(&resource, density), Ok(AssetLoadState::Ready));
            }
            for resource in assets::gameplay_resources() {
                assert_eq!(cache.state(&resource, density), Ok(AssetLoadState::Ready));
            }
            // A cover insertion in the same binding leaves both header textures ready.
            let cover = original
                .selected_files(LocalAssetScene::Setup, density)
                .unwrap()[0];
            cache
                .insert_verified(ready(load_verified(cover, &combined)).unwrap())
                .unwrap();
            assert_eq!(
                cache.state(&mark_asset(), density),
                Ok(AssetLoadState::Ready)
            );
            assert_eq!(
                cache.state(&wordmark_asset(), density),
                Ok(AssetLoadState::Ready)
            );
        }
        assert_eq!(
            cache.stats().uploads,
            6,
            "two game-art densities plus two shared masks"
        );
    }

    #[test]
    fn gameplay_only_entry_preloads_no_brand_masks_or_cover() {
        let (_, combined) = declarations();
        let density = AssetDensity::new(1).unwrap();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        ready(combined.prepare_with_source(
            &mut cache,
            LocalAssetScene::Gameplay,
            density,
            &combined,
        ))
        .unwrap();
        assert_eq!(cache.stats().uploads, 1);
        assert_eq!(cache.stats().decodes, 1);
        for resource in [mark_asset(), wordmark_asset(), assets::cover_asset()] {
            assert_eq!(cache.state(&resource, density), Ok(AssetLoadState::Missing));
        }
        for resource in assets::gameplay_resources() {
            assert_eq!(cache.state(&resource, density), Ok(AssetLoadState::Ready));
        }
    }

    #[test]
    fn exported_brand_masks_are_verified_white_transparent_outlines() {
        let (_, combined) = declarations();
        let files = combined
            .selected_files(LocalAssetScene::Setup, AssetDensity::new(1).unwrap())
            .unwrap();
        for (name, dimensions) in [
            ("brand-mark-mask.png", (256, 256)),
            ("brand-wordmark-mask.png", (1024, 262)),
        ] {
            let file = files
                .iter()
                .find(|file| file.name().as_str() == name)
                .unwrap();
            let verified = ready(load_verified(file, &combined)).unwrap();
            let image = decode_verified_raster(&verified, AssetCacheLimits::default()).unwrap();
            assert_eq!((image.width(), image.height()), dimensions);
            let pixels: Vec<_> = image.rgba().chunks_exact(4).collect();
            assert!(pixels.iter().any(|pixel| pixel[3] == 0));
            assert!(pixels.iter().any(|pixel| pixel[3] == 255));
            assert!(pixels
                .iter()
                .filter(|pixel| pixel[3] != 0)
                .all(|pixel| pixel[..3] == [255, 255, 255]));
        }
    }

    #[test]
    fn corrupted_embedded_mask_cannot_become_a_ready_sprite() {
        let (_, combined) = declarations();
        let manifest = combined_manifest();
        let mut source = MemoryAssetSource::new();
        for file in manifest.files() {
            let (_, bytes) = MASKS
                .iter()
                .chain(assets::ALL_FILES)
                .find(|(name, _)| *name == file.name().as_str())
                .unwrap();
            let mut bytes = bytes.to_vec();
            if file.name().as_str() == "brand-mark-mask.png" {
                bytes[0] ^= 1;
            }
            source.insert(file.path().clone(), bytes);
        }
        let density = AssetDensity::new(1).unwrap();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        let error = ready(combined.prepare_with_source(
            &mut cache,
            LocalAssetScene::Setup,
            density,
            &source,
        ))
        .unwrap_err();
        assert!(error.contains("local art load"), "{error}");
        assert_eq!(
            cache.state(&mark_asset(), density),
            Ok(AssetLoadState::Missing)
        );
        assert_eq!(
            cache.stats().uploads,
            1,
            "only the unmodified cover was uploaded"
        );
    }

    #[test]
    fn composed_setup_rejects_duplicate_resource_metadata() {
        let manifest = combined_manifest();
        let already_branded = format!("{}\n{MANIFEST_FRAGMENT}", assets::MANIFEST);
        let error = local_resources(
            &already_branded,
            manifest.game(),
            manifest.pack_ref(),
            assets::setup_resources(),
            assets::gameplay_resources(),
        )
        .unwrap_err();
        assert!(error.contains("duplicate asset file name"), "{error}");
    }
}
