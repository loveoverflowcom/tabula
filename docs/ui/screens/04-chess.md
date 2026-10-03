# 04 — Chess gameplay

Issue #51; shared [gameplay contract](gameplay.md) and [foundation](foundation.md).
Reviewed `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188` against
the pinned `03-gameplay/screens/04-chess.svg`, its desktop PNG and mobile board.
Those are sample artwork, not runtime screenshots or a supplied Chess position.

## Owner and data

[`ChessPresentation`](../../../games/chess/src/presentation/mod.rs) owns the
board, local interaction, promotion, event motion and descriptions.
[`View`](../../../games/chess/src/rules/state.rs) supplies board, turn,
status, draw offer, clock checkpoint, viewer identity and legal commands.
Chess has public information but still uses a distinct projected type (I-5).
`ChessLocal` owns selection/press/drag, hover, focus, last move, promotion
button interaction, animation and viewport. Rules and clock semantics remain
unchanged by this presentation spec.

The current local driver follows the seat on turn in hot seat. A spectator
or off-turn viewer may inspect/focus the board but cannot command a move.
The future web entry is the separate `/play/:match_id` document; catalog
navigation, account UI and post-match documents remain shell-owned.

## Board and compact HUD

Keep a square 8×8 board, rank/file coordinates, legible pieces, and separate
focus/selected/last-action/threat/valid-target marks. The inspected desktop
art puts the board before move/history and description panels; compact art
puts clock/seat labels above and below the board, with secondary panels after
it. Do not scale a desktop sidebar into unreadable phone text.

The observable HUD target is a bounded turn/status region and two stable
mono-tabular clock labels with explicit White/Black identity. `turn-active`
and `turn-waiting` accompany text; low time has a warning label/glyph and
semantic danger treatment, not blinking digits or color alone. Define any
display warning threshold as presentation policy, without changing timeout,
increment/delay, timer scheduling or match outcome. Untimed games have no
fabricated clock. Terminal HUD shows the projected result and input lock.

The presenter's live clock subtracts presentation elapsed time from the
authoritative checkpoint, with Fischer/Bronstein treatment and saturation
at zero. That display is an estimate. A zero display waits for the actual
rules/authority result; disconnect does not grant a pause or stop the rules
clock. Resync replaces the estimate from the new projected checkpoint.

The current `View` has no chronological move-list field. The artwork's
notation rows/history arrows, flip/fullscreen, guide/settings and resign
buttons are references for future consumers. Do not manufacture move history,
expose unsupported toolbar actions, or use those arrows to mutate the live
match. A future history viewer needs its own projected/history contract.

## UI action to intent/input

| Action | Local transition and source gate | Authoritative consequence |
|---|---|---|
| Tap a movable own piece | Select/press the projected piece; no command | None |
| Tap legal destination | `legal_moves` gates destination; clear local selection when forming intent | `Intent(Command::Move { from, to, promotion: None })` → `Input::Player` |
| Drag a movable piece then drop | Source identity remains fixed; threshold distinguishes tap/drag; legal drop uses the same move constructor | Same move command as tap/keyboard |
| Invalid/outside drop or pointer cancel | Restore useful source selection; no guessed move | None; distinguish preflight cancellation from rules rejection |
| Board arrows / Tab / Enter | Shared focus graph moves focus; activation selects then targets | Same command constructor; focus itself never consumes an input |
| Promotion destination | Enter a local modal before submitting; projected legal upgrades only | None until a choice is activated |
| Queen/Rook/Bishop/Knight | Shared `ActionButton`/`ButtonInteraction`; unavailable choice is disabled with reason | One `Move` carrying that exact `promotion` |
| Promotion Cancel / Escape | Modal closes; source-square focus restored; release armed activation | None |
| Resign/draw controls | Rule command variants exist, but current HUD has no corresponding controls | Future adapter must map supported command explicitly; no fake toolbar |
| Retry/leave or local fatal acknowledgement | Runtime adapter, outside game rules | No game command except a separately authorized rules action |

Promotion captures pointer/keyboard input so the board cannot activate
through it. An opening Enter press cannot repeat into a choice. Up without
Down, release outside, lost focus and repeated keydowns do not activate a
choice twice. A changed projection disables stale promotion choices; Cancel
remains reachable. Animation completion never decides when a move may submit.

## Description, focus and snapshots

`ChessPresentation::a11y`/`chess_a11y` reports turn/status, 64 square items,
selected promotion and enabled legal promotion actions from `(View, Local)`.
Existing IDs include `move-square`, `promote-queen`, `promote-rook`,
`promote-bishop`, `promote-knight` and `cancel-promotion`.
`move-square` is a generic description ID, not a complete per-square action
dispatcher. Web status/actions and action-to-intent mapping await Phase 5;
full navigable Board Reader regions await Phase 9.

Keep the current board/promotion focus graph and visible ≥3 dp focus ring
when changing HUD geometry. Theme/resize or interrupted drag must reconcile
valid focus without emitting a command. At ≥320 dp and short landscape sizes,
promotion choices/Cancel keep ≥44 dp targets. Board-cell disambiguation and
200% scaled-HUD behavior still need actual platform measurement.

Required review cases: default/selected/focused, off-turn/spectator, all four
themes, untimed/Fischer/Bronstein/low/zero/ended clocks, promotion legal and
stale-disabled choices, valid/invalid/outside drag, cancel/blur/resize,
once-only key activation, command reject/fatal, sparse/dense animation and
reduced motion. Preserve pointer/keyboard/drag command equivalence and
canonical bytes. Inspect changed RenderList snapshots deliberately; snapshot
assertions establish command content, not font metrics or rendered pixels.
Execution and remaining scope belong in [the ledger](gameplay-verification.md).
