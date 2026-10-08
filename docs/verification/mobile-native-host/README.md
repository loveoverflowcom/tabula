# Issue #81 native mobile host review and CMP viewport acceptance

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-native-host); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


Date: 2026-10-06 (Asia/Ho_Chi_Minh). Base: `develop@c6d55a6fc3b326e14a466b6e9d988f897bb579e5`.
Scope: [ADR-0043](../../adr/0043-native-mobile-gamehost.md) direction/source spike,
retirement of mobile WebView selection/packaging, and compact shared CMP shell fixes.
**This is PR1, not completed native gameplay or closure of #81.**

The later [2026-10-07 adapter-contract prototype](adapter-prototype.md) has its own
fresh-develop source and local-check scope. The rows below retain this original
2026-10-06 implementation/evidence and do not claim current native device execution.

The later [Android source skeleton](android-skeleton.md) is explicitly skeleton-only,
with typed unavailable operations and an empty runtime inventory. Its fresh-source,
unit/compile, packaging and native-device statuses are separate from both older ledgers.

## Observable change and ownership

Both mobile entrypoints now provide the unavailable default host and an empty launch
catalog. The UI says native gameplay is unavailable; no web fallback is launched.
Retired mobile staging exits 2 without generating a web game package. Native library/
asset packaging awaits the actual adapter. Web build/loader/JS consumers remain.

CMP owns shell layout, navigation and permitted native services. Rust rules,
projection/replay/presenter/renderer are unchanged (I-5/I-6/I-10). The scoped native
voice lifecycle is retained. Source/config and artifact guards prevent specific
mobile web regressions; they are structural checks, not a security sandbox or
native frame evidence. [The checksum-bound source review](upstream-source-review.md)
records the missing iOS attach API and Android runtime teardown constraints.

Design oracle: [compact foundation](../../ui/screens/foundation.md), existing semantic
tokens and the current T Portal brand. This minimal mobile launcher is not Design01
web discovery/catalog. Fixes keep 16 dp gutters and 44 dp minimum action bounds,
wrap voice actions, combine Back/title, and scroll Home/error content. Long toolbar
names intentionally use two-line ellipsis with their full semantic text retained.
The native voice panel is bounded/scrollable to leave space for the game surface.

## Executed checks

| Claim / failure mode | Oracle and selected domain | Evidence / result | Residual |
|---|---|---|---|
| Narrow actions cannot shrink below 44 dp or clip labels | Actual CMP bounds, EN/VI at 320/390 dp, font scale 1/2 | `ShellViewportTest.voiceActionsWrapRatherThanSqueezingTheSecondLabelOnSmallPhones`, interaction-tested PASS | Desktop software render; no device input/IME claim |
| Home/error actions remain reachable | Scroll to/click last action; long failure, 320×640, 390×844, 640×320, text 200% | Two named viewport cases, interaction-tested PASS | Simulated game/catalog only |
| Large voice text leaves a game viewport; scroll preserves runtime | Positive host bounds, one runtime, native-control scroll in 640×320 at 200% | Named landscape case, interaction-tested PASS | Host/media doubles, not a real GPU surface/audio session |
| Unavailable default cannot advertise Play/Ready | Exact default `TabulaApp()` at 320/390; separate dark Home adapter | Two named viewport cases, interaction-tested PASS | Production Android/iOS entrypoints source-read, target app not built here |
| Regression sensitivity | Same four focused viewport tests against baseline source | **FAIL 4/4 before fix; PASS 4/4 after** | Later two unavailable-state cases are policy checks, not baseline regressions |
| Shared shell/host and media seams | Entire preview selection | **PASS 20 tests**, 0 skipped/errors/failures | Simulated game and media; no native host implementation |
| Common policy/parsers/tokens/navigation | Shared desktop target | **PASS 49 tests**, 0 skipped/errors/failures | Historical web GameSession/parser cases are not native lifecycle proof; Android-host-only bridge vectors not run |
| Native-only source/config and payload rejection | Five adversarial fixtures: aliased API, restored staging, renamed WASM, compiled legacy class, missing artifact | Guard **PASS**, 5 tests; production source/config PASS | **0 real APK/app artifacts inspected locally** |
| Core gate | `cargo xtask check` in authoritative order | **PASS**, 1,264 passed / 0 failed / 18 ignored | Ignored cases not counted as executed proof |
| Web target preserved | `node --test apps/game-client/web/tests/*.test.cjs`; selected shared game WASM build | **PASS 122 tests**, 0 skipped; WASM `compiled` PASS | No new web browser pixels/performance measured |
| Retired mobile stage | `cargo xtask stage-mobile-game` | Expected **exit 2**, explicit native-adapter-unavailable message | Not a native artifact builder |
| Optional skill-map checker (no skill sources changed) | `python3 .agents/skills/tabula-engineering/scripts/check_skills.py` | **BLOCKED**, local Python lacks PyYAML | Not part of the portable core gate; no PASS inferred |
| Xcode project syntax | `plutil -lint mobile/ios/TabulaApp.xcodeproj/project.pbxproj` | PASS, syntax only | Does not prove Xcode compile/link |

