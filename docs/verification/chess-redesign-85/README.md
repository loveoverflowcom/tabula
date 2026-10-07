# Chess #85 runtime redesign

This draft implements the board material, compact HUD and accepted-move slice
of [issue #85](https://github.com/loveoverflowcom/tabula/issues/85), using the
original [PR #86 design handoff](https://github.com/loveoverflowcom/tabula/pull/86).
The initial fresh implementation base is
`develop@a3865246ae1ee513b440541f1eb523d4258b0261`. The branch then cleanly
integrates current `develop@face8a3e058e42fc846efa32082a9b76efaef3c2`; upstream
mobile-account changes and their documentation are preserved.
Read [the design acceptance checklist](../../ui/issue-82-game-feel/chess/IMPLEMENTATION.md)
alongside this narrower source/evidence ledger. The issue remains open.

## Implemented ownership and claims

- `games/chess/src/presentation/material.rs` draws a contained frame, lower rim,
  bevel/inlay and quiet per-square managed grain. Game-art tokens carry the
  approved ivory/slate/truffle palette; system primary/focus/legal/check roles
  are unchanged. Original Staunton SVGs and piece-atlas bytes are unchanged
- `motion.rs` keeps one bounded accepted composition in `ChessLocal`. Previous
  and current public projections validate the mover, synchronized castle rook,
  direct/en-passant victim square and accepted promotion endpoint. Absolute
  sampling gives a low arc, landing scale, grounded shadows and capture fade
- Generic `GamePresentation::on_view_event_with_projection` and rejection hooks
  default to existing behavior. Local and isolated online hosts supply only
  authorized projections, after acceptance. No rules, State, wire format,
  replay identity, networking authority or game-id dispatch is changed
- New input, blur, changed endpoint/orientation, reduced motion, definitive
  rejection and host reset/resync snap or discard motion. Render sampling never
  mutates the projection or waits before authority, clocks, outcome or input
- Board-aligned bars, a bounded desktop status rail and compact Actions menu
  retain projected legality/confirmations and keyboard access. The promotion
  chooser opens inward from its target; pieces and coordinates stay upright
- Seat ownership comes from `View`; clocks appear only when projected. Recent
  coordinate moves/captured pieces are labeled as bounded observations from
  this session. They are not SAN, a saved replay or reconstructed full history

## Local verification

The pinned official Rust 1.96.1 toolchain, native/WASM targets and cargo-deny
0.20.2 were installed into a scratch validation workspace. The source uses
the repository's existing Cargo/xtask/host test workflows. Final command
receipts and source identities are retained under `runs/`.

| Claim | Oracle | Status / remaining scope |
|---|---|---|
| Accepted choreography | Real legal move transitions; both-color en passant, four castles, all promotion choices/orientations, absolute sparse/dense sampling and interruption | PASS: eight motion tests in the [final Chess suite](runs/chess-final-tests.log) |
| Projection context/rejection | Generic local/online host dispatch, exact prior/current public Views, receipt rejection and authority loss | PASS: [60 host library tests](runs/host-final-tests.log), plus [60 isolated web/online library and 15 local integration tests](runs/isolated-web-online-host.log); live online fault acceptance remains its existing separate gate |
| HUD geometry/input | 320/390 portrait, short landscape, >=44dp controls, target-inward promotion, keyboard/repeat/cancel and popup shielding | PASS: twenty HUD tests plus layout/chooser cases in [113 Chess library tests](runs/chess-final-tests.log); actual glyph/physical-touch pixels remain unverified |
| Per-game material/assets | Exact six-file `chess@0.2.0` metadata/hash, density selection, cold/retry/warm cache, unchanged Staunton exports | PASS: [reproducibility](runs/assets-reproducibility.log), [host cache tests](runs/host-final-tests.log), [metadata](runs/metadata-pack-tests.log), [12 contrast tests](runs/design-contrast-tests.log); actual GPU/device measurement remains unverified |
| Rules/replay/conformance | Existing affected Chess/testkit targets and committed replay identities | PASS: [194 affected test executions](runs/chess-final-tests.log), including eleven conformance and seven replay tests; one ignored depth-five perft excluded |
| Portable repository gate | Repository-owned `cargo xtask check` order | PASS on exact published `c4572e99` / tree `1a08b652`: [fresh full log](runs/core-approved-final.log.gz), exit 0; [1,311 passing executions, 0 failed, 18 ignored and 731 unchanged source hashes](runs/core-approved-final-receipt.json), including all-feature Clippy, policy and cargo-deny. Earlier cancelled/unconfirmed runs are historical and are not used as this receipt |
| Standalone WASM build/staging | Existing isolated `web` feature, production stager and emitted immutable payloads | PASS: [optimized build](runs/wasm-final-build.log), [staging](runs/wasm-final-stage.log) and [all ten payload size/SHA-256 checks](runs/wasm-final-stage-integrity.json); compilation/staging does not establish browser execution |
| Standalone loading budget | Actual emitted WASM, external artwork/font exclusion and selected-game normal dependency graph | PASS: [budget receipt](runs/wasm-final-loading-budget.json), 960,573 encoded bytes / 384,415 gzip9 bytes; no DOM runtime or unrelated game in the [normal graph](runs/wasm-final-game-tree.txt); no runtime-performance claim |
| Runtime pixels | Actual Rust presenter → Macroquad runtime on named target/source | Local launch BLOCKED: Chromium AF_UNIX singleton socket is denied; source/headless tests are not visual proof |

Intentional RenderList golden updates cover frame/grain, shadows, new compact
geometry/HUD, target-anchored promotion and upright midflight sprites. They
record renderer-neutral commands, not screenshots or pixel acceptance.

The affected command was `cargo test -p tabula-game-chess --features
bots,presentation,testkit`; the host command was `cargo test -p
tabula-game-client --features online --lib`. The [122 browser-host source
tests](runs/web-host-tests.log), [three local-server tests](runs/local-server-tests.log)
and [five native-host policy tests](runs/native-host-policy-tests.log) also
passed. The isolated host command was `cargo test --locked -p
tabula-game-client --no-default-features --features web,online --lib --test
local_match`. These checks do not imply live browser or native mobile gameplay.
The [task-scoped source manifest](runs/source-manifest.json) publishes only
Chess inputs and its directly changed presenter/host/token/asset consumers,
bound to the exact complete repository head/tree. The full local gate checked
731 source/artifact hashes; its broad file inventory remains local.
The [fresh final receipt](runs/core-approved-final-receipt.json) confirms all
checked hashes stayed unchanged after the exact-source gate. The separately retained
[pre-integration manifest](runs/source-manifest-before-integration.json) records
the earlier 713-file full-set digest
`ff0075e65306d24864071fb536e6cb4f4cc8dad0e4591d53301e50c835d167f7`; its
[complete receipt](runs/core-before-integration-receipt.json) confirms every
checked file hash stayed unchanged after that gate. These shared manifests
minimize the file inventory to this task's relevant inputs. Git HEAD metadata names
the local validation checkpoint; the published equivalent is verified by tree
identity. The latest merge changes no Rust/Cargo/Chess assets or goldens.
[Golden review hashes](runs/golden-review.json) record the seven intentionally
changed command snapshots.

The [first aggregate attempt](runs/core-initial-staging-failed.log.gz) passed
formatting and all-feature Clippy, then stopped at
`staging_pins_every_runtime_payload_and_static_host_reference`: its old
eight-payload oracle omitted the two newly declared grain densities. The
[initial source manifest](runs/source-manifest-initial.json) is retained. The
test-only repair checks the exact ten resource paths, including both
hash-pinned grain densities, cover/piece densities, three fonts and WASM;
the existing byte/SHA-256 and host-reference checks remain intact. Its
[one selected test](runs/staging-oracle-final.log) and the pre-integration
aggregate gate pass. Production staging, runtime source and goldens were unchanged by
this repair. Aggregate counts are executions, not unique tests; GitHub CI
was not inspected.

Large historical logs are retained losslessly as deterministic gzip files, with
[uncompressed sizes/hashes and round-trip checks](runs/compressed-logs.json).
The [integrated partial log](runs/core-integrated-pending.log.gz) records the
completed stages before the cancelled result read; its terminal outcome is
not inferred. The earlier full pass remains explicitly qualified. Its original
result session was unavailable when a retry was authorized, so a fresh gate ran
on the clean, verified published head `c4572e99` and completed with exit 0.
The [new log/receipt](runs/core-approved-final-receipt.json) records its source,
counts and lossless compressed-log identity. No incomplete or unconfirmed run supplies the final receipt; the old
pending receipt remains historical. A pending
or cancelled result read does not become a full-gate PASS.

The supported standalone build command was `cargo build --locked -p
tabula-game-client --no-default-features --features web --target
wasm32-unknown-unknown --profile wasm-release`, followed by
`cargo xtask stage-wasm-game`. The [built artifact](runs/wasm-final-artifact.json)
is 960,573 bytes with SHA-256
`a882d57739a9b64e156c1c28785cae658d5f4246c22d96d54f09034dd7caa189`.
Staging produces 24 required host resources plus canonical tokens and ten
immutable runtime payloads totaling 1,468,772 encoded bytes. Each actual
staged payload passes an independent size/SHA-256 recheck. These are build,
graph and integrity results; no runtime pixels or browser input were exercised.

Independent source review found and resolved compact-popup input fall-through
to covered seat bars, narrow check-title wrapping into detail, and stale asset
version assertions. Reachable menu fixtures reproduced the first failure before
the shield fix; twenty HUD tests and focused clippy passed afterward. No open
concrete source defect was reported for the frozen diff. Missing runtime,
accessibility and performance evidence remains an acceptance gap.

## Asset bounds

Pack `chess@0.2.0` contains six physical PNGs and fourteen logical resources.
The two new transparent grain exports total 4,327 encoded bytes, at 128×128
and 256×256. The full fixture is 284,567 encoded bytes and 2,339,840 decoded
RGBA bytes if both densities of all groups are resident. The existing 320KiB
encoded/per-file limits and 4MiB fixture cache-byte limit remain intact; the
context-free fixture texture-count limit changes from four to six exact files.

Explicit gameplay loads only selected-density piece atlas and grain. Setup
cover remains separate; warm restarts/DPR changes reuse verified managed
textures. This bounded local pack does not implement a general CDN delivery
or new cache service.

## Remaining #85 acceptance gates

- Actual initial/select/midflight/capture/castle/en-passant/promotion/check/
  terminal pixels in White/Black, four themes and reduced motion, 1100×850,
  1440×960, 390×844, 320×640, low landscape and DPR1/2
- Physical browser focus/touch, 200% text, assistive-technology Board Reader
  dispatch, real host/footer/safe-area and native/device runtime acceptance
- Target frame pacing, memory and input-latency measurements; no 60FPS claim
- Selection feedback currently lifts immediately; its proposed 160ms profile,
  transient 160ms turn/check emphasis and skippable 800ms outcome spotlight
  remain motion-polish work. Static selected/turn/check/outcome semantics are
  present immediately and never delay authority
- Remote event-origin timestamps are absent from the current host contract.
  The 600ms stale helper boundary is tested for timestamped local schedules,
  not remote receipt age. Collapsed/mismatched transitions and resync snap;
  legacy event-only callers omit ambiguous capture victim fades
- Public account labels/avatar delivery, full history/replay, matchmaking,
  replacement piece designs and native mobile adapters keep their independent
  contracts. This draft does not open those gates or close #85

The browser capture blocker is environmental: installed Chromium requires an
AF_UNIX singleton socket, denied even on the reviewed launch. Native display
is absent too. No prototype/exported reference is relabeled as a runtime
capture. A separately authorized exact-source graphics workflow may add a
bounded browser receipt; native/device acceptance remains separate.
