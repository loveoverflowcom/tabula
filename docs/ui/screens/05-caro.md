# 05 — Caro gameplay (future-gated)

Issue #51; shared [gameplay contract](gameplay.md) and [foundation](foundation.md).
Reviewed `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188` and pinned
`03-gameplay/screens/05-caro.svg` source. The companion SVG includes Caro,
Werewolf and runtime recovery reference panels; it is not a runnable game.

## Gate and owner

[`games/caro/src/lib.rs`](../../../games/caro/src/lib.rs) is a Phase-3 design
placeholder. There is no `GameRules`, `GameModule`, `View`, `game.toml`, bot,
presenter, conformance fixture or replay at the review base. Declared Cargo
features and the grid artwork do not supply them. Phase-2 exit and the Phase-3
implementation gate must be established before the game becomes playable.

The maintained [Caro design](../../games/caro.md) owns unresolved board size,
exact-five/overline/blocked-end and freestyle/restricted-rule decisions.
This UI specification makes no choice among them. Do not infer legality or
board dimensions from the sample marks. Caro is not an unsupported fallback
to a different game's local driver.

## Intended projection and local boundary

A future game-owned distinct `View` carries the public grid, turn, status,
viewer/seat identity and authoritative legal placements. Perfect information
does not justify sending canonical `State`. Last placement/winning line must
come from permitted projected facts/events; a painted line cannot declare
a win. A future `CaroLocal` owns hover, focus, provisional selection and
event animation; pending placement stays outside the authoritative `View`.

| Intended UI action | Future mapping, contingent on implemented contract | Current status |
|---|---|---|
| Focus arrows/Tab, select cell | Local focus graph and coordinate description | NOT_IMPLEMENTED |
| Tap or Enter on legal empty cell | `Intent(Place { at })` → `Input::Player`; exact type owned by future game | Illustrative, NOT_IMPLEMENTED |
| Occupied/out-of-range/wrong-turn cell | No action when affordance disabled; actual rejection preserves context and grid | NOT_IMPLEMENTED |
| Drag/preview then cancel/blur | Local only until committed; no extra placement on release after cancellation | NOT_IMPLEMENTED |
| Resign | Game-defined command only if implemented/advertised | Illustrative, NOT_IMPLEMENTED |
| Undo, bot move, timer, ranked or network resume | Show only with actual capability and functioning adapter | Not supplied by this artwork; NOT_IMPLEMENTED |

The proposed `Place` and `Resign` names are doc-02 sketches, not existing Rust
APIs. Board Reader action IDs and a real action dispatcher must be designed
against the eventual projection/intent types at their owning phase.

## Layout and feedback target

Board first; preserve meaningful grid lines, coordinate labels, last action
and visible focus. Identify players/marks with X/O plus seat labels, not
color alone. Compact portrait uses a readable grid with separately reflowing
HUD; if cells fall below 44 dp, specify and verify nearest-cell disambiguation
or zoom rather than overlapping ambiguous hit rectangles. Keep primary
confirmation near the compact board and destructive actions separate.

Turn, selected coordinate, disabled reason, terminal result and pending/error
feedback share foundation roles in all four schemes. No invented countdown
before a timing contract exists. Real accepted placement events drive motion;
reduced motion retains last-action/focus information. The future description
names grid coordinates, occupant, turn, last placement and legal action.
Screen-reader claims require the actual platform mirror and interaction run.

## Future acceptance

Settle the maintained rule variant, implement the module and conformance,
then test UI input against the game-owned legality oracle: boundaries,
occupied cells, turn/terminal gates, winning-line presentation, equivalent
pointer/keyboard commands, cancellation and once-only activation. Record
320/390/768/1440 dp, short landscape, 200% scaling, safe areas, four schemes
and focus/action accessibility evidence. No Caro test run or empty Cargo
selection may be counted as presentation evidence today; see
[the ledger](gameplay-verification.md).
