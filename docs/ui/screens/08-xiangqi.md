# 08 — Xiangqi Play

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53), following
[#48](https://github.com/loveoverflowcom/tabula/issues/48). Reviewed at
`develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`. Shared
[ownership, availability and adapter gates](xiangqi.md),
[foundation](foundation.md) and [gameplay boundary](gameplay.md) govern this
screen. This is a proposed screen contract; no Xiangqi game, route or launcher
is implemented at this base.

The pinned `030da25d0098e240ab2cf36dacf9892e8b320a89` design pack's
[08 SVG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/screens/08-xiangqi.svg),
[actual desktop PNG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/previews/08-xiangqi.png)
and actual `mobile-states.png` were inspected. The initial board, names,
`09:42` clock and navigation are sample layout data. They supply no rules,
timer, legal-move or availability evidence.

## Task and authority

The user plays a permitted game and can inspect its context without changing
its authority. One Xiangqi module owns rules, position, notation and accepted
history; the normal presenter consumes only its `View`/`ViewEvent` (I-5/I-6).
Selection, orientation, focus, drag and inspection cursor are local (I-10).
Every human or engine move is an intent validated by the game `apply`; an
engine move is never written directly into state. The engine is not a second
legality oracle and tutor text is not a command.

Gameplay remains Macroquad/RenderList. Shell metadata, resource management,
long explanations and text entry use the document/native-shell seam from
doc 04 and ADR-011. A future shell hands off validated context rather than
embedding the gameplay SVG in DOM. Proposed mode paths/identities await the
#48 decision; this document introduces no new product route or game ID.

| Surface | Available only with | Unavailable behavior |
|---|---|---|
| Local human play | Tested Xiangqi rules/module, projection, presenter and actual local driver | Readable reason; no playable sample board or Start action |
| Baseline bot | Real linked projection-only bot and supported seat plan | Explain missing bot; keep a valid human mode available if supported |
| Engine opponent | Approved host adapter and exact verified, compatible, probed engine/network artifacts | No bot move, fabricated success or automatic provider/cloud substitution |
| Analyze / Learn | Supported branch/evidence adapters and permitted assistance mode | Show mode and reason; authority rejects prohibited requests even if a caller bypasses UI |
| Online/ranked play | Actual authoritative service and opened phase gate | No online badge, account flow or rated result from local play |

Coach, hints and candidate requests are allowed only in practice, analysis,
postgame or explicitly authorized modes. Live competitive contexts enforce
this at the owning local/server authority; hiding tabs is insufficient.
Pending or unverified mode permission is denial until resolved. Switching to
Analyze/Learn cannot convert a live rated session into an assisted session.

## Board, modes and states

Keep board and position context first. Present labeled Play / Analyze / Learn
options; Resources and Import position are separate task actions, not modes
that silently replace the game. Selection, focus and mode permission remain
distinct. Mode selection preserves exact session/rules/viewer/position
identity. A mode with no consumer presents its limitation rather than a fake
loading state.

| Transition | Required observable behavior |
|---|---|
| Validated entry → ready | Publish permitted initial view and actual side to move atomically; show clock only when authority supplies one |
| Piece/source selection → destination | Show projected legality hints with text/shape; cancelled drag, Escape or blur clears armed input without a command |
| Submit → pending → accepted | Prevent duplicate activation; keep pending distinct from accepted position; update board/history only from accepted projected data |
| Submit → rejected | Retain the authoritative board, clear pending preview and show readable reason; rejection cannot mutate canonical state |
| Engine job → cancelled/timeout/crashed | Retire request identity, retain current board and actual clock policy, label engine unavailable; require explicit retry/replacement where supported |
| Position/mode/seat change during engine job | Cancel obsolete work and discard every late result; recheck permitted actor before executing any candidate |
| Connection loss/resync | Follow the shared gameplay contract; disable live command affordances, replace projection atomically and clear stale gestures/animations |
| Terminal outcome | Show the actual projected result and allowed next actions; distinguish match end from fatal local runtime stop |

Clock animation is a display of supplied time, never a new timer authority.
Engine failure does not pause a clock or invent a move. A historical position
is visibly read-only: trying a move requires an explicit separate analysis
branch under [generic replay](11-replay.md). Returning to Play restores the
latest permitted live view, not a replay cursor or candidate position.

History rows use module notation and named counter units. First/Previous/
Next/Last consume the generic replay controller only when its supported
projected playback gate opens; no private second controller is introduced.
Empty history means initial position with no invented last move. Viewing a
candidate opens a labeled branch and never edits original history, outcome,
rating or clock. Import validates ruleset/notation/position and preserves
the current context on error; it cannot inject a canonical replay or grant
Audit/seat permission.

## Layout, input and accessibility

Use the foundation's generated `surface`, `container`, `container-high`,
`primary`/`on-primary`, semantic status/focus roles and existing typography.
Board art remains a game asset; the prototype's wood/purple literals do not
become runtime colors. A real principal action is filled and 56–64 dp where
space permits; related actions are tonal, labeled and at least 44 × 44 dp.
Use purposeful connected mode shapes and 2 dp gaps, not radius enlargement
on every surface. Keep functional board lines, selection, validation and
focus outlines; omit ornamental card borders/shadows.

At 905–1440 dp place a bounded history/context column beside the board; at
600–904 dp move supporting panels below when content no longer fits. At
320/390 dp use one column, board then reachable actions and a detail sheet.
All three modes stay reachable and named: the supplied mobile mock omits
Learn and does not establish the compact Analyze layout. Navigation/safe
areas follow the owning shell, not the static rail/bottom bar artwork.

The 390 dp mock has roughly 32 dp intersection spacing; small drawn pieces
are not proof of 44 dp board targets. A future touch adapter must offer an
unambiguous zoom/pan or explicit source/destination selection path when
spacing is too small, without overlapping competing targets. Verify that
path at 320 dp and short landscape. Reflow controls and HUD independently
at 200% text/zoom; never shrink the whole scene to fit.

Tab order follows context → modes → board/description → allowed actions →
history → resources. Board arrows inspect/select intersections; Enter/Space
performs one permitted activation per physical press. Mode navigation does
not steal native field keys or execute a move. Escape cancels local gesture,
then closes a sheet and restores its invoker. Pointer release outside,
pointer cancel, blur, resize and disabled/removed controls cannot activate.
Dialogs capture input and suppress the opening key's repeat/release.

Side, turn, threat and last move use labels/glyphs as well as color; names
and coordinates come from the module, not only Chinese piece glyphs. Board
Reader status/actions and regions remain their Phase-5/9 gates. DOM/native
text input retains IME; screen-reader play cannot be claimed from SVG or
description strings. All four schemes and reduced/no-audio modes retain
current turn, focus, last-action and persistent failure text.

## Acceptance evidence still required

After the rules/host gates, exercise legal and rejected human/engine moves,
repeated input, stale/late engine completion, cancellation/crash/timeout,
mode permission bypass, branch isolation, initial/terminal/empty history,
reconnect/resync and recovery. Separate conformance/projection/replay checks
from actual native/browser interaction, four themes, compact touch targets,
200% reflow, keyboard and AT evidence. This specification is `documented`;
reference PNG inspection is not game/runtime acceptance.
