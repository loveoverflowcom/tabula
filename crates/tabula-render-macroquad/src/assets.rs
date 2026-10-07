//! Verified raster decoding and bounded renderer texture residency (doc 04 §12).
//!
//! This boundary consumes the asset crate's proof-bearing bytes. It neither fetches assets nor
//! chooses delivery policy. PNG is the supported raster format; other media fail explicitly.

use std::{collections::BTreeMap, fmt, io::Cursor, rc::Rc};

use image::{codecs::png::PngDecoder, ImageDecoder};
use tabula_assets::{
    AssetContentHash, AssetDensity, AssetFile, AssetPackManifest, AssetPackRef, AssetPixelRegion,
    OwnedVerifiedAssetBytes,
};
use tabula_core::GameId;
use tabula_presentation::AssetRef;

/// Per-file decoding limits and total texture residency limits (doc 04 §12).
///
/// The encoded input already exists when this boundary is called. These bounds prevent decoding
/// that input into an unbounded allocation. Decoder working memory is additionally limited by the
/// PNG decoder; allocator overhead and driver-internal memory are not measured by these counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetCacheLimits {
    encoded_bytes: usize,
    dimension: u16,
    pixels: u64,
    decoder_bytes: u64,
    resident_bytes: u64,
    entries: usize,
}

impl AssetCacheLimits {
    /// Constructs nonzero explicit limits. The dimension is also bounded by Macroquad's u16 API.
    pub fn new(
        encoded_bytes: usize,
        dimension: u16,
        pixels: u64,
        decoder_bytes: u64,
        resident_bytes: u64,
        entries: usize,
    ) -> Result<Self, AssetTextureError> {
        if encoded_bytes == 0
            || dimension == 0
            || pixels == 0
            || decoder_bytes == 0
            || resident_bytes == 0
            || entries == 0
        {
            return Err(AssetTextureError::InvalidLimits);
        }
        Ok(Self {
            encoded_bytes,
            dimension,
            pixels,
            decoder_bytes,
            resident_bytes,
            entries,
        })
    }

    /// Maximum estimated RGBA texture bytes retained by this cache, including leased entries.
    pub const fn resident_bytes(self) -> u64 {
        self.resident_bytes
    }
}

impl Default for AssetCacheLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: 8 * 1024 * 1024,
            dimension: 4096,
            pixels: 8 * 1024 * 1024,
            decoder_bytes: 64 * 1024 * 1024,
            resident_bytes: 128 * 1024 * 1024,
            entries: 256,
        }
    }
}

/// Exact physical texture identity; semantic atlas resources share one texture (doc 04 §12).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureKey {
    pack: AssetPackRef,
    hash: AssetContentHash,
    density: Option<AssetDensity>,
}

impl TextureKey {
    fn new(pack: &AssetPackRef, file: &AssetFile) -> Self {
        Self {
            pack: pack.clone(),
            hash: *file.hash(),
            density: file.density(),
        }
    }

    /// Pack and version, kept distinct even when two packs happen to share byte content.
    pub const fn pack(&self) -> &AssetPackRef {
        &self.pack
    }

    /// Verified content hash of the physical file.
    pub const fn hash(&self) -> &AssetContentHash {
        &self.hash
    }

    /// Actual resolved file density, rather than the caller's requested density.
    pub const fn density(&self) -> Option<AssetDensity> {
        self.density
    }
}

/// A bounded, fully decoded RGBA raster accepted by the upload seam (doc 04 §12).
///
/// Private fields prevent bypassing verification and decode limits with arbitrary pixel buffers.
#[derive(Debug)]
pub struct DecodedRaster {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
}

impl DecodedRaster {
    /// Physical pixel width.
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Physical pixel height.
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Immutable RGBA8 pixels; their exact size is width × height × 4.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// Why a verified physical file could not become a ready Sprite texture (doc 04 §12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetTextureError {
    /// Zero-valued resource limits have no executable interpretation.
    InvalidLimits,
    /// No exact pack/game binding has been installed.
    Unbound,
    /// The manifest belongs to another game or version.
    Binding(String),
    /// The supplied byte witness does not name a file in the bound manifest.
    ForeignFile,
    /// The supported raster boundary accepts PNG only.
    UnsupportedFormat,
    /// The encoded input exceeds the per-file limit.
    EncodedLimit,
    /// Header dimensions are zero, too large, or exceed the pixel budget.
    DimensionLimit,
    /// Decoder or resulting pixel allocations exceed the per-file budget.
    DecodeLimit,
    /// PNG structural, checksum, or compressed-stream validation failed.
    Decode(String),
    /// A declared atlas region extends beyond the decoded image.
    AtlasBounds { asset: AssetRef },
    /// No resource declaration exists for this semantic identity.
    UnknownResource(AssetRef),
    /// Resolved physical metadata exists but its texture has not been uploaded.
    Missing(TextureKey),
    /// Resident entries or bytes are exhausted; leased textures cannot be evicted.
    CacheBudget,
    /// The uploader rejected the already-validated raster.
    Upload(String),
}

