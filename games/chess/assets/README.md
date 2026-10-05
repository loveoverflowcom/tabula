# Chess standalone artwork fixture

This bounded local pack exports the approved Tabula Carved Staunton pieces and
editorial entry cover from the Chess design handoff. It exercises the production
logical-resource, binding, verification and bounded PNG-decoding path. It is an
explicit small-game local fixture under ADR-017, not a general asset-delivery or
CDN/cache implementation.

## Sources and provenance

- `source/pieces/*.svg` are byte-identical canonical SVGs from the design handoff
- `source/pieces/build_piece_set.py` retains the original editable geometry source
- `source/pieces/PROVENANCE.md` records original authorship; no third-party piece
  images, fonts, icon sets or external SVG dependencies were used
- `source/chess-atmosphere.png` is the retained full-resolution original, with its
  generation prompt and provenance alongside it; it is never embedded in the
  gameplay binary. `xtask` includes it only for build-time host packaging
- `source/standalone-cover-provenance.md` records the standalone host's
  decorative cover aliases; staging derives both from this game-owned pack,
  without retaining app-owned copies of the art
- No third-party font is used by this pack. The surrounding software's existing
  license is unchanged; the retained original-art provenance adds no new license
  grant or third-party attribution requirement

## Bounded exports

`pieces@1x.png` is a transparent 432×144 RGBA atlas. `pieces@2x.png` is 864×288.
Twelve pieces share 64×64 logical regions in 72×72 cells with transparent
4-pixel 1x gutters. White and black use the same source geometry; every region
and gutter is doubled at density 2.

`cover@1x.png` and `cover@2x.png` are 240×160 and 480×320 RGB PNGs. The cover is
only an entry illustration; it contains no controls, rules state, clocks or
board hitboxes. The complete four-file fixture is 280,240 encoded bytes and
approximately 2 MB of decoded RGBA. The largest individual decoded allocation
is below 1 MB. The asset-module regression test caps the inline fixture at
320 KiB and rejects larger dimensions; the renderer applies its own limits too.

Logical IDs are `pieces/{white,black}-{king,queen,bishop,knight,rook,pawn}` and
`catalog/cover`. Presenters use only these IDs. Physical files, densities,
regions, byte counts and full BLAKE3 digests are manifest metadata.

The host must map bytes by exact file name, because cover and pieces both have
1x and 2x variants. `assets::ALL_IMAGES` supplies that mapping. Bytes still pass
through `MemoryAssetSource` and `load_verified` before renderer decode/upload.

## Rebuild and verify

From the repository root, with Inkscape and Pillow installed:

```sh
python3 games/chess/assets/generate.py
cargo xtask pack-assets chess
cp target/asset-packs/chess/0.1.0/pack.toml games/chess/assets/fixture.pack.toml
python3 games/chess/assets/generate.py --check
cargo test -p tabula-game-chess --features presentation --lib presentation::assets::tests
```

The generator exports the unchanged SVG viewBox using Inkscape, preserves
straight-alpha edge coverage, strips PNG metadata, downsamples the cover using
Lanczos and emits explicit source-resource declarations. Runtime builds require
neither Inkscape nor Pillow. `xtask pack-assets` checks the published manifest,
binding, exact sizes and every digest before publication.

Chess's decorative material roles are authored under `game-art.chess` in
`tokens.toml` and generated into the Rust/CSS/JSON adapters. Shared M3 navigation
and functional selection/focus/legal/last-action/threat semantics remain system
roles. `piece-tint` is white in all schemes so colored raster art is unchanged.
