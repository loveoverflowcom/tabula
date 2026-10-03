# Tiles renderer fixture

This original, CC0-1.0 fixture covers all 23 variants in Tabula's own `TILE_SET`.
It is a bounded local renderer demo fixture, not a production delivery policy.
ADR-017 still requires externally delivered per-game production packs.

The editable source is `generate.py` (explicit city/road/monastery/pennant
geometry) and `tiles.svg` (named variant groups). No external artwork or fonts
are used. White coverage is tinted with semantic theme tokens in the presenter;
roads are hollow lanes, cities have stone slots, monasteries have a roof/cross,
and pennants use a cut-out flag. These shapes communicate terrain without
depending on colour.

The atlas is 432×288 at 1x and 864×576 at 2x. Each 64×64 logical region has a
4-pixel transparent gutter inside a 72-pixel cell. Physical 2x regions double
every coordinate and dimension. Resource identity is `tiles/<TileDef.name>`;
density is explicitly declared, never inferred from a filename.

Rebuild from the repository root:

```bash
python3 assets/packs/tiles/generate.py
cargo xtask pack-assets tiles
cp target/asset-packs/tiles/0.1.0/pack.toml assets/packs/tiles/fixture.pack.toml
```

The pack builder validates the generated manifest and checks every staged size
and BLAKE3 digest. The fixture host maps the manifest's selected physical path
to these local PNG bytes through `MemoryAssetSource`; decode and texture upload
still consume only verified bytes. The checked-in `fixture.pack.toml` is the
exact pinned metadata used by the local demo and presentation tests. Regenerate
it after changing source art, and review the manifest and sprite snapshots.