impl fmt::Display for AssetTextureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("asset limits must be nonzero"),
            Self::Unbound => formatter.write_str("no sprite asset pack is bound"),
            Self::Binding(message) => write!(formatter, "asset pack binding: {message}"),
            Self::ForeignFile => {
                formatter.write_str("verified file is not in the bound asset pack")
            }
            Self::UnsupportedFormat => formatter.write_str("sprite decoder supports PNG only"),
            Self::EncodedLimit => formatter.write_str("encoded raster exceeds byte limit"),
            Self::DimensionLimit => formatter.write_str("raster exceeds dimension or pixel limit"),
            Self::DecodeLimit => formatter.write_str("raster exceeds decoder allocation limit"),
            Self::Decode(message) => write!(formatter, "invalid PNG raster: {message}"),
            Self::AtlasBounds { asset } => {
                write!(formatter, "atlas region outside raster: {asset}")
            }
            Self::UnknownResource(asset) => write!(formatter, "unknown sprite resource: {asset}"),
            Self::Missing(key) => write!(formatter, "sprite texture is not ready: {key:?}"),
            Self::CacheBudget => formatter.write_str("sprite texture cache budget exhausted"),
            Self::Upload(message) => write!(formatter, "sprite texture upload: {message}"),
        }
    }
}

impl std::error::Error for AssetTextureError {}

/// Decodes only proof-bearing bytes, checking dimensions before any pixel buffer allocation.
///
/// PNG's fixed IHDR precedes decoder construction so even a hostile header cannot request an
/// oversized pixel allocation. The decoder subsequently validates that header and the stream.
pub fn decode_verified_raster(
    verified: &OwnedVerifiedAssetBytes,
    limits: AssetCacheLimits,
) -> Result<DecodedRaster, AssetTextureError> {
    let bytes = verified.bytes();
    if bytes.len() > limits.encoded_bytes {
        return Err(AssetTextureError::EncodedLimit);
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(AssetTextureError::UnsupportedFormat);
    }
    if bytes.get(8..16) != Some(b"\0\0\0\rIHDR".as_slice()) {
        return Err(AssetTextureError::Decode(String::from(
            "missing initial IHDR",
        )));
    }
    let width = png_u32(bytes, 16)?;
    let height = png_u32(bytes, 20)?;
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > u32::from(limits.dimension)
        || height > u32::from(limits.dimension)
        || pixels > limits.pixels
    {
        return Err(AssetTextureError::DimensionLimit);
    }
    let rgba_bytes = pixels * 4;
    if rgba_bytes > limits.decoder_bytes {
        return Err(AssetTextureError::DecodeLimit);
    }
    let mut decoder_limits = image::io::Limits::default();
    decoder_limits.max_image_width = Some(u32::from(limits.dimension));
    decoder_limits.max_image_height = Some(u32::from(limits.dimension));
    decoder_limits.max_alloc = Some(limits.decoder_bytes);
    let decoder =
        PngDecoder::with_limits(Cursor::new(bytes), decoder_limits).map_err(decode_error)?;
    // Source color can be RGBA16; both source and converted RGBA8 buffers may coexist.
    if decoder
        .total_bytes()
        .checked_add(rgba_bytes)
        .is_none_or(|total| total > limits.decoder_bytes)
    {
        return Err(AssetTextureError::DecodeLimit);
    }
    let image = image::DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    let rgba = image.into_rgba8().into_raw();
    if u64::try_from(rgba.len()).ok() != Some(rgba_bytes) {
        return Err(AssetTextureError::Decode(String::from(
            "unexpected pixel buffer size",
        )));
    }
    Ok(DecodedRaster {
        width: u16::try_from(width).map_err(|_| AssetTextureError::DimensionLimit)?,
        height: u16::try_from(height).map_err(|_| AssetTextureError::DimensionLimit)?,
        rgba,
    })
}

fn png_u32(bytes: &[u8], start: usize) -> Result<u32, AssetTextureError> {
    let value = bytes
        .get(start..start + 4)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| AssetTextureError::Decode(String::from("truncated IHDR")))?;
    Ok(u32::from_be_bytes(value))
}

fn decode_error(error: image::ImageError) -> AssetTextureError {
    match error {
        image::ImageError::Limits(_) => AssetTextureError::DecodeLimit,
        other => AssetTextureError::Decode(other.to_string()),
    }
}

/// Upload boundary, mockable without a graphics context; inputs are bounded decoded rasters.
pub trait TextureUploader {
    /// Strong owned backend handle. Its destructor owns resource release.
    type Texture: fmt::Debug;

    /// Uploads one validated image, reporting backend errors distinctly from decoding failures.
    fn upload(&mut self, image: &DecodedRaster) -> Result<Self::Texture, String>;
}

/// Real Macroquad texture adapter. It never uploads encoded or unverified bytes.
#[derive(Debug, Default)]
pub struct MacroquadTextureUploader;

impl TextureUploader for MacroquadTextureUploader {
    type Texture = macroquad::texture::Texture2D;

    fn upload(&mut self, image: &DecodedRaster) -> Result<Self::Texture, String> {
        let texture =
            macroquad::texture::Texture2D::from_rgba8(image.width(), image.height(), image.rgba());
        texture.set_filter(macroquad::texture::FilterMode::Linear);
        Ok(texture)
    }
}

#[derive(Debug)]
struct ReadyTexture<T: fmt::Debug> {
    texture: T,
    width: u16,
    height: u16,
    bytes: u64,
}

