# Android

> **Foundation slice** of [ADR-0032](../../docs/adr/0032-compose-multiplatform-mobile-host.md).
> Phase 6's gate (the Phase 5 exit) is not met; this module is not a shippable app.

The Android application module of the one mobile tree (see [`../README.md`](../README.md)).
`MainActivity` only calls `installTabulaContent()` from `:shared`; screens and navigation are
Compose Multiplatform code. It supersedes the earlier plan of a `cdylib` + `cargo-apk` Macroquad
app.

## What the Kotlin side owns

UI, navigation, WebView hosting (a later change) and device services. **No game logic**: no rules,
legality, turn order, projection or hashing. If Kotlin needs to know whose turn it is, it shows what
Rust projected; it never computes it (ADR-001 as amended by ADR-0032).

Today the manifest declares no permissions and the app has no network access. Microphone,
notifications, deep links and the secure credential store (ADR-0031) arrive with the changes that
need them.

## Lifecycle is still the part that will bite

Android suspends aggressively. When the WebView `GameHost` lands, suspend/resume must reach
`tabula-net-client` and resume through the normal reconnect path (`Attach { resume_from,
last_client_seq }`), not a bespoke mobile path: two reconnect implementations diverge, and the
divergence shows up as a duplicated move. Process death and WebView context loss are in the
evidence ADR-0032 requires before embedding ships.

## Exit criteria (doc 07 Phase 6, unchanged)

```text
[ ] a full match completes on the device matrix
[ ] suspend/resume works, including a suspend spanning a whole opponent turn
[ ] battery drain < 8% per hour
[ ] crash-free sessions > 99.5%
[ ] the Play Store accepts the build
```

None is met by the foundation.
