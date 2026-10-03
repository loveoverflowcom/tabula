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

## Pack build

The Phase-3 builder is active. The manifest schema is still parsed and
resolved purely by tabula-assets; the builder is tooling in xtask, not a
runtime source adapter. It reads opaque source bytes, computes exact sizes and
full lowercase BLAKE3 digests, writes deterministic content-addressed files,
generates pack.toml, validates it through AssetPackManifest, and verifies
every staged file through AssetFile::verify_bytes before publishing.

```bash
cargo xtask pack-assets <game>
```

The builder reads the game identity and pinned [assets].pack from
games/<game>/game.toml, then reads the builder-only
assets/packs/<game>/pack.source.toml. The source manifest contains only
explicit source files and logical-resource mappings; generated path, hash, and
byte-size fields are not accepted:

```toml
[[files]]
name = "pieces@1x.atlas"
source = "pieces@1x.png"
priority = "critical"
density = 1

[[resources]]
id = "pieces/white-knight"

[[resources.variants]]
file = "pieces@1x.atlas"
region = { x = 0, y = 0, width = 64, height = 64 }
```

It produces content-hashed files plus a runtime pack.toml under
target/asset-packs/<pack>/<version>/:

```toml
pack    = "chess"
version = "1.0.0"
game    = "com.tabula.chess"

[[files]]
name     = "pieces@1x.atlas"
path     = "chess/1.0.0/pieces@1x.b3-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.png"   # full BLAKE3
hash     = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
bytes    = 412_003
priority = "critical"      # critical | high | low
density  = 1

[[resources]]
id = "pieces/white-knight"

[[resources.variants]]
file = "pieces@1x.atlas"
region = { x = 0, y = 0, width = 64, height = 64 }
```

`AssetRef` values resolve only through these explicit resource declarations;
the resolver never guesses from a filename, path, extension, density suffix, or
atlas name. Binding a manifest to its expected pack and game produces the
pure `BoundAssetPack` view, which deterministically returns metadata only
(`AssetFile` plus an optional structural pixel region).

Byte-level integrity verification is also pure: untrusted raw bytes pass through
`AssetFile::verify_bytes` or `AssetFile::verify_owned_bytes`, which enforce declared
byte length before the BLAKE3 digest. The former returns a borrowed
`VerifiedAssetBytes` witness; the latter consumes owned `UnverifiedAssetBytes` and
returns an immutable, owned `OwnedVerifiedAssetBytes` value. `load_verified(file,
source)` is the thin async-capable composition of `AssetSource::fetch` and the
owned trust transition. A successful source read is not an integrity success, and
source errors remain distinct from integrity errors. The source port and memory
adapter neither perform filesystem/network I/O nor create a renderer or audio
handle.

The builder intentionally does not generate atlases, decode or convert media,
load from a filesystem/HTTP/browser source, manage a cache, upload to a CDN, or
create renderer/audio handles. Those remain delivery/runtime work.

## Delivery rules

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
