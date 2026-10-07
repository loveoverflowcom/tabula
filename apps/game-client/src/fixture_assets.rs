//! Bounded local asset sources for the renderer fixture and game host. (doc 04 §12)
//!
//! Embedded native fixtures and the opt-in WASM host use the same manifest
//! resolution, integrity and decode boundaries. The WASM source requests exact
//! pack paths through Macroquad's safe file port; the checked-in bootstrap owns
//! their bounded, same-origin content-hashed delivery (ADR-0030), not a CDN service.

use tabula_assets::{
    load_verified, AssetDensity, AssetFile, AssetPackManifest, AssetPackRef, AssetPath,
    AssetSource, MemoryAssetSource, UnverifiedAssetBytes,
};
use tabula_core::GameId;
use tabula_presentation::{AssetRef, Dpi};
use tabula_render_macroquad::{
    assets::{AssetLoadState, SpriteAssetCache, TextureUploader},
    density_for_dpi, MacroquadRenderer,
};

/// The independently declared local scene that needs resources. (ADR-0030)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAssetScene {
    /// Entry artwork only, before constructing the local match.
    Setup,
    /// Every resource the game can use during the local match.
    Gameplay,
}

/// Game-owned logical declarations with their exact local pack binding.
///
/// The host neither infers resource groups from filenames nor preloads the
/// whole pack. Selection is pure; missing files are fetched and verified at an
/// async frame boundary. Ready renderer textures survive match restarts and
/// density changes without another download, decode or upload.
#[derive(Debug)]
pub struct LocalSpriteResources {
    manifest: AssetPackManifest,
    game: GameId,
    pack: AssetPackRef,
    setup: Vec<AssetRef>,
    gameplay: Vec<AssetRef>,
    shared_images: &'static [(&'static str, &'static [u8])],
    #[cfg(not(target_arch = "wasm32"))]
    images: &'static [(&'static str, &'static [u8])],
}

impl LocalSpriteResources {
    /// Validates a declaration and pack/game binding before any source access.
    pub fn new(
        manifest_text: &str,
        game: &GameId,
        pack: &AssetPackRef,
        setup: Vec<AssetRef>,
        gameplay: Vec<AssetRef>,
    ) -> Result<Self, String> {
        let manifest = AssetPackManifest::from_toml(manifest_text)
            .map_err(|error| format!("local art manifest: {error}"))?;
        let bound = manifest
            .validate_binding(pack, game)
            .map_err(|error| format!("local art binding: {error}"))?;
        // Resolve every declared identity now, while the operation is pure.
        // Missing metadata is never discovered after a false ready notification.
        for resource in setup.iter().chain(&gameplay) {
            bound
                .resolve(resource, AssetDensity::new(1).expect("valid density"))
                .map_err(|error| format!("local art resource: {error}"))?;
        }
        Ok(Self {
            manifest,
            game: game.clone(),
            pack: pack.clone(),
            setup,
            gameplay,
            shared_images: &[],
            #[cfg(not(target_arch = "wasm32"))]
            images: &[],
        })
    }

    /// Supplies only the small shared brand exception (assets/README.md).
    /// Bytes still cross the exact manifest, integrity and decoder boundaries.
    pub(crate) fn with_shared_images(
        mut self,
        images: &'static [(&'static str, &'static [u8])],
    ) -> Self {
        self.shared_images = images;
        self
    }

