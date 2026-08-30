# Android

> **PHASE 6.** Gate: the web shell ships and a stranger can play (Phase 5 exit).

A Gradle wrapper around `apps/game-client` built as a **`cdylib`**. Gameplay is
**native Macroquad**, not a WebView (ADR-019) — WebView input latency and
rendering would become the product's ceiling on the platform where most players
are.

## Build path (doc 01 §7)

```text
apps/game-client  --(crate-type = cdylib)-->  libtabula_game_client.so
                  --(cargo-apk | cargo-ndk)-->  mobile/android
```

Targets: `aarch64-linux-android` (ship), plus `armv7-linux-androideabi` and
`x86_64-linux-android` for emulators.

`cargo-apk` initially. Graduate to `cargo-ndk` + Gradle when we need Play
Billing, notifications, or custom `Activity` behaviour — which is to say, as soon
as this is a real product rather than a demo.

## What the Kotlin side owns

Glue only (ADR-001 permits it, and only it):

```text
Activity + SurfaceView lifecycle          → suspend/resume into tabula-net-client
deep links (tabula://match/<id>)          → MatchContext handoff
push notifications for async turns        → payload schema is a Phase 6 contract
Play Billing (later)
permissions (microphone, Phase 8)
```

No game logic. No rules. No projection. If Kotlin needs to know whose turn it is,
something has gone wrong.

## Lifecycle is the part that will bite

Android suspends aggressively. `tabula-net-client` must receive explicit
suspend/resume events, and resume must go through the normal reconnect path
(`Attach { resume_from, last_client_seq }`) rather than a bespoke mobile path.
Two reconnect implementations diverge, and the divergence shows up as a
duplicated move.

## Exit criteria (doc 07 Phase 6)

```text
[ ] a full match completes on the device matrix
[ ] suspend/resume works, including a suspend spanning a whole opponent turn
[ ] battery drain < 8% per hour
[ ] crash-free sessions > 99.5%
[ ] the Play Store accepts the build
```
