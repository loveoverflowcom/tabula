# Complete Chess platform evidence

## Outcome

Exercise and inspect the real native/WASM standalone Chess UI on permitted
supported platforms; retain runtime screenshots and interaction evidence.
Also exercise the opt-in normal Tabula discovery/setup → separate local game →
Return flow in [ADR-0030](../adr/0030-local-discovery-gameplay-handoff.md).

## Why

The local implementation has rules, presentation and build evidence, but this
cloud browser rejected the local HTTP origin. Artwork and RenderList tests do
not establish glyph/layout pixels, real browser focus or assistive technology.

## Scope

Use the committed native/WASM flow, not a design prototype. Check desktop,
320/390/760 dp, short landscape, all four themes, 200% zoom and reduced motion;
board tap/drag/keyboard, promotion/cancel/stale/repeat, local seat selection,
draw/resign confirmation, endings/restart, setup/leave/error focus and hidden
clock catch-up. Confirm toolbar labels and clock digits do not clip.
For integrated web play, cover normal and missing runtime bindings, loading
cancel/error/retry, duplicate Start, Back/Forward/close/reopen, real BFCache
restoration and owned heap/GPU disposal. Local re-entry starts fresh, never resume.

## Dependencies

A browser/native display that permits the documented local HTTP/runtime path.
See [the implementation ledger](../verification/standalone-chess/README.md).
The [integration ledger](../verification/chess-integration/README.md) records
local core/build/mock/HTTP evidence and the separately blocked real targets.

## Risks / unknowns

Full Board Reader action dispatch, saved/server replay, online/rated/AI and the
remaining renderer migration evidence remain separately gated. Do not expand
this target-validation task into those capabilities or infer phase completion.