#[derive(Debug)]
struct CacheEntry<T: fmt::Debug> {
    ready: Rc<ReadyTexture<T>>,
    retired: bool,
}

/// Strong texture lease plus validated source rectangle for one semantic resource.
///
/// Retain this value until the backend flushes the submitted draw queue. Clearing a cache does
/// not invalidate a lease; only the final strong owner's destruction releases the GPU handle.
#[derive(Debug)]
pub struct ResolvedSprite<T: fmt::Debug> {
    ready: Rc<ReadyTexture<T>>,
    source: AssetPixelRegion,
}

impl<T: fmt::Debug> Clone for ResolvedSprite<T> {
    fn clone(&self) -> Self {
        Self {
            ready: Rc::clone(&self.ready),
            source: self.source,
        }
    }
}

impl<T: fmt::Debug> ResolvedSprite<T> {
    /// The strongly owned texture used by this resource.
    pub fn texture(&self) -> &T {
        &self.ready.texture
    }

    /// Physical pixel width of the whole atlas.
    pub fn width(&self) -> u16 {
        self.ready.width
    }

    /// Physical pixel height of the whole atlas.
    pub fn height(&self) -> u16 {
        self.ready.height
    }

    /// Validated pixel region, or the whole physical image for non-atlas resources.
    pub const fn source(&self) -> AssetPixelRegion {
        self.source
    }
}

/// Observable synchronous load states; readiness means decoding and upload both succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetLoadState {
    /// Resolvable manifest entry whose bytes have not been submitted.
    Missing,
    /// A bounded decoded texture is available for draw submission.
    Ready,
    /// The most recent preparation attempt failed explicitly.
    Failed(AssetTextureError),
}

/// Texture counters and bounded residency estimate for reproducible runtime measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssetCacheStats {
    /// Successful physical decodes, including an upload that subsequently failed.
    pub decodes: u64,
    /// Successful physical uploads.
    pub uploads: u64,
    /// Repeated verified-file insertions served by an existing texture.
    pub hits: u64,
    /// Entries actually released after their final external lease disappeared.
    pub releases: u64,
    /// Estimated RGBA texture bytes, including retired entries that remain leased.
    pub resident_bytes: u64,
    /// Number of resident physical textures, including retired leased entries.
    pub resident_textures: usize,
}

/// Bound asset manifest and bounded physical texture cache (doc 04 §12).
///
/// All metadata passes the existing pack/game binding witness. Cache keys contain pack, exact
/// version, content hash and selected density. A bound pack is pinned; budget exhaustion fails
/// explicitly rather than evicting an image currently needed by a live match.
#[derive(Debug)]
pub struct SpriteAssetCache<U: TextureUploader = MacroquadTextureUploader> {
    uploader: U,
    limits: AssetCacheLimits,
    manifest: Option<AssetPackManifest>,
    entries: BTreeMap<TextureKey, CacheEntry<U::Texture>>,
    failures: BTreeMap<TextureKey, AssetTextureError>,
    stats: AssetCacheStats,
}

impl Default for SpriteAssetCache {
    fn default() -> Self {
        Self::new(MacroquadTextureUploader, AssetCacheLimits::default())
    }
}

impl<U: TextureUploader> SpriteAssetCache<U> {
    /// Creates a context-free empty cache. No GPU call occurs until verified bytes are inserted.
    pub fn new(uploader: U, limits: AssetCacheLimits) -> Self {
        Self {
            uploader,
            limits,
            manifest: None,
            entries: BTreeMap::new(),
            failures: BTreeMap::new(),
            stats: AssetCacheStats::default(),
        }
    }

    /// Binds a manifest to an exact expected game and pack/version, before accepting any file.
    ///
    /// Failed binding leaves the previous pack untouched. Rebinding a different pack retires its
    /// textures; existing leases keep them alive and accounted for until the draw queue is done.
    pub fn bind_pack(
        &mut self,
        manifest: &AssetPackManifest,
        game: &GameId,
        expected: &AssetPackRef,
    ) -> Result<(), AssetTextureError> {
        manifest
            .validate_binding(expected, game)
            .map_err(|error| AssetTextureError::Binding(error.to_string()))?;
        if manifest.files().len() > self.limits.entries {
            return Err(AssetTextureError::CacheBudget);
        }
        if self
            .manifest
            .as_ref()
            .is_some_and(|previous| previous != manifest)
        {
            self.clear();
        }
        self.manifest = Some(manifest.clone());
        Ok(())
    }

