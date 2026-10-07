# Complete Chess #85 target acceptance

## Outcome

Inspect exact-source real Macroquad pixels and interrupted input for the
implemented board/HUD/accepted-motion slice; then complete only the remaining
motion polish recorded in the [#85 ledger](../../verification/chess-redesign-85/README.md).

## Why

Headless commands, deterministic motion tests and managed asset verification
cannot establish actual glyphs, orientation pixels, touch, frame pacing or
native/mobile gameplay. The issue must stay open until those acceptance
claims have named target evidence.

## Dependencies

A permitted actual Macroquad display/capture route. Local Chromium launch is
blocked by AF_UNIX socket permissions. The existing native mobile adapter and
actual device gates remain independent. Remote event-age semantics require
approved timestamp context before claiming stale-arrival behavior.

## Scope / non-goals

Use current Rust presentation/renderer, unchanged Chess rules and original
Staunton assets. Do not substitute design references for runtime screenshots,
enable gated matchmaking/account services, change the engine, merge the draft
or close #85 from source tests. New piece designs require their separate
design-first review before implementation.
