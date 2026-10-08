# Tabula mobile

One CMP app tree for Android/iOS. [ADR-0043](../../docs/adr/0043-native-mobile-gamehost.md)
requires the existing Rust/Macroquad gameplay runtime to run natively in the same
application. It supersedes the gameplay WebView/WKWebView choice and packaging of
ADR-0032/0033. Web gameplay keeps its Rust/WASM document and verified loader.

**Current build: shell only, gameplay unavailable.** Both production entrypoints
use the unavailable default host and an empty packaged-game list. The independent
public discovery catalog is generated from `tabula-registry` under
[ADR-0045](../../docs/adr/0045-mobile-discovery-parity.md). No native game
adapter/library/assets pipeline has been delivered; no hidden web fallback is
selected. The [source spike and evidence ledger](../../docs/verification/mobile-native-host/README.md)
record the pinned Miniquad embedding blockers and actual check results.

The [adapter-contract prototype](../../docs/verification/mobile-native-host/adapter-prototype.md)
now provides a typed native port, pure lifecycle coordinator, joined-worker admission
gate and Android SurfaceView callback binding. Its controlled test port is not a
Macroquad backend. Production gameplay remains unavailable; actual Android/iOS
embedding, native libraries/assets and target/device checks are still required.

```text
apps/mobile/shared/      CMP UI/navigation, generated tokens, GameHost seam, native voice policy
apps/mobile/android/     Android Activity over :shared
apps/mobile/ios/         SwiftUI/Xcode host of the same static CMP framework
apps/mobile/previewApp/  Desktop shell layout/lifecycle tests with a labelled simulated game
```

## Ownership and boundaries

CMP owns shell UI/navigation and permitted device services. Rust continues to own
rules, projection, replay, presenter, RenderList and the Macroquad render loop.
The future native host owns surface/context/thread lifetime and forwards input;
it never calculates rules, receives canonical hidden state or transports drawing
commands to Kotlin/Swift each frame. GameHost carries launch/preferences and
permitted capabilities in, Ready/Failed/Exited out, and a Back port.

Recomposition and resize cannot create another runtime. The next adapter must
separate surface and match lifetime, fence stale callbacks, stop/join before
reopen, handle cancellation/context loss, and preserve rules-owned clock semantics
on suspend. These requirements are not proven by the historical simulated-page
GameSession tests. Native Android/iOS execution and performance remain blocked.

## Build and checks

The configured mobile build requires JDK 17 and Android SDK platform 37. Xcode
16.3+/Swift 6.1 is needed for the exact LiveKit Swift dependency; native iOS linking
requires macOS and the pinned Kotlin/Native toolchain.

```bash
cd apps/mobile
./gradlew :shared:testAndroidHostTest :previewApp:test :android:assembleDebug
./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64
./gradlew :previewApp:run -Ppreview.width=320 -Ppreview.height=720
# On macOS, the Xcode project builds the shared framework itself:
xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug \
  -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build
```

The retired `just mobile-game` / `cargo xtask stage-mobile-game` commands fail
explicitly. Mobile builds do not build/copy HTML/JavaScript/WASM gameplay bundles.
Native artifact/assets packaging is a required follow-up after the adapter spike,
not a rename of the old web-bundle task. CI is configured to inspect source/configuration and built
APK/app contents against the native-only policy; this establishes no native frame.

Desktop CMP tests render actual shared shell pixels and exercise the host seam
with a simulated page. They can inspect 320/390 dp layout, wrapping, scrolling,
recomposition and Back routing. They cannot measure Android/iOS touch latency,
GPU frames, memory, surface loss or process death. A build or simulator result
does not replace issue #81's real-device acceptance.

For semantic inspection, interaction and live Kotlin edit/reload assertions with
the existing Desktop Preview, see the [agentic coding guide](AGENTIC-CODING.md)
and [mobile agent instructions](AGENTS.md). They use the official JetBrains
Compose Hot Reload MCP and keep live MCP evidence separate from deterministic
Desktop Compose tests and Android/iOS native acceptance.

## Design

`tokens.toml` is the single authored source. `cargo xtask gen-tokens` generates
`shared/.../design/TabulaTokens.kt`; do not edit it. `TabulaTheme` maps these roles
to Compose. The adaptive shell follows
[the compact foundation](../../docs/ui/screens/foundation.md), including 16 dp gutters,
44 dp minimum targets, wrapping actions and readable error recovery. The public
catalog uses the same registry metadata/translations as web; remote discovery
remains unavailable. The shell top bar uses the canonical T Portal
identity from the generated brand paths, with one accessible Tabula heading.
Android/iOS launcher assets remain exports of the approved brand artwork. No decorative oversized border or second palette is introduced.

## Shell navigation and copy

