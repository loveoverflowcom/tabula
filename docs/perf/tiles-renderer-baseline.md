# Local Tiles renderer baseline

The checked-in [example](../../apps/game-client/examples/tiles_renderer_baseline.rs)
exercises the real local runtime, presenter, Macroquad renderer and verified
[CC0 fixture pack](../../games/tiles/assets/README.md). It is a Phase-3 local
graphics harness, separate from production routes and delivery policy. This
file specifies reproduction; measured results belong in a dated evidence ledger.

The fixture uses seed `[47; 32]`, three seats, no turn deadline and 24 accepted
commands from the existing Easy bot. Each command passes through `LocalMatch`;
the presenter receives its permitted `View`. An executed example test requires
a nontrivial board and at least one accepted follower claim, and compares the
canonical checkpoint with full and reduced motion. No hand-authored stress board
is presented as a legal state.

Native execution accepts flags. WASM uses the same flags through compile-time
`TABULA_BASELINE_OPTIONS`, because the checked-in Macroquad loader supplies no
command-line arguments or wasm-bindgen glue. Record the source commit and build
profile through `TABULA_BASELINE_COMMIT` and `TABULA_BASELINE_BUILD`; omitted values
are explicitly reported as `UNRECORDED`.

```sh
TABULA_BASELINE_COMMIT="$(git rev-parse HEAD)" \
TABULA_BASELINE_BUILD=release \
cargo run --offline -p tabula-game-client --example tiles_renderer_baseline --release -- \
  --scenario scripted --theme light --motion full --scripted-inputs 8 --samples 900
```

```sh
TABULA_BASELINE_COMMIT="$(git rev-parse HEAD)" \
TABULA_BASELINE_BUILD=wasm-release \
TABULA_BASELINE_OPTIONS='--scenario scripted --theme light --motion full --scripted-inputs 8 --samples 900' \
cargo build --offline -p tabula-game-client --example tiles_renderer_baseline \
  --target wasm32-unknown-unknown --profile wasm-release

python3 - <<'PY'
from pathlib import Path
import shutil
destination = Path('/tmp/tabula-renderer-baseline')
destination.mkdir(exist_ok=True)
host = Path('apps/game-client/web')
shutil.copy2('apps/game-client/examples/renderer_observation.html', destination / 'index.html')
shutil.copy2(host / 'mq_js_bundle.js', destination / 'mq_js_bundle.js')
shutil.copy2(
    Path('target/wasm32-unknown-unknown/wasm-release/examples/tiles_renderer_baseline.wasm'),
    destination / 'tabula-game-client.wasm',
)
PY
python3 -m http.server 8000 --directory /tmp/tabula-renderer-baseline
```

This stages the checked-in isolated observation host and the production loader.
The host reports runtime resource facts through a hidden output element and offers
an optional canvas video recording. Record video in a separate run, because its
control changes focus/input. It exposes no game state or production route. If `CARGO_TARGET_DIR` is set, use its actual build directory in the
copy step. Serve loopback locally; no deployment or `/play` change is involved.

| Flag | Behavior |
|---|---|
| `--scenario interactive` | Normal pointer/keyboard play; interaction events are counted in the report |
| `--scenario static` | Fixed reachable projected fixture; host input is drained and ignored during measurement |
| `--scenario scripted` | Existing presenter rotates the preview 300 ms before accepted bot commands, one every 750 ms by default; normal host input remains available and is counted as uncontrolled |
| `--theme light/dark/hc-light/hc-dark` | Existing semantic theme |
| `--motion full/reduced` | Existing local reduced-motion policy |
| `--primitive-control` | Same current presenter/view/scopes/rotation, replacing textured quads with solid primitive quads; this is a labelled control, not historical artwork |
| `--sprite-probe` | Additional atlas Sprite control with rotation, affine transform, camera, logical clip, tint, inherited opacity, overlap and foreground ordering; incompatible with primitive control |
| `--initial-inputs N`, `--scripted-inputs N` | Accepted fixture/script command counts, bounded to 0–100 and 1–100 respectively |
| `--warmup-ms N`, `--samples N`, `--interval-ms N` | Warm-up, nonempty sample minimum, and scripted cadence; scripted runs continue sampling until all configured commands finish, bounded to 10,000 samples |
| `--width N`, `--height N` | Native window request; browser viewport is supplied by the actual host/container |

Both density variants are verified, decoded and uploaded once before the loop.
`TABULA_BASELINE` console lines contain JSON: a `ready` receipt records file hashes,
fixture checkpoint, options, actual viewport/DPI, preload time and fresh renderer-instance
cache counters; a `measurement` receipt records nearest-rank p50/p95/mean/max values,
accepted command kinds, final checkpoint and decode/upload deltas. Errors produce
a structured failure and a visible stopped surface; rejected interactive commands
report their controlled code without exposing detailed rule payloads.

`submit_end_cpu_us` is the wall time inside backend submit/end methods, including
their immediate work. It excludes presenter construction, bot/rules work, the
final Macroquad frame flush, GPU completion and `next_frame`/vsync. It is not
whole-process CPU consumption. `frame_interval_ms` observes consecutive loop starts;
it includes scheduling and presentation work, so it is not a GPU timer. Texture
`estimated_resident_rgba_bytes` is the cache's RGBA residency estimate, not process
RSS, browser heap, driver allocation or WASM memory. Observe process RSS/CPU or
browser/linear memory separately and name the measured source.

Use at least three fresh renderer-instance runs and distinguish preload from warm
steady rendering. A page reload creates a new WASM/cache instance in an existing
browser process; it does not establish cold browser, disk, network or driver caches.
Record whole-process restarts separately if performed. Keep the tab visible and record device, OS, browser/version/backend,
build profile, commit, asset hashes, scene, viewport, actual DPI, motion and cache
state. Do not manipulate the window or input during a controlled run. A scripted
comparison needs `script_complete=true`, `uncontrolled_input_events=0`, matching
accepted command counts/checkpoints and comparable sample duration. A script that
terminates early or exceeds the 10,000-frame bound reports a failure, never a
successful measurement with a partial workload. Resizes during a sample change
the workload; repeat at each settled viewport/DPI instead.

For #60, reuse these exact assets, projected fixture and command/motion settings.
Compare renderer and wrapper overhead separately. A screenshot from this running
example is runtime evidence; its Rust tests, compile results and control commands
alone do not establish visible pixels, native execution or browser execution.
Do not infer cross-browser performance or a universal 60 FPS guarantee from one run.

Focused checks for the harness are:

```sh
cargo test --offline -p tabula-game-client --example tiles_renderer_baseline
cargo clippy --offline -p tabula-game-client --example tiles_renderer_baseline -- -D warnings
cargo check --offline -p tabula-game-client --example tiles_renderer_baseline \
  --target wasm32-unknown-unknown --no-default-features --features web
```

The repository's consumer conformance, authoritative `just check`, feature matrix
and target checks still apply. Native and each browser runtime result must be
recorded separately as executed, `NOT_RUN` or `BLOCKED`.
