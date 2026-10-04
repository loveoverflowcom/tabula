# iOS

> **Foundation slice** of [ADR-0032](../../docs/adr/0032-compose-multiplatform-mobile-host.md) with the
> first-party WKWebView host of [ADR-0033](../../docs/adr/0033-webview-gamehost-first-party-embedding.md).
> Phase 6's gate (the Phase 5 exit) is not met. **This Xcode project has not been built or run, and the
> WKWebView host has never executed**: it was authored on Linux, where the iOS framework link and Xcode do
> not exist. Kotlin/Native compiles the host to a klib; that is all that has been shown.

A thin SwiftUI container over the `TabulaShared` static framework from `:shared`. `TabulaApp.swift`
presents `TabulaViewController()` (Compose Multiplatform) and nothing else. It supersedes the earlier
plan of a `staticlib` Macroquad wrapper.

## Build path

```text
mobile/shared  --(Kotlin/Native iosArm64 | iosSimulatorArm64)-->  TabulaShared.framework (static)
               --(embedAndSignAppleFrameworkForXcode, first Xcode build phase)-->  TabulaApp.xcodeproj
```

On a Mac: open `TabulaApp.xcodeproj`, choose the `Tabula` scheme and an iOS simulator. Signing is
automatic and needs your team. Deployment target is iOS 15.

## The WKWebView host and its first macOS check

`WKWebViewGameHost` / `IosGameRuntime` (Kotlin, in `:shared/iosMain`) serve the bundle through a
`WKURLSchemeHandler` on `tabula-game://app/`, inject the bridge port with a main-frame document-start
script, accept messages only from the main frame of that origin and cancel every other navigation. The
Xcode "Package game bundle" phase copies `target/tabula-mobile-game` into the app (and fails if it was not
staged). **First thing to verify on a Mac:** that the game's loader finds `crypto.subtle` and streamed
response bodies in a custom-scheme document. It fails closed with a visible error if not; the recorded
fallback is a loopback `http://127.0.0.1` document (ADR-0033), never a weaker loader.

## What the Swift side owns

UI hosting, scene phase, and device services (AVAudioSession and microphone permission with voice in
Phase 8, universal links, APNs, the Keychain per ADR-0031). **No game logic.** None of those services
exists yet; the Info.plist declares no usage descriptions.

## Practical notes

- Every line of Swift is a line that cannot be tested by the Rust or shared Kotlin suites. Keep it thin.
- App Store review rejects apps that look like a web wrapper. The shell screens are native Compose, and
  the WebView is confined to the game surface.
- Audio session category matters for voice (Phase 8): getting it wrong either ducks the user's music
  forever or cannot capture the microphone.

## Exit criteria (doc 07 Phase 6, unchanged)

Same as Android: full match on the device matrix, suspend/resume, battery drain < 8%/hour,
crash-free sessions > 99.5%, and store acceptance. None is met by the foundation.
