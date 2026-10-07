# Mobile `GameHost` evidence ledger (ADR-0033)

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


> Historical ADR-0033 WebView evidence. [ADR-0043](../../adr/0043-native-mobile-gamehost.md) retires this mobile gameplay direction and packaging; the native-only source spike/current checks are in [the new ledger](../mobile-native-host/README.md). None of the results below establish native Android/iOS gameplay.


Date: 2026-10-04. Baseline: `develop @ a871e4a` (PR #64, the CMP foundation, merged).
Scope: a WebView `GameHost` for the **one first-party packaged game** (the ADR-0030 local document), a
typed bridge for lifecycle, launch preferences and `keep-awake`, and the shared `GameSession`. It is an
embedded first-party game, **not** a plugin system: no marketplace, third-party sandbox, remote update,
networked play, voice or further host service.

## Read this first: what was and was not run

The owner decided not to set up an Android emulator for this change and asked for acceptance on the
Compose Multiplatform preview and desktop target (testing only). There is no macOS here. So:

| Target | Status | What stands in for it |
|---|---|---|
| Android WebView (emulator/device) | **NOT_RUN** | APK assembles with the game packaged (`compiled`); `source-read` only for `AndroidGameRuntime` |
| iOS WKWebView / Xcode / simulator | **NOT_RUN** (no macOS) | Kotlin/Native klib compile for `iosArm64` and `iosSimulatorArm64` (`compiled`); the Xcode project edit is **unvalidated** |
| Real device: touch latency, frame pacing, memory, context loss, process death, soft keyboard, orientation, system back gesture | **NOT_RUN** | none |
| Shared Kotlin shell, `GameSession`, codec, path policy, recomposition rules | executed | desktop JVM unit tests and Compose Desktop UI tests |
| The real game document under the host's CSP and bridge protocol | executed in **desktop Chrome 153**, phone-sized viewport | a JavaScript stand-in for the native port; this is **not** a WebView |

ADR-0032's embedding evidence requirement (input latency on touch, frame pacing on a mid-tier device, WASM
instantiate cost and memory on a device, WebGL context-loss recovery, suspend/resume and process death,
safe areas/orientation/keyboard, back gesture — for the shipping Android WebView and iOS WKWebView) is
therefore **still owed**. Nothing below may be read as that evidence, and the result must not be described
as a finished dynamic plugin.

## Environment

Linux x86_64, Temurin JDK 17.0.20, Gradle wrapper, Kotlin 2.4.20, Compose Multiplatform 1.12.0, AGP 9.3.1,
`androidx.webkit` 1.16.0, Rust 1.96, Node v24.21.0, Google Chrome 153.0.8010.36 (headless, SwiftShader
software GL, phone viewport 412×800 CSS px at DPR 2 and 860×412 landscape). No emulator, no device, no Xcode.

## Executed checks

