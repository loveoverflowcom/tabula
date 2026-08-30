# iOS

> **PHASE 6.** Gate: the web shell ships and a stranger can play (Phase 5 exit).

A thin Xcode wrapper around `apps/game-client` built as a **`staticlib`**.
Gameplay is **native Macroquad**, not a WebView (ADR-019).

## Build path (doc 01 §7)

```text
apps/game-client  --(crate-type = staticlib)-->  libtabula_game_client.a
                  --(cargo-lipo style packaging)-->  mobile/ios
```

Targets: `aarch64-apple-ios` (ship) and `aarch64-apple-ios-sim` (simulator).

## What the Swift side owns

Glue only:

```text
UIViewController + MTKView/CAEAGLLayer lifecycle
scene phase changes → suspend/resume into tabula-net-client
universal links (tabula://match/<id>) → MatchContext handoff
APNs push for async turns
StoreKit (later)
AVAudioSession + microphone permission (Phase 8)
```

No game logic.

## Practical notes

- **Thin is the requirement, not the aspiration.** Every line of Swift is a line
  that must be reimplemented in Kotlin, and a line that cannot be tested by the
  Rust test suite.
- Audio session category matters for voice (Phase 8) — get it wrong and the game
  either ducks the user's music forever or cannot capture the microphone.
- App Store review rejects apps that look like a web wrapper. Native Macroquad
  helps here, but the shell screens must not look like a website in a frame.

## Exit criteria (doc 07 Phase 6)

Same as Android: full match on the device matrix, suspend/resume, battery drain
< 8%/hour, crash-free sessions > 99.5%, and store acceptance.
