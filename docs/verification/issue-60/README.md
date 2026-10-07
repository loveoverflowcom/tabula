# Issue 60 — isolated renderer / embedding evidence

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


Scoped delivery for [issue 60](https://github.com/loveoverflowcom/tabula/issues/60),
based on verified `develop @ 2de46d27efafd078ba2d27b027a54de565e2fbe5`.
The [RFC](../../rfcs/issue-60-renderer-embedding.md) separates renderer choice
from runtime containment. [ADR-0029](../../adr/0029-renderer-embedding-spike.md)
**defers production migration** and retains Macroquad and ADR-011. The working
prototype is a separate Leptos Cargo example plus tooling; production routes,
rules, protocol, dependencies of shipped crates and phase gates are unchanged.
The earlier #59 issue remains open with its own residual acceptance work.

## Runnable deliverable and authority

[RUNNING.md](../../../tools/renderer-embedding-spike/RUNNING.md) gives exact
build, stage and serve instructions. `/macroquad.html` is the original #59
Macroquad document control; `/leptos.html` mounts that document in a same-origin
iframe or the minimal Pixi 8.22.0 adapter in an actual isolated Leptos shell.
`/index.html` is explicitly the surrogate DOM diagnostic shell. No production
route, deployment, auth credential or match socket is used.

The [Rust tooling example](../../../xtask/examples/embedding_fixture.rs) owns
`LocalMatch`, existing rules/presenter, accepted input, projected RenderList,
checkpoint and contract export. JS lowers the permitted commands and emits raw
input; its projected DTOs contain no canonical State, bag order or private
seed. The Macroquad ready receipt includes the known offline fixture seed as
public experiment provenance. The native loopback
JSON-lines/HTTP shim is a measurement adapter, not a production WASM FFI or
online transport. Static Pixi drawing reuses one view without per-frame HTTP.
The authority shim is a native `dev`-profile tool; timing includes that prototype
boundary. Interactive motion uses a bounded 20 Hz/500 ms Rust presentation pump. Script
checks step the real Rust timeline and are not realtime timing benchmarks.

Both paths use the [current editable, CC0 atlas pack](../../../games/tiles/assets/README.md),
its 1×/2× density variants and the existing ProggyClean font bytes. Staging
snapshots and verifies actual PNG bytes using Rust's compiled BLAKE3 manifest;
JS validates SHA256, size, relative path, decode dimensions and atlas regions.
[Npm dependency policy and pins](../../../tools/renderer-embedding-spike/DEPENDENCIES.md)
and [license notices](../../../tools/renderer-embedding-spike/licenses/) cover
all twelve locked packages and the font. No additional engine is installed.

The initial 24 accepted-command fixture checkpoint is
`e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032`.
The 20 scripted additions end at 44 accepted commands and checkpoint
`b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73`.
These public test identities support comparisons; they are not additions to the
production wire protocol.

## Executed runtime and captures

[Historical Environment](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/environment.json), [Historical final source/build manifest](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/source-manifest.json),
[Historical raw runs](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs), and [measurement tables](measurements.md) retain provenance.
Each static target has three runs with 300 samples after 3,000 ms warm-up,
900×720 logical surface, DPR1, light theme/full motion and the same checkpoint.
Run 1 has a fresh browser process; runs 2–3 use the same context and new runtime.
Disk/OS/driver caches were not purged. Native is a separately launched control.

| Target | Executed result | Scope |
|---|---|---|
| Macroquad separate document, Chrome 154 | 3 PASS | Actual WASM rendering, zero uncontrolled input events |
| Macroquad iframe in isolated Leptos, Chrome 154 | 3 PASS | Same engine/workload; 100 postMessage round trips/run |
| Pixi direct canvas in isolated Leptos, Chrome 154 | 3 PASS | Actual WebGL drawing of Rust-permitted commands, zero host inputs in static comparison |
| Native Macroquad | 3 PASS | Actual native window and measurement receipts; pixels NOT_CAPTURED |
| Safari 18.6 | BLOCKED | [Historical WebDriver probe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/safari-probe.json): remote automation disabled; setting unchanged |
| Alternate Playwright WebKit | NOT_INSTALLED | No runtime or browser equivalence claim |
| Chrome DPR2 | PASS | Emulated DPR2, 1800×1440 framebuffer; physical high-DPI display NOT_RUN |
| Mobile / Tauri / other WebViews | NOT_RUN | No target introduced or certified |

Actual captures are [Historical Macroquad document](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/chromium-document-3.png),
[Historical Macroquad in Leptos](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/chromium-iframe-shell-3.png),
[Historical Pixi in Leptos](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/chromium-pixi-shell-3.png),
[Historical accepted placement](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-accepted-placement.png),
[Historical resized camera](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-interactive-resize.png), and
[Historical DPR2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-dpr2.png). Scripted captures include
[Historical light](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-script-light.png), [Historical dark](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-script-dark.png),
[Historical high contrast light](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-script-hc-light.png),
[Historical high contrast dark](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-script-hc-dark.png), and
[Historical reduced motion](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/screenshots/pixi-script-light-reduced.png).
The board geometry, source assets, tints and checkpoint correspond. Font
rasterization/legibility and disabled-button appearance differ; full pixel or
backend equivalence is not established. No video export is claimed.

## Lifecycle, input and integrity

[Historical Chromium interaction receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-interactions.json) is **PARTIAL:
11 passed, 1 blocked**, with zero uncaught browser exceptions. Passed checks
exercise four themes/reduced motion with the exact 44-command Rust checkpoint;
actual keyboard, pointer drag/capture and resize; a Rust-accepted placement and
exact returned-revision draw; modal containment and iframe Tab/Shift-Tab exit;
late native response cleanup; disposal during delayed PNG initialization;
same-size PNG corruption rejection before decode; context loss with structured
failure and explicit remount recovery; manual suspend/resume; and DPR2.
Expected corruption/context-loss diagnostics are retained rather than hidden.

Two DOM Enter activations observed on one permitted view produce one accepted
Rust transition and one `dropped_stale_checkpoint` receipt. Callback identity and
observed revision are validated at ingress; the queue checks the captured Rust
checkpoint before sending each callback. Same-checkpoint focus/pointer events
retain order. Programmatic timing inputs deliberately await a returned revision.
This is a prototype guard, not proof of production online intent identity or
state-version semantics.

Automatic background visibility is **BLOCKED**: real tab switching and window
minimization did not change Chrome's `document.visibilityState` from visible,
even with automation occlusion suppression removed. Each attempt was bounded;
no synthetic visibility event replaced actual-runtime evidence. Unit tests cover
the visibility callback and manual runtime suspension cancels/resumes RAF.
Authoritative online turn/network timers do not exist in this local fixture and
are not certified by the suspension check.

[Historical Pixi 50 cycles](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-cycles.json) and
[Historical iframe 50 cycles](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-cycles.json) both PASS. Every mount waits
for a rendered-frame callback/child RAF receipt, then disposes and flushes Rust
cleanup. Owned canvases/iframes, per-mount host listeners/observers/timers,
Pixi RAF/font/texture-source handles and bridge listeners are zero afterward.
Two permanent harness document handlers remain constant. Child RAF is a lower
bound, not an exact Macroquad frame count. Observed process RSS rises during
these short runs; exact heap/process/GPU reclamation or absence of all memory
leaks is **unproven**. No forced GC is used, and diagnostic history is bounded.

The native authority [Historical 12-request smoke](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-authority-smoke.json),
[Historical asset verification/tamper smoke](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/asset-verifier-smoke.json) and meaningful
Rust/JS/Python negatives cover stale generations/revisions, duplicated init,
disposed capability, malformed/extra/oversized fields, bounded geometry,
unknown/prototype message kinds, origin/source identity, late initialization,
texture paths/dimensions, immutable input and cleanup.

## Verification commands and status

`just` is absent on this Mac. `cargo xtask check` is the exact authoritative
implementation behind `just check`, as the repository's justfile specifies.
No goldens were regenerated. Source/document whitespace checks pass; original
compiler log spacing and upstream notice line endings are retained verbatim,
so the final whitespace scan excludes those raw-output files. Zero-test scaffolds and ignored tests are not
counted as executed verification.

| Check | Result and exact command source |
|---|---|
| Core gate | PASS, 923 executed / 21 ignored; [Historical receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/core-check.json), [Historical log](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/logs/core-check.log) |
| Rust focused / feature / WASM | [Historical Exact commands and results](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/rust-target-checks.json); PASS: 349 focused + 8 fixture tests / no ignored; feature/WASM builds reported separately |
| JavaScript / dependency policy | PASS, 18 tests / no skips; 16 module syntax checks; 12 pinned packages allowed; npm audit 0 vulnerabilities; [Historical receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/js-python-checks.json) |
| Python native-backed stage / live HTTP | PASS, 2 staging + 9 HTTP checks; same receipt contains exact commands and actual binding/authority scope |
| Existing Leptos example / baseline builds | PASS compiled; [Historical build commands and source hashes](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/wrapper-build-checks.json), [Historical native](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/logs/baseline-native-build.log), [Historical WASM](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/logs/baseline-wasm-build.log) |
| Fixture / source binding freshness | PASS, fresh export byte-identical and independently verified tooling/presenter BLAKE3; JS/Python receipt |
| Existing skill metadata / helper checks | PASS: metadata/link checks + 32 failure-fixture tests + 6 document-contract tests; [Historical exact commands](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/skills-checks.json); PyYAML 6.0.3 used from a temporary directory |
| Actual runtime | [Historical Exact driver commands](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/runtime-checks.json) and raw named-target receipts above; distinct from compile/mock/Node evidence |

Historical unsuccessful attempts remain in the logs: the skill-helper setup
initially lacked PyYAML and then passed using the CI-pinned temporary dependency; initial workspace manifest
rejection was fixed with a workspace dependency; the game-ID scan rejected test
URL literals which were replaced by generated asset metadata; sandbox socket
binding failed before the approved actual HTTP run. The first interaction driver
used the wrong frame-sample field and was corrected. Failed actual visibility
attempts remain separate from the final PARTIAL receipt. Static pre-freeze
receipts are retained in `runs/pre-freeze-static/`; original Pixi run 1 overlapped
host compilation and is excluded from the final timed comparison. The final
static refresh begins after verification builds have completed.

## Remaining acceptance and decision limits

Keep issue 60 open for residual acceptance. Production renderer/embedding choice
is evidence-based **DEFER**: incomparable submit/draw boundaries, different frame
pacing, visible text/opacity differences and native HTTP interop prevent a global
engine conclusion. Same-engine document/iframe controls isolate some wrapper
behavior, but boot clocks and process membership do not provide a clean overhead
subtraction. See [measurements.md](measurements.md) for CPU/RSS/download/interop
values and their exact limits.

Outstanding work is [queued explicitly](../../work-plan/backlog/issue-60-renderer-embedding-evidence.md):
Safari/WebKit; actual automatic hidden-tab transition; isolated Rust/WASM
serialization/copy/validation timing; matched realtime Macroquad/Pixi scripted
motion and input distributions; first usable/presented-frame startup; audio
unlock/preferences; navigation/back/deep-link and online handoff; full Board
Reader/action accessibility; physical high-DPI/native pixels; stress fixture and
GPU-completion/allocation evidence. These limits do not authorize production
implementation or a Safari setting change.