Counts overlap and are not additive. Raw commands/results and source/screenshot
hashes are preserved under `logs/`, `results/` and `evidence.json`. The core log is
gzip-compressed to retain its exact diagnostic whitespace without adding text-diff errors.

## Toolchain scope and reproduction

Host: macOS 15.6 arm64, Xcode 16.3, Rust 1.96.1, Gradle wrapper 9.7.0,
Kotlin 2.4.20 / CMP 1.12.0. The repository configures JDK **17**; it is absent
locally. Configured Android host-test/APK and preview commands fail before test
selection at that prerequisite. Android platform **37** is also absent; a separate
JDK21 Android retry reached that explicit SDK error. No SDK/JDK/toolchain was installed.

For useful viewport evidence, an isolated `/tmp/tabula-issue81/mobile-jdk21` copy
uses the installed Microsoft JDK **21**, with only the two `jvmToolchain(17)` calls
changed to `jvmToolchain(21)` in shared/previewApp Gradle files. Production source
is unchanged. Final Kotlin/render sources match the repo; the manifest records
the expected Gradle adaptation. This is **JDK21 desktop evidence**, never a PASS
for the configured JDK17 Android gate.

```bash
# Configured prerequisite check (BLOCKED locally):
cd mobile
./gradlew --console=plain --offline :shared:testAndroidHostTest :android:assembleDebug

# In the isolated copy with the documented toolchain adaptation:
JAVA_HOME=/Library/Java/JavaVirtualMachines/microsoft-21.jdk/Contents/Home \
  ./gradlew --console=plain :shared:desktopTest :previewApp:test

# Root checks:
cargo xtask check
node --test apps/game-client/web/tests/*.test.cjs
cargo check -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown
python3 tools/check-mobile-native-policy.py
python3 -m unittest discover -s tools/tests -p test_mobile_native_policy.py -v
plutil -lint mobile/ios/TabulaApp.xcodeproj/project.pbxproj
```

## Rendered images

Images are actual CMP desktop renders. No synthetic game image is represented as
native execution. File names mark fixture catalogs, simulated hosts and media
doubles. The default-app capture shows the current unavailable state; dark-theme
Home is a separate theme-adapter render because the default root follows OS theme.

Selected default 320 light, dark 390, scaled voice and short landscape failure
images were visually inspected for crisp brand, aligned gutters/card shapes,
complete action labels, reachable recovery and absence of image stretching or
border offsets. Other matrix images are captures supported by geometry/interaction
assertions, not a claim that every screenshot received a full design audit.

- [Historical Default app, 320×844](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-native-host/cmp/cmp-native-unavailable-default-app-320x844.png): native gameplay unavailable; no Play or Ready surface.
- [Historical Dark Home theme adapter, 390×844](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-native-host/cmp/cmp-native-unavailable-dark-theme-adapter-390x844.png): same unavailable shell policy and canonical brand.
- [Historical Landscape error recovery, 640×320, text 200%](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/mobile-native-host/cmp/cmp-host-failure-640x320-font2-simulated-host.png): Back remains visible while the long simulated failure scrolls to Retry.

## Issue #81 acceptance still owed

| Acceptance | Status | Reason |
|---|---|---|
| CMP → native Macroquad → play/input/animation → Back on both platforms | **NOT_IMPLEMENTED / BLOCKED** | No native adapter; pinned iOS bootstrap does not attach to an existing CMP app, Android ownership/teardown requires an adapter |
| Unified native direction and retirement guards | **source-read / statically checked PASS** | Docs and active consumers changed; actual target builds and real package checks remain separate |
| Native game library/assets packaging | **NOT_IMPLEMENTED** | Removing a web bundle is not native packaging |
| Native surface/lifecycle/generation/input cancel/preload tests | **NOT_IMPLEMENTED** | Common/preview tests exercise historical model/shared seams only; they do not reach a native render thread/context |
| Actual Android/iOS APK/app builds and payload guard | **BLOCKED locally** | Missing JDK17/Android37 and pinned Kotlin/Native toolchain; no real artifact is claimed |
| iOS klib/Xcode/simulator run | **NOT_RUN** | Required Kotlin/Native 2.4.20 is not installed; cache has 2.2.0. Xcode/simulators exist but cannot execute missing gameplay |
| Real Android/iOS device interaction/rotation/suspend/resume | **NOT_RUN** | No attached real devices; no adapter. Available simulators/AVD were not booted |
| Cold/warm usable frame, p50/p95/p99 frame times, dropped frames, touch latency, CPU/RAM/background | **NOT_IMPLEMENTED / NOT_RUN** | No native workload/harness or device-specific baseline/budget yet; no FPS promise |
| CI execution / merge enforcement | **Not established at local capture** | CI is configured to run JDK17 target builds and post-build artifact checks; source YAML alone is not a run or required-status proof |

Keep the PR draft and use **Refs #81**, not a closing keyword. The
[next adapter/device queue](../../work-plan/backlog/mobile-game-host-device-evidence.md)
requires a bounded reviewed upstream patch/API, target-specific adapters/native
packaging and real-device evidence. Production network/auth/voice authority,
store release and broad phase exits remain closed.
