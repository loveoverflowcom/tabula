# Issue #102 — CMP discovery parity evidence

Date: 2026-10-07 (Asia/Ho_Chi_Minh). Baseline:
`origin/develop @ 1147f8f861e0ad59d996a917bf79f0bc670f65b3`.
The [mobile source manifest](mobile-source-manifest.json) pins the originally checked Kotlin,
fixtures, tests and build configuration by SHA-256. ADR-0045 opens public mobile
discovery; ADR-0043 still keeps native gameplay unavailable. Discovery metadata
never establishes a packaged runtime, match authority or game state (I-5/I-9/I-10).

## Develop conflict integration

The execution evidence below belongs to the original discovery snapshot
`04925476fcace9bb1cb7cae03e472f4421f98489`; it is not a rerun on the merged tree.
The integration incorporates `develop @ 1b6b823f3a84b8fcf410dbbf6471f875243668a1`.
Its account/social decision retains ADR-0044; this discovery decision moves to
ADR-0045. Mobile runtime source differs from the original discovery snapshot only
in the corresponding `TabulaApp.kt` documentation comment. The retained source
manifest therefore continues to identify the original tested snapshot.

Local merge-resolution checks:

- **PASS:** `git diff --cached --check`, native-only source/configuration policy,
  all five `test_mobile_native_policy.py` tests, and the three-entrypoint skill
  structural check. No packaged APK/app was inspected.
- **BLOCKED:** `cargo xtask check`, because Cargo/Rust are not installed.
- **BLOCKED:** `./gradlew :shared:testAndroidHostTest :previewApp:test
  :android:assembleDebug`, before configuration, because the pinned Gradle
  distribution download has no network route. JDK 17 and SDK 37 are also absent.
- **NOT_RUN:** iOS/Xcode/device execution and full Rust feature/target builds.
  This Linux environment cannot establish native-device acceptance. GitHub CI
  was neither queried nor awaited for this conflict resolution.

## Executed checks

| Claim / owner and failure mode | Oracle and exercised domain | Result / residual |
|---|---|---|
| Repository contracts remain valid; stale generated facts, dependency drift or game-ID dispatch | Authoritative `cargo xtask check`, including current mobile-catalog generation and Kotlin game-ID scan | **PASS**, 1,268 Rust tests, 0 failed, 18 ignored across 102 test suites; ignored examples are not acceptance. [Log](logs/core-gate.log.gz) |
| Workspace feature consumers compile | `cargo check --workspace --no-default-features` and `cargo check --workspace --all-features` | **PASS**, compiled; no target execution implied |
| Catalog facts, localization, search, exact allowed seats, filters and defaults remain owned by generated registry metadata | Supplemental shared host tests; five `DiscoveryCatalogTest` cases among 68 selected tests | **PASS**, `example-tested`; 68 passed, 0 failed/skipped. [Log](logs/mobile-supplemental.log.gz), [XML](logs/test-results.tar.gz) |
| Actual shell reflow, data states, query recovery, navigation and simulated-host lifetime | Supplemental `:previewApp:test`, five discovery tests plus retained navigation, brand, host and voice tests | **PASS**, 28 passed, 0 failed/skipped; `interaction-tested` and `screenshot-inspected` on desktop CMP. Native gameplay/audio/device execution is not established |
| Browsing cannot mount native gameplay | Production defaults with populated discovery catalog and no packaged host; detail/setup plus disabled launch; explicit simulated-host fixtures separately exercise handoff | **PASS**, runtime creation counts remain zero during production browsing, search, filters and retry; only explicit simulated setup launch creates a host |
| Back preserves discovery input and selected filters | Actual search/category → detail → Back → no results → clear search → reset filters | **PASS**, input, selection and result identity/count assertions; no unsupported resume strip appears without a resume adapter |
| Nested saved shell state remains valid without restoring match authority | Actual saveable registry save, dispose and remount, including Compose's nested route-state maps | **PASS**, setup restores and a live simulated route returns to setup without creating another runtime; OS process death **NOT_RUN** |
| Current common UI compiles for both iOS targets and simulator framework | Final-source `:shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64 :shared:linkDebugFrameworkIosSimulatorArm64` | **PASS**, `compiled`. [Log](logs/ios.log.gz) |
| Current iOS application/framework composition builds | Final-source Xcode simulator build, code signing disabled; native-only package policy executes in the existing build | **PASS**, `compiled`; app not launched. [Log](logs/xcode.log.gz) |
| Required pinned Android host/APK gate | `:shared:testAndroidHostTest :android:assembleDebug`, JDK 17 / SDK 37 pins unchanged | **BLOCKED** locally before source compilation: JDK 17 unavailable; SDK 37 also absent. Supplemental checks below do not replace this gate. [Log](logs/mobile-required.log.gz) |

The final supplemental test invocation explicitly used `--rerun-tasks`: all 21 selected Gradle
tasks executed. Source hashes were checked again after execution and matched the manifest.
Existing lifecycle/voice assertions remain active after adapting entry to Home → detail → setup.
The restoration fixture now recursively admits Compose's saved map/list values; its actual
save/dispose/restore assertions were retained. Native compile/framework and Xcode were rerun
after final UI copy, metadata, detail-state and rules-URL changes.

## Rendered matrix and visual inspection

Actual shared-shell surfaces are 844 dp tall. Generated registry Home, Library, detail and
unavailable setup run at 320/light/en, 390/light/en, 390/dark/vi, 768/light/vi and 768/dark/en.
Explicit fixture catalogs cover zero entries at 320, one long Vietnamese entry at 390 and eight
long English entries at 768. Loading, unavailable and failure are distinct catalog states;
retry invokes a real injected caller and recovers both Library and detail. Synthetic backend
error detail is not displayed as product copy.

