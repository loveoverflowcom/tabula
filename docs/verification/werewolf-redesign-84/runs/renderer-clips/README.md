# Actual renderer clip and host-row fixture

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


These receipts came from a disposable local fixture, not a second gameplay UI.
Its Rust `RenderList` uses the repository's actual `MacroquadRenderer`, verified
managed sprites and existing Open Sans/Noto Serif resources. It renders a
four-color image and Vietnamese text in unbounded, clipped, nested and restored
clip scopes. Chromium is headless Google Chrome on macOS, DPR1/2, 960×360 CSS
pixels. No browser or console errors were recorded.

The [Historical pixel receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/clip-pixel-assertions.json) has eight sets of four
upright corner samples and two identical 180×40 logical-pixel ink strips, one
outside and one inside clipping. The [Historical negative receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/clip-negative-pixel-assertions.json)
uses one injected defect in a disposable copy of the current backend: only clip
`Camera2D.zoom.y` changes from `+2.0 / rect.size().y` to
`-2.0 / rect.size().y`. This flips clipped/nested images at both densities and
keeps unbounded/restored images upright. It is not a comparison against a full
historical renderer tree. The [Historical exact pixel script](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/pixel-assertions.cjs.txt)
was rerun against retained PNGs: PASS, 16 corner sets and two ink comparisons.

The [Historical footer receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/footer-layout.json) checks actual shared CSS against a
minimal canvas/host-row document at 1200×880, 1100×850, 390×844, 320×640 and
844×390, each at DPR1/2. Ten current cases have a positive-height canvas above
the in-flow host row. Ten controls using the original CSS show overlap. This
proves those slot bounds, not the game's portrait/card or accessibility layout.

Original fixture sources and scripts are archived as `.txt` so evidence does
not introduce another Rust workspace or executable test target. Their archived originals preserve the recorded machine paths; reproduction commands
below use generic local placeholders. To rebuild, download the linked archived sources, then copy them into a disposable
`/tmp/tabula-issue84-renderer` with the following names:

| Retained source | Recreated file |
|---|---|
| [Historical Rust source](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/src-main.rs.txt) | `src/main.rs` |
| [Historical Manifest](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/Cargo.current.toml.txt) | `Cargo.toml` |
| [Historical Current page](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/web-index.html.txt) | `web/index.html` |
| [Historical Negative page](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/web-before.html.txt) | `web/before.html` |
| [Historical Current browser runner](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/clips.cjs.txt) | `clips.cjs` |
| [Historical Negative runner](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/clips-before.cjs.txt) | `clips-before.cjs` |
| [Historical Footer runner](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/footer.cjs.txt) | `footer.cjs` |
| [Historical Original CSS control](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/standalone-before.css.txt) | `standalone-before.css` |
| [Historical Pixel assertions](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/pixel-assertions.cjs.txt) | `pixel-assertions.cjs` |

If using another checkout or runtime, replace the recorded repo/font,
Playwright, PNGJS and Chrome paths first. The recorded current build command was:

```bash
cargo build --manifest-path /tmp/tabula-issue84-renderer/Cargo.toml --target wasm32-unknown-unknown --release --offline --target-dir /path/to/tabula/target
cp /path/to/tabula/target/wasm32-unknown-unknown/release/tabula-issue84-renderer.wasm /tmp/tabula-issue84-renderer/web/fixture.wasm
cp /path/to/tabula/apps/game-client/web/mq_js_bundle.js /tmp/tabula-issue84-renderer/web/mq_js_bundle.js
python3 -m http.server 8084 --bind 127.0.0.1 --directory /tmp/tabula-issue84-renderer/web
```

The Macroquad loader may instead be copied from the staged Werewolf host.
Create `evidence/`, then run the three `.cjs` runners with the configured bundled
Node runtime. For the negative build, copy the current renderer crate into
`before-backend`, inject the one Y-sign defect described above, point the
fixture's renderer dependency to that copy, and build `fixture-before.wasm`.
Replace that copied crate's manifest with the [Historical retained standalone manifest](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-redesign-84/runs/renderer-clips/fixture/before-backend-Cargo.toml.txt)
so its workspace dependencies resolve from the recorded checkout.
The negative page loads that separate binary. Finally run:

```bash
node /tmp/tabula-issue84-renderer/pixel-assertions.cjs /path/to/tabula/docs/verification/werewolf-redesign-84/screenshots/renderer-clips
```

No compiled binary, generated loader, font or whole disposable backend copy is
retained here. The fixture applies no canonical rules or online session changes.
Full Werewolf runtime evidence is indexed in [the main ledger](../../README.md).
