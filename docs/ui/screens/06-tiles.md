# 06 — Tiles gameplay

Issue #51; shared [gameplay contract](gameplay.md) and [foundation](foundation.md).
Reviewed `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188` against pinned
`03-gameplay/screens/06-tiles.svg`, desktop PNG and mobile board. Sample scores,
tile counts and controls in the artwork are not a rules oracle.

## Owner and projected data

[`TilesPresentation`](../../../games/tiles/src/presentation.rs) owns map/HUD,
local camera, rotation preview, cursor and input interpretation.
[`View`](../../../games/tiles/src/rules/state.rs) supplies public board/drawn
tile/discards, remaining bag count, roster, turn, phase, status/pause,
scores, meeples, followers, last placement, claimable segments and viewer.
The remaining bag order is absent from `View` (I-5); no preview, description,
error or resource list may reveal it. The maintained
[Tiles model](../../games/tiles.md) governs scoring and secrecy.

`TilesLocal` owns camera/origin/zoom, preview `Rotation`, cursor, hover,
armed pan/tap, viewport and last placed cue. Camera/rotation preview are
local until a placement command carries coordinate/rotation; camera never
travels upstream (I-10). The two player phases are `PlaceTile` and
`PlaceMeeple`; draw/scoring occur in rules transitions. Do not invent an
extra EndTurn action or treat preview rotation as camera rotation.

## Compact HUD and targets

Keep the pan/zoom map dominant. Use a bounded tonal status region, labeled
camera/rotation controls and readable current-tile/player/score/meeple
information. Current baseline controls are 34 dp and have `+`, `-`, `@`,
`R`, `X` labels. The required correction is ≥44 × 44 logical dp actual
hit regions, fixed under zoom and density, with understandable labels/names:
Zoom in, Zoom out, Recenter, Rotate tile and Skip follower.

The desktop artwork separates rotate-left/right, Place and Deselect.
Current source has one clockwise Rotate action and placement by board tap
or Enter; it does not have a deselection command or two rotation controls.
Implement only a supported intent/local operation, with a matching label.
In particular, Skip follower ends the claim step; label it as a game action,
never as harmless Cancel/Bỏ chọn. Escape has the same claim-step semantics.

At 320/390 dp, wrap or dock controls without covering the only visible
placement/claim target. Player text stays readable and indicates turn with
words and glyph/shape plus color; score/bag/meeple counters use stable
numeric typography. Long status text must wrap/reflow rather than run off
the map. At 768/1440 dp keep HUD geometry screen-fixed while the camera
moves the board. Short landscape must preserve reachable controls and status.

## UI action to intent/input

| Action | Current local behavior / source gate | Authoritative consequence |
|---|---|---|
| Primary board tap in `PlaceTile` | Convert logical point through inverse camera; check public board/drawn tile placement for current preview | `Intent(PlaceTile { at, rotation })` → `Input::Player` |
| Primary board tap in `PlaceMeeple` | Only tile `last_placed`; choose an available segment from `meeple_slots` | `Intent(PlaceMeeple { segment })` → `Input::Player` |
| Board drag beyond threshold | Pan from press-origin camera; release cannot also place | None |
| Zoom in/out / recenter | Clamp zoom to source bounds; preserve visual focus; screen-fixed control hit test | None |
| Rotate / Space | Advance local preview rotation | None until `PlaceTile` includes rotation |
| Arrow keys / Tab | Move cursor, keep visible; Tab finds next legal spot for preview rotation | None |
| Enter | Use same `act_on_square` path at local cursor | Placement/claim intent as above |
| Skip follower / Escape | Only on-turn `PlaceMeeple` action | `Intent(SkipMeeple)` → `Input::Player`; this is not local cancel |
| Focus loss / pointer cancel | Clear armed pan/hover so late release cannot activate | None |
| Off-turn/spectator/paused/terminal | No gameplay command; safe camera inspection may remain | None |

Rules validate all inputs again. Placement calculation on public board is an
affordance/preflight, not authority. Pending never inserts a tile, awards
score or consumes a meeple in `View`; rejection leaves those facts intact.
The claim chooser currently selects among advertised slots; no explicit
keyboard focus/action for every segment exists. Multi-segment choice parity
needs its own bounded design/test, not an assertion derived from one-slot
fixtures or the toolbar artwork.

## Description, focus and snapshots

`TilesPresentation::a11y`/`describe` emits status with cursor coordinate and
gated actions `place-tile`, `skip-follower`, `zoom-in`, `zoom-out`, `recenter`
and `rotate`. It emits no Board Reader regions. These describe actual intent
and local controls, but no DOM/native action dispatcher exists at the review
base. Status/actions integration waits for Phase 5; full region navigation
waits for Phase 9. Canvas keyboard support does not prove screen-reader play.

Retain board cursor feedback and make any added HUD focus distinct from
selection. A control blocked by phase/viewer/runtime state must show why and
cannot activate via pointer, keyboard or future AT. Active target geometry
must not move with button press shape or camera zoom. Repeated keydown,
release without press, pan cancellation, control-to-board release and stale
view changes need explicit once-only/capture evidence.

Required checks cover target size plus corner/center hit tests, compact
wrapping, screen/world round trips, zoom clamps, pan-versus-tap, rotation,
placement/claim/skip gating, camera noninterference, descriptions and
four schemes. Review existing opening/zoomed-dark/claim snapshots plus any
new compact/focus states deliberately. Headless render commands cannot
establish font wrapping or actual Mac/native pixels. Wheel/pinch zoom,
camera rotation and full Board Reader are not implemented and must be
reported separately in [the ledger](gameplay-verification.md).
