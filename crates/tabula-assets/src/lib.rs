//! # `tabula-assets` — versioned, hashed asset packs
//!
//! > ## PHASE 3
//!
//! The planned product ships **per-game packs delivered from a CDN and cached
//! locally**, not bundled into app releases (ADR-017). This crate currently
//! owns only the pure manifest and identity contract that those future clients,
//! servers, and pack-build tools will consume.
//!
//! ## The manifest (doc 04 §12.3)
//!
//! ```toml
//! # assets/packs/chess/pack.toml  →  served as pack.json
//! pack    = "chess"
//! version = "1.0.0"
//! game    = "com.tabula.chess"
//!
//! [[files]]
//! name     = "pieces@2x.atlas"
//! path     = "chess/1.0.0/pieces@2x.b3-4f8a....png"   # content-hashed path
//! hash     = "4f8a..."
//! bytes    = 412_003
//! priority = "critical"      # critical | high | low
//! density  = 2
//!
//! [atlas.pieces]
//! white-knight = [0, 0, 128, 128]   # so presenters use AssetRef::new("pieces/white-knight")
//! ```
//!
//! ## Planned rules for trustworthy asset delivery
//!
//! 1. **Content-hashed paths.** A given URL's bytes never change, so the cache is
//!    immutable and `Cache-Control: immutable` is honest.
//! 2. **Integrity check on load.** A cached file whose blake3 does not match the
//!    manifest is discarded, not used. Silent corruption is worse than a re-fetch.
//! 3. **Priority-driven loading.** `critical` blocks the branded loader; `high`
//!    loads during the first turn; `low` loads lazily. A player should be looking
//!    at a board, not a spinner.
//! 4. **Density variants** are picked from `FrameCtx.dpi`, so a phone does not
//!    download a desktop atlas.
//!
//! ## Current implementation boundary
//!
//! Implemented now:
//! - manifest TOML parsing with unknown-field rejection;
//! - validated pack, file-name, path, size, density, priority, and hash metadata;
//! - duplicate file-name/path detection;
//! - typed pack binding through [`AssetPackRef`].
//!
//! Not implemented yet:
//! - asset sources, network fetch, or filesystem loading;
//! - cache management or CDN URL/signature generation;
//! - byte-level integrity verification;
//! - density or atlas resolution;
//! - decoding or renderer handles.

#![forbid(unsafe_code)]

mod manifest;

pub use manifest::{
    AssetByteSize, AssetByteSizeError, AssetContentHash, AssetContentHashError, AssetDensity,
    AssetDensityError, AssetFile, AssetFileName, AssetFileNameError, AssetPackId, AssetPackIdError,
    AssetPackManifest, AssetPackRef, AssetPackRefError, AssetPackVersion, AssetPath,
    AssetPathError, AssetPriority, ManifestBindingError, ManifestError,
};
