# Issue 59 — verified assets, Sprite backend and Tiles slice

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


Date: 2026-10-03. Baseline was re-pinned locally and remotely at
`7ffda7d00f17cc085f2c5169d68c05da13c23dab`. Only #59 is implemented here.
This ledger records executed scope and residual evidence; #59 remains open
because native runtime evidence is not complete.

## Delivered behavior and boundaries

`MemoryAssetSource` → `load_verified` → bounded PNG decode → renderer-owned
texture cache → complete Sprite preflight → queued execution is implemented.
No unverified-byte upload entrypoint exists. The cache validates file binding,
all atlas aliases, dimensions, pixel/allocation budgets, density, pack/version/hash
identity and live/retired texture residency. Ready textures are reused; submitted
frames retain strong leases through the host's `next_frame` flush. Missing,
failed, corrupt and unsupported resources return explicit errors.

Sprite honors physical atlas regions, local pivot/rotation, affine/camera
composition, logical clip, tint, inherited opacity and command order. The
capability inventory includes Sprite with its actual ready-resource conditions.
The browser probe exposed an inverted clip Y; execution and preflight now share
the tested top-left logical → bottom-left device conversion.

The prior #52 WebGL deleted-texture symptom was investigated against pinned
Macroquad source: its default glyph atlas deletes its old unmanaged texture
when growing. All accepted frame glyphs are now prepared before any primitive
queues an atlas reference. This addresses that renderer path; it is not proof
that every historical #52 session or native driver was reproduced. Browser
sessions recorded here have no captured WebGL errors. Small fallback glyphs
also use a minimum 16-pixel raster and a shared logical scale for measurement,
wrapping, tabular spacing and drawing. Token sizes remain unchanged.

Tiles uses the original, editable [CC0 atlas](../../../games/tiles/assets/README.md),
with semantic tint and terrain shapes. Preview rotation remains local;
accepted placement gets one bounded token-driven settle animation. Reduced
motion retains informative fade without geometric motion. Replacement views,
pause/blur, terminal transitions and motion preference changes converge to the
permitted view. Existing selection, camera, placement and claim paths remain
the owners of interaction. No rules, canonical state, replay format/goldens,
engine, production asset delivery policy or `/play` navigation was changed.

## Claim ledger

| Claim / owner | Evidence and outcome | Residual domain |
|---|---|---|
| Integrity before decode/upload; asset backend | Real PNG decoding plus negative tests for wrong size/hash/binding, missing, corrupt/unsupported, huge/zero dimensions, atlas bounds and allocation budgets: PASS | PNG only; production I/O adapters remain future work |
| Cache ownership and bounded residency; renderer | Version/hash/density/reuse, failed insertion/retry, leased retirement/budget and queued failure tests: PASS; measured frames add zero decodes/uploads | Runtime driver/context-loss recovery not certified |
| Sprite contract; renderer | Geometry/UV/tint/opacity/order tests and [Historical actual browser probe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/sprite-probe.jpg): PASS within Chromium/DPI1 | Native pixels and device DPI2 runtime NOT_RUN |
| Motion/command authority; Tiles Local + LocalMatch | Focused presentation, conformance, replay, rejection and interruption tests: PASS; actual script accepts all 20 steps and records matching checkpoints across preferences | Scripted timings are coverage only (nonzero host events); native input/audio runtime NOT_RUN |
| Theme/readability | Actual light/dark/high-contrast captures; token raster mapping tests in all four themes | Accessibility certification and new font delivery are outside scope |
| Historical visual difference | [Historical Before](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-before.jpg) and [Historical after](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-after.jpg) share the reachable view/checkpoint, seed, roster, viewport and motion | Historical renderer had no Sprite; this is not a same-asset performance comparison |
| Runtime performance | [Historical Raw controlled runs](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/runs) and [measurement table](measurements.md), with actual browser/backend/cache provenance | Total process CPU/RSS, GPU completion and other platforms NOT_RUN |

## Exact checks

Final logs are in [logs](logs/). A successful compile is labelled compiled,
never runtime. Zero-test suites and ignored tests are not counted as executed.

| Command | Final result |
|---|---|
| `cargo nextest run --offline -p tabula-assets -p renderer-macroquad -p tabula-game-client -p tabula-game-tiles --all-targets` | PASS: 350 executed, 350 passed, 0 skipped |
| `cargo xtask check` (the `just check` implementation; `just` absent) | PASS: fmt, all-feature/all-target workspace lint, 923 executed tests; 21 ignored; dependency/game-ID/manifest/token/colour/security gates pass |
| `cargo check --offline --workspace --no-default-features` | PASS, compiled |
| `cargo check --offline --workspace --all-features` | PASS, compiled |
| `cargo build --offline -p tabula-game-client` | PASS, native aarch64-apple-darwin build; UI NOT_RUN |
| `cargo build --offline -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release` | PASS, production WASM compiled |
| `cargo check --offline -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown` | PASS, compiled |
| `cargo check --offline -p tabula-web --no-default-features --target wasm32-unknown-unknown` | PASS, compiled |
| `cargo xtask replay tests/replays/{chess-golden,chess-clock-golden,tiles-golden}.tbr --verify --diagnose` (three separate calls) | PASS: 4 + 2 + 102 exact checkpoints; outcome verified, diagnosis NONE |
| Existing Python environment: `check_skills.py`, `test_check_skills.py`, `test_ai_doc_contracts.py` | PASS: validation, 32 executed skill tests and 6 contract tests |
| `ai_doc_contracts.py check` on renderer/presentation/Tiles/client/example sources | PASS: 17 annotated items, no errors/warnings; repaired inherited LocalMatch evidence links |
| Nine isolated WASM baseline variants | PASS, compiled; exact environment/options/hash/size in [Historical build receipts](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/build-receipts.json) |
| `cargo xtask pack-assets tiles` + `cmp` generated/fixture metadata | PASS: rebuilt pack exactly matches the checked-in fixture manifest |

