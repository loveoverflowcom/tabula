# iOS

> **Foundation slice** of [ADR-0032](../../docs/adr/0032-compose-multiplatform-mobile-host.md).
> Phase 6's gate (the Phase 5 exit) is not met. **This Xcode project has not been built or run**:
> it was authored on Linux, where neither the iOS framework link nor Xcode exists.

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
