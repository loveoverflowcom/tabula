# Tabula mobile

> **Foundation only.** This is the bounded Phase 6 slice opened by
> [ADR-0032](../docs/adr/0032-compose-multiplatform-mobile-host.md). It does not open Phase 6:
> there is no embedded game, no networking, no voice, no accounts and no store build.

One Gradle root for the one mobile app. There is no second mobile tree.

```text
mobile/
├── shared/    Compose Multiplatform library: screens, navigation, theme, GameHost interface
├── android/   Android application module (a thin Activity over :shared)
└── ios/       Xcode project (a thin SwiftUI container over the :shared framework)
```

## Who owns what

| Owner | Responsibility |
|---|---|
| Compose Multiplatform (`shared/`) | App screens, navigation, theming from generated tokens |
| `GameHost` (a later change) | A platform WebView presenting the existing Rust/WASM game document |
| Mobile host (Kotlin/Swift) | Voice, device permissions, native services — none exist yet |
| Rust (unchanged) | Rules, `project`, presentation, `RenderList`, renderer |

Kotlin and Swift carry **no game logic**: no legality, turn order, projection, hashing or replay.
Whatever they display about a match is what Rust projected.

## Design tokens

`tokens.toml` is the only authored source. `cargo xtask gen-tokens` also writes
`shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/design/TabulaTokens.kt`; never edit it.
`design/TabulaTheme.kt` only maps those values onto Compose. A missing role is a `tokens.toml`
change, not a literal in Kotlin. `cargo xtask check-no-raw-colors` rejects `Color(...)`,
palette colours and hex literals under `mobile/` (the generated file is the one exemption), and
`cargo xtask check` fails when the Kotlin adapter is stale. Screens follow
[`docs/ui/screens/foundation.md`](../docs/ui/screens/foundation.md).

## The GameHost seam

`shared/.../host/GameHost.kt` is the only contract between the shell and a game surface:
a launch request in, `Ready` / `Failed` / `Exited` out. The default `PlaceholderGameHost` draws a
reserved slot and reports `Failed`, so the shell never mistakes it for a playable game. The next
change supplies the WebView implementation; only the interfaces a change needs are added.

## Build and test

Requires JDK 17 and, for Android, an Android SDK (`ANDROID_HOME`) with platform 37.

```bash
cd mobile
./gradlew :shared:testAndroidHostTest :android:assembleDebug   # Android: tests + debug APK
./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64   # iOS Kotlin → klib (any OS)
./gradlew :shared:embedAndSignAppleFrameworkForXcode           # iOS framework: called by Xcode, macOS only
```

Open `ios/TabulaApp.xcodeproj` on a Mac; its first build phase runs the Gradle task above.

## What each environment proves

| Check | Linux | macOS |
|---|---|---|
| Shared Kotlin compiles for Android; unit tests | yes (`testAndroidHostTest`) | yes |
| Debug APK assembles | yes (`assembleDebug`) | yes |
| Shared Kotlin compiles for iOS (klib) | yes (`compileKotlinIos*`) | yes |
| iOS static framework links | **no** (task is skipped off macOS) | needed |
| Xcode project builds / app runs in the simulator | **no** | needed |
| App launches on an Android emulator or device | needs an emulator or device | — |
| Real game, WebView, device matrix, stores | not part of this change | — |

Compilation is not execution. See [`android/README.md`](android/README.md) and
[`ios/README.md`](ios/README.md) for platform notes.
