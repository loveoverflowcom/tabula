//! # `tabula-assets` — versioned, hashed asset packs
//!
//! > ## PHASE 3
//!
//! Assets ship as **per-game packs delivered from a CDN and cached locally**, not
//! bundled into app releases (ADR-017). Otherwise every app release grows with
//! every game, which is fatal for mobile.
//!
//! Shared by the client (loading), the server (manifest validation, CDN URL
//! signing), and the pack build tooling in `xtask` — which is why it is a crate
//! rather than a module in the client.
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
//! ## Rules that make the cache trustworthy
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
//! This phase owns only the pure, validated manifest contract. Fetching,
//! caching, integrity checking against bytes, density selection, decoding, and
//! backend handles are deliberately deferred.

#![forbid(unsafe_code)]

mod manifest;

pub use manifest::{
    AssetByteSize, AssetByteSizeError, AssetContentHash, AssetContentHashError, AssetDensity,
    AssetDensityError, AssetFile, AssetFileName, AssetFileNameError, AssetPackId, AssetPackIdError,
    AssetPackManifest, AssetPackVersion, AssetPath, AssetPathError, AssetPriority, ManifestError,
};