| Claim | Evidence kind | Command | Result |
|---|---|---|---|
| Authoritative gate: fmt, clippy `-D warnings`, `cargo test --workspace`, check-deps, check-no-game-ids, check-manifests, tokens current, check-no-raw-colors, cargo deny | `statically-checked`, `example-tested` | `cargo xtask check` (`just` is the wrapper) | see [Final gate](#final-gate) |
| Staging: registry-derived games list, pruned document, SRI-pinned bridge, atomic failure | `integration-tested` | `cargo test -p xtask mobile_stage` | PASS, 6 tests (107 xtask tests overall PASS) |
| Page-side bridge, host-mode bootstrap, loader, launch options (existing 58 tests unchanged) | `example-tested` | `node --test apps/game-client/web/tests/*.test.cjs` | PASS, **79** tests (was 58), 0 skipped |
| Shared wire grammar, same bytes on both sides | `differentially-tested` | the vectors in `apps/game-client/web/tests/bridge-vectors.json` run by `host-bridge.test.cjs` and Kotlin `BridgeVectorsTest` | PASS (`host-bridge.test.cjs`: 10 tests, 2 of them run the vectors; Kotlin `BridgeVectorsTest`: 3) |
| Kotlin: strict JSON, codec, `GameSession` (16), manifest, `BundlePaths`/CSP | `example-tested` | `./gradlew --rerun-tasks :shared:testAndroidHostTest` | PASS, 36 tests (7 pre-existing), 0 skipped |
| Shell on desktop: open/close/reopen, Back routing, suspend/resume, host and page failure, Try again, late events, recomposition | `interaction-tested` (simulated page) | `./gradlew --rerun-tasks :previewApp:test` | PASS, 10 tests, 0 skipped |
| Desktop window opens at the requested size | `interaction-tested` | `./gradlew :previewApp:run -Ppreview.smokeWindow=true` | PASS: `content=390x844 (requested 390x844)` |
| APK assembles with the bundle; build fails without it | `compiled` | `./gradlew :android:assembleDebug -Ptabula.requireGameBundle=true`, and the same with the bundle moved away | PASS / correct FAIL; 18 bundle files, 1,792,311 bytes in the APK |
| iOS Kotlin compiles | `compiled` (not executed) | `./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` | PASS |
| The **real** staged game, served by the host's path/CSP policy, in a phone-sized Chrome | `interaction-tested`, `screenshot-inspected` | `cargo xtask stage-mobile-game && node tools/mobile-host-check/run.mjs` | PASS 20/20, three consecutive runs ([Historical receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/desktop-receipt.json), [Historical runs](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/runs.json)) |

The desktop-Chrome receipt asserts, against the real WASM game: hello→init→ready with generation 1 and a
granted `keep-awake`; host preferences applied (dark theme, English); no CSP violation, console error or
exception; every request served by the bundle policy; the WASM fetched through the verified loader; a real
**e2→e4 move through pointer input** changing and then settling the board; suspend stopping the frame loop and
resume restarting it (repeated messages idempotent); landscape resize keeping the match and rendering;
Back opening, dismissing and reopening the page's leave confirmation; one `exit` and no navigation; a retired
document ignoring `resume`/`back`; reopen producing generation 2 and a fresh board; `dispose` silencing the
runtime; a missing WASM and a WASM with a flipped byte (SHA-256 mismatch) each showing the page's recovery
overlay and reporting `failed`; a silent host failing closed after 5 s with no game fetch; and Try again
re-handshaking and starting.

Screenshots (actual renders, inspected): [Historical portrait ready](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/01-ready-portrait.png),
[Historical after e2-e4](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/02-after-e2-e4.png), [Historical landscape](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/03-landscape.png),
[Historical leave confirmation](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/04-leave-dialog.png), [Historical fresh reopen](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/05-reopened-fresh.png),
[Historical missing WASM](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/desktop/screenshots/06-failure-missing-wasm.png); shell with the **simulated** page:
[Historical Home](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/cmp-desktop/01-home.png), [Historical ready](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/cmp-desktop/02-game-ready.png),
[Historical Back → leave confirmation](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/cmp-desktop/03-back-leave-confirmation.png), [Historical host failure](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-game-host/cmp-desktop/04-host-failure-panel.png).

### Mutation sensitivity

The new tests were each broken on purpose and had to fail: stale generation accepted, replayed request
accepted, ungranted service honoured, duplicate JSON keys allowed, closed session accepting input, Back always
consumed (Kotlin); `remember` keyed on the event lambda, stale event lambda captured, `dispose` not called,
lifecycle observer leaked, Back bypassing the host (desktop UI); suspend ignored, resume double-scheduling,
stale generation accepted, grant ignored, preferences ignored, disposed port still sending (JS). Four
survivors were found and fixed: the JS "port still sends after dispose" (a bridge unit test added), the
recomposition bug (the first test could not see it: a direct host-contract test with changing parameters
was added), a redundant `key(attempt)` (removed) and the leaked lifecycle observer (observer count is now
asserted). Mutation testing is manual here, not a tool run.

## Basic measurements (desktop Chrome only; not device numbers)

| Quantity | Value | Method and limit |
|---|---|---|
| Game WASM | 1,128,942 bytes | staged artifact |
| Packaged game document | 18 files, 1.8 MB (was 7.3 MB before pruning to what the document references) | `stage-mobile-game` |
| Debug APK | 10,260,897 bytes (baseline PR #64: 9,186,907) | includes `androidx.webkit` and the bundle |
| Navigation → board ready, cold (empty CacheStorage) | 1426 / 840 / 887 ms | page `bootMs` over 3 runs, SwiftShader, loopback |
| Same, warm (verified bytes reused from CacheStorage) | 439 / 729 / 829 ms | reopen in the same profile |
| Frame cadence | 19 / 17 / 17 frames/s | rAF count over 1 s under **software** GL: a harness smoke, not frame pacing |
| Bridge traffic during play | 0 messages per frame | structural: only `hello`, `ready`, `service`, and user-driven lifecycle messages exist |

Three runs on one laptop show variation (840–1426 ms cold); no ranking, percentile or device claim is made.

## Residuals (all unproven)

- **Android WebView:** the injected-port origin restriction, request interception under a real WebView,
  `onRenderProcessGone`, `configChanges` keeping the match through rotation, `WebView.onPause/onResume`
  interaction with the page's own pause, back gesture, keep-awake effect, memory and GPU behaviour.
- **iOS:** that a `tabula-game://` document is a secure context with `crypto.subtle` and streamed
  `fetch` bodies (the loader fails closed if not; loopback fallback recorded in ADR-0033), the
  `WKURLSchemeHandler` behaviour, `WKScriptMessageHandler` origin check, lifecycle mapping, and that the
  hand-edited `project.pbxproj` "Package game bundle" phase is valid in Xcode.
- **Both:** touch input latency vs the web document, frame pacing on a mid-tier device, WASM
  instantiate/memory cost, WebGL context-loss recovery inside a WebView, process death, soft keyboard,
  safe areas, accessibility with TalkBack/VoiceOver, landscape layout (the existing page overlaps its
  action buttons with the side panel at 860×412).
- The shell's `BackHandler` uses an API deprecated in favour of `NavigationEventHandler` in Compose 1.12.
- The desktop UI tests run in CI's `mobile` job; `tools/mobile-host-check` is a local command (CI execution:
  NOT_IMPLEMENTED).

The [follow-up queue](../../work-plan/backlog/mobile-game-host-device-evidence.md) lists the narrow next checks.

## Final gate

On the final tree, which includes `origin/develop @ 8ddb640` merged in (that commit itself fails `cargo fmt --check`
on one signature in `apps/game-client/tests/local_match.rs`; a separate mechanical rustfmt commit fixes it):

| Check | Result |
|---|---|
| `cargo xtask check` (fmt, clippy `-D warnings`, `cargo test --workspace`, check-deps, check-no-game-ids, check-manifests, tokens current, check-no-raw-colors, cargo deny) | **PASS**: "all gates passed"; `cargo test` summed 1004 passed, 0 failed, 21 ignored |
| `cargo check --workspace --no-default-features` | PASS |
| `cargo check -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web`; `cargo check -p tabula-web --target wasm32-unknown-unknown` | PASS |
| `check-loading-budgets.py` (emitted loading budgets, selected game only, no DOM runtime) | PASS (a byte/dependency budget, not runtime performance) |
| `cargo xtask stage-wasm-game` (standalone bundle, now with the bridge file) | PASS |
| Native and web regression: Chess/Tiles rules, presentation and game-client tests | PASS inside the workspace run; **no game crate, rule, projection or renderer source changed** |
| `node tools/tests/check-local-handoff.cjs` | **FAILS identically on unmodified `develop @ a871e4a`** ("exporter must execute both locales and every clock corner"); it is not wired in CI and its prerequisite (a built shell) is absent here, so it is neither passed nor attributable to this change |

Not part of the gate and **not run**: `cargo check --workspace --all-features` (CI's `features` job) and
`cargo nextest` (the gate runs `cargo test`; CI's `test` job uses nextest), the Xcode build, any emulator.
CI (`.github/workflows/ci.yml`): the PR's run [37210473249](https://github.com/loveoverflowcom/tabula/actions/runs/37210473249)
on `af315c5` finished **12/12 jobs success**, including `mobile` (stages the real bundle, `:shared:testAndroidHostTest`,
`:previewApp:test`, `:android:assembleDebug -Ptabula.requireGameBundle=true`, iOS klib compile) and `wasm`
(web tests incl. the 79). That is CI execution of compile/unit/desktop-UI checks only: CI never runs an Android
WebView, an emulator or Xcode, and does not run `tools/mobile-host-check`. Merge enforcement (required status
checks) is unknown (not inspectable here).
