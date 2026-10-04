//! Host adapter for the explicitly bundled local renderer fixture. (doc 04 §12)
//!
//! This memory source exercises the production integrity/resolution boundary;
//! it does not establish filesystem, HTTP, browser caching, or CDN delivery.

use renderer_macroquad::MacroquadRenderer;
use tabula_assets::{
    load_verified, AssetDensity, AssetPackManifest, AssetPackRef, MemoryAssetSource,
};
use tabula_core::GameId;

/// Binds one fixture pack and uploads its verified density variants once.
///
/// The same cache survives local match restarts. Insertion reuses a ready
/// texture by pack/version/content identity. Both tiny variants are prepared
/// so a later DPI change needs no I/O or decoding inside the render loop.
/// Errors retain their stage and are surfaced by the host's render recovery.
pub async fn preload_sprite_fixture(
    renderer: &mut MacroquadRenderer,
    manifest_text: &str,
    game: &GameId,
    pack: &AssetPackRef,
    images: &[(AssetDensity, &[u8])],
) -> Result<(), String> {
    let manifest = AssetPackManifest::from_toml(manifest_text)
        .map_err(|error| format!("fixture manifest: {error}"))?;
    renderer
        .assets_mut()
        .bind_pack(&manifest, game, pack)
        .map_err(|error| format!("fixture binding: {error}"))?;
    let mut source = MemoryAssetSource::new();
    for file in manifest.files() {
        let bytes = images
            .iter()
            .find(|(density, _)| file.density() == Some(*density))
            .map(|(_, bytes)| *bytes)
            .ok_or_else(|| format!("fixture variant missing: {}", file.name()))?;
        source.insert(file.path().clone(), bytes.to_vec());
    }
    for file in manifest.files() {
        let verified = load_verified(file, &source)
            .await
            .map_err(|error| format!("fixture load: {error}"))?;
        renderer
            .assets_mut()
            .insert_verified(verified)
            .map_err(|error| format!("fixture texture: {error}"))?;
    }
    Ok(())
}

/// Preloads a bounded local art pack by exact physical file identity.
///
/// Unlike the legacy density-only fixture helper, this permits several files
/// at one density and a density-independent cover without ambiguous selection.
/// Every file still goes through pack binding, integrity checks and bounded
/// renderer decode. Repeated preloads reuse content-identified textures.
pub async fn preload_named_sprite_fixture(
    renderer: &mut MacroquadRenderer,
    manifest_text: &str,
    game: &GameId,
    pack: &AssetPackRef,
    images: &[(&str, &[u8])],
) -> Result<(), String> {
    let manifest = AssetPackManifest::from_toml(manifest_text)
        .map_err(|error| format!("local art manifest: {error}"))?;
    for (index, (name, _)) in images.iter().enumerate() {
        if images[..index].iter().any(|(previous, _)| previous == name) {
            return Err(format!("duplicate local art file: {name}"));
        }
    }
    let mut source = MemoryAssetSource::new();
    for file in manifest.files() {
        let bytes = images
            .iter()
            .find(|(name, _)| *name == file.name().as_str())
            .map(|(_, bytes)| *bytes)
            .ok_or_else(|| format!("local art file missing: {}", file.name()))?;
        source.insert(file.path().clone(), bytes.to_vec());
    }
    renderer
        .assets_mut()
        .bind_pack(&manifest, game, pack)
        .map_err(|error| format!("local art binding: {error}"))?;
    for file in manifest.files() {
        let verified = load_verified(file, &source)
            .await
            .map_err(|error| format!("local art load: {error}"))?;
        renderer
            .assets_mut()
            .insert_verified(verified)
            .map_err(|error| format!("local art texture: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    #[rustfmt::skip]
    use tabula_game_chess::presentation::assets; // xtask-allow-game-id: local Phase 2 fixture boundary regression only.

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("memory fixture validation must complete without I/O"),
        }
    }

    #[test]
    fn duplicate_named_files_fail_before_any_graphics_upload() {
        let mut renderer = MacroquadRenderer::new();
        let game = GameId::new("com.tabula.chess").unwrap(); // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
        let result = ready(preload_named_sprite_fixture(
            &mut renderer,
            assets::MANIFEST,
            &game,
            &assets::asset_pack(),
            &[
                ("pieces@1x.atlas", assets::ATLAS_1X),
                ("pieces@1x.atlas", assets::ATLAS_1X),
            ],
        ));
        assert_eq!(
            result,
            Err(String::from("duplicate local art file: pieces@1x.atlas"))
        );
    }

    #[test]
    fn missing_named_cover_fails_instead_of_substituting_same_density_piece_bytes() {
        let mut renderer = MacroquadRenderer::new();
        let game = GameId::new("com.tabula.chess").unwrap(); // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
        let images: Vec<_> = assets::ALL_IMAGES
            .iter()
            .copied()
            .filter(|(name, _)| !name.starts_with("cover@"))
            .collect();
        let result = ready(preload_named_sprite_fixture(
            &mut renderer,
            assets::MANIFEST,
            &game,
            &assets::asset_pack(),
            &images,
        ));
        assert!(result
            .unwrap_err()
            .starts_with("local art file missing: cover@"));
    }
}
