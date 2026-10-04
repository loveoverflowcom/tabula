# Isolated prototype dependencies

This directory is issue #60 tooling. None of these npm packages enters a Cargo workspace
dependency graph or production route. `pixi.js` is pinned to **8.22.0**, the official
[1 October 2026 release](https://github.com/pixijs/pixijs/releases/tag/v8.22.0).
The [tagged PixiJS license](https://github.com/pixijs/pixijs/blob/v8.22.0/LICENSE) is MIT.
`package-lock.json` fixes every transitive version, registry URL and SHA-512 npm integrity value.

`npm run licenses` checks the installed lock against package versions, registry integrity pins,
and the relevant MIT / BSD-3-Clause / ISC subset of `deny.toml`'s third-party license policy.
It writes `dependency-inventory.json` and `licenses/npm-NOTICES.txt`. All twelve installed
packages passed. The `@pixi/colord` tarball declares MIT but omits a standalone license file;
its [Pixi-maintained fork notice](https://github.com/pixijs/colord/blob/master/LICENSE.md)
is retained separately as `licenses/colord-MIT.txt` and included in the combined notices.
This supplementary notice was fetched on 3 October 2026; no source code is fetched from that
unpinned branch.

`npm audit --json` executed against the exact lock on 3 October 2026 with Node 22.15.0 /
npm 10.9.2 and reported **zero** known vulnerabilities. This is the registry advisory result
at that time, not a guarantee about browser plugins, GPU drivers, or all future advisories.
Installation used `--ignore-scripts`; the dependency has no prototype install-time scripts.

Tiles atlas sources, generated PNGs, manifest hashes and CC0 notice remain under
`games/tiles/assets/`. Rust verifies the production manifest's BLAKE3 bytes before export;
staging computes an additional SHA-256 pin for the same bytes. Browser Web Crypto verifies
that additional pin and expected byte count before decoding. The SHA-256 staging check does
not replace the production BLAKE3 pack contract.

The comparison reuses `macroquad` **0.4.16**'s exact embedded `src/ProggyClean.ttf` bytes
from the Cargo.lock-resolved source. The font is authored by Tristan Grimmer and carries its
own [MIT license](https://github.com/bluescan/proggyfonts/blob/master/LICENSE), retained
as `licenses/ProggyClean-MIT.txt`; the crate license alone is not its provenance.
`stage.py` records font byte size and SHA-256 in `runtime-assets.json`, and the adapter loads
only those verified bytes as `TabulaSpikeProggy`. The font binary is staged from the pinned
dependency, not committed as a new authored asset. Keep its notice when copying a stage.

The two backends share font bytes, token sizes, atlas resources and RenderList geometry.
Font metrics, wrapping and rasterization differ between Macroquad/fontdue and Pixi/canvas
text. The prototype deliberately keeps Macroquad's current fallback mapping of font family,
weight and tracking; it does not imply that either backend implements full semantic typography.