    /// Supplies the native host's tiny embedded fixtures, never original artwork.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_embedded_images(
        mut self,
        images: &'static [(&'static str, &'static [u8])],
    ) -> Self {
        self.images = images;
        self
    }

    fn resources(&self, scene: LocalAssetScene) -> &[AssetRef] {
        match scene {
            LocalAssetScene::Setup => &self.setup,
            LocalAssetScene::Gameplay => &self.gameplay,
        }
    }

    fn selected_resources(
        &self,
        scene: LocalAssetScene,
        density: AssetDensity,
    ) -> Result<Vec<(&AssetRef, &AssetFile)>, String> {
        let bound = self
            .manifest
            .validate_binding(&self.pack, &self.game)
            .map_err(|error| format!("local art binding: {error}"))?;
        let mut selected: Vec<(&AssetRef, &AssetFile)> = Vec::new();
        for resource in self.resources(scene) {
            let resolved = bound
                .resolve(resource, density)
                .map_err(|error| format!("local art resource: {error}"))?;
            let file = self
                .manifest
                .files()
                .iter()
                .find(|file| *file == resolved.file())
                .expect("validated resource references a declared physical file");
            if !selected.iter().any(|(_, previous)| *previous == file) {
                selected.push((resource, file));
            }
        }
        Ok(selected)
    }

    /// Selects distinct physical files without source, decoder or graphics calls.
    pub fn selected_files(
        &self,
        scene: LocalAssetScene,
        density: AssetDensity,
    ) -> Result<Vec<&AssetFile>, String> {
        self.selected_resources(scene, density)
            .map(|selected| selected.into_iter().map(|(_, file)| file).collect())
    }

    /// Prepares only absent selected files through the existing trust boundaries.
    ///
    /// Ready checks precede source access, so a new match or return to an already
    /// prepared DPI never downloads or decodes those textures again. A failed
    /// fetch, integrity check, decode or upload returns an explicit staged error.
    /// The caller owns recovery and must not claim gameplay readiness on error.
    pub async fn prepare_with_source<U, S>(
        &self,
        cache: &mut SpriteAssetCache<U>,
        scene: LocalAssetScene,
        density: AssetDensity,
        source: &S,
    ) -> Result<(), String>
    where
        U: TextureUploader,
        S: AssetSource,
        S::Error: std::fmt::Display,
    {
        let selected = self.selected_resources(scene, density)?;
        cache
            .bind_pack(&self.manifest, &self.game, &self.pack)
            .map_err(|error| format!("local art binding: {error}"))?;
        for (resource, file) in selected {
            if matches!(cache.state(resource, density), Ok(AssetLoadState::Ready)) {
                continue;
            }
            let verified = load_verified(file, source)
                .await
                .map_err(|error| format!("local art load {}: {error}", file.path()))?;
            cache
                .insert_verified(verified)
                .map_err(|error| format!("local art texture {}: {error}", file.path()))?;
        }
        Ok(())
    }

    /// Resolves renderer density and prepares it before the next frame begins.
    ///
    /// Native uses named embedded fixtures; WASM requests exact physical paths
    /// through the bootstrap's allowlisted safe file port. Both verify BLAKE3
    /// and size before the renderer is permitted to decode bytes.
    pub async fn prepare(
        &self,
        renderer: &mut MacroquadRenderer,
        scene: LocalAssetScene,
        dpi: Dpi,
    ) -> Result<(), String> {
        self.prepare_with_source(renderer.assets_mut(), scene, density_for_dpi(dpi), self)
            .await
    }
}

impl AssetSource for LocalSpriteResources {
    type Error = String;

