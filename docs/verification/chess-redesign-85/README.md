# Chess #85 runtime redesign

Draft [PR #112](https://github.com/loveoverflowcom/tabula/pull/112) implements the
board material, compact HUD and accepted-move slice of
[issue #85](https://github.com/loveoverflowcom/tabula/issues/85). It starts from
fresh `develop@a3865246` and integrates `develop@face8a3e` then `80d9fdb9`, preserving upstream
mobile-account/responsive work. The original [PR #86 handoff](https://github.com/loveoverflowcom/tabula/pull/86)
and its [immutable design archive](https://github.com/loveoverflowcom/tabula/tree/dac1c2940a7d1fa22f6057074618ab3377d8604f/docs/ui/issue-82-game-feel/chess)
are design references, not runtime screenshots. The issue remains open.

## Implemented ownership

- Game-local frame, lower rim, bevel/inlay and quiet managed grain use the
  approved ivory/slate/truffle game-art roles. System primary/focus/legal/check
  roles and original Staunton SVG/atlas bytes remain unchanged
- One bounded accepted composition validates the mover, synchronized castle
  rook, direct/en-passant victim and promotion endpoint against previous/current
  authorized public projections. Absolute sampling gives an arc, landing scale,
  grounded shadows and capture fade without mutating rules or delaying authority
- Generic projection/rejection hooks supply accepted local/online host context.
  Input, blur, changed endpoint/orientation, reduced motion, rejection and
  reset/resync snap or discard motion. No State, wire, replay identity, authority,
  engine or game-id dispatch change is involved
- Board-aligned player bars, a bounded desktop rail, compact Actions disclosure
  and inward-opening promotion choices preserve legality, confirmations,
  keyboard access and upright piece/coordinate semantics. Open menu padding and
  disabled targets shield covered board/seat controls
- Ownership/clocks come only from `View`. Recent coordinates and captures are
  explicitly bounded observations from this session, not SAN or saved history
- Actual light-theme pixels prompted a bounded game-local readability follow-up:
  informational HUD roles use existing 14px/20px tokens and important text uses
  `game-art.chess.ink`. Two player lines stay inside 44px bars; compact session
  detail appears only in a 48px-or-taller slot and uses Q/R/B/N promotion notation.
  Shorter compact titles keep accepted draw offers and simultaneous CHECK visible. The desktop helper
  uses two complete explicit lines inside its reserved slot.
  Tiny board coordinates, font loading, renderer and theme tokens are unchanged

## Verification summary

Raw generated logs, JSON, images and compressed receipts are intentionally kept
out of source control. Deterministic tests and runtime assets remain versioned.
Earlier local runs and the cancelled result read are historical; neither an
incomplete run nor source tests are used as visual acceptance.

| Claim | Executed evidence | Status / limit |
|---|---|---|
| Accepted motion | Legal transitions, both-color en passant, all four castles, all promotion choices/orientations, sparse/dense sampling and interruption | PASS: eight focused motion tests; no remote origin-age claim |
| Host projection/rejection | 60 host library tests; 60 isolated web/online library plus 15 local integration tests | PASS on the published runtime baseline; live online fault acceptance remains separate |
| HUD input/layout | Popup shielding, repeat/cancel, keyboard, inward promotion and bounded 320/390 portrait/short-landscape controls | PASS: renderer-neutral tests; physical touch/assistive technology remains NOT_RUN |
| Readability role regression | Four themes × six viewport bounds, existing line slots, important ink roles, compact promotions/session qualifier and short-status accepted draw/CHECK cues | PASS: four focused tests; old 11/12px roles fail the new minimum-size assertion. 14px after-change pixels are inspected; final explicit-line helper recheck pending |
| Pointer capture oracle | Five independent fixed board-bound rows; 640 White/Black square centers in Python and actual Rust; exactly one online Rust status-dock 56px reservation | PASS: 52 focused Python checks and one Rust fixture. The old helper mismatches all 640 centers; this is not a live online browser result |
| Rules/replay/conformance | Existing Chess/testkit targets, eleven conformance and seven replay tests | PASS: 199 affected executions, 0 failed, one ignored depth-five perft excluded; rules source/replay identities unchanged |
| Portable repository gate | Repository-owned `cargo xtask check` order | PASS: 1,316 executions, 0 failed, 18 ignored; fmt, all-feature Clippy, policy and cargo-deny passed. The final gate follows clean develop integration; 391 frozen source hashes are checked unchanged |
| Original assets/managed pack | Exact six-file `chess@0.2.0` metadata/hash, density/cache behavior and reproducible exports | PASS on the published baseline; no new assets in the readability follow-up |
| WASM/staging/loading | Existing isolated web feature, production stager, immutable byte/SHA checks | PASS: final local WASM 961,136 bytes / gzip9 384,507; all ten immutable payload size/SHA checks. Build/staging is not browser proof |
| Actual browser pixels | Exact-source Rust presenter → Macroquad WASM, source-separated dedicated capture | PARTIAL on `c61dbc62`: 27 PNGs, 42 PASS / 2 OCR FAIL. `94dc7df5` after-source: seven PNGs, 11 PASS / 2 unchanged OCR FAIL; final guidance recheck pending |
| Native/device/performance | Native mobile adapter, physical device, frame pacing/memory/input latency | NOT_RUN; no device, native gameplay or 60FPS claim |

The affected commands are `cargo test -p tabula-game-chess --features
bots,presentation,testkit`, `cargo test -p tabula-game-client --features online
--lib`, and `cargo test --locked -p tabula-game-client --no-default-features
--features web,online --lib --test local_match`. Existing source-side host checks
also passed: 122 Node, three local-server and five native-host policy tests.
RenderList golden updates are reviewed command traces, not rendered pixels.

The standalone build is `cargo build --locked -p tabula-game-client
--no-default-features --features web --target wasm32-unknown-unknown --profile
wasm-release`, followed by `cargo xtask stage-wasm-game`. Official Rust 1.96.1,
native/WASM targets and cargo-deny 0.20.2 are installed in scratch. Baseline
staging independently verified 24 host resources and ten immutable runtime
payloads. The final local WASM SHA-256 is
`614c64f1f3ac2aee357bfd29504d1cab8308bdd1daed16c2fb95bff4853e8bd9`. The local and Actions WASM builds retain separate provenance; their
binary hashes are not asserted equal. Unrelated GitHub CI was not polled.

## Actual browser receipt and follow-up

The [source-pinned findings](https://github.com/loveoverflowcom/tabula/pull/112#issuecomment-6033618370)
link the [dedicated capture run](https://github.com/loveoverflowcom/tabula/actions/runs/37589145897)
and its [27-image artifact](https://github.com/loveoverflowcom/tabula/actions/runs/37589145897/artifacts/11468256290).
The capture source is `c61dbc62271cd4a5554dd62bc139a06bab58fb44`; all original
PNG identities/dimensions and source/harness pins were checked and actual pixels
inspected. Upright pieces, selection/Flip, public captures, both castles,
promotions, menu shielding and actual Black-wins/checkmate/CHECK terminal states
were observed. The raw 42-PASS/2-OCR-FAIL result remains PARTIAL.

Independent final source review found no remaining actionable HUD defect; the
short-status draw-offer regression discovered during review was repaired before
publication. The after-source actual pixels show stronger text; final explicit-line guidance
wrapping still needs its affected-state recheck.

Light 11/12px helper/history/result-detail text visibly appeared thin/pale.
Authored muted/surface contrast is 6.17:1, so this is not evidence of a token
contrast failure. A possible browser text-edge compositing cause is unproven;
this follow-up changes only Chess text roles/ink and requires new exact-source
pixel inspection. The initially alleged title clipping was rechecked against
original-resolution pixels and retracted: title ink/bounds match across initial,
castles and full/reduced-motion frames. No title/renderer fix follows that claim.

The [after-source report](https://github.com/loveoverflowcom/tabula/pull/112#issuecomment-6034448779)
links [run 37595499517](https://github.com/loveoverflowcom/tabula/actions/runs/37595499517)
and its [seven-image artifact](https://github.com/loveoverflowcom/tabula/actions/runs/37595499517/artifacts/11470817114).
Source `94dc7df5012640e7d6909d1ab5c02eb273d1dc37` / tree `7a2f8d0d`
shows visibly stronger 14px seat/guidance/session/result text and bounded 390/320
initial/after-e2e4 layouts. The raw 11-PASS/2-OCR-FAIL receipt remains PARTIAL:
original PNGs visibly show d5/f6/checkmate, while the unchanged Tesseract checks
misrecognize them. Actual Tesseract is 4.1.1/PSM11; the old hand-written version-5
label was corrected in the report. No defect is inferred from OCR alone.

Desktop helper pixels still split “destination” inside the word. The final
one-string follow-up uses `Select a piece\nthen a legal destination` with two
complete lines at +44/+64 before the next heading at +96. Its source/slot
regression passes; a new exact-source affected-state screenshot is pending.

The bounded existing-context probe recorded alpha/premultipliedAlpha enabled and
RGB/alpha source factors 770/771. Its sampled foreground/background alpha was
255, with no pure-white candidates in that region. It is inconclusive for a
per-glyph blend diagnosis; renderer/vendor settings remain unchanged. These
Actions artifacts currently expire on 2026-10-21, rather than providing permanent
repo image storage; no raw images/receipts are copied back into docs.

Local Chromium launch remains BLOCKED by denied AF_UNIX singleton sockets and
there is no native display. The dedicated, separately authorized Actions route
supplies actual browser evidence without bypassing the local denial. Design
exports and headless traces are never relabeled as runtime captures.

## Asset bounds

Pack `chess@0.2.0` contains six physical PNGs and fourteen logical resources.
New transparent 128²/256² grain totals 4,327 encoded bytes. The full fixture is
284,567 encoded bytes and 2,339,840 decoded RGBA bytes if both densities/groups
are resident. Existing 320KiB encoded/per-file and 4MiB fixture-cache limits
remain. Gameplay loads only selected-density piece atlas and grain; setup cover
is separate and verified warm caches are reused. No general CDN/cache service
is added.

## Remaining #85 acceptance gates

- Exact-source after-change initial/capture/result pixels and raw OCR; broader
  White/Black/theme/DPR/viewport coverage, physical focus/touch, 200% text and
  assistive-technology Board Reader dispatch remain named target work
- Target frame pacing, memory and input latency; native/device gameplay remains
  independent and NOT_RUN
- Selection feedback currently lifts immediately. Proposed 160ms selection and
  turn/check emphasis plus a skippable 800ms outcome spotlight remain polish.
  Static semantics appear immediately and never delay authority
- Remote event-origin timestamps are absent. The 600ms stale helper boundary is
  tested for timestamped local schedules, not remote receipt age. Collapsed or
  mismatched transitions/resync snap; event-only legacy callers omit ambiguous
  capture victim fades
- Account labels/avatars, full history/replay, matchmaking, replacement pieces
  and native mobile adapters retain their separate contracts. No automatic
  merge, deploy or closure of #85 follows this draft
