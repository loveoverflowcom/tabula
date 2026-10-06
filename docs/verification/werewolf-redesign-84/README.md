# Werewolf redesign #84 verification

Scope: [issue #84](https://github.com/loveoverflowcom/tabula/issues/84), the
original design reference directories `tabula-redesign 3` and `tabula-redesign 2`,
and [ADR-0035](../../adr/0035-werewolf-local-simulator.md)'s opt-in local simulator.
The later reference adds plain account-avatar/fallback identity and the compact
first-screen roster/action requirement to the village/motion design.

The implementation remains Rust `View`/`ViewEvent` → `GamePresentation::Local` →
`RenderList` → Macroquad. Rules, canonical replay and network authority are not
redesigned. Use [Tabula engineering](../../../.agents/skills/tabula-engineering/SKILL.md)
and [game audit](../../../.agents/skills/tabula-game-audit/SKILL.md). GitHub CI
checking is excluded by the owner's request; this does not remove local checks.

## Changed claims and check ledger

Checks below apply to this source change and the named local simulator. Counts
come from the retained command output, with nonempty selections. Build, headless
and browser evidence have different scopes; an asset pack or compilation is not
runtime acceptance. Intermediate failures are retained with their resolution.

| Claim / owner | Oracle and domain | Check / status | Remaining scope |
|---|---|---|---|
| Village/roster/dock geometry; game presenter | Desktop ellipse, compact 4×3, landscape, 6/12/20 seats; enabled controls at least 44 dp; tiny positive viewport conceals and disables interaction | PASS: [26 presenter tests](runs/werewolf-final.log), including responsive slots, paging and modal geometry | Physical text/raster quality is a separate browser check |
| Own private card and selection fail closed; game presenter | Six-role concealed output equality, authorized reveal, phase/viewer/lifecycle/blur/held keys | PASS: presenter privacy/input tests and [4 simulator tests](runs/simulator-final.log) | Actual browser interruption recorded separately; authenticated output fencing remains gated |
| Portrait selection constructs real legal intent; presenter/local host | Pointer/keyboard selection then submit/pass, Witch mode, ballots/unvote | PASS: presenter input tests roundtrip through real rules; simulator 4/4 | Physical touch and assistive-technology dispatch NOT_RUN |
| Motion is local, bounded and interruptible; presenter | Reduced/full, sparse/dense time, completed timelines, focus/permission changes; public-only ballot/death/win effects | PASS: presenter 26/26, including added motion/layout cases | Frame pacing/memory profiling and future network stale-arrival handling NOT_RUN |
| Public portraits are role-independent; host display boundary | Neutral fallback; slot/occupant replacement, stale revisions and host epoch | PASS: 7 public-display tests in the [final workspace receipt](runs/core-final-pass.log); [repaired web suite](runs/web-final.log) 72/72 separately | Account/avatar provider integration unavailable; game uses approved guest fallback |
| Assets retain pack identity/hash/bounds; game/resource shell | Pack 0.2.0: eighteen PNGs/nine role-back-scene resources at DPR1/2; avatar fallback is geometry | PASS: [reproducibility](runs/assets-reproducibility.log), presenter metadata/hash tests, [WASM build](runs/wasm-final.log) and [staging](runs/stage-final.log) | GPU allocation, cold/cache waterfall and device decode measurements NOT_RUN |
| Existing rules/privacy stay intact; rules/testkit | Real conformance, SecretModel and exact committed replay fixtures | PASS: [142 affected game tests](runs/werewolf-final.log), including 11 conformance, 20 security and 7 replay tests; no canonical rules edit | Native/WASM deterministic execution comparison NOT_RUN |
| Shared clip/text/host slots work; renderer/web host | Asymmetric corner image, Vietnamese baseline, nested clip/reset and host bounds | PASS: actual Macroquad fixture 8 scope/DPR sets and 2 ink comparisons, negative control; 10 current footer layouts plus 10 original CSS controls; [122 host tests](runs/host-tests.log) | Fixture pixel scope is separate from game interaction acceptance |
| Portable core gate; repository | Repository-owned fmt/clippy/test/architecture/token/deny order | PASS: [`cargo xtask check`](runs/core-final-pass.log), exit 0; 1,249 passing test executions and 18 ignored | Counts are executions, not unique tests; GitHub CI not inspected |
| Real Werewolf visual/interaction quality; Macroquad | Night/reveal/select/submit, public ballot, dawn/death/terminal; four themes, reduced motion and full reveal/phase motion | PASS: [15 actual browser cases](runs/summary.json), 0 failed, final artifact 999,543 bytes; [27 captures visually inspected](runs/visual-inspection.json) | Native/device, 200% text, safe area and assistive technology NOT_RUN |

## Executed checks and reproducibility

The affected game command was:

```bash
cargo test -p tabula-game-werewolf --features presentation,testkit
cargo test -p tabula-game-werewolf --features presentation --lib presentation
cargo test -p tabula-game-client --no-default-features --features werewolf --test werewolf_simulator
node --test apps/game-client/web/tests/*.test.cjs
python3 games/werewolf/assets/generate.py --check
```

The full game run selects 142 tests: presenter/assets 26, assignment 8, config
17, conformance 11, metadata 3, replay 7, rules 21, security 20 and state 29.
The separate simulator feature explicitly selects four tests. The default
workspace simulator test target selects zero tests, so it cannot replace that
feature-specific privacy check. An [earlier focused presenter receipt](runs/presenter-final.log)
passed 25 tests before the added dead Full-vision regression; the final full game
receipt contains all 26 presenter tests. Source review found and repaired the
missing authorized role-name list inside the explicitly revealed dead-player
drawer; that added regression checks its concealment guard. The host command
selects 122 tests; its existing
online host tests do not imply live online acceptance.

The renderer fixture exercises the real `MacroquadRenderer` with managed
asymmetric artwork and Vietnamese text, with unbounded, clipped, nested and
reset scopes at DPR1/2. [Corner and ink receipt](runs/renderer-clips/clip-pixel-assertions.json)
and [browser receipt](runs/renderer-clips/clip-browser.json) show eight upright
corner sets and two identical text strips. The [negative control](runs/renderer-clips/clip-negative-pixel-assertions.json)
restores only the wrong Y-sign in a disposable current backend: clipped/nested
images flip at both densities while unbounded/reset stay upright. This is a
sensitivity check, not a whole historical renderer comparison.

[Footer bounds](runs/renderer-clips/footer-layout.json) check five viewports at
DPR1/2: ten current layouts keep a positive canvas above the host row, and ten
original CSS controls overlap. Fixture sources, commands and artifact scope are
in [the retained reproduction notes](runs/renderer-clips/README.md). Selected
[current DPR1](screenshots/renderer-clips/actual-macroquad-clips-dpr1.png),
[current DPR2](screenshots/renderer-clips/actual-macroquad-clips-dpr2.png) and
[negative DPR2](screenshots/renderer-clips/negative-macroquad-clips-dpr2.png)
images are retained alongside all ten footer captures. The [independent pixel
recheck](runs/renderer-clips/pixel-recheck.json) repeats all sixteen current and
negative corner sets and both ink comparisons against those durable images.

The actual gameplay browser runner is [verify-redesign.mjs](../../../games/werewolf/tests/verify-redesign.mjs).
It uses only public host controls and real pointer/keyboard input, not WASM
memory or canonical/projection instrumentation. Run it against the staged
loopback host with `TABULA_PLAYWRIGHT` and `TABULA_PNGJS` naming the installed
package directories and `TABULA_CHROME` naming the browser executable:

```bash
node games/werewolf/tests/verify-redesign.mjs http://127.0.0.1:8085 docs/verification/werewolf-redesign-84
```

[Runtime summary](runs/summary.json) and per-case receipts record browser
version, artifact bytes/SHA-256, error lists, input steps, host bounds and capture
hashes. The ten viewport/DPR cases explicitly request reduced motion. Dark is
covered by that matrix, with light, high-contrast light and high-contrast dark
captures separately. [Public phase captures](runs/public-phase-captures.json)
include Dawn, a seven-vote public tally, a public elimination and terminal draw.
Their semantic verdict requires visual inspection; capture existence alone is
not an assertion that a phase was reached. The dead Full-vision drawer was
opened during the long Night phase: its page changes 2,492 information pixels
with zero change in the card crop, and blur/resume stays concealed. The retained
second page visibly lists the projection-authorized role names inside the
guarded region.

The separate [full-motion case](runs/full-motion.json) uses `motion=system` and
browser `no-preference`. Its retained reveal start/settle, Dawn start/settle and
blur/resume captures demonstrate actual pixel changes and interruption. Capture
completion times include encoding overhead; the middle reveal sample is already
settled, so they do not establish exact intermediate progress or frame pacing.
Full public-death/win timing remains covered by headless timeline tests rather
than these browser samples. [Capture integrity](runs/capture-integrity.json)
independently verifies 31 unique retained file/SHA-256 pairs.
The [visual inspection receipt](runs/visual-inspection.json) names the 27
inspected captures and their semantic verdicts. All fifteen runtime cases use
the final artifact above; disposable browser profiles were closed and the
loopback acceptance server stopped after the run.

## Failures and resolutions

The [initial simulator run](runs/simulator-initial-failed.log) failed
`private_actions_do_not_change_outsider_projection_or_public_render` (3 passed,
1 failed): private submission changed an outsider's rendered output. The
presenter was corrected while preserving that independent test; the final
simulator run passes 4/4, including exact outsider projection/RenderList equality.

The [initial core gate](runs/core-initial-clippy-failed.log) stopped at clippy's
103/100-line limit in presenter `a11y`. The code was refactored and the next gate
passed all-feature clippy. That [intermediate gate](runs/core-intermediate-account-test-failed.log)
then stopped at the web static account test: the avatar's erased `AnyView`
branch panicked during HTML rendering without `ssr`. A typed avatar branch
preserves the existing SSR oracle; the repaired web suite passes 72/72. The
final authoritative gate receipt below includes that repair.

The [next gate](runs/core-intermediate-pack-oracle-failed.log) passed the web
suite but stopped at `simulator_stage_uses_only_its_pack_and_keeps_runtime_identity`:
the old oracle still expected 18 runtime files from pack 0.1.0. It now checks pack
0.2.0's 18 PNG exports and 22 staged immutable resources, preserving its locality
and identity assertions. The [focused staging oracle](runs/pack-stage-final.log)
passes its one selected test (113 filtered out).

The [following gate](runs/core-intermediate-game-id-failed.log) passed workspace
tests and dependency checks, then rejected five game identifiers in the browser
runner's `tools/` path under I-9. The runner was moved to the game-owned `games/werewolf/tests/verify-redesign.mjs`;
[the game-identifier check](runs/game-id-final.log) passes with its policy unchanged.

The [next gate](runs/core-intermediate-color-policy-failed.log) passed those
checks and rejected the render opacity helper's raw `Color` constructor. The
helper now calls `Color::with_alpha` in the design boundary, preserving the
existing semantic RGB channels exactly. The color policy was not relaxed; its
[focused check](runs/color-final.log) passes. Rebuilding this opacity cleanup
produces the same optimized WASM bytes as the active final runtime run:
999,543 bytes, SHA-256
`82f52fc44d66c369b2108b0e96d8e8d3fed3afc6c79486a0525c4b3e7c0df7c0`.
The final core gate passes with exit 0.

The [last sandboxed attempt](runs/core-environment-limited-deny.log) passed all
code/test/architecture checks but could not acquire cargo-deny's advisory-cache
lock on a read-only path outside the workspace. Automatic approval allowed the
same authoritative `cargo xtask check` command access to that local cache. The
[final run](runs/core-final-pass.log) passes every portable gate, including
advisories, bans, licenses and sources. Existing unmatched-wrapper warnings are
non-failing. Its 1,249 passing test executions and 18 ignored executions do not
replace separately gated native/device, database or online acceptance.

The first two landscape browser attempts used a stale close-button coordinate
and clicked the host keyboard Help button. [Those original receipts](runs/runner-correction/summary.json)
and failure images are retained as runner drift. Correcting the runner makes
both full interaction cases pass. Subsequent visual review found that the
landscape public action hint wrapped under its button; [the before/after images
and artifact identities](runs/review-corrections/landscape-hint.json) are retained.
The concise-label repair is visually confirmed in the final landscape image:
the one-line hint sits above the primary action without overlap.

The maintained screen/rubric and verification documents passed a [local Markdown
path check](runs/document-links.json), which records its exact existing-target
selection. The Python skill validator could not run because
its `yaml` module is unavailable in both configured Python environments; no
dependency was installed solely for that documentation check.

## Local execution paths

From the repository root:

```bash
cargo nextest run -p tabula-game-werewolf --features presentation
cargo test -p tabula-game-client --no-default-features --features werewolf --test werewolf_simulator
cargo run -p tabula-game-client --no-default-features --features werewolf --bin tabula-werewolf-client -- --seats 12
cargo build -p tabula-game-client --no-default-features --features web-werewolf --bin tabula-werewolf-client --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-wasm-game --game werewolf
python3 -m http.server 8768 --bind 127.0.0.1 --directory target/tabula-web-werewolf
```

Browser staging is build evidence until `play.html` is loaded and the real
canvas/inputs are exercised. Inspect 1200×880, 1100×850, 390×844, 320×640 and
844×390 at DPR1/2. At 390×844 the twelve-seat roster and primary action should fit
the first screen, with a separate host leave/help slot. Include short/landscape
drawer, focus restore and public/dead views. Static design PNGs and headless
RenderLists cannot substitute for these screenshots.

## Explicit limits

Account/avatar provider integration is unavailable; the simulator uses a neutral
host fallback, not design fixture accounts. No arbitrary avatar URL, raw account
identity or profile fetch belongs in game rules or canonical state. Real dashboard
avatar parity needs a later implemented shared provider and permitted data source.

Online/social play, auth/seat ownership, chat/socket enforcement, voice, persisted
resume/replay, CMP WebView embedding, production rollout, publishing and phase
exits remain outside this slice. Historical standalone evidence applies only to
its own named source. Native/WASM compilation is not cross-target deterministic
execution, and successful screenshots do not establish performance or private
transport fencing.

## 2026-10-06 options review follow-up

The owner requested review/polish of the single “Tùy chọn” control and direct
commit of the entire existing workspace to `develop`. Upstream dashboard work
was integrated from `666474c`; its navigation/focus behavior and discovery
composition are preserved while the shared neutral avatar remains typed.

The footer trigger now uses a compact labeled pill with a primitive slider
mark and shared hover/press/focus handling. A centered, bounded panel separates
“Chuyển động” from “Công cụ mô phỏng”; short landscape keeps the same controls
with tighter spacing. “Bật” uses active emphasis. Hit testing and rendering
share the options geometry; opening/closing and simulator requests keep the
existing concealment boundary. Accessibility retains the complete effects label.

This follow-up's evidence supersedes the earlier artifact only for these
changed claims. [Browser inspection](runs/options-review/browser-inspection.json)
records the 1,001,301-byte WASM SHA-256, ten retained inspected JPEG captures,
public pointer/keyboard steps, viewport/theme scope and limits. Desktop,
320px portrait and low landscape show readable modal labels; the 390px capture
shows the trigger after a real phase change. Four themes are visually inspected.
This is actual Codex in-app browser evidence at DPR1, not a new full gameplay,
DPR2, device, shell pixel or frame-pacing acceptance run. An intermediate icon
was hidden below the shared button fill; its HUD ordering was repaired before
these final captures.

Final executed checks:

- [`cargo xtask check`](runs/options-review/core-check.log): PASS, all gates;
  1,256 passing test executions, 18 ignored.
- [`cargo test -p tabula-game-werewolf --features presentation,testkit`](runs/options-review/werewolf-tests.log): PASS, 142 tests including conformance/security/replay.
- [`cargo test -p tabula-game-client --no-default-features --features werewolf --test werewolf_simulator`](runs/options-review/simulator-tests.log): PASS, 4 tests.
- [`node --test apps/game-client/web/tests/*.test.cjs`](runs/options-review/host-js-tests.log): PASS, 122 tests.
- [`python3 games/werewolf/assets/generate.py --check`](runs/options-review/assets-check.log): PASS.
- [WASM build](runs/options-review/wasm-build.log) and [staging](runs/options-review/stage.log): PASS; browser ran the resulting artifact.
