# Android

> **Foundation slice** of [ADR-0032](../../../docs/adr/0032-compose-multiplatform-mobile-host.md) with the
> first-party WebView host of [ADR-0033](../../../docs/adr/0033-webview-gamehost-first-party-embedding.md).
> Phase 6's gate (the Phase 5 exit) is not met; this module is not a shippable app, and the WebView host
> **has not been run on an emulator or device** (see the [ledger](../../../docs/verification/mobile-game-host/README.md)).

The Android application module of the one mobile tree (see [`../README.md`](../README.md)).
`MainActivity` only calls `installTabulaContent()` from `:shared`; screens and navigation are
Compose Multiplatform code. It supersedes the earlier plan of a `cdylib` + `cargo-apk` Macroquad
app.

## What the Kotlin side owns

UI, navigation, WebView hosting and device services. **No game logic**: no rules, legality, turn order,
projection or hashing. If Kotlin needs to know whose turn it is, it shows what Rust projected; it never
computes it (ADR-001 as amended by ADR-0032).

The WebView host (`WebViewGameHost` / `AndroidGameRuntime` in `:shared`):

- serves the packaged document from `assets/tabula-game/` by request interception on
  `https://game.tabula.invalid` — no `file://`, no `INTERNET` permission, `blockNetworkLoads`, file and
  content access off, navigation locked to the bundle origin;
- injects the bridge port `TabulaHostNative` only into main-frame documents of that origin
  (`WebViewCompat.addWebMessageListener`) and re-checks origin and frame on every message; if the feature
  is unsupported the game is not loaded;
- forwards `ON_PAUSE`/`ON_RESUME` as suspend/resume and `WebView.onPause/onResume`, keeps the activity
  from being recreated on rotation or resize (`configChanges`), and handles `onRenderProcessGone`.

`MainActivity` still only calls `installTabulaContent()`.

## Lifecycle is still the part that will bite

Android suspends aggressively. The host stops drawing on suspend but the game clock keeps wall-clock time
(ADR-0030). Process death discards the local match (it is never saved) and returns to Home. When networked
play exists, suspend/resume must reach `tabula-net-client` and resume through the normal reconnect path
(`Attach { resume_from, last_client_seq }`), not a bespoke mobile path. WebView renderer loss, process
death, soft keyboard and the system back gesture are part of the evidence ADR-0032 requires and are
**not yet executed**.

## Exit criteria (doc 07 Phase 6, unchanged)

```text
[ ] a full match completes on the device matrix
[ ] suspend/resume works, including a suspend spanning a whole opponent turn
[ ] battery drain < 8% per hour
[ ] crash-free sessions > 99.5%
[ ] the Play Store accepts the build
```

None is met by the foundation.
