# Tabula mobile

> **First-party embedding, not a plugin system.** [ADR-0032](../../docs/adr/0032-compose-multiplatform-mobile-host.md)
> opened the Compose Multiplatform foundation; [ADR-0033](../../docs/adr/0033-webview-gamehost-first-party-embedding.md)
> adds a WebView `GameHost` for the first-party games packaged with the app.
> [ADR-0037](../../docs/adr/0037-native-mobile-voice-client.md) adds a bounded native voice client and
> isolated loopback development harness; production voice remains unavailable. It does not open
> Phase 6/8, production networking/accounts, store builds, third-party games or remote updates. **The Android WebView and the iOS
> WKWebView have not been run** — see the [evidence ledger](../../docs/verification/mobile-game-host/README.md).

One Gradle root for the one mobile app. There is no second mobile tree.

```text
apps/mobile/
├── shared/       Compose Multiplatform library: screens, navigation, theme, GameHost, bridge, session
├── android/      Android application module (a thin Activity over :shared) + the packaged game assets
├── ios/          Xcode project (a thin SwiftUI container over the :shared framework) + a bundle phase
└── previewApp/   Desktop preview and UI tests of the shared shell (testing only; simulated game page)
```

## Who owns what

| Owner | Responsibility |
|---|---|
| Compose Multiplatform (`shared/`) | App screens, navigation, theming from generated tokens, the shared `GameSession` lifecycle, the bridge codec |
| `GameHost` (`WebViewGameHost`, `WKWebViewGameHost`) | A platform WebView presenting the packaged Rust/WASM game document and executing `GameSession` effects |
| Mobile host (Kotlin/Swift) | Keep-awake plus the distinct native `VoiceClient` adapters/permission/audio lifecycle; no push or production voice grant source |
| The game document (Rust/WASM + `host-bridge.js`) | Rules, `project`, presentation, `RenderList`, renderer, its own loading/error/leave UI |

Kotlin and Swift carry **no game logic**: no legality, turn order, projection, hashing or replay. They
never branch on a game id; the launch query comes from the Rust registry through `tabula-games.json`.

## Playing the packaged game

```bash
just mobile-game            # builds the wasm game and runs `cargo xtask stage-mobile-game`
cd apps/mobile && ./gradlew :android:assembleDebug -Ptabula.requireGameBundle=true
```

`cargo xtask stage-mobile-game` writes `target/tabula-mobile-game/` (the integrated `/play/local/`
document pruned to what it references, plus `tabula-games.json`). Gradle packages it as assets under
`tabula-game/`; the Xcode "Package game bundle" phase copies it into the app. Without the staged bundle
the app still builds and its Home screen says no game is packaged; `-Ptabula.requireGameBundle=true`
(CI) turns that into a build failure.

How the document is served, what the bridge carries and what lifecycle rules apply are in ADR-0033.
The short version: the document is served from the app bundle on a virtual origin by request
interception (no `file://`, no network, no permissions), the game's own loader still checks origin, SHA-256
and size limits, and the only things crossing the bridge are lifecycle events, launch preferences and one
capability-gated service request.

## Design tokens

`tokens.toml` is the only authored source. `cargo xtask gen-tokens` also writes
`shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/design/TabulaTokens.kt`; never edit it.
`design/TabulaTheme.kt` only maps those values onto Compose. A missing role is a `tokens.toml`
change, not a literal in Kotlin. `cargo xtask check-no-raw-colors` rejects `Color(...)`,
palette colours and hex literals under `apps/mobile/` (the generated file is the one exemption), and
`cargo xtask check` fails when the Kotlin adapter is stale. Screens follow
[`docs/ui/screens/foundation.md`](../../docs/ui/screens/foundation.md).

## App identity

The existing Home heading uses `design/TabulaBrand.kt`, which draws the approved T Portal mark
and outlined wordmark from `assets/brand/` with the generated `brandMark` and `brandWordmark`
roles. `TabulaBrandPaths.kt` is an export of those SVG paths and lockup coordinates, not a second
authored logo. It has one accessible heading named Tabula; the mark is decorative. Game titles,
game artwork, occupant avatars, gameplay and navigation are unchanged. There is no additional
app loading or About screen in this foundation.

