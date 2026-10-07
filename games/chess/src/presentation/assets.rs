//! Bounded exports of the licensed Chessnut artwork. (doc 04 §12, ADR-017)
//!
//! The local host resolves game-owned resource groups through the exact manifest.
//! Native fixtures use embedded bytes; WASM fetches only the selected physical
//! density through the host's bounded file source before verification and decode.
//! Full-resolution source art is retained outside Rust's `include_bytes!` paths.

use tabula_game_api::AssetRef;
use tabula_presentation::AssetPackRef;

use crate::rules::{Color, Piece, PieceKind};

/// Exact pinned metadata emitted by `cargo xtask pack-assets chess`.
pub const MANIFEST: &str = include_str!("../../assets/fixture.pack.toml");
/// Transparent 432×144 piece atlas at explicitly declared density 1.
#[cfg(not(target_arch = "wasm32"))]
pub const ATLAS_1X: &[u8] = include_bytes!("../../assets/pieces@1x.png");
/// Transparent 864×288 piece atlas at explicitly declared density 2.
#[cfg(not(target_arch = "wasm32"))]
pub const ATLAS_2X: &[u8] = include_bytes!("../../assets/pieces@2x.png");
/// Bounded 240×160 editorial entry artwork, never a board or interactive surface.
#[cfg(not(target_arch = "wasm32"))]
pub const COVER_1X: &[u8] = include_bytes!("../../assets/cover@1x.png");
/// Bounded 480×320 editorial entry artwork at density 2.
#[cfg(not(target_arch = "wasm32"))]
pub const COVER_2X: &[u8] = include_bytes!("../../assets/cover@2x.png");
/// Subtle transparent 128×128 wood grain, containing no gameplay or labels.
#[cfg(not(target_arch = "wasm32"))]
pub const GRAIN_1X: &[u8] = include_bytes!("../../assets/grain@1x.png");
/// The same restrained decorative grain at explicitly declared density 2.
#[cfg(not(target_arch = "wasm32"))]
pub const GRAIN_2X: &[u8] = include_bytes!("../../assets/grain@2x.png");
/// Exact manifest-local file names and bytes for the host's named preload adapter.
///
/// Density alone is insufficient because pieces and cover share 1x/2x densities.
#[cfg(not(target_arch = "wasm32"))]
pub const ALL_IMAGES: &[(&str, &[u8])] = &[
    ("pieces@1x.atlas", ATLAS_1X),
    ("pieces@2x.atlas", ATLAS_2X),
    ("cover@1x.atlas", COVER_1X),
    ("cover@2x.atlas", COVER_2X),
    ("grain@1x.atlas", GRAIN_1X),
    ("grain@2x.atlas", GRAIN_2X),
];

/// Exact retained third-party rights documents, never decoded as textures.
///
/// Native bundles retain these bytes; WASM staging distributes them as hashed
/// non-image pack files without adding them to first-board resource selection.
pub const NOTICES: &[(&str, &[u8])] = &[
    (
        "COPYRIGHT.txt",
        include_bytes!("../../assets/source/pieces/COPYRIGHT.txt"),
    ),
    (
        "LICENSE-Apache-2.0.txt",
        include_bytes!("../../assets/source/pieces/LICENSE-Apache-2.0.txt"),
    ),
    (
        "NOTICE.txt",
        include_bytes!("../../assets/source/pieces/NOTICE.txt"),
    ),
    (
        "PIECE-PROVENANCE.md",
        include_bytes!("../../assets/source/pieces/PIECE-PROVENANCE.md"),
    ),
];

/// Complete physical pack file set for host packaging, including legal text.
/// Runtime texture loading uses [`ALL_IMAGES`] and logical resource selection.
#[cfg(not(target_arch = "wasm32"))]
pub const ALL_FILES: &[(&str, &[u8])] = &[
    ("grain@1x.atlas", GRAIN_1X),
    ("grain@2x.atlas", GRAIN_2X),
    ("pieces@1x.atlas", ATLAS_1X),
    ("pieces@2x.atlas", ATLAS_2X),
    ("cover@1x.atlas", COVER_1X),
    ("cover@2x.atlas", COVER_2X),
    ("COPYRIGHT.txt", NOTICES[0].1),
    ("LICENSE-Apache-2.0.txt", NOTICES[1].1),
    ("NOTICE.txt", NOTICES[2].1),
    ("PIECE-PROVENANCE.md", NOTICES[3].1),
];

/// Logical resources needed by any local board, including later promotions.
///
/// The platform resolves these declarations without recognizing piece names,
/// filenames, resource prefixes or game ids. All twelve share one physical
/// atlas at the density selected by the pack's ordinary resolution contract.
#[must_use]
pub fn gameplay_resources() -> Vec<AssetRef> {
    let mut resources: Vec<_> = [Color::White, Color::Black]
        .into_iter()
        .flat_map(|color| {
            [
                PieceKind::King,
                PieceKind::Queen,
                PieceKind::Bishop,
                PieceKind::Knight,
                PieceKind::Rook,
                PieceKind::Pawn,
            ]
            .into_iter()
            .map(move |kind| piece_asset(Piece { color, kind }))
        })
        .collect();
    resources.push(grain_asset());
    resources
}

