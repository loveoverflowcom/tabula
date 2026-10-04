# Tabula mobile

> **First-party embedding, not a plugin system.** [ADR-0032](../docs/adr/0032-compose-multiplatform-mobile-host.md)
> opened the Compose Multiplatform foundation; [ADR-0033](../docs/adr/0033-webview-gamehost-first-party-embedding.md)
> adds a WebView `GameHost` for the one game packaged with the app. It does not open Phase 6: no networking,
> voice, accounts, store build, third-party games or remote updates. **The Android WebView and the iOS
> WKWebView have not been run** — see the [evidence ledger](../docs/verification/mobile-game-host/README.md).

One Gradle root for the one mobile app. There is no second mobile tree.

```text
mobile/
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
| Mobile host (Kotlin/Swift) | Device services — only `keep-awake` exists; voice, permissions, push do not |
| The game document (Rust/WASM + `host-bridge.js`) | Rules, `project`, presentation, `RenderList`, renderer, its own loading/error/leave UI |

Kotlin and Swift carry **no game logic**: no legality, turn order, projection, hashing or replay. They
never branch on a game id; the launch query comes from the Rust registry through `tabula-games.json`.

## Playing the packaged game

```bash
just mobile-game            # builds the wasm game and runs `cargo xtask stage-mobile-game`
cd mobile && ./gradlew :android:assembleDebug -Ptabula.requireGameBundle=true
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
palette colours and hex literals under `mobile/` (the generated file is the one exemption), and
`cargo xtask check` fails when the Kotlin adapter is stale. Screens follow
[`docs/ui/screens/foundation.md`](../docs/ui/screens/foundation.md).

## The GameHost seam

`shared/.../host/GameHost.kt` is the only contract between the shell and a game surface: a launch request
in, `Ready` / `Failed` / `Exited` out, plus a `GameBackPort` through which the shell offers Back to the
host first. A host builds its runtime through `rememberGameRuntime`, which creates it **once per
composition entry** and disposes it exactly once; a recomposition, resize or new lambda cannot rebuild it.

## Build and test

Requires JDK 17 and, for Android, an Android SDK (`ANDROID_HOME`) with platform 37.

```bash
cd mobile
./gradlew :shared:testAndroidHostTest :previewApp:test           # unit tests + desktop UI tests (no device)
./gradlew :android:assembleDebug -Ptabula.requireGameBundle=true  # debug APK with the game packaged
./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64   # iOS Kotlin → klib (any OS)
./gradlew :previewApp:run                                         # desktop window of the shell (simulated game page)
./gradlew :shared:embedAndSignAppleFrameworkForXcode              # iOS framework: called by Xcode, macOS only
node ../tools/mobile-host-check/run.mjs                           # real staged game in phone-sized Chrome
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
