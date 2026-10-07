# Chess standalone artwork fixture

This bounded local pack exports the approved Apache-2.0 Chessnut pieces and
editorial entry cover from the Chess design handoff. It exercises the production
logical-resource, binding, verification and bounded PNG-decoding path. It is an
explicit small-game local fixture under ADR-017, not a general asset-delivery or
CDN/cache implementation.

## Sources and provenance

- `source/pieces/*.svg` are byte-identical official Lichess Chessnut SVGs by
  Alexis Luengas, pinned to lila@5820854ca42e082891d6a47ceb3b57d33ebb7fe1
- `source/pieces/manifest.json` retains upstream Git blob identities and SHA-256;
  the generator checks all twelve exact byte streams and self-contained viewBoxes
- `source/pieces/PIECE-PROVENANCE.md`, COPYRIGHT.txt, LICENSE-Apache-2.0.txt and
  NOTICE.txt retain the source, copyright, license and added attribution notice.
  All four are also hashed, low-priority non-image files in the published pack;
  LILA-COPYING.md retains Lichess's explicit third-party license exception
- `source/chess-atmosphere.png` is the retained full-resolution original, with its
  generation prompt and provenance alongside it; it is never embedded in the
  gameplay binary. `xtask` includes it only for build-time host packaging
- `source/standalone-cover-provenance.md` records the standalone host's
  decorative cover aliases; staging derives both from this game-owned pack,
  without retaining app-owned copies of the art
- No font or external SVG dependency is used by this pack. Chessnut uses Apache
  2.0; the surrounding software's existing license remains unchanged

## Bounded exports

`pieces@1x.png` is a transparent 432×144 RGBA atlas. `pieces@2x.png` is 864×288.
Twelve pieces share 64×64 logical regions in 72×72 cells with transparent
4-pixel 1x gutters. Source black/white geometry and internal detail are retained; every region
and gutter is doubled at density 2.

`cover@1x.png` and `cover@2x.png` are 240×160 and 480×320 RGB PNGs. The cover is
only an entry illustration; it contains no controls, rules state, clocks or
board hitboxes. The four PNGs total 303,308 encoded bytes. The complete eight-file
pack, including legal text, is 317,401 bytes (310 KiB rounded up), with
approximately 2 MB of decoded RGBA. The largest individual decoded allocation
is below 1 MB. The asset-module regression test caps the inline fixture at
320 KiB and rejects larger dimensions; the renderer applies its own limits too.

Logical IDs are `pieces/{white,black}-{king,queen,bishop,knight,rook,pawn}` and
`catalog/cover`. Presenters use only these IDs. Physical files, densities,
regions, byte counts and full BLAKE3 digests are manifest metadata.

The host must map bytes by exact file name, because cover and pieces both have
1x and 2x variants. `assets::ALL_IMAGES` supplies the four PNGs;
`assets::ALL_FILES` adds the four rights documents for packaging. Bytes still pass
through `MemoryAssetSource` and `load_verified` before renderer decode/upload.
Native main retains that complete file set in its live, manifest-bound generic
`LocalSpriteResources` owner via `with_embedded_files`; `with_embedded_images`
remains compatible for image-only fixtures. A native test fetches and verifies
each rights document, then proves setup/gameplay decode only their two PNGs.
Legal text has no sprite resource declaration and never enters texture decode or
first-board loading. WASM staging keeps each file's validated PNG/TXT/MD extension.

## Rebuild and verify

From the repository root, with Inkscape and Pillow installed:

```sh
python3 games/chess/assets/generate.py
cargo xtask pack-assets chess
cp target/asset-packs/chess/0.3.0/pack.toml games/chess/assets/fixture.pack.toml
python3 games/chess/assets/generate.py --check
cargo test --locked -p tabula-game-chess --features presentation --lib presentation::assets::tests
```

The generator verifies the pinned source identities, exports the unchanged square
SVG viewBox using Inkscape with uniform aspect-ratio-preserving scale, preserves
straight-alpha edge coverage, strips PNG metadata, downsamples the cover using
Lanczos and emits explicit source-resource declarations. Runtime builds require
neither Inkscape nor Pillow. `xtask pack-assets` checks the published manifest,
binding, exact sizes and every digest before publication.

The game-owned sprite helper uses a 97%-of-cell square viewport, with the source's
own transparent inset intact. Board, moving/dragged pieces, promotion and HUD
piece examples share that helper. No rotation, whole-piece state tint, new
renderer branch or runtime SVG rasterizer is introduced. Earlier issue-85
material/motion work remains independent; its reserved 0.2.0 pack is not reused.

Chess's decorative material roles are authored under `game-art.chess` in
`tokens.toml` and generated into the Rust/CSS/JSON adapters. Shared M3 navigation
and functional selection/focus/legal/last-action/threat semantics remain system
roles. `piece-tint` is white in all schemes so colored raster art is unchanged.