The first sandboxed aggregate attempt reached the advisory database gate but
could not write its lock outside the workspace. The authorized escalated full
gate was rerun successfully; no gate was weakened. Early failed build/check
iterations were fixed before the final checks. Three presentation command
snapshots changed intentionally to reflect Sprite/motion; replay goldens were
verified unchanged.

## Runtime conditions and honest limits

Mac mini, Apple M4, 24 GB, macOS 15.6. Rust/cargo 1.96.1; Macroquad 0.4.16,
Miniquad 0.4.11; wasm-release optimizes for size with fat LTO. The actual in-app
browser reports Chromium 154, WebGL1 and ANGLE Metal Apple M4. Its user-agent
reports a compatibility OS version; host macOS facts are recorded separately.
Canvas and logical viewport are 900×720 at device DPI1. Theme/control options,
actual resource observations and log timestamps are preserved per run.

The fixed reachable fixture accepts 24 inputs (12 placements, 6 claims, 6 skips),
13 cells, checkpoint
`e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032`.
Scripted variants accept 20 additional inputs (44 total, 24 placements, 10 claims,
10 skips), 25 cells, final checkpoint
`b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73`.
Full and reduced motion use the same accepted workload. Static solid-quad
control uses the same new presenter, view and loaded atlas; it is labelled
primitive control, not historical artwork or an alternative engine.

Warm-up is 3 seconds, minimum 600 measured frames. Each reload creates a fresh
WASM renderer/cache instance in the existing visible browser process. Browser,
HTTP and driver caches were not explicitly purged. Controlled static measurements use no recording or deliberate input, and no
concurrent build/check workload. Scripted theme/motion coverage reports its
nonzero host-event count explicitly; those runs are excluded from the controlled
performance table. The event was not classified by this harness, so a later
motion comparison must establish zero uncontrolled events or record/classify
setup events explicitly rather than silently subtracting one.
`submit_end_cpu_us` is method wall time (coarsely quantized around 1 ms here),
excluding the final engine/GPU flush and vsync. Frame interval is observed loop
time. WASM linear memory and browser heap are separate sampled resource facts;
neither is process RSS. No universal FPS or engine ranking is claimed.

Native Macroquad UI: **NOT_RUN** under this task's known native automation hang
constraint; native build is not pixel proof. Safari/WebKit UI: **NOT_RUN**, no
supported browser automation surface here. Actual device DPI2: **NOT_RUN**;
density2 decode/selection, clipping and camera contracts are exercised in tests.
Total process CPU/RSS and GPU timing: **NOT_RUN**. The optional MediaRecorder
run reached a completed download link, but both supported download routes
stalled; exported video is **BLOCKED**, with the [attempt retained separately](recording-attempt.md) from performance runs. Runtime screenshots remain the delivered visual evidence.

## Publication and #60 handoff

Direct `develop` publication is user-authorized; no PR, force push, protection
bypass or deployment is requested. The checked-in workflow triggers PRs and
`main` pushes, not `develop`; verify the new SHA's actual remote checks rather
than attributing the prior 11 checks to this change. Publication receipt and
issue comment link are returned in the task handoff.

Scripted/theme coverage receipts are in [Historical coverage](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/coverage); their nonzero
uncontrolled host-event counts prevent using them as controlled timing comparisons.
The screenshots show [Historical light](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-light-script.jpg), [Historical dark](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-script-dark.jpg),
[Historical high-contrast light](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-script-hc-light.jpg),
[Historical high-contrast dark](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-script-hc-dark.jpg) and
[Historical reduced motion](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/tiles-script-reduced.jpg) after the same accepted script.

Interactive browser input was also exercised separately from timing rows.
[Historical Rotation/selection](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-preview-after.jpg) displays R180 and a legal
cursor; [Historical invalid placement](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-invalid.jpg) displays an explicit Invalid
label while the board stays unchanged. [Historical Accepted placement](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-placement.jpg)
enters the claim phase with visible settle motion; [Historical claim/pan/zoom](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-claim-camera.jpg)
advances to seat1, 57 tiles left and a follower on the accepted tile. These are
actual keyboard/pointer paths. The [Historical settled resize](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-resize.jpg) and
[Historical resource observation](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/interactive-resize-environment.json) confirm both canvas
and viewport at 600×500, DPI1, with no captured WebGL errors. The interactive console measurement preceded
later smoke actions, so it is not offered as the final canonical hash of those
actions; canonical no-op/accepted replay claims are covered by the executed tests.

For #60, re-pin the published remote SHA and use
[the reproducible protocol](../../perf/tiles-renderer-baseline.md),
[Historical source fingerprint](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/source-manifest.json), exact pack hashes and accepted
checkpoints above. Keep renderer cost separate from embedding cost. Native,
WebKit, higher-DPI, process and video limitations remain explicit. #60's RFC
and gate decisions belong to its separate chat; nothing here approves an engine
or production route migration.
