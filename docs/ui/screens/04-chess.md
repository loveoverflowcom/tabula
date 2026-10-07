# 04 — Chess gameplay

Issue #51; shared [gameplay contract](gameplay.md) and [foundation](foundation.md).
The original #51 review used `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188`.
The standalone material/player/control slice was implemented from
`develop @ c0a62088c159cc1f20409de41af245c98dcfcfee` using the approved ivory/petrol
design and original piece/cover art. Its [evidence ledger](../../verification/standalone-chess/README.md)
distinguishes tests/builds from blocked real runtime screenshots.

## Owner and data

[`ChessPresentation`](../../../games/chess/src/presentation/mod.rs) owns the
board, local interaction, promotion, event motion and descriptions.
[`View`](../../../games/chess/src/rules/state.rs) supplies board, turn,
status, draw offer, clock checkpoint, viewer identity and legal moves.
`View.actions` supplies non-move eligibility from the reducer validation and
`View.in_check` supplies the side-to-move check fact without reconstructing State.
Chess has public information but still uses a distinct projected type (I-5).
`ChessLocal` owns selection/press/drag, hover, focus, last move, promotion
button interaction, animation and viewport. Rules and clock semantics remain
unchanged by this presentation spec.

The standalone local driver follows the seat on turn in hot seat by default.
It explicitly admits player-bar seat selection for off-turn offers/resignation
and a Follow turn action. That presentation-local override is disabled unless
the local shell admits two-human hot seat; a network host must not enable it. A spectator
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

`View` still has no chronological move-list field. A bounded local strip records
at most 256 observed `ViewEvent::Moved` entries in coordinate notation. It is
labelled accordingly and does not invent SAN, complete chronology, saved replay
or engine analysis. Flip changes local geometry only. Eligible resign, offer,
accept/decline and claim controls use projected actions and confirmation against
the same visible position. Fullscreen, online/rated/AI and saved/server replay
remain unavailable. The #52 replay viewer still needs its own approved contract.

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
| Resign/draw controls | Only `View.actions`-eligible choices; confirmation captures the visible position, viewer and actions; Cancel/stale/repeat/blur cannot submit | One existing command through ordinary `Intent` → `Input::Player`; rules decide timeout/result |
| Flip / hot-seat seat control | Local geometry/viewer choice only; cancel armed interaction before changing orientation/seat | None; shell-admitted hot seat reprojection only |
| Retry/leave or local fatal acknowledgement | Runtime adapter, outside game rules | No game command except a separately authorized rules action |

Promotion captures pointer/keyboard input so the board cannot activate
through it. An opening Enter press cannot repeat into a choice. Up without
Down, release outside, lost focus and repeated keydowns do not activate a
choice twice. A changed board/turn/castling/en-passant/offer/status/viewer projection disables
stale promotion choices even if the same from/to remains superficially legal; Cancel
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

## Standalone startup and distribution

`apps/game-client/web/index.html` is accessible setup, with only hot seat enabled.
`play.html` runs the separate Macroquad gameplay WASM; configuration uses bounded
allowlisted arguments through the existing safe file-loading seam. Readiness follows
a real submitted gameplay frame and flush, not merely WASM download/compile.
DOM leave/help dialogs cancel board-local input and restore board focus; leaving or
reloading starts a fresh unsaved local game. Ordinary Tab reaches presenter controls;
Shift+Tab returns to host Leave/Help. Full Board Reader play is not claimed.

Native default startup uses bounded keyboard/pointer ActionButtons, supports compact
setup down to 320×390 dp, and starts clock authority only after Start.
Explicit direct launch can skip setup with validated untimed/Fischer/Bronstein values.

The tiny pinned local art pack is verified before bounded texture decoding. Canonical
SVG/cover originals and licenses are retained outside the inline export paths. The
shared system token values and navigation remain unchanged; decorative game roles
are authored once and generated for all four schemes.
