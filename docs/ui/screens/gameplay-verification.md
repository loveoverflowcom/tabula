# Gameplay verification ledger

Issue #51 Stage A source review at `develop @
1d8fab294931750f45ef5d7b498f34b5b0417188`, 2026-10-03.
[`tabula-engineering`](../../../.agents/skills/tabula-engineering/SKILL.md)
and [`tabula-game-audit`](../../../.agents/skills/tabula-game-audit/SKILL.md),
including presentation and game rubrics, govern evidence vocabulary.

## Specification evidence

| Claim / invariant | Owner and failure mode | Oracle / scope | Evidence / status | Residual scope |
|---|---|---|---|---|
| Canonical state/secret data never becomes UI input (I-5/I-6) | `project`/`view_event`; canonical bag order/role map sent to presenter | doc 00/02, actual Chess/Tiles `View`, Werewolf creation kernel | `source-read`, documented PASS for boundary mapping | No new security execution; Werewolf projection/reducer absent |
| Local interaction and pending cannot become authority (I-10/I-12) | Presenter vs local/session shell; optimistic board/clock mutation | `GamePresentation`, `LocalMatch`, actual Local structs, doc 04 §4 | `source-read`, documented PASS | Network pending queue NOT_IMPLEMENTED; runtime adapter changes require focused execution |
| Each rendered action has an honest command/local owner | Chess/Tiles `on_input`, controls and descriptions; wrong Cancel/Skip or invented EndTurn | Actual command constructors/phase gates compared with artwork | `source-read`, documented PASS | Unsupported artwork controls are named; multi-segment Tiles parity needs separate evidence |
| Future work does not cross gates | doc 07/AGENTS/ADR-0028; treating discovery slice as multiplayer authorization | Actual Caro/Werewolf and Phase-4 scaffolds | `source-read`, documented PASS | Phase-3/4/5 exits not established; Phase-7/8/9 work deferred |
| Board-first M3 Expressive treatment matches inspected reference | Foundation/Theme, pack notes and images | Pinned `04-chess.png`, `06-tiles.png`, `mobile-states.png` | `screenshot-inspected` PASS as static design review | Runtime visuals/behavior are not established by artwork |
| Companion planned screens retain provenance | Pinned 05/07 SVG, screen index and implementation prompts | SVG/XML text/structure plus parent-provided rasterizations; both contain Caro/private-role/recovery panels | `source-read` and `screenshot-inspected` PASS as static design review | No gameplay or interactive prototype execution |
| Documentation links and whitespace are sound | Screen specs/index; broken relative links or patch whitespace | Python repository-relative link walk over seven changed Markdown files; `git diff --check -- docs/ui/screens` | PASS: seven files, zero missing local links; whitespace check exit 0 | Neither check tests runtime behavior |

The reference pack was extracted from the pinned Git object into temporary
files; it is not copied into develop. Three supplied PNGs were viewed as
images, including desktop board/HUD hierarchy and compact/recovery examples.
Screen 05 and 07 were also viewed through parent-provided PNG rasterizations
of their pinned SVGs (`/tmp/tabula-caro.png`, `/tmp/tabula-werewolf.png`).
These are static samples, not screenshots of a running Tabula client.

## Runtime acceptance to execute with each presenter slice

| Scope / claim | Meaningful evidence target | Stage-A status |
|---|---|---|
| Chess compact HUD/clock/turn/low-time | Nonempty presenter tests, clock rules regression, conformance, deliberate snapshot review; real Mac keyboard/pointer and four-scheme inspection | NOT_RUN by documentation slice; current presenter is implemented |
| Tiles ≥44 dp hit targets, labels and compact HUD | Nonempty presenter tests for hit size/corners, no overlay capture, camera invariance, placement/claim/skip/keyboard, descriptions; conformance/security and snapshot review; real Mac interaction | NOT_RUN by documentation slice; current presenter is implemented |
| Shared local reject/fatal/pending feedback | Real local adapter results, retained View, once-only submit, visible reason and locked fatal input, cancellation/repeat/blur; truthful local busy lifecycle | NOT_RUN; error types exist, visible binding was missing at review base |
| Workspace gate | `just check` in repository-owned order; feature/target builds required by actual change | NOT_RUN by this delegated docs-only slice; implementing owner records final run |
| Network disconnect/resume/resync/version/asset delivery | Actual session/server/asset boundaries with idempotent identity and late-result tests | NOT_IMPLEMENTED, Phase 4 gate |
| Board Reader status/actions and `ActionId → Intent` | Real web DOM/native adapter, disabled/stale/once-only actions, keyboard-only completion and AT announcements | NOT_IMPLEMENTED, Phase 5 gate; full regions Phase 9 |
| Caro / Werewolf gameplay | Actual future rules/projection/presenter implementations and their nonempty checks | NOT_IMPLEMENTED; Caro Phase 3, Werewolf Phase 7 UI |

Headless snapshots assert ordered draw commands; they do not establish native
font metrics, wrapping, device scaling or focus contrast. Compilation/WASM
builds are `compiled`, not interaction or cross-target determinism evidence.
Zero selected tests, ignored tests and unsupported game launches cannot pass
the rows above. Keep original snapshot expectations and inspect each intended
change before updating.

## Known gaps at the review base

- Tiles targets are 34 dp and symbolic labels lack their intended readable
  command names. Short/compact status and HUD text need measured reflow.
- Chess clock text uses ordinary body style; active/low-time visual treatment
  needs a defined presentation policy and four-scheme measurement.
- `apps/game-client/src/main.rs` reports command rejection/fatal errors only
  to stderr and leaves the loop on fatal; there is no visible error adapter.
- `apps/game-client/web/index.html` contains `maximum-scale=1.0` and
  `user-scalable=no`. Required user zoom is blocked. The canvas uses
  `outline: none`/`tabindex="1"`, and there is no DOM Board Reader or action
  dispatcher. Keyboard canvas descriptions do not resolve these web gaps.
- No current game-client preference adapter selects all four themes at runtime
  from user/system preferences. Source token availability is not theme-switch
  interaction evidence. Native/mobile AT, touch/pinch, safe-area and 200%
  text-scaling runs remain separate platform evidence.

Stage B keeps one existing game per change plus genuinely required shared
support. Stage C must re-pin source and prove Phase-4/5 prerequisites before
connecting network loader/recovery and the status/actions mirror. This ledger
does not convert a future acceptance requirement into a passing runtime test.

## Stage B implementation and provisional evidence

The following implementation is on top of the Stage-A review base. Results
in this section were reported by the implementing/review agents and checked
against their retained local logs/source. They used Rust **1.94.0** as an
available fallback (Tiles, client and presentation explicitly passed
`--offline`). They do not replace the repository-pinned Rust 1.96.0
aggregate gate, final feature/target checks, or the real Mac UI acceptance;
the implementing owner records those separately after they finish.

| Changed claim / invariant | Owner and named failure mode | Oracle / scope | Provisional evidence / status | Remaining scope |
|---|---|---|---|---|
| Chess clocks are readable, separate from board geometry and remain presentation-only (I-10/I-12) | Chess presenter; body-font drifting clock, hidden turn/low-time or terminal checkpoint continuing to count down | Compact/short-landscape bounds, four-scheme status/clock commands, authoritative checkpoint and existing interaction/motion regressions | `source-read`: `MonoMd`, textual active/`LOW` markers, low-time threshold ≤30 s for the active player, no terminal countdown; package run/count to be recorded by owner | Native glyph metrics/pixels and final package/aggregate evidence still owner-held |
| Tiles commands retain accessible-sized hit geometry and map to actual intents (I-10) | Tiles presenter/shared `ActionButton`; 34 dp targets, ambiguous symbolic labels, wrong Skip/Cancel meaning | 56 dp HUD targets, labeled camera/rotation plus Place/Claim primary, phase/viewer/pause/terminal gates, once-only input and every advertised claim segment | `example-tested` PASS: 155 package tests, including 28 presentation and 11 conformance; existing SecretModel/security suite and bag-permutation noninterference pass in the same run | This is Rust execution, not measured touch targets or rendered pixels; network and Board Reader remain gated |
| A zoomed short board keeps keyboard cursor/focus outside HUD | Tiles camera/map layout; maximum zoom centred under the toolbar | `maximum_zoom_keeps_the_short_map_cursor_and_focus_marker_visible`, short viewport at maximum source zoom | Regression reproduced FAIL before fix, then `example-tested` PASS; full suite rerun PASS | Actual Mac compact/landscape interaction still pending |
| Local rejection/fatal feedback is visible and honest (I-5/I-10/I-12) | `runtime_ui::LocalFeedback` and local loop; stderr-only error, raw detail disclosure or fatal session accidentally resumed | Stable error-code copy, latched fatal gate, actual accepted-input clearing, terminal distinction, screen-space feedback and fresh-match recovery | `example-tested` PASS: client 34 library + 2 main + 11 integration tests; controlled message never copies arbitrary `RuleError::detail` | No async/pending/network/loading state exists: local execution is synchronous. New local game starts fresh, never claims resume |
| Feedback cannot accidentally release a board gesture or latch a held key | Runtime pointer ownership and local cancellation; overlay captures Down/Move, then dismissal delivers Up to board; keyboard Tab/Escape swallows release | Three real Tiles integration regressions; pointer owner retained until Up/Cancel, exactly-once local cancellation; physical key releases forwarded | `integration-tested` PASS within the 11 client integration tests; `cancel_presentation_pointer` neither fires timers, consumes an input index nor applies any cancellation-generated Intent | Platform-native callback/input behavior still needs Mac exercise |
| Native interruption/reentry cannot activate stale input | Macroquad input normalizer/shared button interaction; blur/minimize/window movement plus synthetic release or held reentry submits a command | Focus-before-Cancel ordering, contact ownership, authentic fresh press, same-frame leave/reentry and key-release guards | `example-tested` PASS: renderer 50 tests; presentation 50 tests, including 13 button tests. Button regression subset first FAIL (2 of 13), then PASS (13 of 13) | Lifecycle callback semantics are broader than an OS focus query; native/browser event wiring still needs actual interaction |
| Tabular ASCII clock figures share layout advances without unsafe text preflight | Renderer text backend; narrow/wide digits shift clock or validation requires GPU/font context | Equal digit-cell layout; measurement/wrap/draw share layout; Unicode scalar progress, oversized scalar and preflight capacity boundaries | `example-tested` PASS within renderer 50; pure preflight/wrapping/layout checks | Default-font fallback, not a loaded monospaced face; no new complex shaping/font-family/weight support or pixel claim |
| Native theme selection chooses generated schemes without game branching | game-client option parsing; requested scheme silently lost or unrelated selected game changed | `--theme light`, `dark`, `hc-light`, `hc-dark`; invalid/missing value keeps Light and selected game/options | `example-tested` PASS within 2 main tests; source consumes existing `ThemeKind`/generated tokens | Native CLI selection only; no persisted/system-following preference controller or runtime switch/AT evidence |
| Final independent source review finds no remaining functional findings | Read-only reviewer; geometry, authority or interrupted-input regression after integration | Final Chess/Tiles/runtime/button/text/input source and retained evidence | `source-read` PASS, no remaining functional findings; `git diff --check` PASS reported by reviewer | Reviewer did not independently execute tests or native UI; final pinned gates/UI still owner-held |

Retained provisional logs: `/tmp/issue51-tiles-tests-1.94.log`,
`/tmp/issue51-tiles-clippy-1.94.log`,
`/tmp/issue51-tiles-short-zoom-before.log`,
`/tmp/issue51-tiles-short-zoom-after.log`,
`/tmp/tabula-51-renderer-tests.log`, `/tmp/tabula-51-renderer-clippy.log`,
`/tmp/tabula-51-client-local-match.log`,
`/tmp/tabula-51-button-before.log`, `/tmp/tabula-51-button-after.log`.
These temporary paths are local evidence, not durable CI artifacts. The
client's full 34/2/11 run and presentation's 50-test run retain tool output,
not standalone log files; the client integration log is the separate
11-test run. All nonempty counts above are executed tests; zero Tiles/client
doc-test selections supply no additional test evidence. Implementing agents
also reported formatting and Clippy `-D warnings` PASS on the fallback
toolchain.

Recorded test invocations, all from
`/Users/manhblue/Documents/Codex/2026-10-03/task-2/tabula`:

```sh
cargo +1.94.0 test --offline -p tabula-game-tiles --features bots,presentation
cargo +1.94.0 test --offline -p tabula-game-tiles --features presentation --lib maximum_zoom_keeps_the_short_map_cursor_and_focus_marker_visible
RUSTUP_TOOLCHAIN=1.94.0 cargo test -p renderer-macroquad --lib
RUSTUP_TOOLCHAIN=1.94.0 cargo test -p tabula-game-client --test local_match
env RUSTUP_TOOLCHAIN=1.94.0 /Users/manhblue/.cargo/bin/cargo test --offline -p tabula-game-client
RUSTUP_TOOLCHAIN=1.94.0 CARGO_TARGET_DIR=/tmp/tabula-button-focused cargo test --offline -p tabula-presentation --lib
RUSTUP_TOOLCHAIN=1.94.0 CARGO_TARGET_DIR=/tmp/tabula-button-focused cargo test --offline -p tabula-presentation --lib button::tests -- --nocapture
```

The focused Tiles regression used the same test invocation before and after
the fix, with output redirected to its two named logs. The button run was
captured through `tee` without `pipefail`; its PASS derives from the explicit
13-pass test output, not the shell pipeline's exit code. Its direct baseline
test exited 101; the subsequently retained baseline output confirms two
failures. The other successful test invocations reported exit 0.

Recorded Clippy invocations all exited 0:

```sh
cargo +1.94.0 clippy --offline -p tabula-game-tiles --features bots,presentation --all-targets -- -D warnings
RUSTUP_TOOLCHAIN=1.94.0 cargo clippy -p renderer-macroquad --all-targets -- -D warnings
env RUSTUP_TOOLCHAIN=1.94.0 /Users/manhblue/.cargo/bin/cargo clippy --offline -p tabula-game-client --all-targets -- -D warnings
RUSTUP_TOOLCHAIN=1.94.0 CARGO_TARGET_DIR=/tmp/tabula-button-focused cargo clippy --offline -p tabula-presentation --all-targets -- -D warnings
```

### Source limits retained after Stage B

- Macroquad lifecycle callbacks conflate blur, minimization and, on macOS,
  window movement. They are treated as an input interruption, followed by
  restored callback or authentic fresh press; this is not an exact OS focus
  query. Hover, held-repeat or synthetic mouse release cannot restore it.
- Text preflight without a width bound counts explicit lines exactly. With
  a width bound it conservatively reserves one slot per Unicode scalar and
  one per empty paragraph, capped at **65,535 slots**. An over-capacity value
  is rejected before drawing even if measured wrapping might use fewer
  lines. Actual measure/draw wrapping remains font-based; this conservative
  capacity barrier is deliberate, not a measured universal glyph budget.
- ASCII digits receive equal advances inside the existing default-font
  fallback. A loaded mono face, complex shaping, locale typography and
  native raster quality remain separate font/backend evidence.
- The local adapter has no artificial pending spinner, reconnect, byte
  progress, resume or async acknowledgement. Its error/recovery labels do
  not imply any Phase-4 session or server operation.
- Stage C is still BLOCKED by Phase-4/5 prerequisites: no actual session/
  asset-delivery loader/recovery, no DOM Board Reader/action dispatcher,
  and no network acceptance. Full regions remain Phase 9; Caro and Werewolf
  gameplay remain under the gates recorded in Stage A. The existing web
  zoom restriction and native/mobile AT gaps are not resolved by CLI themes
  or presenter descriptions.

The changed Chess/Tiles snapshots are RenderList descriptions, reviewed as
intended command/geometry changes. They provide no renderer-pixel claim.
Real UI, keyboard/theme/interruption/repeat evidence and the pinned aggregate
gate are pending final owner verification at the time of this append.

## Final pinned verification (2026-10-03)

The final implementation was checked in the isolated Mac checkout
`/Users/manhblue/Documents/Codex/2026-10-03/task-2/tabula`, starting from
`1d8fab294931750f45ef5d7b498f34b5b0417188`. The original checkout was untouched.
Rust `1.96` resolved to Cargo/Rust 1.96.1; the pin was retained.

| Executed check | Result and scope | Evidence |
|---|---|---|
| `cargo xtask check` | PASS, all nine ordered gates; workspace tests 877 passed, 0 failed, 21 ignored; 25 dependency rows, 333 source files and 28 manifests checked | `/tmp/tabula-51-gate-1.96.log` |
| `cargo check --workspace --no-default-features` | PASS, compiled native feature matrix | `/tmp/tabula-51-features-none-1.96.log` |
| `cargo check --workspace --all-features` | PASS, compiled native feature matrix | `/tmp/tabula-51-features-all-1.96.log` |
| `cargo nextest run --workspace` | PASS, 875 passed, 2 skipped | `/tmp/tabula-51-nextest-1.96.log` |
| `cargo check -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web` | PASS, compiled, `RUSTFLAGS=-D warnings` | `/tmp/tabula-51-wasm-client-check-1.96.log` |
| `cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release` | PASS, compiled, `RUSTFLAGS=-D warnings` | `/tmp/tabula-51-wasm-client-release-1.96.log` |
| `cargo xtask stage-wasm-game` | PASS, staged only under ignored `target/tabula-web-game`; WASM 841,802 bytes | `/tmp/tabula-51-wasm-stage-1.96.log` |
| `cargo check -p tabula-web --target wasm32-unknown-unknown` | PASS, compiled, `RUSTFLAGS=-D warnings` | `/tmp/tabula-51-wasm-web-check-1.96.log` |
| `cargo build -p tabula-game-client` | PASS, native binary compiled with pinned toolchain | `/tmp/tabula-51-native-final-build.log` |
| Skill drift checker / checker fixtures / AI doc contracts | PASS: two canonical entrypoints, 32 fixture tests, 6 AI contract tests; PyYAML 6.0.3 installed in temporary venv | `/tmp/tabula-51-skills-check.log`, `/tmp/tabula-51-skills-tests.log`, `/tmp/tabula-51-ai-doc-tests.log` |

The first aggregate run failed the I-9 scan because a newly added backend
test label named Chess. It was replaced with the neutral label `Board`,
then the entire gate was rerun in its own order and passed. The failed run
is retained as `/tmp/tabula-51-gate-1.96-before-fix.log`. No scan suppression,
toolchain change or gate bypass was added.

The two skipped nextest targets are deep Chess perft and the explicitly
ignored raster-fixture regeneration tool. Cargo's remaining ignored
doctests are scaffold examples. Ignored targets are NOT_RUN, not passing
tests. Existing normal perft, clock, conformance, projection-security,
replay, presenter, local integration, renderer and shared-button assertions
ran in the nonempty workspace selection. The separate provisional Chess
package run had 147 passing tests and one ignored deep-perft target.

Ten changed RenderList snapshots were deliberately inspected before
replacement (seven Chess, three Tiles); the Chess separator was then
changed from an unsupported default-font em dash to ASCII `/` after an
actual native screenshot showed missing glyphs. Generated duplicate
snapshot files were quarantined outside the checkout. Snapshots still
provide command/geometry assertions, not raster evidence.

Exact additional-check commands, exit codes and elapsed times are retained
in `/tmp/tabula-51-additional-results.json`; tested Rust-source SHA-256
receipts are retained in `/tmp/tabula-51-tested-sources.sha256`. None of
these builds was a deployment or a native/WASM determinism comparison.

### Target-floor residual discovered in final acceptance review

The blanket issue acceptance “no target under 44 dp” is **NOT_MET**.
HUD/recovery/promotion controls meet the floor, but board cells do not:
Chess uses exact visual-cell picking (`BoardLayout::square_at`), producing
40 dp cells at 320×568 and 29 dp cells at 320×320/640×320. Tiles uses
inverse-camera grid picking (`TilesLocal::coord_at`), producing 16 dp cells
at `MIN_ZOOM`. Its nearest-claim feature choice only runs after a tap hits
the placed tile. Neither presenter has expanded screen-space board hit
rectangles. This is source/geometry evidence, not a mobile-device run.

Doc 04 §10.2 requires expanded hits with nearest-neighbor disambiguation;
doc 07 Phase 6 explicitly schedules “expanded hit rects, lifted-piece preview,
thumb-reach action placement”. That phase is still gated. A future permitted
implementation must use View-derived actionable candidates, deterministic
nearest-center/tie resolution, HUD exclusion and tap/drag/keyboard
equivalence tests; merely enlarging a focus rect or raising `MIN_ZOOM`
would not establish the actual input floor. Keep this acceptance item
open alongside Stage C.

## Actual Mac UI evidence and limitations

Computer interaction used the CUA native-app API, with screenshots captured
and inspected in the task transcript. No scripted OS input, deployment or
system appearance change was used.

| Native scenario | Observed evidence | Status / scope |
|---|---|---|
| Chess Light desktop, 900×720 content | Pointer e2→e4 changed board and active clock; keyboard arrows/Enter moved e7→e5; focus and last-action marks remained visible | interaction-tested and screenshot-inspected PASS on provisional 1.94 build |
| Chess compact window, 390×610 content | Bottom clock dock stayed visible, board stayed square; active clock displayed `0:19 LOW` alongside turn label and non-color focus/last-action cues | screenshot-inspected PASS on provisional build |
| Tiles Light, five seats, 900×720 content | Tab selected legal placement, Enter placed tile, Tab changed claim feature, pointer Claim follower advanced Seat0→Seat1 and changed follower count7→6/remaining tiles70→69 | interaction-tested and screenshot-inspected PASS on provisional build |
| Final pinned binary / four themes / final focus and maximum-zoom fixes | Native binary built and Dark process launched; native binding repeatedly hung although inventory/process probes succeeded | BLOCKED for final pixel/interaction certification; not inferred from compiled or headless tests |
| Actual fatal/rejected/recovery UI | Temporary harness preserves final main loop and injects stable error/fatal only in first session; compiled on1.96. Source diff and counters retained in `/tmp/tabula-51-runtime-qa` | NOT_RUN on native UI; compile does not establish recovery interaction |
| Touch/pinch, safe areas, 200% text scaling, native/mobile AT | No real device/AT exercise or end-to-end Board Reader adapter | NOT_RUN / NOT_IMPLEMENTED as mapped in StageA/C |

The provisional screenshots predate the final native mouse-edge
authentication, runtime-overlay routing and maximum-zoom repair. Those
repairs have executed headless/integration regressions and pinned gates;
the provisional captures do not certify their final native wiring. The
unsupported Chess em dash seen in pixels was replaced with ASCII `/` and
its six affected snapshot texts deliberately reviewed.

UI tooling first stalled while opening/binding a QA app, then its transport
closed during an executor outage. After reconnection, no previous launch,
build or publish operation was blindly repeated: retained logs and
processes were inspected. Resetting the CUA REPL restored inventory in
2.34seconds and revealed both owned QA apps, but final app binding remained
unreliable. This external limitation is recorded instead of calling all
themes, focus interruption or native recovery PASS. UI screenshots remain
in the task transcript; the repository contains no fabricated raster
fixtures or fault-injection production flag.
