# 12 — Xiangqi Learn

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53), within the
ownership requested by [#48](https://github.com/loveoverflowcom/tabula/issues/48)
and its [ownership comment](https://github.com/loveoverflowcom/tabula/issues/48#issuecomment-5904880767).
Reviewed at `develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`.
Use the shared [gates/adapters](xiangqi.md), [foundation](foundation.md),
[Play](08-xiangqi.md) and [analysis evidence](11-xiangqi-analysis.md).
No tutor, LLM, knowledge provider or Learn route is implemented here.

The pinned `030da25d0098e240ab2cf36dacf9892e8b320a89`
[12 SVG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/screens/12-learn.svg)
and actual `mobile-states.png` were inspected. No desktop Learn PNG is
supplied. “Ply 1”, `P2–5`, the candidate arrow and conversation are mock
examples, not tested notation, engine claims or generated lesson evidence.

## Learning without a second game authority

Tabula owns the board, rules/notation, analysis branches, hint policy and
explanation-to-move mapping. Savy can be an optional knowledge/evidence
provider; it does not own position, engine transport or game state. LLM
generation is optional wording over permitted evidence, not a source of
legal moves or evaluation. No generic AI/chat framework is introduced.

Entry preserves the exact permitted position/context from Play/Analyze or a
validated postgame replay. Show side to move, original/branch identity and
named cursor/ply beside the learning task. Coach requests require practice,
analysis, postgame or another explicitly allowed mode at the owning
authority. A live competitive context cannot gain hints by changing the
selected tab, importing its position or calling the provider directly.

| Capability | Honest learning path |
|---|---|
| Rules/position available; engine absent | Explain validated rules/notation and permitted source material only; positional ranking/best-move claims are unavailable |
| Valid current engine evidence; LLM absent | Structured/template hints with candidate/PV and actual metrics; game and engine analysis remain usable |
| Knowledge provider absent | Keep engine/rules-backed hints; no invented book title, quotation or citation |
| LLM available; insufficient or stale evidence | Explain the limitation; do not upgrade plausible prose into certain tactics |
| Assistance denied or position invalid | Persistent reason and permitted return action; no job and no leaked evidence |

“Send · no LLM” in the mock is not the complete fallback: structured hints
remain usable when their rules/engine evidence and policy permit them. A
freeform explanation request may be unavailable while a separate labeled
hint action works. Missing LLM does not block ordinary human/engine play.

## Hints, provenance and transitions

Hints progress deliberately: (1) a bounded observation/question about the
position, (2) relevant candidate/rule comparison, (3) validated PV with
bounded explanation. The user chooses the next level; no automatic reveal
spoils a lesson. Each tier needs the evidence for its particular claim; an
engine PV alone does not justify an invented strategic motif. Actual lessons
or puzzles require a selected versioned source and supported validator; no
sample answer key or completion badge is treated as assessed progress.

An explanation binds its request to the same position/rules/viewer/branch,
engine/network revision, budget and evidence IDs as analysis. It identifies
which claims use rules, engine result or book material. An engine evidence
card opens the exact source candidate/PV and metrics. A book citation exposes
source/title/author where known, edition/revision, locator and a supported
excerpt/reference, with unavailable fields named. It does not prove a motif
applies to this position or imply the engine independently verified it.

| Transition | Required observable behavior |
|---|---|
| Request/hint → validating → busy | Check permission, exact current context and necessary evidence first; allocate one bounded request; show source and named Cancel |
| Repeated Send/hint | No duplicate work or duplicated conversation entry; native IME composition is never treated as submit |
| Completion | Publish only a validated response linked to this request/evidence; retain the original prompt and source identities |
| Position/branch/viewer/provider/evidence change | Retire obsolete work; late results cannot become current hints; label retained old messages with their previous position and stale status |
| Cancel/timeout/crash/invalid response | Keep prompt and permitted context, show persistent limitation/retry; no automatic cloud/provider fallback |
| Candidate/PV activation | Open the analysis branch at its exact authorized source position; never execute a move in the original match |
| Back/leave with draft or branch | Follow shared dirty-branch policy; preserve supported draft state, restore focus and cancel pending work |

Stale messages remain historical only when their data is still permitted;
permission/scope loss clears restricted evidence rather than merely applying
a stale badge. Returning to a previous position does not automatically
reactivate old evidence without full identity/resource/policy validation.
Recorded analysis/tutor artifacts retain their original provenance where a
real storage adapter exists. Regeneration is a new explanation, not replay
of an identical past answer.

“Show candidate on board” and “Try in analysis branch” have distinct meanings
and labels. Preview arrows are local evidence overlays, never authoritative
last-move marks. Trying a line validates each move with rules; partial/illegal
PV is reported, not silently repaired. The original board/history/outcome
and rating remain unchanged after preview, exploration, save or discard.

## Layout, text input and accessibility

Keep the board/context as the first major region and a bounded explanation
column beside it on expanded screens. Compact layout retains position and
mode controls, then board/collapsible context, hints, evidence and question
field. The mobile reference's text-only tutor panel does not define a second
board authority. Controls and long citations reflow at 320/390, 768, 1440 dp,
short landscape and 200% text/zoom; the composer stays reachable above safe
areas/IME without covering current evidence. Do not create a canvas rich-text
editor or move Leptos into the game runtime (I-15).

Use generated foundation typography and tonal `container` sections; the
question field uses `container-high`, persistent label and functional input
boundary. A permitted Hint/Send is the task's filled principal action;
Cancel, evidence links and tier controls are quieter, labeled, at least
44 × 44 dp. Connected tier/mode options keep selected semantics independent
of focus. Do not use decorative message borders/shadows or color alone for
source, stale or error status.

Tab order follows context/modes → board/description → hint controls →
explanation/evidence links → question → Send/Cancel. Semantic DOM text entry
supports IME and native shortcuts. Enter submits only in the explicit field
policy when composition is inactive; Shift+Enter retains a newline where
multiline input is supported. Enter/Space on buttons activates once, not on
repeat. Escape cancels armed interaction or closes a sheet/dialog and
restores its invoker; a separate Cancel ends provider work. Pointer cancel,
blur and removed/disabled controls cannot dispatch a question or move.

Messages have a clear heading/source relationship and named evidence links;
no message/card is an unnecessary focus stop. Announce a completed answer
politely once, not every streamed token; failure remains persistent readable
text. User focus and scroll are preserved when results arrive. All four
schemes, reduced motion and no-audio behavior retain tier, source, context
and failure information. Board Reader and actual screen-reader operation
need their Phase-5/9 bridge and platform evidence.

## Verification still required

Test each hint tier's claim-to-evidence mapping, no-engine/no-LLM/no-knowledge
fallbacks, permission bypass, stale/out-of-order/cancelled responses,
unsupported/malformed citations, changing evidence revisions, candidate
branch isolation, IME/repeated submit and recovery with retained prompt.
Separate rules/engine integration and tutor quality checks from provider
doubles. Runtime keyboard/focus, compact/200%/four-theme and AT checks remain
unexecuted by this specification; eloquent mock prose is not quality evidence.