/// Entry-only artwork, unnecessary when the host skips standalone setup.
#[must_use]
pub fn setup_resources() -> Vec<AssetRef> {
    vec![cover_asset()]
}

/// Exact game-version-pinned artwork identity, independent of file resolution.
#[must_use]
pub fn asset_pack() -> AssetPackRef {
    AssetPackRef::from_static("chess", "0.3.0")
}

/// Logical piece identity only; physical regions and density remain in the pack.
#[must_use]
pub fn piece_asset(piece: Piece) -> AssetRef {
    let color = match piece.color {
        Color::White => "white",
        Color::Black => "black",
    };
    let kind = match piece.kind {
        PieceKind::King => "king",
        PieceKind::Queen => "queen",
        PieceKind::Bishop => "bishop",
        PieceKind::Knight => "knight",
        PieceKind::Rook => "rook",
        PieceKind::Pawn => "pawn",
    };
    AssetRef::new(format!("pieces/{color}-{kind}"))
        .expect("the closed Chess piece vocabulary contains canonical logical ids")
}

/// Logical entry-cover identity, available only to the game-specific entry view.
#[must_use]
pub fn cover_asset() -> AssetRef {
    AssetRef::new("catalog/cover").expect("the fixed cover resource is canonical")
}

/// Decorative square material; geometry and semantic state stay presenter-owned.
#[must_use]
pub fn grain_asset() -> AssetRef {
    AssetRef::new("board/grain").expect("the fixed grain resource is canonical")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_assets::{AssetDensity, AssetPackManifest};
    use tabula_core::GameId;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn all_bundled_files_are_exact_manifest_bytes_and_bounded_pngs() {
        let manifest = AssetPackManifest::from_toml(MANIFEST).unwrap();
        assert_eq!(manifest.files().len(), ALL_FILES.len());
        let mut encoded_total = 0;
        for (name, bytes) in ALL_IMAGES {
            let file = manifest
                .files()
                .iter()
                .find(|file| file.name().as_str() == *name)
                .unwrap();
            file.verify_bytes(bytes).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert!(width <= 864 && height <= 320);
            assert!(u64::from(width) * u64::from(height) * 4 <= 1024 * 1024);
            encoded_total += bytes.len();
        }
        // An explicit tiny local-pack bound, not permission to inline original art.
        assert!(encoded_total <= 320 * 1024);
    }

    #[test]
    fn legal_documents_are_hashed_pack_files_outside_sprite_resources() {
        let manifest = AssetPackManifest::from_toml(MANIFEST).unwrap();
        assert_eq!(NOTICES.len(), 4);
        for (name, bytes) in NOTICES {
            let file = manifest
                .files()
                .iter()
                .find(|file| file.name().as_str() == *name)
                .unwrap();
            file.verify_bytes(bytes).unwrap();
            assert_eq!(file.priority(), tabula_assets::AssetPriority::Low);
            assert!(file.density().is_none());
            assert!(manifest.resources().iter().all(|resource| {
                (0..resource.variant_count())
                    .all(|index| resource.variant(index).unwrap().file() != file.name())
            }));
        }
        assert_eq!(NOTICES[0].1, b"Copyright 2015 Alexis Luengas\n");
        assert!(std::str::from_utf8(NOTICES[1].1)
            .unwrap()
            .contains("Apache License"));
    }

    #[test]
    fn every_piece_and_cover_resolve_at_both_declared_densities() {
        let manifest = AssetPackManifest::from_toml(MANIFEST).unwrap();
        let game = GameId::new("com.tabula.chess").unwrap();
        let pack = asset_pack();
        let bound = manifest.validate_binding(&pack, &game).unwrap();
        assert_eq!(manifest.resources().len(), 14);
        for color in [Color::White, Color::Black] {
            for kind in [
                PieceKind::King,
                PieceKind::Queen,
                PieceKind::Bishop,
                PieceKind::Knight,
                PieceKind::Rook,
                PieceKind::Pawn,
            ] {
                for density in [1, 2] {
                    let density = AssetDensity::new(density).unwrap();
                    let selected = bound
                        .resolve(&piece_asset(Piece { color, kind }), density)
                        .unwrap();
                    assert_eq!(selected.file().density(), Some(density));
                    let region = selected.region().unwrap();
                    assert_eq!(region.width(), 64 * u32::from(density.get()));
                    assert_eq!(region.height(), 64 * u32::from(density.get()));
                    assert!(region.x() + region.width() <= 432 * u32::from(density.get()));
                    assert!(region.y() + region.height() <= 144 * u32::from(density.get()));
                }
            }
        }
        for density in [1, 2] {
            let density = AssetDensity::new(density).unwrap();
            let selected = bound.resolve(&cover_asset(), density).unwrap();
            assert_eq!(selected.file().density(), Some(density));
            assert!(selected.region().is_none());
            let grain = bound.resolve(&grain_asset(), density).unwrap();
            assert_eq!(grain.file().density(), Some(density));
            assert!(grain.region().is_none());
        }
    }
}
