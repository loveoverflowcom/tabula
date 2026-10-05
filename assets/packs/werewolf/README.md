# Werewolf standalone original-art fixture

The approved six-role folk-fantasy portraits are cropped from the supplied
original generated artwork. No publisher artwork, logos or fonts are imported.
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

The two-density pack is 1,639,008 encoded bytes and 8,970,240 estimated decoded
RGBA8 bytes across *both* densities. At a single selected density the renderer
loads seven separate files; maximum individual RGBA8 allocation is 1 MiB. These
are static byte/dimension budgets, not measured GPU/decode latency claims.
[`budgets.json`](../../../docs/ui/werewolf-approved/budgets.json) pins every export's bytes, SHA-256 and decoded estimate; the pack
manifest additionally pins full BLAKE3 hashes. Native host fixture bytes pass
through the same verified LocalSpriteResources path as browser-fetched files.

Rebuild with Pillow, from repository root:

    python3 assets/packs/werewolf/generate.py
    cargo xtask pack-assets werewolf
    cp target/asset-packs/werewolf/0.1.0/pack.toml assets/packs/werewolf/fixture.pack.toml
    python3 assets/packs/werewolf/generate.py --check
    cargo test -p tabula-game-werewolf --features presentation --lib presentation

The generator needs Pillow only during authoring. Runtime does not need Python,
WebP decoding, new fonts or a renderer-specific API. Game material uses approved
`game-art.werewolf` semantics; functional selection/focus/disabled states remain
shared Tabula semantics. The simulator is explicitly local, controls seat
perspectives, and provides no networking, accounts, chat or voice transport.
