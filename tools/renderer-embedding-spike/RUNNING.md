# Reproduce the isolated renderer / embedding spike

This directory is tooling. The Leptos example is a separate Cargo example, not
a production route. The plain `index.html` is explicitly a surrogate DOM shell.
`macroquad.html` runs the same unchanged #59 example in its own document, and the
iframe path embeds that document without auth context, credentials or URL tokens.

From the repository root, with the locked dependencies cached:

```sh
npm --prefix tools/renderer-embedding-spike ci --ignore-scripts
cargo build --offline -p xtask --example embedding_fixture
target/debug/examples/embedding_fixture --export tools/renderer-embedding-spike/fixtures

TABULA_BASELINE_COMMIT="$(git rev-parse HEAD)" \
TABULA_BASELINE_BUILD=wasm-release \
TABULA_BASELINE_OPTIONS='--scenario static --theme light --motion full --initial-inputs 24 --warmup-ms 3000 --samples 300 --width 900 --height 720' \
cargo build --offline -p tabula-game-client --example tiles_renderer_baseline \
  --target wasm32-unknown-unknown --profile wasm-release

python3 tools/renderer-embedding-spike/stage.py \
  --destination /tmp/tabula-issue-60-spike \
  --build-leptos --wasm-bindgen /path/to/wasm-bindgen-0.2.129

python3 tools/renderer-embedding-spike/serve.py \
  --directory /tmp/tabula-issue-60-spike \
  --authority target/debug/examples/embedding_fixture --port 8060
```

When `CARGO_TARGET_DIR` is set, use that directory for the native authority path
and pass `--target-dir` to `stage.py`. The Mac evidence uses
`/tmp/tabula60-target`. A matching official wasm-bindgen CLI may be unpacked under
`/tmp`; its exact version must match `Cargo.lock`. The stage command rejects a
mismatch. It does not install a global CLI or alter dependency locks.

Open `http://127.0.0.1:8060/leptos.html` for the real isolated Leptos shell;
`/index.html` is the diagnostic DOM fallback, and `/macroquad.html` is the separate
document control. Keep the drawing surface exactly 900×720 logical units and
record browser viewport, actual DPI, theme, motion, source hashes and build profile.
`runtime-assets.json` records staged bytes and their SHA256; the Rust fixture
exporter checks the existing pack's BLAKE3 and size before exporting its descriptor.
Before staging, `stage.py` copies the two current atlas files into a fixed-scope
snapshot and runs the same native tool's `--verify-assets` check. It writes those
exact verified bytes, preventing a later checkout read from being assigned the
old fixture identity. `--authority-tool` can override the native tool path.

The browser exposes `window.spikeHarness` for reproducible local automation:

```js
await spikeHarness.mount('iframe'); // resolves only after original Rust ready receipt
await spikeHarness.roundtrips(100); // postMessage task round trip, not input-to-paint
spikeHarness.dispose();
await spikeHarness.mount('pixi', {scenario: 'interactive'});
await spikeHarness.input({kind: 'key', key: 'Space', pressed: true});
await spikeHarness.advance(); // Rust produces the next permitted RenderList
await spikeHarness.cycles('pixi', 50); // await ready, then dispose, each cycle
spikeHarness.report();
spikeHarness.dispose();
await spikeHarness.settled(); // flush queued Rust cleanup before closing the page
```

The static Pixi fixture advances in Rust to presentation time 3000 ms before its
ready receipt, settling the same initial placement motion as #59's warm capture.
The static draw loop repeats that permitted RenderList and performs no frame HTTP
requests. Interactive input travels through the bounded native Rust authority shim;
a finite 50 ms / 500 ms frame pump advances Rust motion after interaction. The HTTP
shim is prototype interop cost and is not a production match transport. Scripted
experiments explicitly call `advance(now_ms)` using the same Rust timeline.
Input callback identity and the observed controller view revision are admitted at
ingress together with the permitted frame's Rust checkpoint. Before a queued callback
executes, the host rechecks that checkpoint. If an earlier command changed it, the
callback is dropped with a `dropped_stale_checkpoint` receipt and increments
`dropped_callback_inputs`; it cannot acquire a new meaning against the next turn or
phase. Same-checkpoint focus and pointer callbacks retain FIFO order. Receipts retain
both revision meanings; the JavaScript shell never resolves or classifies a game
command. Programmatic `input()` is a separate automation API: callers await each
returned Rust response before intentionally issuing the next operation.

`dispose()` removes each frame/controller, bridge listener, resize observer,
presentation timer and per-mount host listener. `cycles()` reports exact owned
counts. The two permanent harness click and modal-keyboard handlers remain constant
across every cycle.
Each cycle also waits for a draw callback at the settled Pixi revision, or a child
RAF receipt following Macroquad's completed ready frame. The iframe count is an
explicit lower bound, not a counter of all engine frames. Await `settled()` after
disposal before an automated page close so the native shim receives final cleanup.
Page destruction alone cannot guarantee completion of that local HTTP cleanup;
stopping the loopback host terminates the native shim and all remaining sessions.
Macroquad owns anonymous loader handlers until its iframe browsing context is
destroyed; no owned Macroquad engine dispose API is claimed. DOM/RAF counts do not
establish process, WASM-page allocator or GPU-memory reclamation.

The iframe accepts only the parent's exact origin and source and uses a scoped
session/generation plus monotonic transport revision. It is trusted same-origin
code and is not a security sandbox. Tab / Shift-Tab escape through a typed focus
message; shell modal focus stays in the DOM. Live Macroquad motion/theme changes
require a separately compiled matching baseline; a changed preference is rejected
explicitly. Pixi uses Rust-projected commands and never reconstructs rules, bag or
canonical state. Both experiment paths use no match socket and no production auth.

Audio playback, network resume, native embedding and production routing are
unimplemented. Safari/WebKit and native execution must have separate executed
receipts or be marked blocked/not run. Compilation, Node tests, mocks and screenshot
capture alone are not evidence of those runtimes or a performance advantage.

## Evidence output

The browser runners default to ignored `verification/issue-60/`. Supply an explicit
output directory to keep runs separate. `summarize.py --evidence verification/issue-60`
requires the complete declared input receipts, including environment and the twelve
static runs; current source does not bundle historical measurements. Restore old
inputs from the source-pinned archive in the issue60 ledger when reproducing that
record. Keep new raw output in ignored paths or Actions Artifacts.
