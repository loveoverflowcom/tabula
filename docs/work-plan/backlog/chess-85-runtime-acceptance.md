# Complete Chess #85 target acceptance

## Outcome

Recheck the HUD readability follow-up against exact-source real Macroquad
pixels, preserve the earlier PARTIAL receipt, and inspect interrupted input for
the implemented board/HUD/accepted-motion slice; then complete the remaining
motion polish recorded in the [#85 ledger](../../verification/chess-redesign-85/README.md).

## Why

Headless commands, deterministic motion tests and managed asset verification
cannot establish actual glyphs, orientation pixels, touch, frame pacing or
native/mobile gameplay. The issue must stay open until those acceptance
claims have named target evidence.

## Dependencies

A separately authorized dedicated browser-capture workflow is available; the
first source-pinned receipt is linked in the ledger. Local Chromium launch is
blocked by AF_UNIX socket permissions. The existing native mobile adapter and
actual device gates remain independent. Remote event-age semantics require
approved timestamp context before claiming stale-arrival behavior.

## Scope / non-goals

Use current Rust presentation/renderer, unchanged Chess rules and original
Staunton assets. Do not substitute design references for runtime screenshots,
enable gated matchmaking/account services, change the engine, merge the draft
or close #85 from source tests. New piece designs require their separate
design-first review before implementation.
