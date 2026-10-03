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
