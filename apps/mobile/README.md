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
The Account entry uses a neutral human silhouette; native account and full catalog
services have visible unavailable states.

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

## Voice and gates

[ADR-0037](../../docs/adr/0037-native-mobile-voice-client.md) retains its bounded native
LiveKit adapters and isolated loopback fixture. Production grants remain unavailable;
credentials/audio stay outside GameHost. Foreground/background, permission and
audio-session handling remain owned by the native host. Actual device/SFU/audio
acceptance is still separate. See [the harness](../../tools/native-voice-harness/README.md)
and [its ledger](../../docs/verification/native-mobile-voice/README.md).

No production mobile auth/network, new voice authority, third-party game loading,
store release or Phase 6/8 exit follows from this change.
