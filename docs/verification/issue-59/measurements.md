# Browser measurements, 2026-10-03

See [conditions and limits](README.md) and the [reproduction protocol](../../perf/tiles-renderer-baseline.md).

Every row is one fresh WASM renderer/cache instance in the existing visible Chromium process.
Three static runs per rendering mode are controlled repeats. Each theme/motion script is a single
runtime coverage run with a nonzero host-event count, excluded from this table. Preload verifies
and uploads both atlas densities. Browser/network/driver caches were not purged.

Method timing is wall time, with roughly 1 ms browser clock quantization. Frame interval includes
browser scheduling. Memory is a post-run readback, not peak or process RSS. No forced GC was used.
All rows have zero input events and zero captured warnings/errors. Script coverage is separate.

| Run | Frames | Preload ms | Method mean/p95 ms | Frame p50/p95 ms | WASM MiB | JS heap MiB at readback | Readback after start s |
|---|---:|---:|---:|---:|---:|---:|---:|
| [static-primitive-1](runs/static-primitive-1.json) | 600 | 8.0 | 1.040/2.000 | 27.0/29.0 | 8.94 | 6.46 | 74.0 |
| [static-primitive-2](runs/static-primitive-2.json) | 600 | 2.0 | 0.772/1.000 | 26.0/28.0 | 8.94 | 6.00 | 107.0 |
| [static-primitive-3](runs/static-primitive-3.json) | 600 | 2.0 | 0.815/1.000 | 26.0/28.0 | 8.94 | 6.62 | 79.0 |
| [static-sprite-1](runs/static-sprite-1.json) | 600 | 8.0 | 0.590/1.000 | 26.0/27.0 | 8.94 | 6.39 | 47.0 |
| [static-sprite-2](runs/static-sprite-2.json) | 600 | 3.0 | 0.720/1.000 | 26.0/27.0 | 8.94 | 6.34 | 107.0 |
| [static-sprite-3](runs/static-sprite-3.json) | 600 | 3.0 | 0.770/1.000 | 26.0/28.0 | 8.94 | 6.22 | 41.0 |

Each run retains 2 textures, estimated RGBA residency 2,488,320 bytes (2.37 MiB),
with exactly 2 decodes/uploads before the loop and 0 during the measured workload.
Static rows share 24 accepted inputs/13 cells; scripts share 44 inputs/25 cells and
the exact checkpoints listed in the ledger. Static primitive control loads the same
pack and replaces only Sprite geometry with solid quads; it is not old artwork.

static-sprite: median of run means = 0.720 ms; observed frame p95 range = 27.0–28.0 ms.
static-primitive: median of run means = 0.815 ms; observed frame p95 range = 28.0–29.0 ms.

The primitive control is a local contract/workload check, not evidence that another
engine or embedding would be slower. These coarse method samples exclude the final
Macroquad/GPU flush. Native/WebKit, physical DPI2, GPU and total process CPU/RSS remain NOT_RUN.