    /// Validates, decodes and uploads one exact bound physical file, or reuses its ready texture.
    ///
    /// Atlas validation covers every declared region using this physical file before the first
    /// upload. Failed preparation does not add a ready cache entry or increment upload counters.
    /// Taking ownership releases the encoded buffer after this preparation attempt; this cache
    /// retains textures and manifest metadata, rather than duplicating encoded file residency.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert_verified(
        &mut self,
        verified: OwnedVerifiedAssetBytes,
    ) -> Result<TextureKey, AssetTextureError> {
        let manifest = self.manifest.as_ref().ok_or(AssetTextureError::Unbound)?;
        if !manifest.files().contains(verified.file()) {
            return Err(AssetTextureError::ForeignFile);
        }
        let key = TextureKey::new(manifest.pack_ref(), verified.file());
        if let Some(entry) = self.entries.get_mut(&key) {
            if let Err(error) =
                validate_regions(manifest, &key, entry.ready.width, entry.ready.height)
            {
                self.failures.insert(key, error.clone());
                return Err(error);
            }
            entry.retired = false;
            self.failures.remove(&key);
            self.stats.hits = self.stats.hits.saturating_add(1);
            return Ok(key);
        }
        let result = self.prepare(&verified, &key);
        match result {
            Ok(ready) => {
                self.failures.remove(&key);
                self.entries.insert(
                    key.clone(),
                    CacheEntry {
                        ready: Rc::new(ready),
                        retired: false,
                    },
                );
                self.update_residency();
                Ok(key)
            }
            Err(error) => {
                self.failures.insert(key, error.clone());
                Err(error)
            }
        }
    }

    fn prepare(
        &mut self,
        verified: &OwnedVerifiedAssetBytes,
        key: &TextureKey,
    ) -> Result<ReadyTexture<U::Texture>, AssetTextureError> {
        self.collect_released();
        if self.entries.len() >= self.limits.entries {
            return Err(AssetTextureError::CacheBudget);
        }
        let image = decode_verified_raster(verified, self.limits)?;
        self.stats.decodes = self.stats.decodes.saturating_add(1);
        let manifest = self.manifest.as_ref().ok_or(AssetTextureError::Unbound)?;
        validate_regions(manifest, key, image.width(), image.height())?;
        let bytes = u64::from(image.width()) * u64::from(image.height()) * 4;
        if self.stats.resident_bytes.saturating_add(bytes) > self.limits.resident_bytes {
            return Err(AssetTextureError::CacheBudget);
        }
        // A key cannot be substituted by the uploader; the owned entry keeps its exact identity.
        debug_assert_eq!(key.hash(), verified.file().hash());
        let texture = self
            .uploader
            .upload(&image)
            .map_err(AssetTextureError::Upload)?;
        self.stats.uploads = self.stats.uploads.saturating_add(1);
        Ok(ReadyTexture {
            texture,
            width: image.width(),
            height: image.height(),
            bytes,
        })
    }

    /// Resolves a semantic resource using the existing pure density-selection contract.
    ///
    /// This makes no decoder, uploader or graphics call, and is suitable for complete-list
    /// preflight. Its strong returned lease must outlive the backend's queued draw submission.
    pub fn resolve(
        &self,
        asset: &AssetRef,
        density: AssetDensity,
    ) -> Result<ResolvedSprite<U::Texture>, AssetTextureError> {
        let (key, region) = self.resolve_metadata(asset, density)?;
        if let Some(error) = self.failures.get(&key) {
            return Err(error.clone());
        }
        let entry = self
            .entries
            .get(&key)
            .filter(|entry| !entry.retired)
            .ok_or_else(|| AssetTextureError::Missing(key.clone()))?;
        let source = region.unwrap_or_else(|| {
            AssetPixelRegion::new(
                0,
                0,
                u32::from(entry.ready.width),
                u32::from(entry.ready.height),
            )
            .expect("decoded raster dimensions are positive")
        });
        // A manifest change may reuse bytes and introduce new atlas coordinates; check again.
        if !region_fits(source, entry.ready.width, entry.ready.height) {
            return Err(AssetTextureError::AtlasBounds {
                asset: asset.clone(),
            });
        }
        Ok(ResolvedSprite {
            ready: Rc::clone(&entry.ready),
            source,
        })
    }

    fn resolve_metadata(
        &self,
        asset: &AssetRef,
        density: AssetDensity,
    ) -> Result<(TextureKey, Option<AssetPixelRegion>), AssetTextureError> {
        let manifest = self.manifest.as_ref().ok_or(AssetTextureError::Unbound)?;
        let bound = manifest
            .validate_binding(manifest.pack_ref(), manifest.game())
            .map_err(|error| AssetTextureError::Binding(error.to_string()))?;
        let resolved = bound
            .resolve(asset, density)
            .map_err(|_| AssetTextureError::UnknownResource(asset.clone()))?;
        Ok((
            TextureKey::new(manifest.pack_ref(), resolved.file()),
            resolved.region(),
        ))
    }

    /// Returns the real synchronous preparation state for one resolved resource.
    pub fn state(
        &self,
        asset: &AssetRef,
        density: AssetDensity,
    ) -> Result<AssetLoadState, AssetTextureError> {
        let (key, _) = self.resolve_metadata(asset, density)?;
        Ok(if let Some(error) = self.failures.get(&key) {
            AssetLoadState::Failed(error.clone())
        } else if self.entries.get(&key).is_some_and(|entry| !entry.retired) {
            AssetLoadState::Ready
        } else {
            AssetLoadState::Missing
        })
    }

    /// Retires all entries in one exact pack version, preserving outstanding texture leases.
    pub fn release_pack(&mut self, pack: &AssetPackRef) {
        for (key, entry) in &mut self.entries {
            if key.pack() == pack {
                entry.retired = true;
            }
        }
        self.failures.retain(|key, _| key.pack() != pack);
        if self
            .manifest
            .as_ref()
            .is_some_and(|manifest| manifest.pack_ref() == pack)
        {
            self.manifest = None;
        }
        self.collect_released();
    }

    /// Retires every texture and unbinds the manifest; leases remain valid until dropped.
    pub fn clear(&mut self) {
        for entry in self.entries.values_mut() {
            entry.retired = true;
        }
        self.manifest = None;
        self.failures.clear();
        self.collect_released();
    }

    /// Releases retired entries only after their final external lease has been dropped.
    pub fn collect_released(&mut self) {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| !entry.retired || Rc::strong_count(&entry.ready) > 1);
        self.stats.releases = self
            .stats
            .releases
            .saturating_add(u64::try_from(before - self.entries.len()).unwrap_or(u64::MAX));
        self.update_residency();
    }

    fn update_residency(&mut self) {
        self.stats.resident_bytes = self.entries.values().map(|entry| entry.ready.bytes).sum();
        self.stats.resident_textures = self.entries.len();
    }

    /// Actual decode/upload counts and current estimated residency.
    pub const fn stats(&self) -> AssetCacheStats {
        self.stats
    }
}

