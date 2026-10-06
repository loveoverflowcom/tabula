# iOS

> **Foundation slice** of [ADR-0032](../../../docs/adr/0032-compose-multiplatform-mobile-host.md) with the
> first-party WKWebView host of [ADR-0033](../../../docs/adr/0033-webview-gamehost-first-party-embedding.md).
> Phase 6's gate (the Phase 5 exit) is not met. **This Xcode project has not been built or run, and the
> WKWebView host has never executed**: it was authored on Linux, where the iOS framework link and Xcode do
> not exist. Kotlin/Native compiles the host to a klib; that is all that has been shown.

A thin SwiftUI container over the `TabulaShared` static framework from `:shared`. `TabulaApp.swift`
injects the native `VoiceClient` into `TabulaViewController` (Compose Multiplatform). It supersedes the
earlier plan of a `staticlib` Macroquad wrapper. The bounded voice slice is [ADR-0037](../../../docs/adr/0037-native-mobile-voice-client.md);
production grant issuance and the phase exits remain closed.

## Build path

```text
apps/mobile/shared  --(Kotlin/Native iosArm64 | iosSimulatorArm64)-->  TabulaShared.framework (static)
               --(embedAndSignAppleFrameworkForXcode, first Xcode build phase)-->  TabulaApp.xcodeproj
```

On a Mac with Xcode 16.3 or newer: open `TabulaApp.xcodeproj`, choose the `Tabula` scheme and an iOS
simulator. Signing is automatic and needs your team. Deployment target is iOS 15. Xcode resolves the
official LiveKit Swift package at **exactly 2.17.0**; its package requires a Swift 6.1 compiler. The
thin application host retains Swift 5 language mode for the Kotlin Objective-C protocol boundary.

## Native voice and the isolated development fixture

`NativeVoiceClient.swift` owns SDK rooms, receive-only joining, OS microphone permission and publication,
audio interruption/headset-removal handling, and teardown. `VoiceController` owns the app/session policy,
grant deadline, explicit join/mute intent, and background/leave/logout retirement. Capture and tokens
never enter the WKWebView or the game bridge. The WKWebView explicitly denies all media-capture requests,
including requests from the main document after native microphone permission has been granted.

- The normal source is unavailable. There is no production grant service or provider credential fallback.
- The Debug-only build phase optionally copies the same ignored `apps/mobile/voice-dev-grant.json` consumed by
  the Android harness. The Swift host reads it only under `#if DEBUG`; Kotlin validates it through
  `DevVoiceGrantSource.parse`. Missing, invalid, expired or oversized input fails closed.
- Release removes any stale fixture from its resources and supplies no fixture text. Never commit a
  real provider token or signing secret. The development fixture is memory-only after loading and is
  discarded on session leave/logout/close.
- `Info.Debug.plist` allows insecure transport only for `127.0.0.1` and `localhost`, so the isolated
  loopback SFU works with modern ATS restrictions. Release uses `Info.plist` with no ATS exception.
  The Android emulator alias `10.0.2.2` is not an iOS harness destination.
- Join does not ask for mic access. Mic-on asks the OS and publishes only after permission, current
  attempt/command and grant deadline checks. The client reports actual publication state.
- Actual `UIApplication.didEnterBackground` retires voice; becoming active does not autojoin/unmute.
  A permission dialog's temporary inactive state is not treated as background entry. No audio background
  capability or entitlement is declared.
- A fresh Room waits for the previous room's teardown, including in-flight join/publication completion.
  SDK callbacks are delivered on the main thread and retired synchronously before asynchronous cleanup.
  After those operations and final room disconnect, SDK-global recording is explicitly stopped off the
  UI thread; a stop failure blocks future joins for this client. Failed/throwing publication terminates
  the room rather than assuming a missing publication means no capture was started.
- Quick reconnect and receive-only full reconnect are supported. A full reconnect with an enabled or
  in-flight microphone is terminated at its start (including quick-to-full upgrades) and requires an
  explicit fresh join with mic off. This conservative policy avoids the SDK's asynchronous local-track
  republishing; it is not a claim of uninterrupted microphone recovery.

Source/API review and project/fixture-script validation are **not native compilation or hardware audio
evidence**. The Swift adapter, Kotlin export conformance, framework linking and device audio flows are
**NOT_COMPILED / NOT_RUN locally**: this change was authored where Swift/Xcode are absent. The macOS CI
gate must compile/link the actual application before those build claims can be made; simulator/device
permission denial, interruptions, headset routing, backgrounding and two-client audio still need execution.

## The WKWebView host and its first macOS check

`WKWebViewGameHost` / `IosGameRuntime` (Kotlin, in `:shared/iosMain`) serve the bundle through a
`WKURLSchemeHandler` on `tabula-game://app/`, inject the bridge port with a main-frame document-start
script, accept messages only from the main frame of that origin and cancel every other navigation. The
Xcode "Package game bundle" phase copies `target/tabula-mobile-game` into the app (and fails if it was not
staged). **First thing to verify on a Mac:** that the game's loader finds `crypto.subtle` and streamed
response bodies in a custom-scheme document. It fails closed with a visible error if not; the recorded
fallback is a loopback `http://127.0.0.1` document (ADR-0033), never a weaker loader.

## What the Swift side owns

UI hosting and the bounded native LiveKit adapter, including microphone permission and audio-session
interruption handling. **No game logic.** Universal links, APNs and the Keychain remain future services
per ADR-0031. `Info.plist` declares only the microphone purpose for this slice.

## Practical notes

- Every line of Swift is a line that cannot be tested by the Rust or shared Kotlin suites. Keep it thin.
- App Store review rejects apps that look like a web wrapper. The shell screens are native Compose, and
  the WebView is confined to the game surface.
- Audio session category matters for voice (Phase 8): getting it wrong either ducks the user's music
  forever or cannot capture the microphone.

## Exit criteria (doc 07 Phase 6, unchanged)

Same as Android: full match on the device matrix, suspend/resume, battery drain < 8%/hour,
crash-free sessions > 99.5%, and store acceptance. None is met by the foundation.
