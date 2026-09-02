# assets/

```text
brand/   logo, shared fonts, shared icons — the things that are the same in every game
packs/   per-game source assets and pack build inputs
```

## Planned asset packs, not bundled assets (ADR-017)

The target architecture ships **versioned, hashed, per-game packs** delivered
from a CDN and cached locally. They are not bundled into app releases.

The reason is arithmetic: bundling means every app release grows with every game.
At game five that is annoying; at game twenty it is fatal on mobile, and it means
an art fix requires a store review.

## Planned pack build

The pack builder and delivery pipeline are future work. The command and manifest
below describe the target shape; the current repository does not read asset
files, emit packs, fetch resources, or manage a cache.

```bash
cargo xtask pack-assets <game>  # planned
```

Reads `assets/packs/<game>/`, produces content-hashed files plus a
`pack.toml` → served as `pack.json` (doc 04 §12.3):

```toml
pack    = "chess"
version = "1.0.0"
game    = "com.tabula.chess"

[[files]]
name     = "pieces@2x.atlas"
path     = "chess/1.0.0/pieces@2x.b3-4f8a....png"   # content-hashed
hash     = "4f8a..."
bytes    = 412_003
priority = "critical"      # critical | high | low
density  = 2

[atlas.pieces]
white-knight = [0, 0, 128, 128]   # presenters use AssetRef::new("pieces/white-knight")
```

## Planned delivery rules

1. **Content-hashed paths.** A URL's bytes never change, so `immutable` caching
   is honest and cache invalidation is not a problem we have.
2. **Integrity-checked on load.** A cached file whose blake3 does not match the
   manifest is discarded, not used.
3. **Priority-tagged.** `critical` blocks the branded loader; `high` loads during
   the first turn; `low` loads lazily. Players should be looking at a board.
4. **Density variants** (`@1x/@2x/@3x`) selected from `FrameCtx.dpi` — a phone
   does not download a desktop atlas.
5. **No assets in the binary** beyond a single placeholder. If a game renders
   without its pack, it renders placeholders — it does not fail to start.

## Planned brand-asset exception

`brand/` is small, shared, and needed before any pack loads (the loader itself is
branded). It may be embedded.