fn region_fits(region: AssetPixelRegion, width: u16, height: u16) -> bool {
    region
        .x()
        .checked_add(region.width())
        .is_some_and(|right| right <= u32::from(width))
        && region
            .y()
            .checked_add(region.height())
            .is_some_and(|bottom| bottom <= u32::from(height))
}

fn validate_regions(
    manifest: &AssetPackManifest,
    key: &TextureKey,
    width: u16,
    height: u16,
) -> Result<(), AssetTextureError> {
    for resource in manifest.resources() {
        for index in 0..resource.variant_count() {
            let Some(variant) = resource.variant(index) else {
                continue;
            };
            let Some(region) = variant.region() else {
                continue;
            };
            // Physical files with the same content and density share a texture. Validate every
            // such alias, so an invalid secondary region cannot hide behind a cache hit.
            let file = manifest
                .files()
                .iter()
                .find(|file| file.name() == variant.file())
                .expect("manifest validation guarantees resource file references");
            if TextureKey::new(manifest.pack_ref(), file) == *key
                && !region_fits(region, width, height)
            {
                return Err(AssetTextureError::AtlasBounds {
                    asset: resource.id().clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fmt::Write};

    use image::ImageEncoder;
    use tabula_assets::UnverifiedAssetBytes;

    use super::*;

    #[derive(Debug)]
    struct MockTexture {
        upload: u64,
        drops: Rc<Cell<u64>>,
    }

    impl Drop for MockTexture {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[derive(Debug, Default)]
    struct MockUploader {
        calls: Rc<Cell<u64>>,
        drops: Rc<Cell<u64>>,
        fail: Rc<Cell<bool>>,
    }

    impl TextureUploader for MockUploader {
        type Texture = MockTexture;

        fn upload(&mut self, image: &DecodedRaster) -> Result<Self::Texture, String> {
            assert_eq!(
                image.rgba().len(),
                usize::from(image.width()) * usize::from(image.height()) * 4
            );
            self.calls.set(self.calls.get() + 1);
            if self.fail.get() {
                return Err(String::from("mock device unavailable"));
            }
            Ok(MockTexture {
                upload: self.calls.get(),
                drops: Rc::clone(&self.drops),
            })
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let pixels = [17, 47, 97, 255].repeat(usize::try_from(width * height).unwrap());
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&pixels, width, height, image::ColorType::Rgba8)
            .unwrap();
        bytes
    }

    fn file(name: &str, payload: &[u8], density: Option<u8>) -> String {
        let density = density.map_or_else(String::new, |density| format!("density = {density}\n"));
        format!("[[files]]\nname = \"{name}\"\npath = \"fixture/1.0.0/{name}.png\"\nhash = \"{}\"\nbytes = {}\npriority = \"critical\"\n{density}", blake3::hash(payload), payload.len())
    }

    fn resource(id: &str, variants: &[(&str, Option<AssetPixelRegion>)]) -> String {
        let mut value = format!("[[resources]]\nid = \"{id}\"\n");
        for (file, region) in variants {
            writeln!(value, "[[resources.variants]]\nfile = \"{file}\"").unwrap();
            if let Some(region) = region {
                writeln!(
                    value,
                    "region = {{ x = {}, y = {}, width = {}, height = {} }}",
                    region.x(),
                    region.y(),
                    region.width(),
                    region.height()
                )
                .unwrap();
            }
        }
        value
    }

    fn manifest(version: &str, files: &[String], resources: &[String]) -> AssetPackManifest {
        AssetPackManifest::from_toml(&format!(
            "pack = \"fixture\"\nversion = \"{version}\"\ngame = \"com.example.fixture\"\n{}{}",
            files.concat(),
            resources.concat()
        ))
        .unwrap()
    }

    fn one_file(payload: &[u8]) -> AssetPackManifest {
        manifest(
            "1.0.0",
            &[file("atlas", payload, Some(1))],
            &[resource("tile/a", &[("atlas", None)])],
        )
    }

    fn verified(
        manifest: &AssetPackManifest,
        index: usize,
        payload: &[u8],
    ) -> OwnedVerifiedAssetBytes {
        manifest.files()[index]
            .verify_owned_bytes(UnverifiedAssetBytes::new(payload.to_vec()))
            .unwrap()
    }

    fn bind(cache: &mut SpriteAssetCache<MockUploader>, manifest: &AssetPackManifest) {
        cache
            .bind_pack(manifest, manifest.game(), manifest.pack_ref())
            .unwrap();
    }

    fn density(value: u8) -> AssetDensity {
        AssetDensity::new(value).unwrap()
    }

    fn asset() -> AssetRef {
        AssetRef::from_static("tile/a")
    }

    #[test]
    fn verified_png_decodes_exact_pixels_at_encoded_and_allocation_boundaries() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let verified = verified(&manifest, 0, &bytes);
        let exact = AssetCacheLimits::new(bytes.len(), 2, 4, 32, 16, 1).unwrap();
        let decoded = decode_verified_raster(&verified, exact).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2, 2));
        assert_eq!(decoded.rgba(), &[17, 47, 97, 255].repeat(4));
        let too_small = AssetCacheLimits {
            encoded_bytes: bytes.len() - 1,
            ..exact
        };
        assert_eq!(
            decode_verified_raster(&verified, too_small).unwrap_err(),
            AssetTextureError::EncodedLimit
        );
        let too_small = AssetCacheLimits {
            decoder_bytes: 31,
            ..exact
        };
        assert_eq!(
            decode_verified_raster(&verified, too_small).unwrap_err(),
            AssetTextureError::DecodeLimit
        );
    }

    #[test]
    fn oversized_and_zero_headers_reject_before_decoder_allocation() {
        for (width, height) in [
            (0, 2),
            (2, 0),
            (4097, 1),
            (1, 4097),
            (u32::MAX, u32::MAX),
            (3000, 3000),
        ] {
            let mut bytes = png(2, 2);
            bytes[16..20].copy_from_slice(&width.to_be_bytes());
            bytes[20..24].copy_from_slice(&height.to_be_bytes());
            let manifest = one_file(&bytes);
            assert_eq!(
                decode_verified_raster(
                    &verified(&manifest, 0, &bytes),
                    AssetCacheLimits::default()
                )
                .unwrap_err(),
                AssetTextureError::DimensionLimit
            );
        }
    }

    #[test]
    fn valid_hash_does_not_make_corrupt_or_unsupported_images_uploadable() {
        let valid = png(2, 2);
        let mut corrupt_crc = valid.clone();
        corrupt_crc[29] ^= 1;
        let corrupt = [corrupt_crc, valid[..24].to_vec(), b"not an image".to_vec()];
        for bytes in corrupt {
            let manifest = one_file(&bytes);
            let uploader = MockUploader::default();
            let calls = Rc::clone(&uploader.calls);
            let mut cache = SpriteAssetCache::new(uploader, AssetCacheLimits::default());
            bind(&mut cache, &manifest);
            assert_eq!(
                cache.state(&asset(), density(1)).unwrap(),
                AssetLoadState::Missing
            );
            let error = cache
                .insert_verified(verified(&manifest, 0, &bytes))
                .unwrap_err();
            assert!(matches!(
                error,
                AssetTextureError::Decode(_) | AssetTextureError::UnsupportedFormat
            ));
            assert_eq!(
                cache.state(&asset(), density(1)).unwrap(),
                AssetLoadState::Failed(error.clone())
            );
            assert_eq!(cache.resolve(&asset(), density(1)).unwrap_err(), error);
            assert_eq!(calls.get(), 0);
            assert_eq!(cache.stats().resident_textures, 0);
        }
    }

    #[test]
    fn byte_size_hash_and_foreign_file_fail_before_upload() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let mut changed = bytes.clone();
        changed[0] ^= 1;
        assert!(manifest.files()[0]
            .verify_owned_bytes(UnverifiedAssetBytes::new(changed))
            .is_err());
        assert!(manifest.files()[0]
            .verify_owned_bytes(UnverifiedAssetBytes::new(bytes[..bytes.len() - 1].to_vec()))
            .is_err());
        let foreign = super::tests::manifest(
            "1.0.0",
            &[file("other", &bytes, Some(1))],
            &[resource("tile/a", &[("other", None)])],
        );
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        assert_eq!(
            cache
                .insert_verified(verified(&foreign, 0, &bytes))
                .unwrap_err(),
            AssetTextureError::ForeignFile
        );
        assert_eq!(cache.stats().uploads, 0);
        assert_eq!(cache.stats().decodes, 0);
    }

    #[test]
    fn all_atlas_regions_and_physical_aliases_are_checked_before_upload() {
        let bytes = png(2, 2);
        let exact = AssetPixelRegion::new(1, 1, 1, 1).unwrap();
        for region in [
            AssetPixelRegion::new(2, 0, 1, 1).unwrap(),
            AssetPixelRegion::new(0, 2, 1, 1).unwrap(),
            AssetPixelRegion::new(0, 0, 3, 1).unwrap(),
        ] {
            let manifest = manifest(
                "1.0.0",
                &[
                    file("atlas", &bytes, Some(1)),
                    file("alias", &bytes, Some(1)),
                ],
                &[
                    resource("tile/a", &[("atlas", Some(exact))]),
                    resource("tile/b", &[("alias", Some(region))]),
                ],
            );
            let mut cache =
                SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
            bind(&mut cache, &manifest);
            assert_eq!(
                cache
                    .insert_verified(verified(&manifest, 0, &bytes))
                    .unwrap_err(),
                AssetTextureError::AtlasBounds {
                    asset: AssetRef::from_static("tile/b")
                }
            );
            assert_eq!(cache.stats().uploads, 0);
        }
        let manifest = manifest(
            "1.0.0",
            &[file("atlas", &bytes, Some(1))],
            &[resource("tile/a", &[("atlas", Some(exact))])],
        );
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        assert_eq!(cache.resolve(&asset(), density(1)).unwrap().source(), exact);
    }

    #[test]
    fn shared_resources_reuse_one_physical_decode_and_upload() {
        let bytes = png(2, 2);
        let manifest = manifest(
            "1.0.0",
            &[file("atlas", &bytes, Some(1))],
            &[
                resource("tile/a", &[("atlas", None)]),
                resource(
                    "tile/b",
                    &[("atlas", Some(AssetPixelRegion::new(0, 0, 1, 1).unwrap()))],
                ),
            ],
        );
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        for _ in 0..10 {
            cache
                .insert_verified(verified(&manifest, 0, &bytes))
                .unwrap();
        }
        let first = cache.resolve(&asset(), density(1)).unwrap();
        let second = cache
            .resolve(&AssetRef::from_static("tile/b"), density(1))
            .unwrap();
        assert!(std::ptr::eq(first.texture(), second.texture()));
        assert_eq!((first.width(), first.height()), (2, 2));
        assert_eq!(first.source(), AssetPixelRegion::new(0, 0, 2, 2).unwrap());
        assert_eq!(second.source(), AssetPixelRegion::new(0, 0, 1, 1).unwrap());
        assert_eq!(
            cache.stats(),
            AssetCacheStats {
                decodes: 1,
                uploads: 1,
                hits: 9,
                resident_bytes: 16,
                resident_textures: 1,
                ..AssetCacheStats::default()
            }
        );
    }

    #[test]
    fn cache_key_preserves_pack_version_hash_and_selected_density() {
        let bytes = png(2, 2);
        let manifest = manifest(
            "1.0.0",
            &[file("one", &bytes, Some(1)), file("three", &bytes, Some(3))],
            &[resource("tile/a", &[("one", None), ("three", None)])],
        );
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        let one = cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        let three = cache
            .insert_verified(verified(&manifest, 1, &bytes))
            .unwrap();
        assert_ne!(one, three);
        assert_eq!(one.hash(), three.hash());
        assert_eq!(one.density(), Some(density(1)));
        assert_eq!(three.density(), Some(density(3)));
        // Exact and nearest-density tie selection are delegated to BoundAssetPack.
        assert_eq!(
            cache
                .resolve(&asset(), density(1))
                .unwrap()
                .texture()
                .upload,
            1
        );
        assert_eq!(
            cache
                .resolve(&asset(), density(2))
                .unwrap()
                .texture()
                .upload,
            2
        );
        let v2 = super::tests::manifest(
            "2.0.0",
            &[file("one", &bytes, Some(1))],
            &[resource("tile/a", &[("one", None)])],
        );
        bind(&mut cache, &v2);
        let next = cache.insert_verified(verified(&v2, 0, &bytes)).unwrap();
        assert_ne!(one, next);
        assert_eq!(next.pack(), v2.pack_ref());
        let different = png(1, 1);
        let other = one_file(&different);
        assert_ne!(TextureKey::new(other.pack_ref(), &other.files()[0]), one);
        let another_pack = AssetPackRef::from_static("another", "1.0.0");
        assert_ne!(TextureKey::new(&another_pack, &manifest.files()[0]), one);
    }

    #[test]
    fn missing_resource_and_unbound_states_are_explicit() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        assert_eq!(
            cache.resolve(&asset(), density(1)).unwrap_err(),
            AssetTextureError::Unbound
        );
        bind(&mut cache, &manifest);
        assert!(matches!(
            cache.resolve(&asset(), density(1)).unwrap_err(),
            AssetTextureError::Missing(_)
        ));
        let unknown = AssetRef::from_static("unknown/resource");
        assert_eq!(
            cache.resolve(&unknown, density(1)).unwrap_err(),
            AssetTextureError::UnknownResource(unknown)
        );
    }

    #[test]
    fn release_and_rebind_keep_leases_alive_and_accounted_until_last_consumer() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let uploader = MockUploader::default();
        let drops = Rc::clone(&uploader.drops);
        let mut cache = SpriteAssetCache::new(uploader, AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        let first = cache.resolve(&asset(), density(1)).unwrap();
        let second = first.clone();
        cache.release_pack(manifest.pack_ref());
        assert_eq!(drops.get(), 0);
        assert_eq!(cache.stats().resident_bytes, 16);
        assert_eq!(first.texture().upload, 1);
        drop(first);
        cache.collect_released();
        assert_eq!(drops.get(), 0);
        bind(&mut cache, &manifest);
        cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        assert_eq!(cache.stats().uploads, 1);
        cache.clear();
        drop(second);
        cache.collect_released();
        assert_eq!(drops.get(), 1);
        assert_eq!(cache.stats().resident_bytes, 0);
        assert_eq!(cache.stats().releases, 1);
    }

    #[test]
    fn leased_retired_texture_blocks_budget_until_safe_release() {
        let bytes = png(2, 2);
        let first = one_file(&bytes);
        let next = manifest(
            "2.0.0",
            &[file("atlas", &bytes, Some(1))],
            &[resource("tile/a", &[("atlas", None)])],
        );
        let limits = AssetCacheLimits {
            resident_bytes: 16,
            ..AssetCacheLimits::default()
        };
        let mut cache = SpriteAssetCache::new(MockUploader::default(), limits);
        bind(&mut cache, &first);
        cache.insert_verified(verified(&first, 0, &bytes)).unwrap();
        let lease = cache.resolve(&asset(), density(1)).unwrap();
        bind(&mut cache, &next);
        assert_eq!(
            cache
                .insert_verified(verified(&next, 0, &bytes))
                .unwrap_err(),
            AssetTextureError::CacheBudget
        );
        assert_eq!(cache.stats().uploads, 1);
        assert_eq!(cache.stats().resident_bytes, 16);
        assert_eq!(lease.texture().upload, 1);
        drop(lease);
        cache.collect_released();
        cache.insert_verified(verified(&next, 0, &bytes)).unwrap();
        assert_eq!(
            cache.state(&asset(), density(1)).unwrap(),
            AssetLoadState::Ready
        );
        assert_eq!(cache.stats().uploads, 2);
        assert_eq!(cache.stats().resident_bytes, 16);
    }

    #[test]
    fn upload_failure_reports_failed_state_and_can_retry_without_poisoning_cache() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let uploader = MockUploader::default();
        let fail = Rc::clone(&uploader.fail);
        fail.set(true);
        let mut cache = SpriteAssetCache::new(uploader, AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        let error = cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap_err();
        assert_eq!(
            error,
            AssetTextureError::Upload(String::from("mock device unavailable"))
        );
        assert_eq!(cache.stats().uploads, 0);
        assert_eq!(cache.stats().resident_bytes, 0);
        assert_eq!(
            cache.state(&asset(), density(1)).unwrap(),
            AssetLoadState::Failed(error)
        );
        fail.set(false);
        cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        assert_eq!(
            cache.state(&asset(), density(1)).unwrap(),
            AssetLoadState::Ready
        );
        assert_eq!(cache.stats().decodes, 2);
        assert_eq!(cache.stats().uploads, 1);
    }

    #[test]
    fn failed_binding_is_transactional_and_cache_drop_does_not_invalidate_lease() {
        let bytes = png(2, 2);
        let manifest = one_file(&bytes);
        let uploader = MockUploader::default();
        let drops = Rc::clone(&uploader.drops);
        let mut cache = SpriteAssetCache::new(uploader, AssetCacheLimits::default());
        bind(&mut cache, &manifest);
        cache
            .insert_verified(verified(&manifest, 0, &bytes))
            .unwrap();
        let wrong = AssetPackRef::from_static("fixture", "9.0.0");
        assert!(matches!(
            cache.bind_pack(&manifest, manifest.game(), &wrong),
            Err(AssetTextureError::Binding(_))
        ));
        assert_eq!(
            cache.state(&asset(), density(1)).unwrap(),
            AssetLoadState::Ready
        );
        let lease = cache.resolve(&asset(), density(1)).unwrap();
        drop(cache);
        assert_eq!(drops.get(), 0);
        assert_eq!(lease.texture().upload, 1);
        drop(lease);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn bounded_entry_count_and_nonzero_configuration_are_enforced() {
        assert_eq!(
            AssetCacheLimits::new(0, 1, 1, 1, 1, 1).unwrap_err(),
            AssetTextureError::InvalidLimits
        );
        let bytes = png(2, 2);
        let manifest = manifest(
            "1.0.0",
            &[file("one", &bytes, Some(1)), file("two", &bytes, Some(2))],
            &[resource("tile/a", &[("one", None), ("two", None)])],
        );
        let mut cache = SpriteAssetCache::new(
            MockUploader::default(),
            AssetCacheLimits {
                entries: 1,
                ..AssetCacheLimits::default()
            },
        );
        assert_eq!(
            cache
                .bind_pack(&manifest, manifest.game(), manifest.pack_ref())
                .unwrap_err(),
            AssetTextureError::CacheBudget
        );
        assert_eq!(cache.stats().uploads, 0);
    }

    #[test]
    fn reused_bytes_revalidate_new_atlas_metadata_and_record_failure_state() {
        let bytes = png(2, 2);
        let original = one_file(&bytes);
        let changed = manifest(
            "1.0.0",
            &[file("atlas", &bytes, Some(1))],
            &[resource(
                "tile/a",
                &[("atlas", Some(AssetPixelRegion::new(2, 0, 1, 1).unwrap()))],
            )],
        );
        let mut cache = SpriteAssetCache::new(MockUploader::default(), AssetCacheLimits::default());
        bind(&mut cache, &original);
        cache
            .insert_verified(verified(&original, 0, &bytes))
            .unwrap();
        let lease = cache.resolve(&asset(), density(1)).unwrap();
        bind(&mut cache, &changed);
        let error = AssetTextureError::AtlasBounds { asset: asset() };
        assert_eq!(
            cache
                .insert_verified(verified(&changed, 0, &bytes))
                .unwrap_err(),
            error
        );
        assert_eq!(
            cache.state(&asset(), density(1)).unwrap(),
            AssetLoadState::Failed(error)
        );
        assert_eq!(cache.stats().uploads, 1);
        assert_eq!(cache.stats().decodes, 1);
        assert_eq!(lease.texture().upload, 1);
    }
}