Issue #101 adds the [adaptive shell foundation](../../docs/ui/screens/mobile-shell.md):
Home, Library, detail/setup scaffolding, Account and the existing `GameHost` seam.
Issue #102 adds registry-backed discovery, metadata filters and detail/setup review.
Production entrypoints show the generated public catalog and keep an empty runtime
inventory with unavailable gameplay under ADR-0043;
the desktop preview supplies explicit catalog and game doubles for navigation tests.
Home/Library/Account use labeled bottom navigation on phones and a rail from 600 dp.
Content scrolls within the remaining space, with safe insets owned by the outer shell.
Compact page/card insets are 16 dp. Nested shell Back uses a localized accessible
icon target, and rail width follows measured navigation labels at the OS font
scale. Account text uses compact generated roles and stacks identity details
when a row would leave too little reading width; 200% text remains supported.
The Account entry uses a neutral human silhouette; native account and full catalog
services have visible unavailable states.

Issue #103 adds the [bounded account task slice](../../docs/ui/screens/mobile-account.md):
Account, Sign in, Create account, read-only self Profile and Friends. The account port has explicit
unknown/loading/signed-out/authenticated/expired/unavailable/error outcomes. Its production
default remains unavailable; isolated web provider/enrollment/social capabilities do not enable
native auth. No passwords, sign-in codes, local accounts, profile edits or fabricated friend data
are collected. A supplied current read-only identity may show only its returned fields, and an
already-loaded managed avatar must match that exact identity snapshot. Background/expiry,
cancelled requests and unconfirmed sign-out mask private facts; toolbar/system Back first dismiss
a local sign-out confirmation. Account facts and operations are never saved with shell routes.
See the [issue #103 evidence ledger](../../docs/verification/issue-103-mobile-account/README.md)
for executed focused checks and unexecuted Gradle/Compose/device acceptance.

`navigation/BackStack.kt` owns bounded public routes and saved-state restoration.
Nested navigation returns to its caller; the simulated game receives Back first.
An interrupted local preview restores to setup and requires a fresh explicit launch.
OS deep links, credentials and native match restoration remain gated.

`localization/ShellStrings.kt` owns exhaustive vi/en shell keys, regional locale
normalization and English fallback. Fixture game names stay supplied display data.
Reusable components in `shell/ShellComponents.kt` consume generated semantic tokens
for surfaces, emphasis, state layers, focus and minimum targets.

`:previewApp:test` covers navigation, unavailable services, simulated local return,
phone/tablet layouts, themes/locales, large text, long names, voice-control wrapping
and error recovery. [Issue #101's ledger](../../docs/verification/issue-101-mobile-shell/README.md)
retains its historical checks separately from native device acceptance. Inspect
this shell with `:previewApp:run -Ppreview.width=320 -Ppreview.language=vi
-Ppreview.dark=true -Ppreview.fontScale=2` (simulated game page).
`ResponsiveShellTest` also exercises synthetic long Account/Profile data at
320×844, 390×844 and 844×390, across all four schemes, vi/en and 100%/200% text.

The [2026-10-07 prototype adaptation](../../docs/research/main-recovery-20261007/README.md)
adds a packaged decorative Home image, directly accessible registry categories, and Account
menu links to Rooms, History and Settings. Rooms/History expose native-adapter unavailability;
they supply no sample matches, rooms, replay or ratings. Settings changes local appearance,
language and reduced motion, restored only through bounded public shell saved state. Resolved
preferences reach a supplied GameHost on fresh launch. Rust rules and the native gameplay
availability boundary remain owned by ADR-0043.
Its captures distinguish font scale, viewport and scroll position; desktop pixels
establish shared layout rather than native device rendering.

Account preview data is explicitly synthetic and restricted to `previewApp`. Run
`:previewApp:run -Ppreview.account=authenticated -Ppreview.accountLongFields=true
-Ppreview.accountAvatar=managed -Ppreview.width=320 -Ppreview.language=vi -Ppreview.fontScale=2`
and select Account. Other account cases are `unknown`, `signed-out`, `loading`, `expired`,
`unavailable` (default) and `error`; `accountAvatar=neutral` is the default. A preview uses the real
presentation coordinator over a labelled test adapter, never a provider or native session.

For focused JVM account/fixture/copy/Back tests without Gradle, use
`tools/test-mobile-account-contract.sh` from the repository root, setting `KOTLIN_HOME`,
`COROUTINES_JAR`, `JUNIT_JAR` and `HAMCREST_JAR` to official local tooling. Record their versions.
This check does not compile Compose, run Android/iOS, or replace the configured mobile gate.

## Voice and gates

[ADR-0037](../../docs/adr/0037-native-mobile-voice-client.md) retains its bounded native
LiveKit adapters and isolated loopback fixture. Production grants remain unavailable;
credentials/audio stay outside GameHost. Foreground/background, permission and
audio-session handling remain owned by the native host. Actual device/SFU/audio
acceptance is still separate. See [the harness](../../tools/native-voice-harness/README.md)
and [its ledger](../../docs/verification/native-mobile-voice/README.md).

No production mobile auth/network, new voice authority, third-party game loading,
store release or Phase 6/8 exit follows from this change.