    #[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
    async fn fetch<'a>(&'a self, path: &'a AssetPath) -> Result<UnverifiedAssetBytes, Self::Error> {
        // Even this host adapter cannot request a path outside its bound pack.
        let file = self
            .manifest
            .files()
            .iter()
            .find(|file| file.path() == path)
            .ok_or_else(|| format!("local art path is undeclared: {path}"))?;
        if let Some((_, bytes)) = self
            .shared_images
            .iter()
            .find(|(name, _)| *name == file.name().as_str())
        {
            return Ok(UnverifiedAssetBytes::new(bytes.to_vec()));
        }
        #[cfg(target_arch = "wasm32")]
        {
            macroquad::file::load_file(file.path().as_str())
                .await
                .map(UnverifiedAssetBytes::new)
                .map_err(|error| error.to_string())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.images
                .iter()
                .find(|(name, _)| *name == file.name().as_str())
                .map(|(_, bytes)| UnverifiedAssetBytes::new(bytes.to_vec()))
                .ok_or_else(|| format!("local art file missing: {}", file.name()))
        }
    }
}

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
        cell::RefCell,
        future::Future,
        task::{Context, Poll, Waker},
    };
    #[rustfmt::skip]
    use tabula_game_chess::presentation::assets; // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
    use tabula_render_macroquad::assets::{AssetCacheLimits, DecodedRaster};

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

    #[derive(Debug, Default)]
    struct FixtureUploader;

    impl TextureUploader for FixtureUploader {
        type Texture = ();

        fn upload(&mut self, _image: &DecodedRaster) -> Result<Self::Texture, String> {
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct RecordingSource {
        memory: MemoryAssetSource,
        requests: RefCell<Vec<AssetPath>>,
    }

    impl AssetSource for RecordingSource {
        type Error = tabula_assets::MemoryAssetSourceError;

        fn fetch<'a>(
            &'a self,
            path: &'a AssetPath,
        ) -> impl Future<Output = Result<UnverifiedAssetBytes, Self::Error>> + 'a {
            self.requests.borrow_mut().push(path.clone());
            self.memory.fetch(path)
        }
    }

    fn resources() -> LocalSpriteResources {
        let game = GameId::new("com.tabula.chess").unwrap(); // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
        LocalSpriteResources::new(
            assets::MANIFEST,
            &game,
            &assets::asset_pack(),
            assets::setup_resources(),
            assets::gameplay_resources(),
        )
        .unwrap()
    }

    fn recording_source() -> RecordingSource {
        let mut source = RecordingSource::default();
        let manifest = AssetPackManifest::from_toml(assets::MANIFEST).unwrap();
        for file in manifest.files() {
            let bytes = assets::ALL_IMAGES
                .iter()
                .find(|(name, _)| *name == file.name().as_str())
                .unwrap()
                .1;
            source.memory.insert(file.path().clone(), bytes.to_vec());
        }
        source
    }

    fn density(value: u8) -> AssetDensity {
        AssetDensity::new(value).unwrap()
    }

    #[test]
    fn gameplay_and_setup_select_only_their_actual_density_file() {
        let resources = resources();
        for (target, pieces, grain, cover) in [
            (1, "pieces@1x.atlas", "grain@1x.atlas", "cover@1x.atlas"),
            (2, "pieces@2x.atlas", "grain@2x.atlas", "cover@2x.atlas"),
            (3, "pieces@2x.atlas", "grain@2x.atlas", "cover@2x.atlas"),
        ] {
            let gameplay = resources
                .selected_files(LocalAssetScene::Gameplay, density(target))
                .unwrap();
            assert_eq!(
                gameplay.len(),
                2,
                "twelve pieces share one atlas; decorative grain is separate"
            );
            assert_eq!(gameplay[0].name().as_str(), pieces);
            assert_eq!(gameplay[1].name().as_str(), grain);
            let setup = resources
                .selected_files(LocalAssetScene::Setup, density(target))
                .unwrap();
            assert_eq!(setup.len(), 1);
            assert_eq!(setup[0].name().as_str(), cover);
            assert_ne!(gameplay[0].path(), setup[0].path());
        }
    }

    #[test]
    fn skip_setup_loads_no_cover_and_fetches_exact_manifest_path_once() {
        let resources = resources();
        let source = recording_source();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        ready(resources.prepare_with_source(
            &mut cache,
            LocalAssetScene::Gameplay,
            density(1),
            &source,
        ))
        .unwrap();
        let expected: Vec<_> = resources
            .selected_files(LocalAssetScene::Gameplay, density(1))
            .unwrap()
            .into_iter()
            .map(|file| file.path().clone())
            .collect();
        assert_eq!(source.requests.borrow().as_slice(), expected.as_slice());
        assert_eq!(cache.stats().decodes, 2);
        assert_eq!(cache.stats().uploads, 2);
        assert_eq!(cache.stats().resident_textures, 2);
    }

    #[test]
    fn restart_and_density_return_do_not_refetch_or_redecode_ready_textures() {
        let source = recording_source();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        for target in [1, 1, 2, 2, 3, 1] {
            // A new declaration models a fresh local match using the same renderer.
            ready(resources().prepare_with_source(
                &mut cache,
                LocalAssetScene::Gameplay,
                density(target),
                &source,
            ))
            .unwrap();
        }
        assert_eq!(source.requests.borrow().len(), 4);
        assert_eq!(cache.stats().decodes, 4);
        assert_eq!(cache.stats().uploads, 4);
        assert_eq!(cache.stats().resident_textures, 4);
    }

    #[test]
    fn missing_selected_file_fails_before_decode_and_retries_after_source_recovers() {
        let resources = resources();
        let mut source = RecordingSource::default();
        let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
        let error = ready(resources.prepare_with_source(
            &mut cache,
            LocalAssetScene::Gameplay,
            density(1),
            &source,
        ))
        .unwrap_err();
        assert!(error.contains("asset source failed"));
        assert_eq!(cache.stats().decodes, 0);
        assert_eq!(cache.stats().uploads, 0);
        let resource = &resources.resources(LocalAssetScene::Gameplay)[0];
        assert_eq!(
            cache.state(resource, density(1)).unwrap(),
            AssetLoadState::Missing
        );
        for file in resources
            .selected_files(LocalAssetScene::Gameplay, density(1))
            .unwrap()
        {
            let bytes = assets::ALL_IMAGES
                .iter()
                .find(|(name, _)| *name == file.name().as_str())
                .unwrap()
                .1;
            source.memory.insert(file.path().clone(), bytes.to_vec());
        }
        ready(resources.prepare_with_source(
            &mut cache,
            LocalAssetScene::Gameplay,
            density(1),
            &source,
        ))
        .unwrap();
        assert_eq!(
            cache.state(resource, density(1)).unwrap(),
            AssetLoadState::Ready
        );
        assert_eq!(source.requests.borrow().len(), 3);
        assert_eq!(cache.stats().uploads, 2);
    }

    #[test]
    fn corrupted_and_truncated_selected_bytes_never_reach_decode_or_ready() {
        let resources = resources();
        let file = resources
            .selected_files(LocalAssetScene::Gameplay, density(1))
            .unwrap()[0];
        let mut corrupt = assets::ATLAS_1X.to_vec();
        corrupt[0] ^= 1;
        for (bytes, diagnostic) in [
            (corrupt, "asset content hash mismatch"),
            (assets::ATLAS_1X[..32].to_vec(), "asset byte size mismatch"),
        ] {
            let mut source = RecordingSource::default();
            source.memory.insert(file.path().clone(), bytes);
            let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
            let error = ready(resources.prepare_with_source(
                &mut cache,
                LocalAssetScene::Gameplay,
                density(1),
                &source,
            ))
            .unwrap_err();
            assert!(error.contains(diagnostic), "{error}");
            assert_eq!(cache.stats().decodes, 0);
            assert_eq!(cache.stats().uploads, 0);
            assert_eq!(cache.stats().resident_textures, 0);
            assert_eq!(
                cache
                    .state(
                        &resources.resources(LocalAssetScene::Gameplay)[0],
                        density(1)
                    )
                    .unwrap(),
                AssetLoadState::Missing,
            );
        }
    }

    #[test]
    fn wrong_pack_version_game_and_undeclared_resources_fail_without_source_access() {
        let game = GameId::new("com.tabula.chess").unwrap(); // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
        let other_game = GameId::new("com.example.other").unwrap();
        for (game, pack, gameplay, diagnostic) in [
            (
                &game,
                AssetPackRef::from_static("chess", "0.3.0"), // xtask-allow-game-id: local Phase 2 fixture boundary regression only.
                assets::gameplay_resources(),
                "local art binding",
            ),
            (
                &other_game,
                assets::asset_pack(),
                assets::gameplay_resources(),
                "local art binding",
            ),
            (
                &game,
                assets::asset_pack(),
                vec![AssetRef::new("unavailable").unwrap()],
                "local art resource",
            ),
        ] {
            let error = LocalSpriteResources::new(
                assets::MANIFEST,
                game,
                &pack,
                assets::setup_resources(),
                gameplay,
            )
            .unwrap_err();
            assert!(error.contains(diagnostic), "{error}");
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
