# Werewolf village and card artwork — pack 0.2.0

The approved six-role folk-fantasy portraits are cropped from the supplied
original generated artwork. The #84 village-night and village-dawn scenes come
from the owner's `tabula-redesign 3` reference. No publisher artwork, logos or
fonts are imported.
See `source/PROVENANCE.md` and [the unchanged design provenance JSON](../../../docs/ui/werewolf-approved/design-provenance.json) for the
creation prompt, input hash and visual-reference boundary. The unavailable
unpublished Mac implementation was not read; this gameplay implementation is
recreated from inspected design references and the maintained rules contract.

Each front resource is an independent 256×256 / 512×512 PNG. One opaque common
back is 192×288 / 384×576. Front faction/title/rules, frame and selection labels
are live Rust typography. The common back carries no role-dependent pixel or
filename choice. All role art is publicly available; the private fact is which
role a player received, disclosed by the authorized projection and explicit
reveal. The full source atlas is never bundled as a runtime resource.

Each scene resource is a separate 640×360 / 1280×720 PNG, resized and quantized
from the retained original source. Pack 0.2.0 contains eighteen PNG exports for
nine logical resources: six role fronts, one common back and two village scenes.
All nine are prepared uniformly at the selected density, independent of roles.
Public avatar fallback silhouettes are geometry; this pack contains no account
avatar images or profile information.

| Density | PNG files | Encoded bytes | Estimated RGBA8 bytes | Largest individual RGBA8 image |
|---|---:|---:|---:|---:|
| 1x | 9 | 716,132 | 3,637,248 | 921,600 |
| 2x | 9 | 2,487,651 | 14,548,992 | 3,686,400 |
| Both exports | 18 | 3,203,783 | 18,186,240 | 3,686,400 |

These are static byte/dimension budgets, not measured GPU/decode latency claims.
[`budgets.json`](../../../docs/ui/werewolf-approved/budgets.json) pins every export's bytes, SHA-256 and decoded estimate; the pack
manifest additionally pins full BLAKE3 hashes. Native host fixture bytes pass
through the same verified LocalSpriteResources path as browser-fetched files.

Rebuild with Pillow, from repository root:

    python3 games/werewolf/assets/generate.py
    cargo xtask pack-assets werewolf
    cp target/asset-packs/werewolf/0.2.0/pack.toml games/werewolf/assets/fixture.pack.toml
    python3 games/werewolf/assets/generate.py --check
    cargo test -p tabula-game-werewolf --features presentation --lib presentation

The generator needs Pillow only during authoring. Runtime does not need Python,
WebP decoding, new fonts or a renderer-specific API. Game material uses approved
`game-art.werewolf` semantics; functional selection/focus/disabled states remain
shared Tabula semantics. The simulator is explicitly local, controls seat
perspectives, and provides no networking, accounts, chat or voice transport.
