# 11 — Xiangqi analysis and evidence extension

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53). Reviewed at
`develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`. Use the shared
[Xiangqi gates and adapters](xiangqi.md), [foundation](foundation.md),
[generic replay controller](11-replay.md) and [Learn](12-learn.md).
This proposed extension implements no replay controller, engine or route.

Inspected the pinned `030da25d0098e240ab2cf36dacf9892e8b320a89`
[analysis SVG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/screens/11-xiangqi-analysis.svg),
[actual analysis PNG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/previews/11-xiangqi-analysis.png)
and actual mobile/state PNG. Their PV, arrow and `25% CPU · 1 thread · 5s`
are sample data, not a current evaluation or measured resource guarantee.
The mobile pack supplies no complete Analyze screen.

## Shared position, separate evidence

Analyze takes the exact permitted position from the module and shared
replay/branch boundary. It adds evaluation, candidate and provenance panels;
it does not reconstruct history with AI or create another Xiangqi state
authority. Original match/history/outcome stays immutable. Rules validate
imported positions and every move used to explore a candidate branch.
Projection alone must not be assumed to provide reconstructable canonical
state, secrets or permission (I-5/I-6).

An analysis request is authorized before host work starts. It binds game,
session/artifact, rules version/hash, viewer scope, original cursor/branch,
position digest, provider/build, evaluation-network revision/hash, validated
budget and request/generation identity. The exact digest representation is
the future module/adapter contract, not a UI string computed from pixels or
chat. A URL, selected tab or cached evidence cannot grant assistance.

Display engine source separately from book/knowledge citations. A valid
engine result includes score kind and units, side/perspective, candidate/PV,
depth, nodes, elapsed time, source identity and actual budget. Missing or
unsupported metrics say Unknown/Unavailable; zero is a real measurement,
not a missing-value placeholder. A mate score is not formatted as centipawns.
Scores from different providers or perspectives are not silently compared.
Finite-budget search is evidence, not proof of a best move or certain motif.

The adapter validates output shape, bounds and PV legality against its exact
position/rules before it becomes usable evidence. Malformed output cannot
draw a current candidate arrow. A book citation supports the cited source
claim; it does not establish that the engine found the same tactic.

## Request and candidate lifecycle

| State / transition | UI and effect |
|---|---|
| Unavailable / denied | Explain missing rules, engine, network, host/platform or permission; no score/PV, fake progress or executable Start |
| Ready → validating → running | Validate resource identity and CPU/thread/time/node/RAM limits; allocate one bounded job; retain readable Running and Cancel |
| Repeated Start / retry | Busy prevents duplicate jobs; idempotency is scoped to this authorized request, not a global provider cache |
| Running → partial evidence | Display only validated output for this request, labeled In progress with actual metrics; candidate preview remains separate |
| Running → complete | Publish validated evidence atomically with matching position/request identity; completion does not execute a move |
| Cancel / leave / changed position, branch, viewer or provider | Retire generation immediately and stop accepting output; host cancellation is bounded; late partial/final results remain discarded |
| Timeout / crash / malformed output | Persistent failure with source and retry reason; retain permitted board; no automatic download, provider substitution or cloud fallback |
| Evidence → stale | Keep old evidence only in a visibly labeled previous-position record; exclude it from current score/arrows/hints and require explicit reanalysis |

Absent engine means Cancel is also unavailable: the reference's visible
Cancel shape does not imply an active job. Budget controls expose only
enforceable fields. A CPU percentage is not offered as an enforceable limit
unless that platform adapter proves it; thread count or time limits do not
by themselves establish CPU/RAM safety. Measurable progress uses actual
counts; no fabricated percent, depth or completion.

Selecting a candidate previews its evidence. The distinct “Explore in
analysis branch” action creates or switches to a branch tied to that exact
permitted source position. It never calls a live match command, replaces
original history or relabels the candidate as accepted play. Exploration
uses the shared branch dirty/save/discard/Back policy; failed save keeps the
draft. Returning to Original retires branch jobs and restores the validated
original cursor. A stale completion cannot resurrect a discarded branch.

Replay transport, log, cursor mapping, seek cancellation and counter units
come from screen 11. Canonical `InputIndex`, accepted `StateVersion`,
permitted playback cursor and Xiangqi ply/notation remain distinct. Analysis
artifacts may record their original provenance where an authorized storage
adapter exists; regenerating an evaluation later produces new evidence,
never a supposedly identical historical replay.

## Layout and interaction

Board/context is the first large region. Expanded layout puts engine status,
candidate/evidence and budget in bounded tonal sections beside the board;
compact layout places board, mode actions, status and candidates before a
reachable evidence/budget sheet. Keep source, position and stale labels
visible when details collapse. Long PVs wrap or scroll within a named region
without forcing page overflow; text/controls reflow at 320/390, 768, 1440 dp,
short landscape and 200% zoom. Do not shrink board and text together.

Reuse generated semantic roles and existing foundation components: filled
Start when it is a valid principal action, tonal labeled Cancel/Explore,
contained candidate rows, named budget fields and persistent error/status.
Every action is at least 44 × 44 dp; selected candidate, current position and
keyboard focus have separate semantics. Functional arrows/board lines,
selection and focus remain; ornamental outlines/shadows do not. No raw
prototype palette or second plugin/chat component system is introduced.

Tab order is context/modes → board/description → engine status → candidates
→ budget → Start/Cancel → provenance. In candidate lists, arrows move focus
and Enter/Space activates once; Home/End act only within that list. Transport
shortcuts remain in replay scope and native field keys remain native. Escape
cancels an armed gesture or dismisses a sheet/dialog; job cancellation is an
explicit named action with a persistent result. Restore focus to the invoker
or logical successor when a running control disappears. Blur releases armed
input; resume/retry requires a deliberate action.

Announce completed position/evidence changes politely, not each engine
line/node. Urgent failure is announced once and remains readable. A busy
panel does not trap keyboard focus. Reduced motion retains stationary
status and current candidate marking; all four themes/no-audio modes retain
the same information. Actual Board Reader/AT play remains gated and needs
runtime evidence, independent of this design.

## Required verification after adapters exist

Exercise position/branch/viewer/rules/provider changes while running,
out-of-order partial/final output, repeated Start/Cancel, timeout/crash,
invalid/oversized PV and score, stale cache, enforced budgets, permission
bypass and immutable original after candidate exploration. Real engine
integration is separate from doubles. Compare branch rules/projection and
replay identity against actual fixtures, then capture runtime themes,
keyboard/focus, compact/200% and failure recovery. At this base these are
future checks, not passing engine or replay evidence.
