# iOS

A thin SwiftUI container hosts the same TabulaShared static CMP framework.
[ADR-0043](../../../docs/adr/0043-native-mobile-gamehost.md) requires native Macroquad
gameplay in this application and retires WKWebView gameplay and web-bundle copying.
**Current build: shell only, no playable game or web fallback.**

Pinned Miniquad enters UIApplicationMain for a standalone application and exposes
no supported attach/detach controller API for an already-running CMP app. Native
integration requires a reviewed bounded upstream extraction/patch; native adapter,
link/assets packaging and actual device acceptance are blocked. See
[the source review and ledger](../../../docs/verification/mobile-native-host/README.md).

## Build path

```text
apps/mobile/shared -- Kotlin/Native iosArm64 | iosSimulatorArm64 --> TabulaShared.framework
              -- embedAndSignAppleFrameworkForXcode --> TabulaApp.xcodeproj
```

Use JDK 17 and Xcode 16.3+/Swift 6.1 on macOS. Xcode resolves LiveKit Swift at exactly
2.17.0. Deployment target is iOS 15; signing needs a team for devices. The project
builds the CMP framework and packages the optional Debug voice fixture, not a game
web document. Native-only source/configuration and built-app guards run in CI.

## Native voice and the isolated development fixture

`NativeVoiceClient.swift` owns SDK rooms, receive-only joining, OS microphone permission and publication,
audio interruption/headset-removal handling, and teardown. `VoiceController` owns the app/session policy,
grant deadline, explicit join/mute intent, and background/leave/logout retirement. Capture and tokens
never enter GameHost. The mobile WebView gameplay adapter has been retired by ADR-0043.

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

The original voice change had source/API review and project/fixture-script validation only locally.
[Issue #101's historical ledger](../../../docs/verification/issue-101-mobile-shell/README.md)
records a later Xcode build of the old WebView composition, including this unchanged native
media adapter and Kotlin export boundary. That establishes **compiled** evidence for the
adapter at that source, not native gameplay or hardware audio. Permission denial, interruptions,
headset routing, backgrounding and two-client audio still need execution.

## Next native acceptance

The adapter must run Macroquad on a surface/controller inside this app and return
to CMP, with explicit context/thread/lifetime ownership. Repeat open/close, input
cancel, safe area/DPI/orientation, surface loss, suspend/resume, stale callbacks,
process death and memory pressure need execution. Actual device first-frame,
frame-time, touch latency, CPU/RAM and background measurements remain required.
Desktop shell pixels, compilation and simulator runs establish their limited scope.

Swift owns UI hosting and the bounded LiveKit/device adapter; no game logic crosses
this boundary. Native accounts/networking, new voice authority, Keychain/deep links,
store release and Phase 6 exit remain gated by their separate contracts.