Android's manifest selects `@mipmap/ic_launcher`; legacy square PNGs and the adaptive foreground
are exported from the canonical square app-icon artwork. Android supplies the adaptive mask.
The Xcode project's Resources phase includes `TabulaApp/Assets.xcassets` and both build
configurations select its `AppIcon` set. The iOS icon PNGs are square, opaque and unmasked.
Regenerate these adapters using the brand export command documented in `assets/brand/README.md`.

`:previewApp:test` includes `BrandIdentityTest` for one accessible name and the rendered identity
at 320/390 dp in all four authored schemes. Its screenshots are shared-shell preview evidence
only. Launcher appearance still requires Android/iOS execution; source wiring or exported PNGs
do not establish a device run. The optional `apps/desktop` product remains a gated no-op skeleton;
`previewApp` is testing only and does not add a desktop installer or icon target.

## The GameHost seam

`shared/.../host/GameHost.kt` is the only contract between the shell and a game surface: a launch request
in, `Ready` / `Failed` / `Exited` out, plus a `GameBackPort` through which the shell offers Back to the
host first. A host builds its runtime through `rememberGameRuntime`, which creates it **once per
composition entry** and disposes it exactly once; a recomposition, resize or new lambda cannot rebuild it.

## Build and test

Requires JDK 17 and, for Android, an Android SDK (`ANDROID_HOME`) with platform 37.

```bash
cd apps/mobile
./gradlew :shared:testAndroidHostTest :previewApp:test           # unit tests + desktop UI tests (no device)
./gradlew :android:assembleDebug -Ptabula.requireGameBundle=true  # debug APK with the game packaged
./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64   # iOS Kotlin → klib (any OS)
./gradlew :previewApp:run                                         # desktop window of the shell (simulated game page)
./gradlew :shared:embedAndSignAppleFrameworkForXcode              # iOS framework: called by Xcode, macOS only
node ../../tools/mobile-host-check/run.mjs                           # real staged game in phone-sized Chrome
```

Open `ios/TabulaApp.xcodeproj` on a Mac; its first build phase runs the Gradle task above and its last
packages the game bundle.

## What each environment proves

| Check | Linux | macOS |
|---|---|---|
| Shared Kotlin compiles for Android, iOS (klib) and desktop; unit tests | yes | yes |
| Shell navigation, `GameSession`, recomposition and back/suspend/failure rules (`:previewApp:test`, simulated page) | yes (headless Skiko) | yes |
| Debug APK assembles with the game packaged | yes | yes |
| The real staged game document under the host CSP, bridge stand-in, in phone-sized Chrome (`tools/mobile-host-check`) | yes (Chrome) | yes |
| iOS static framework links; Xcode project builds; app runs in the simulator | **no** | needed |
| Android WebView / emulator / device: input latency, frame pacing, memory, context loss, process death | **no — NOT_RUN** | — |
| iOS WKWebView: custom-scheme secure context, streamed bodies, lifecycle | **no — NOT_RUN** | needed |

Compilation is not execution, a simulated page is not a WebView, and desktop Chrome is not either target.

## Native voice (isolated development)

CMP owns the voice controls; LiveKit Android 2.29.0 / Swift 2.17.0 own native media.
Audio, room endpoints and credentials never enter the WebView bridge/game WS.
The default production source says unavailable before constructing a native room
or asking for a microphone. A local mute is not SFU/game-policy enforcement.

The optional ignored `apps/mobile/voice-dev-grant.json` is packaged only in Debug and
accepted only for the fixed synthetic scope, an exact loopback/emulator endpoint
and at most ten minutes. See [the harness](../../tools/native-voice-harness/README.md)
and [evidence ledger](../../docs/verification/native-mobile-voice/README.md).

The client is foreground-only: actual background entry, audio interruption,
route leave/logout hook, grant deadline or disposal disconnects and releases
native media. Returning never autojoins or unmutes. A game WebView reload/retry
alone keeps voice; no room credentials are attached to its launch/navigation.
Native audio focus, simultaneous game audio and headset routing need real
Android/iOS acceptance. Swift needs Xcode 16.3+ / Swift 6.1; CI compiles/links
its SDK adapter on macOS without claiming simulator/audio execution.
