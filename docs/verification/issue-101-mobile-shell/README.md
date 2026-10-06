# Issue #101 — CMP shell foundation evidence

**Historical source evidence.** The ledger below records PR #105 at
`2db7602e36a03aa93d8c182f1082bd698a2cf9b3`, before composition with ADR-0043's
mobile WebView retirement. Production now has an empty catalog and unavailable
gameplay; desktop tests retain an explicit simulated host. The retained staging/Xcode
logs describe the old web-bundle composition and establish no native gameplay claim.
That PR's Linux CI later found a 320 dp/200% Vietnamese Account-label word break;
the integration reduces navigation label insets without changing fonts or its
word-boundary assertion. Current-source check results belong to the integration
run, not to the historical PASS rows below.

Date: 2026-10-06. Checks ran on the contribution based on remote
`origin/develop @ a5feb5d032eddb3cd9946eb548426857dc1d797a` (mobile relocation #97).
The final PR was rebased onto `origin/develop @ 8c5e3e8d9df133e14d30b0c61b34c3fae97332a8`
(documentation-only review workflow #104). That rebase changed no mobile source, Rust source,
generated tokens or build configuration; the checked implementation is identical.
Scope: the shell/navigation changes in this PR under ADR-0032/0033.
[Contract and ownership](../../ui/screens/mobile-shell.md). No Rust rules, projection, protocol,
game-host bridge, native-media adapter or backend behavior changed (I-5/I-9/I-10).

## Executed ledger

| Claim / invariant | Owner and failure mode | Oracle / check | Status and residual |
|---|---|---|---|
| Portable core and token authority remain valid | Repository gate; stale adapters/raw colors/dependency drift | `cargo xtask check` | **PASS**: all gates; 1,270 Rust tests passed, 18 ignored, 0 failed. Ignored cases are not acceptance evidence. [Log](logs/core-gate.log.gz) |
| Bounded public routes, caller return, no automatic local-match restoration | `BackStack`/Compose saved state; persisted launch authority or lost caller | 12 navigation tests and the Compose save/dispose/remount test | **PASS**, `example-tested`/`interaction-tested`; actual OS process death **NOT_RUN** |
| vi/en copy and locale fallback | `ShellStrings`; missing/incorrect key or regional lookup | 3 locale tests, exhaustive switches, full-shell vi/en interactions | **PASS**, `example-tested`/`compiled`; game names remain manifest data |
| Shared units and host lifecycle remain valid | Session/codec/navigation/controller; runtime leaks or changed Back/retry | Supplemental `:shared:testAndroidHostTest` | **PASS**: 63 tests, 0 skipped; JDK 21/SDK 36 override, not the pinned Android build |
| Adaptive chrome, selected destinations and accessible labels | Shell components; hidden navigation, overflow or unreachability | Supplemental `:previewApp:test` | **PASS**: 17 tests, 0 skipped; simulated game/voice, not device execution. [Log](logs/mobile-tests.log.gz), [raw XML](logs/test-results.tar.gz) |
| Local gameplay Back/retry/lifecycle remains intact | Existing `GameHost`/voice composition; duplicate mount, missed disposal | 10 lifecycle and 3 voice UI tests within the 17 | **PASS**, `interaction-tested` with doubles; real WebView/native audio **NOT_RUN** |
| iOS Kotlin and static framework still build | Shared target adapters; unavailable common/native API | `:shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64 :shared:linkDebugFrameworkIosSimulatorArm64` | **PASS**, `compiled`. [Log](logs/ios.log.gz) |
| iOS application compiles and packages the existing game | Xcode host/export boundary | `xcodebuild -project TabulaApp.xcodeproj -scheme Tabula -destination 'generic/platform=iOS Simulator' -derivedDataPath /tmp/tabula-101-ios-build CODE_SIGNING_ALLOWED=NO build` | **PASS**, `compiled`; app was not launched. [Log](logs/xcode.log.gz) |
| Pinned Android unit/APK gate | Host toolchain, JDK 17 and SDK 37 | `./gradlew --console=plain :shared:testAndroidHostTest :android:assembleDebug` | **BLOCKED** locally before source compilation: no JDK 17. SDK 37 is also absent. [Log](logs/mobile-required.log.gz) |
| APK with installed platform 36 | Android dependency metadata; incompatible compile SDK | Supplemental full command including `:android:assembleDebug` | **BLOCKED**: Compose 1.12 requires SDK 37; no metadata check was bypassed. [Log](logs/mobile-apk-sdk36.log.gz) |

The final core gate includes fmt, clippy with all features and warnings denied, workspace tests,
dependency/game-id/manifest policy, generated adapter freshness, raw-color checks and cargo-deny.
Existing cargo-deny duplicate/unused-wrapper warnings remain warnings; its four categories passed.

## Preview domain and visual review

All test surfaces are 844 dp tall. Full Home → Library → detail → setup → simulated game →
confirmed return → nested Back → Account → Home runs at 320/light/en, 390/dark/vi,
768/light/vi and 768/dark/en. This covers the named dimensions, not their full Cartesian product.
Identity also runs at 320/390 in all four authored schemes. Selected/unselected controls expose
selected semantics and at least 44 × 44 dp measured targets; account uses a neutral silhouette.

The 320/dark/vi stress case uses 200% text and eight long synthetic game names. It requires
positive vertical scrolling, reaches the final entry's action and launches/returns from setup.
Unclipped text-node positions and rendered line bounds must fit, text callbacks must execute,
and no line may be ellipsized. Short navigation/Back labels must break at whitespace. The initial
mid-word splits at 200% failed visual review and were fixed with generated compact control insets.
The text oracle uses glyph bounds because Compose's offered paragraph width can exceed its
final shrink-to-text view width without any glyph overflow.

Saved-state UI evidence uses the actual `LocalSaveableStateRegistry`: save, dispose the app,
create a registry from its payload and recompose. Setup restores its route; an active local
runtime is disposed and restoration returns to setup without constructing a second runtime.
The bundled desktop `StateRestorationTester` throws `NotImplementedError`; it was replaced,
not ignored or reported passing. This still does not simulate an OS process or restore a match.

The suite captured 38 images; retained review examples below were inspected for hierarchy,
surface/brand consistency, label wrapping and containment. Other images remain reproducible
under `apps/mobile/previewApp/build/reports/shell-screenshots` and are uploaded by mobile CI.

- [320 light English Home](screenshots/parity-320-light-en-home.png)
- [390 dark Vietnamese Home](screenshots/parity-390-dark-vi-home.png)
- [768 light Vietnamese Home](screenshots/parity-768-light-vi-home.png)
- [768 dark English Library](screenshots/parity-768-dark-en-games.png)
- [390 dark Vietnamese Account](screenshots/parity-390-dark-vi-account.png)
- [320 dark Vietnamese, 200% Home](screenshots/parity-320-dark-vi-font200-home.png)
- [320 dark Vietnamese, 200% setup after scrolling](screenshots/parity-320-dark-vi-font200-setup.png)
- [320 high-contrast light](screenshots/brand-home-hclight-320.png)
- [390 high-contrast dark](screenshots/brand-home-hcdark-390.png)

## Environment and reproduction

macOS 15.6/aarch64, Rust 1.96.1, Gradle 9.7.0, Kotlin 2.4.20, Compose 1.12.0, Xcode 16.3.
Installed JVMs are 21/24 and Android platforms stop at 36. Repository pins remain JDK 17 and
SDK 37. The supplemental check used the installed JVM/platform via an uncommitted init script;
its [exact script](logs/local-toolchains.gradle) is retained as evidence, not build configuration.

```sh
# From apps/mobile; the Android SDK path is host-specific.
ANDROID_HOME=/Users/manhblue/Library/Android/sdk ./gradlew --console=plain \
  --no-configuration-cache -I /tmp/tabula-101-local-toolchains.gradle \
  :shared:testAndroidHostTest :previewApp:test
```

Before the Xcode build, the unchanged game was built with
`cargo build -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown --profile wasm-release`
and staged using `cargo xtask stage-mobile-game` ([build log](logs/game-build.log.gz),
[staging log](logs/stage-game.log.gz)). This is packaging/compilation evidence only.
Retained logs normalize the absolute worktree root to `<worktree>`; results are otherwise preserved.

Existing `.github/workflows/ci.yml` runs the pinned Android test/APK and desktop preview commands,
uploads screenshots/XML, and has a macOS application build. Workflow configuration is
source-read evidence; this local ledger does not claim a PR CI execution or merge enforcement.
Check the PR's current statuses. Required branch rules are unknown.

## Residual scope

Android APK on SDK 37 remains a CI/build prerequisite. Android WebView/iOS WKWebView execution,
device/system Back gestures, hardware safe areas, TalkBack/VoiceOver, process death, real audio,
latency and store acceptance are **NOT_RUN**. Safe-area APIs and Back handlers are source-read;
toolbar Back and Compose saved-state recreation are interaction-tested. Native catalog/account
adapters and OS deep-link registration remain explicit unavailable/future surfaces. No phase
exit or gameplay-host acceptance follows from these shared-shell renders or compiled artifacts.