The 320/dark/vi and 390/light/en stress cases use 200% text and eight long localized names.
They require actual positive vertical scrolling to the last card, complete ≥44 dp action
bounds, nonempty text-layout callbacks, glyph containment and no ellipsized lines. The retained
navigation test additionally requires whole-word short-label wrapping and exercises the
explicit simulated-host launch/return at 320 dp. These are selected partitions, not a complete
Cartesian product or a mobile screen-reader claim.

The suite produced 89 rendered screenshots; 37 current discovery examples are retained in
[screenshots](screenshots/). Named images were visually inspected for brand identity, serif
heading hierarchy, semantic light/dark surfaces, faithful lightweight covers, labeled bottom
navigation/rail, complete wrapped labels and scroll reachability. At 200%, headings and cards
grow and scroll; fonts are not reduced to fit. No horizontal glyph clipping or displaced
navigation was found within these selected captures.

- [320 light English Home](screenshots/discovery-320-light-en-registry-home.png)
- [390 dark Vietnamese Library](screenshots/discovery-390-dark-vi-registry-library.png)
- [768 light Vietnamese Home](screenshots/discovery-768-light-vi-registry-home.png)
- [768 dark English Library](screenshots/discovery-768-dark-en-registry-library.png)
- [320 empty Library](screenshots/discovery-library-320-count0-long-names.png)
- [390 one-entry Library](screenshots/discovery-library-390-count1-long-names.png)
- [768 eight-entry Library](screenshots/discovery-library-768-count8-long-names.png)
- [No matching games and reset](screenshots/discovery-390-light-en-no-results.png)
- [320 dark Vietnamese, 200% Home](screenshots/discovery-320-dark-vi-font200-home.png)
- [390 light English, 200% last card](screenshots/discovery-390-light-en-font200-library-scrolled.png)
- [390 dark Vietnamese detail](screenshots/discovery-390-dark-vi-registry-detail.png)

## Actual web comparison

The web authority is the current application built with
`NO_COLOR=true CARGO_BUILD_JOBS=2 trunk build --release --cargo-profile wasm-release`
and actually loaded/captured through the in-app browser. [Build log](logs/web-build.log).
The browser requested a 390 × 844 px viewport; measured document/body content width is
375 px because of the 15 px scrollbar. Original full-page JPEG captures remain in
[web](web/). They establish observed public discovery rendering, not full browser acceptance.

The [comparison script](compare.py) preserves each screenshot's pixel scale, crops only the
web's original first viewport for the labelled board, and leaves source captures intact:
[Home](comparison-home-390-light-en.png), [Library](comparison-library-390-light-en.png),
[detail](comparison-detail-390-light-en.png).

Inspection confirms the canonical T Portal identity, shared paper/lavender hierarchy, serif
discovery headings, authored Home/hero copy, principal purple action and registry-owned
Chess/Tiles covers carry the same design language. CMP adapts native account/navigation,
filter expansion and larger text controls; its decorative neutral-card hero uses shared
semantic roles. CMP intentionally omits the web's unavailable Continue strip because no real
mobile resume state/capability exists. Registry mode descriptions are neutral display facts;
the mobile native-unavailable reason appears before those descriptions. No browser-only local
or bot capability is presented as working native gameplay.

## Environment and exact reproduction

macOS 15.6/aarch64, Gradle 9.7.0, Kotlin 2.4.20, Compose 1.12.0, Xcode 16.3.
Installed JVMs are 21/24; Android platforms stop at 36. Pins remain JDK 17 / SDK 37.
The supplemental [init script](logs/local-toolchains.gradle) uses installed JDK 21 / SDK 36
only for shared host/desktop tests; it is evidence, not repository build configuration.

```sh
cd apps/mobile
# Required pinned gate, locally blocked:
ANDROID_HOME=/Users/manhblue/Library/Android/sdk ./gradlew --console=plain \
  :shared:testAndroidHostTest :android:assembleDebug

# Copy the retained init script to /tmp/tabula-102-local-toolchains.gradle first.
ANDROID_HOME=/Users/manhblue/Library/Android/sdk ./gradlew --console=plain \
  --no-configuration-cache -I /tmp/tabula-102-local-toolchains.gradle \
  :shared:testAndroidHostTest :previewApp:test --rerun-tasks

ANDROID_HOME=/Users/manhblue/Library/Android/sdk ./gradlew --console=plain \
  :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64 \
  :shared:linkDebugFrameworkIosSimulatorArm64

xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath /tmp/tabula-102-ios-build CODE_SIGNING_ALLOWED=NO build
```

The preview runner supports `-Ppreview.catalog=registry|zero|one|many|loading|error|unavailable|simulated`
alongside width, height, language, theme and fontScale. The default registry preview displays
real generated metadata; launch authority remains an independent explicit simulated fixture.
Example: `:previewApp:run -Ppreview.width=320 -Ppreview.language=vi -Ppreview.dark=true
-Ppreview.fontScale=2 -Ppreview.catalog=many`.

## Residual scope

Android APK on SDK 37/JDK 17 remains a prerequisite. Real Android/iOS app execution, hardware
insets, touch/system Back, TalkBack/VoiceOver, native text input/IME, process death and gameplay
performance are **NOT_RUN** here. Native game adapters remain blocked under ADR-0043; no web
fallback, canonical state, backend discovery service, mobile account flow, resume history or
phase exit was added. Existing CI config selects pinned mobile gates and uploads XML/screens;
configuration is source-read evidence, not a claim that this PR's CI passed. Merge enforcement
is unknown.
