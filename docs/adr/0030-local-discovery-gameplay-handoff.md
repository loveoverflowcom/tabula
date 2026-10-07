# ADR-0030: opt-in discovery handoff to the existing local gameplay document

- **Status:** accepted for the requested local integration scope
- **Date:** 2026-10-04
- **Extends:** ADR-0028's bounded discovery/setup slice
- **Supersedes:** none; ADR-010, ADR-011 and ADR-0029 remain in force
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-10 and I-15 preserved

## Context

The owner requested a fresh implementation session to integrate the existing
standalone Chess into Tabula. The fetched baseline is
`develop @ 8891bb34382dac4605c3aa29cf406528a737ff40`.
That revision already owns the pure rules, presenter, approved artwork and
native/WASM local hot-seat loop. Reimplementing those would duplicate authority.

The discovery shell has registry-owned forms and normalized launch arguments,
but an ordinary shell build is intentionally unbound. ADR-0028 did not ship a
gameplay distribution. ADR-0029's isolated embedding evidence does not authorize
iframe/Pixi migration. Native catalog scenes and server identities remain gated.

## Decision

Extend the existing local vertical slice with an **opt-in web handoff**:

- The common Leptos home, Library, detail and setup retain the common M3 theme
  and consume erased registry data. The game-specific look stays inside Chess
- The registry validates supported local two-human configuration and owns the
  declared runtime support and trusted same-origin return target. Merely binding
  a runtime does not make every catalog entry launchable
- `TABULA_PLAY_BASE=/play` binds this bundle. Without the explicit build binding,
  setup reports the missing runtime instead of claiming launch. Unrecognized
  bindings do not create external navigation targets
- `cargo xtask stage-local-play` stages the existing game bundle at
  `apps/web/dist/play/local/`, promoting `play.html` to `index.html`. It requires
  an already-built shell, validates required resources, replaces only local play
  output atomically and invalidates stale local output on a failed attempt
- `/play/local/` is a **separate HTML document**, not a Leptos gameplay route.
  Macroquad owns its single frame loop, canvas and Rust match. The URL carries
  only bounded public configuration plus host-owned navigation preferences;
  no state, credential, user identity, join token or server match id is invented
- The host offers loading/error/retry and explicit Return to Tabula. Its
  generation/abort lifecycle rejects late loading callbacks after departure,
  failure or page restoration. A reload, retry, reopening or BFCache restoration
  starts a fresh local match; none is saved/resumed. The shell revalidates before
  a later explicit launch and rejects duplicate submissions
- Local clocks keep elapsed time during dialogs, blur and hidden documents.
  Input focus loss clears held input/drag through the existing runtime focus
  callback. Leaving destroys the document's local match; it does not resign a
  remote match or change any service state
- Local terminal/restart stays in the existing runtime. Return navigates to the
  ordinary detail/setup route; no persisted `/matches/:id` result is fabricated

The standalone host remains available through its existing independent staging
command. Both builds reuse one presenter/runtime and the same assets.

The local loading refinement keeps that containment. A deployed Chess-only
WASM build excludes unrelated games; explicit launch requests external fonts
and the manifest-selected critical atlas, with another density loaded only on
an actual DPI change. Content-versioned public resources are size/SHA-256
checked by the host and pack bytes additionally retain their Rust BLAKE3 proof.
An optional bounded CacheStorage/Web Locks file cache reuses verified public
bytes across fresh documents; it stores no launch configuration, assignments,
credentials or match state. This is a local host adapter, not the deferred
general CDN/native/offline service. See the
[loading evidence ledger](../verification/game-loading/README.md).

## Remaining gates

This does not complete Phase 3, 4, 5 or 6. The registry's `ErasedMatch`, network
protocol, server/match/storage/lobby, login, online/rated/AI creation, saved replay,
resume, native catalog scenes and versioned CDN asset-delivery service remain
unimplemented or outside this slice. Chess's bot policy source is not evidence
that this browser host supports AI play. Other games' declared modes are not
silently activated through the Chess-only deployed host.

No new dependency arrow, renderer, shared WASM memory, wire schema or rules version
is introduced. This ADR records the local gate extension and is not publication,
merge or deployment authorization.

## Evidence and revisit

The [integration ledger](../verification/chess-integration/README.md) records
executed checks separately from unavailable browser/native/mobile pixels.
Builds, mocked host lifecycle tests and headless presenter tests do not establish
real target visual quality, assistive-technology play or total heap/GPU cleanup.

Revisit before adding another game host, native discovery scenes, online/resume
handoff or production hosting. Any embedding change still follows ADR-0029's
evidence requirements and explicitly supersedes ADR-011 if containment changes.
