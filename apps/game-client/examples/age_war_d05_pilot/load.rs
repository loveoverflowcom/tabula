//! Private sealed packs through the existing verified-asset path (doc 04 §12).
//!
//! Files come from a local directory, cross `load_verified` (size + BLAKE3)
//! and only then reach the renderer's bounded decoder/texture cache. Groups are
//! selected by pure manifest resolution, never by guessing file names.

use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    pin::pin,
    task::{Context, Poll, Waker},
};

use tabula_assets::{
    load_verified, AssetDensity, AssetFile, AssetPackManifest, AssetPackRef, AssetPath,
    AssetSource, OwnedVerifiedAssetBytes, UnverifiedAssetBytes,
};
use tabula_core::GameId;
use tabula_presentation::AssetRef;
use tabula_render_macroquad::assets::{SpriteAssetCache, TextureUploader};

/// Provisional, unregistered reverse-DNS binding for the private packs.
pub const GAME_ID: &str = "com.tabula.agewar";

/// Reads exact manifest paths below one private pack root.
#[derive(Debug)]
pub struct DirAssetSource {
    root: PathBuf,
}

impl DirAssetSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetSource for DirAssetSource {
    type Error = std::io::Error;

    fn fetch<'a>(
        &'a self,
        path: &'a AssetPath,
    ) -> impl Future<Output = Result<UnverifiedAssetBytes, Self::Error>> + 'a {
        // AssetPath is a validated canonical relative path (no `..`, no root).
        std::future::ready(
            std::fs::read(self.root.join(path.as_str())).map(UnverifiedAssetBytes::new),
        )
    }
}

/// Drives an already-ready local future; the directory source never suspends.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// One parsed and bound private pack.
#[derive(Debug)]
pub struct Pack {
    pub manifest: AssetPackManifest,
    pub reference: AssetPackRef,
    pub game: GameId,
    pub source: DirAssetSource,
    pub manifest_bytes: usize,
}

impl Pack {
    pub fn open(root: &Path, pack: &str, version: &str) -> Result<Self, String> {
        let path = root.join(pack).join(version).join("pack.toml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest = AssetPackManifest::from_toml(&text).map_err(|e| format!("{pack}: {e}"))?;
        let reference =
            AssetPackRef::parse(&format!("{pack}@{version}")).map_err(|e| e.to_string())?;
        let game = GameId::new(GAME_ID).map_err(|e| e.to_string())?;
        manifest
            .validate_binding(&reference, &game)
            .map_err(|e| format!("{pack}: {e}"))?;
        Ok(Self {
            manifest,
            reference,
            game,
            source: DirAssetSource::new(root),
            manifest_bytes: text.len(),
        })
    }

    /// Resolves one logical resource to its physical file at a density.
    pub fn file_for(&self, resource: &str, density: u8) -> Result<&AssetFile, String> {
        let asset = AssetRef::new(resource).map_err(|e| format!("{resource}: {e}"))?;
        let bound = self
            .manifest
            .validate_binding(&self.reference, &self.game)
            .map_err(|e| e.to_string())?;
        let resolved = bound
            .resolve(
                &asset,
                AssetDensity::new(density).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("{resource}: {e}"))?;
        self.manifest
            .files()
            .iter()
            .find(|file| *file == resolved.file())
            .ok_or_else(|| format!("{resource}: unresolved file"))
    }

    /// Distinct files selected by every resource with a prefix (pure resolution).
    pub fn files_for_prefix(&self, prefix: &str, density: u8) -> Result<Vec<&AssetFile>, String> {
        let mut seen = BTreeSet::new();
        let mut files = Vec::new();
        for resource in self.manifest.resources() {
            if resource.id().as_str().starts_with(prefix) {
                let file = self.file_for(resource.id().as_str(), density)?;
                if seen.insert(file.name().as_str().to_string()) {
                    files.push(file);
                }
            }
        }
        Ok(files)
    }

    pub fn verified(&self, file: &AssetFile) -> Result<OwnedVerifiedAssetBytes, String> {
        block_on(load_verified(file, &self.source)).map_err(|e| format!("{}: {e}", file.path()))
    }

    pub fn text(&self, resource: &str) -> Result<String, String> {
        let file = self.file_for(resource, 1)?;
        String::from_utf8(self.verified(file)?.bytes().to_vec()).map_err(|e| e.to_string())
    }
}

/// Wall time and byte facts for preparing one set of textures.
#[derive(Clone, Debug, Default)]
pub struct LoadStats {
    pub files: usize,
    pub encoded_bytes: u64,
    pub verify_ms: f64,
    pub decode_upload_ms: f64,
}

/// Verifies then decodes/uploads files into a cache already bound to `pack`.
pub fn prepare<U: TextureUploader>(
    pack: &Pack,
    cache: &mut SpriteAssetCache<U>,
    files: &[&AssetFile],
    now: impl Fn() -> f64,
) -> Result<LoadStats, String> {
    let mut stats = LoadStats::default();
    for file in files {
        let started = now();
        let verified = pack.verified(file)?;
        let verified_at = now();
        stats.encoded_bytes += verified.bytes().len() as u64;
        cache
            .insert_verified(verified)
            .map_err(|e| format!("{}: {e}", file.path()))?;
        stats.verify_ms += (verified_at - started) * 1000.0;
        stats.decode_upload_ms += (now() - verified_at) * 1000.0;
        stats.files += 1;
    }
    Ok(stats)
}

/// Linux process memory from `/proc/self/status` (KiB); absent elsewhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessMemory {
    pub rss_kib: u64,
    pub peak_kib: u64,
}

pub fn process_memory() -> ProcessMemory {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next()?.parse().ok())
            .unwrap_or(0)
    };
    ProcessMemory {
        rss_kib: field("VmRSS:"),
        peak_kib: field("VmHWM:"),
    }
}
