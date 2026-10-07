# ADR-0029: isolate renderer/embedding evidence and defer production migration

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/adr); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


- **Status:** accepted for tooling scope; production renderer/embedding choice deferred
- **Date:** 2026-10-03
- **Supersedes:** none; ADR-010, ADR-011 and ADR-0028 remain in force
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-10, I-12 and I-15 preserved

## Context

[Issue #60](https://github.com/loveoverflowcom/tabula/issues/60) asks for a runnable
isolated prototype, an explicit lifecycle/trust contract and measured evidence,
with either a justified renderer/embedding choice or a reasoned defer. The owner
authorized this local tooling scope and direct verified `develop` publication.
It is not authorization to migrate the production engine or implement gated
multiplayer, lobby or gameplay routes.

The input baseline is
`develop @ 2de46d27efafd078ba2d27b027a54de565e2fbe5`.
`apps/web/src/main.rs` implements the bounded discovery/setup slice in ADR-0028;
it still requires the separate-document gameplay handoff in ADR-011. Phase 3 has
not exited, and the rest of Phase 4/5 remains gated. #59 has verified Sprite,
atlas/cache and Tiles presentation work, and comparable local graphics fixtures;
[its ledger](../verification/issue-59/README.md) names native, WebKit,
physical DPI2, process/GPU and exported-video residuals. Its scripted measurements
have an unclassified host event and are not controlled motion comparisons.
Issue #60 adds its own executed native controls and process observations below;
it does not rewrite the historical #59 evidence.

Embedding a game in a DOM-heavy shell and changing its drawing backend are
different decisions. Comparing Macroquad-in-iframe with PixiJS-direct changes
both. A native Rust fixture reached through loopback HTTP introduces a third
variable relative to an in-browser Rust/WASM game. Compile results and library
claims cannot remove those confounders.

## Decision

Keep Macroquad as the production backend and ADR-011's separate-document
handoff. Build and retain a bounded experiment under
`tools/renderer-embedding-spike`, isolated examples and
`docs/verification/issue-60`, following
[the RFC](../rfcs/issue-60-renderer-embedding.md).

The controls are the same Macroquad artifact in a separate document and in a
same-origin iframe, plus a minimal PixiJS 8.22.0 direct-canvas adapter consuming
Rust-derived presentation DTOs. The iframe and direct canvas execute in the
actual isolated Leptos example; its plain-HTML host remains separately labelled
as a harness. Native Macroquad is a separate executed control.

Rust's existing rules/`LocalMatch` own legality and state transitions; the
existing presenter owns camera, selection, preview, motion and input-to-intent.
Browser data is derived only from permitted projections. JS is graphics and
platform glue, with schema/runtime validation and generation-scoped lifecycle.
The fixture authority is local tooling, not a second match session or production
network adapter. No production route, dependency arrow, protocol, replay or
phase gate changes. The same-origin iframe is not an untrusted-plugin sandbox.

The production choice is deferred. The experiment executes on one Chromium/Mac
configuration and a separate native control. Safari/WebKit execution is blocked,
the native-loopback shim does not measure a deployable Rust/WASM boundary, and
static drawing plus prototype lifecycle cannot prove complete backend parity or
online handoff correctness. Observed font rasterization and disabled-fill
differences prevent a pixel-parity claim. Audio is unimplemented; context loss
requires an explicit remount. The results establish useful local feasibility without enough
evidence to supersede ADR-011. No global performance advantage is claimed.

## Executed evidence behind this decision

The [issue-60 ledger](../verification/issue-60/README.md) contains exact commands,
source/build/asset fingerprints, measurements, captures and residuals. Three
static runs each execute Macroquad in a document, the same artifact in an iframe,
and PixiJS in the isolated Leptos shell on Chrome 154.0.8037.97. Each uses the
same permitted initial #59 checkpoint, assets, 900×720 logical viewport and DPR1.
See the [Historical document](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-1.json),
[Historical iframe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) and
[Historical PixiJS](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-1.json) representative receipts
and the [RFC's target table](../rfcs/issue-60-renderer-embedding.md#executed-evidence-and-present-limits).
Three [Historical native Macroquad runs](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-1.json)
provide a separate control with process observations.
The [Historical native authority smoke](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-authority-smoke.json)
also executes 12 stdin requests with recorded session/generation/revision checks;
it establishes the local process boundary rather than browser WASM interop.

The renderer timings have different method boundaries: Pixi includes scene
reconstruction and `app.render`, while Macroquad submit/end excludes its final
frame flush. Their frame pacing and sample durations differ. Browser process-tree
CPU/RSS observations include shell/utility/GPU processes; summed RSS double-counts
shared pages and is not unique resident memory. A bridge ping is a message
round trip, not input-to-visible or match latency. These differences prevent an
engine ranking from the measured values.

The [Historical Safari probe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/safari-probe.json) records Safari 18.6
session creation as BLOCKED because remote automation is disabled, with the
setting unchanged; the alternate Playwright WebKit runtime is not installed.
No Safari/WebKit rendering result is inferred. The final
[Historical Chromium interaction receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-interactions.json)
is PARTIAL: 11 checks PASS and one BLOCKED. Actual automatic visibility suspension
was not reached because Chrome stayed `visible` after bounded tab-switch and
minimization attempts. Manual suspend/resume exercises real RAF cancellation and
restart; mock visibility tests are a separate evidence kind.

The earlier queued-input reinterpretation defect is guarded in the final host:
admission captures Rust checkpoint, observed revision and input sequence; dispatch
drops an input when that checkpoint no longer matches the latest frame before
choosing a native revision. The interaction receipt shows two same-view DOM Enter
activations yield one accepted command and one `dropped_stale_checkpoint` result.
Production still needs its own typed state-version/resolved-intent boundary and
concurrency coverage; the local checkpoint guard is not a shipping protocol.

[Historical Pixi](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-cycles.json) and
[Historical iframe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-cycles.json) each complete
50 actual render/dispose cycles with owned callback/listener/resource counts at
zero after cleanup. Process RSS rises during these short campaigns; complete
heap/GPU reclamation and absence of all memory leaks are unproven. The
[Historical final source/build manifest](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/source-manifest.json)
binds the refreshed static control inputs after compilation finished.

Audio, navigation/back/deep-link and online handoff, paired realtime motion/input
timings, a labelled stress benchmark, physical higher-DPI/mobile execution and
full accessibility action play remain outside the demonstrated scope. Stepping
the real Rust script timeline is not a realtime benchmark. Numerical performance
and resource tables belong to the ledger rather than this decision record.

Pure rules, replay and production routes remain unchanged. No result from the
local authority shim activates the remaining Phase 4/5 implementations.

## Evidence required to resolve the defer

The ledger must retain source/build/dependency/asset fingerprints, exact commands
and nonempty test counts; real target screenshots and measurements; explicit
outcomes for blocked targets; and the following controlled evidence:

1. Macroquad A/B uses one byte-identical WASM artifact and matching view/assets,
   viewport/DPI, workload/motion and cache classes, isolating iframe overhead.
2. PixiJS receives the same allowed fixture from the Rust presenter. Unsupported
   drawing operations, fonts, preferences or action paths are named; timing
   splits lowering/serialization/native transport from drawing work.
3. Init/dispose races, generation/session/revision rejection, bridge
   origin/source/schema limits, asset errors, focus/resize/input and at least
   50 lifecycle cycles have appropriate tests and actual-runtime coverage.
4. Chromium and Safari/WebKit shipping claims have their own executed evidence;
   native is separate. Heap/WASM/texture estimates are not total process RSS,
   submission/frame intervals are not GPU timers, and cold cache claims name
   the cache layer actually reset.
5. Theme/reduced-motion and command rejection/replay/privacy are exercised on
   the same accepted workload. Controlled motion timing excludes unclassified
   host events. Audio/context recovery/accessibility play are claimed only at
   their implemented and executed scope.

The RFC's measurement definitions remain the comparison contract. Numbers
observed through a local adapter are not a production handoff/network SLA.

## Consequences

This gives #60 a concrete runnable seam to inspect without freezing a still-moving
multiplayer contract or coupling Leptos to the native gameplay dependency graph.
The experiment can find bridge/lifecycle defects, expose interop costs and guide
a later renderer decision while keeping current Rust authority intact.

It adds bounded npm tooling and another graphics implementation for the spike.
Its exact package/lock/license/audit inventory must be reviewed independently of
`cargo deny`. PixiJS's MIT license fits the current third-party license policy;
that does not certify every transitive package or shipping target. Upstream
Application APIs are documented in
[the official v8 guide](https://pixijs.com/8.x/guides/components/application)
and the exact release is
[8.22.0](https://github.com/pixijs/pixijs/releases/tag/v8.22.0).

No enforcement exception is introduced. The doc 00 register links this record;
doc 09 keeps the production handoff experiment's future gate and points to the
tooling evidence. A reader must not treat accepted tooling scope as a superseding
renderer decision or a Phase 3/4/5 exit.

## Revisit when

Revisit at a proposed production embedding change, after a concrete DOM-heavy
product requirement is named and the above evidence is complete for the selected
shipping targets or its residuals are explicitly accepted. At least three
comparable controlled runs and 50 lifecycle cycles are the local evidence floor,
not universal performance certification. A later decision records measured
tradeoffs, native/web maintenance cost, phase scope and rollback, and supersedes
ADR-011 only if the chosen production containment actually changes it.

Removing or no longer serving the tooling host/examples rolls this spike back;
production remains on ADR-011 throughout. Bevy, Phaser, Rive, Tauri gameplay,
plugin sandboxing and global engine migration are outside this decision.
