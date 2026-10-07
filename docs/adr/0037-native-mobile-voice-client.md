# ADR-0037: native mobile VoiceClient, isolated development scope

- **Status:** owner-authorized bounded implementation; production voice unavailable
- **Date:** 2026-10-05
- **Base:** `develop @ aee07128d111c0bd5c15eb2ccaacc40c880ba5c3`
- **Extends:** ADR-0032/0033 for native host voice only; ADR-0031/0034/0036 unchanged
- **Invariants:** none relaxed; I-1, I-5/I-6, I-9/I-10/I-13/I-15 preserved

## Context and gate

The owner requested the next PR after the isolated account series to add native
voice for mobile CMP, with a real provider path, isolated dev/test grants when
production grants are absent, and explicit evidence gaps. This is a narrow
exception to the mobile/voice phase ordering, not Phase 6/8 completion or a
production account, multiplayer, backend voice or Werewolf policy launch.

The current GameHost serves bundled local Rust/WASM gameplay through a restricted
WebView. `tabula-voice` remains the **backend** VoiceService scaffold. The isolated
account work supplies no production provider authentication, native secure store,
match authority or voice grant API. A UI mute cannot enforce speaking/listening
rules or revocation against a hostile participant.

## Decision

### Ownership and provider

- Shared CMP `VoiceClient` is the native **client** media port. `VoiceController`
  owns observable connection/publication state and operation generations; CMP
  owns controls. This is distinct from backend `VoiceService` room/scope authority
- Android uses official `io.livekit:livekit-android:2.29.0`; the iOS Swift app uses
  SPM `client-sdk-swift` exact `2.17.0`, injected through the exported Kotlin
  interface. SDKs own native WebRTC/media; no custom WebRTC/SFU is introduced
- Pins were assessed against official release/source on 2026-10-05. Android needs
  its upstream pinned AudioSwitch fork from JitPack. Swift requires Swift tools
  6.1 / Xcode 16.3+. This selection is an experiment, not a measured provider winner
- The host selects an opaque session scope. The page cannot supply credentials,
  endpoint, channel, room or speaking permission. No new bridge service is needed
  by the existing local game, so its bounded grammar and WebView network/capture
  restrictions stay unchanged. No media or grants cross the bridge or game WS

### Grant and lifecycle boundary

Production uses `UnavailableVoiceGrantSource` and fails before native SDK room
construction, connect, microphone permission or capture. A future grant source
must revalidate current account/auth-session/membership/voice scope, bind its
purpose/audience and deadline, and connect revocation to SFU enforcement before
production activation. No auth/session mock makes that production path available.

A debug native build may load an ignored `apps/mobile/voice-dev-grant.json`. Its strict
versioned grammar admits one fixed dev scope, an exact loopback/emulator endpoint,
a bounded token and at most ten minutes of remaining authority. It is never read
by Release or by JavaScript. The local harness uses ephemeral signing keys and two
unique participants in one **public synthetic** room; it does not simulate a
Werewolf secret channel or prove game policy. Grant objects have redacted
`toString`, no serialization, and stay out of navigation/UI state/logs.

Receive-only join does not request microphone permission. Explicit mic-on requests
OS permission, then native publication; denial preserves listening. The SDK’s
actual result determines displayed publication state. The SFU enforces its token
permissions independently of local `canPublish` presentation state.

Voice is app/session-owned outside GameHost runtime composition. Reload/Retry
replaces the WebView without an automatic voice leave. Explicit game route leave,
logout owner hook, grant deadline, interruption and final disposal retire pending
work and disconnect/release the native room. Every async grant/join/mic callback
is attempt/command fenced. A late join performs native cleanup, never restores UI.
The mobile app currently has no account owner; `onLogout` is a tested seam, not a
claim of provider/secure-store integration.

### Platform audio policy

This slice is **foreground-only**. Android Activity `onStop` and iOS actual
`didEnterBackground` stop voice; permission-dialog pause/inactive state is not
background. Foreground return never autojoins or republishes the microphone.
No microphone foreground service, background audio entitlement or notification
subscription is enabled. SDK capture is released at terminal cleanup; merely
muting is not called resource cleanup.

Android’s SDK owns communication audio mode/focus and headset routing. Focus loss
stops the room and requires explicit rejoin. Swift’s SDK owns AVAudioSession;
interruption/unplug policies conservatively stop voice, preserving OS route
selection rather than inventing an unsupported iOS device picker. Simultaneous
WebView game audio, headset changes, speaker behavior and background cleanup
require real target observations, not common-state tests. Swift recorder cleanup
explicitly stops SDK-global local recording after cancelled join/mic work settles,
off the UI thread; a failed drain blocks new rooms. Swift mic-on/pending-mic full
reconnect (including quick-to-full upgrades) stops and requires a fresh explicit
join, with mic off, instead of trusting asynchronous automatic republishing.
Quick reconnect and receive-only full reconnect remain supported.

## Evidence and consequences

[Evidence ledger](../verification/native-mobile-voice/README.md) separates common
controller/UI doubles, Kotlin/Android compilation, Swift/framework compilation,
and real SFU/device audio. An ephemeral fixture or a successful native build is
not a successful two-client connection. Phase 8 SFU scope enforcement and provider
swap acceptance are still owed. Existing production service/provider/session/WS
output gates remain closed. No persistent credentials, OAuth grant, hosted SFU,
deployment, payment or account security setting is created.

## Upstream sources

- [Android release](https://github.com/livekit/client-sdk-android/releases/tag/v2.29.0)
- [Android Room](https://github.com/livekit/client-sdk-android/blob/v2.29.0/livekit-android-sdk/src/main/java/io/livekit/android/room/Room.kt)
- [Android local publication](https://github.com/livekit/client-sdk-android/blob/v2.29.0/livekit-android-sdk/src/main/java/io/livekit/android/room/participant/LocalParticipant.kt)
- [Swift release](https://github.com/livekit/client-sdk-swift/releases/tag/2.17.0)
- [Swift package/toolchain](https://github.com/livekit/client-sdk-swift/blob/2.17.0/Package.swift)
- [Swift audio policy](https://github.com/livekit/client-sdk-swift/blob/2.17.0/Docs/audio.md)
- [Kotlin Objective-C/Swift interoperability](https://kotlinlang.org/docs/native-objc-interop.html)

## Revisit when

Before production grant issuance, native account activation, background voice,
a non-loopback dev deployment, secret-channel/Werewolf enforcement claims, SDK
upgrade, or shipping either platform without its native device acceptance.
