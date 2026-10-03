# 11 — Generic replay

Issue #52; shared [authority, gates and failures](results-replay.md),
[foundation](foundation.md), [results](09-results.md) and
[history](10-history.md). Reviewed
`develop @ eb7803b8a304f2be4ae9890ea6484dab02688540` against the pinned
`04-replay/screens/11-analysis.svg` and actual mobile PNG. The desktop source
has no supplied PNG. Its sample Chess board, “ply 12 / 84”, notation and branch
are layout references, not a real replay, engine evaluation or runtime proof.

## Task, owner and gate

The user watches a permitted immutable match, navigates its recorded timeline
and understands compatibility/fidelity. Result/history belongs to the future
shell `/matches/:id`/profile flow; gameplay remains its separate document
(ADR-011). The shell supplies validated replay context and controls, and a
compatible normal presenter renders only the permitted view/events. Native
uses the equivalent scene boundary; Leptos never enters game-client (I-15).

Projected client playback, checkpoint fold/seek, speed and scrub are Phase 9
(doc 05 §8.3, doc 07). Existing canonical `ReplayRunner<R>` is typed offline
verification tooling and rejects projected playback. A supported parser or
local accepted trace does not open this gate. No generic runtime/controller,
branch adapter or engine is implemented by this spec.

On entry pin artifact identity, game/package/rules/format version, permitted
viewer, compatible resources and projected initial data. The original stays
read-only. Imported/URL-selected viewer values cannot grant seat/Audit
permission; changing scope reauthorizes/reloads, never relabels cached bytes.
Default to a stable paused initial position after real validation. Restored
cursor/speed is a local preference checked against this exact artifact/scope;
it does not authoritatively alter the match or grant additional knowledge.

## Reusable controller, timeline and notation

| Controller concern | Shared behavior | Module/authorized adapter data |
|---|---|---|
| First / previous / next / last | Move among permitted cursor positions with boundary reasons | Initial/end range and mapping to game notation |
| Cursor / timeline | Distinct current position, selected log row and keyboard focus; immutable original marker | Safe projected event order, labels/markers and allowed checkpoints |
| Read-only board | Normal presenter receives projected view/events; inspection is local | Board geometry, orientation, accessible descriptions, permitted pieces/tiles/roles |
| Move/event log | Readable rows, current-position marker, activation seeks | Game-specific notation and explicit counter unit; never hardcoded White/Black columns |
| Play/pause and speed | Real local playback, 0.5×/1×/2×/4× when implemented | Recorded logical timing; no fabricated clocks or hidden event timing |
| Compatibility/resources | Separate identity, integrity and reconstruction dispositions | Recorded rules hash/version, linked support/migration and required/optional pack status |
| Extensions | Shared controller exposes exact scoped position and safe availability | Separately registered module extension/branch capability with real consumer and reason |

“Ply” is appropriate only when the game's adapter defines it. Canonical
`InputIndex`, accepted `StateVersion`, permitted playback cursor and game
notation remain separate as specified in [the boundary](results-replay.md).
Timer/seat/admin transitions may advance accepted state without a player
move. Hidden events may be omitted entirely, so a visible log may not contain
one row per canonical transition; do not reveal omissions through counts,
placeholder rows or private timing/phase markers.

At cursor zero show initial permitted view and no invented last move. At end
pause, retain the last view and actual terminal summary when authorized;
Next/Last are disabled with readable “At end” reason. At start Previous/First
are similarly unavailable. A zero-event artifact has one initial position,
no artificial slider range and no claim that it completed a match. Focus and
selection stay independent from cursor and current-position highlight.

Seek validates target, restores an allowed checkpoint at/before it and folds
recorded permitted events in exact order, publishing view/cursor atomically.
Without a checkpoint use the initial view. A slider drag may preview a target
label but cannot publish speculative authoritative-looking positions. Each
new committed seek supersedes older work by request generation. Repeated
same-target seek is idempotent; reverse seek and rapid first/end/first must
converge to the requested permitted position. Cancel/leave/changed artifact or
viewer retires pending work; stale completion cannot flash an old board.

Seek/pause drops or compresses presentation animations and clears stale live
selection/drag/promotion/pending commands. Playback never sends a live rules
command. ViewEvent motion uses recorded logical timing and actual speed;
skipped/reduced/interrupted animation converges to the same view (I-10).
No replay time display ends a live match or changes original clock/outcome.
Blur cancels armed actions and pauses local autoplay; refocus requires an
explicit Play rather than silently catching up. Inspection camera remains
local and cannot change the cursor or reconstructed authority.

## Branches and Xiangqi extension seam

A branch requires a real supported reconstruction/analysis adapter and is an
explicit separate action at an exact permitted original position. It carries
original artifact/version/viewer/cursor identity and its own draft/dirty/save
state. Original inputs, projected events, checkpoints, outcome and stored
result are immutable. Branch moves never enter the live match, overwrite its
history or receive its rating/verification label.

Do not assume a projected view contains enough rules state to simulate a
branch, especially for hidden-information games. Unavailable reconstruction/
branch capability has a visible reason; canonical secrets/seed cannot be
fetched to fill the gap. Returning to Original is a local context switch;
dirty branch asks Save/Discard/Cancel only when a real save adapter exists,
otherwise Discard/Cancel with an explicit unsaved limitation. Failed save
keeps the draft and original intact. Back follows the same dirty policy and
focus restoration; stale seek/extension completion cannot revive a discarded
branch or apply to a newer original position.

Xiangqi reuses this controller/timeline/log and supplies board geometry,
seat/side labels, notation, supported cursor mapping and extension capability
through its module/registry boundary. There is no platform game-id branch or
duplicated Xiangqi controller. Xiangqi engine evaluation/PV/evidence/tutor
panels belong to design group `05-xiangqi` and a separate authorized supported
provider. This issue adds no engine, AI reconstruction, evaluation score,
accuracy or tutor output. A later extension keys evidence to artifact/rules
identity, viewer scope, branch identity and cursor/request generation, and
must discard stale results. A label/capability declaration without an actual
consumer never enables the panel or proves engine quality.

## Layout, input and accessibility

The board is the first large region. Expanded/large layout places a bounded
notation panel beside it and a tonal transport/timeline below; compact layout
puts board before reachable controls and a detail sheet/log. Timeline and
original/branch labels remain readable without the sidebar. Verify 320/390,
768 and 1440 dp, short landscape, safe areas and 200% zoom/text scaling.
Controls wrap without shortening targets below 44 × 44 dp; boards retain
aspect ratio while HUD text scales independently. No ornamental borders/
shadows; functional board lines, focus/selection/last-action marks remain.

First/Previous/Next/Last form related foundation controls with separate
accessible names, selected/busy/disabled reasons and visible focus; pictograms
alone are insufficient. Log rows expose current-position semantics without
stealing focus after seek. A real slider has a visible label, correct min/max
and localized value text including its counter unit. Range and keyboard
actions depend on the actual projected artifact, not sample “84” plies.

| Input | Required behavior when the runtime exists |
|---|---|
| Tab / Shift+Tab | Source order: return context, replay/viewer summary, board/description, transport, timeline, log and supported extension; no invisible inactive panel stops |
| Enter / Space | One activation per physical press on the focused control; Space toggles Play only in the appropriate transport scope |
| Left / Right | Previous/Next within transport/timeline scope; board arrows keep board inspection meaning; text inputs/extension fields retain native keys |
| Home / End | First/Last within transport/timeline; no interception of native text-editing shortcuts |
| Escape | Cancel active drag/seek/dialog, then close sheet and restore invoker; never resumes live commands or silently discards dirty branch |
| Pointer/touch | Down/up on the same enabled action; release outside/cancel/blur does not act; scrub has visible commit/cancel behavior |
| Browser Back / forward | Preserve history filter/row and validated local cursor; dirty branch policy applies; no automatic import, command resend or viewer widening |

Repeated keydown cannot launch concurrent seeks or duplicate branch/save.
Opening-key release is suppressed for a new dialog. Resize/theme/density
changes preserve valid focus/cursor and release geometry-dependent armed
gestures. All four schemes keep focus and timeline status legible; cursor/
branch state uses text/shape as well as semantic color. Reduced motion removes
ambient effects and retains immediate cursor, board and last-action feedback.

DOM shell controls use semantic buttons/range/list and announce completed
cursor changes politely with notation, not every drag pixel/frame. Errors and
paused/seeking/end state remain persistent text. The canvas requires the
real Board Reader status/actions bridge (Phase 5) and full region navigation
(Phase 9); a description string or headless snapshot is not screen-reader
playback proof. No-audio behavior preserves every event/status cue visually.

## Failure and verification cases

Use the [shared failure matrix](results-replay.md#compatibility-resources-and-failure-matrix).
Keep loading metadata, loading resource, validating artifact, seeking, paused,
playing, ended, stale/offline, unsupported, denied and corrupt states distinct.
Until a real adapter exists, present Unavailable with its prerequisite rather
than a fake loading bar, playable board or working scrubber. Imported canonical
data is refused by the user playback boundary. Unknown format/rules, missing
pack/migration and corruption keep safe result metadata where permitted;
never show an approximate/AI-reconstructed match.

The eventual supported slice compares reconstruction against real authority
fixtures at initial/end and intermediate checkpoints, including real rejected
attempt gaps without renumbering. Test repeated/reversed seek, no checkpoints,
timer/non-move transitions, concurrent stale/cancelled work, changed viewer,
forbidden Audit/canonical data, malformed/oversized input, resource/version
mismatch, immutable original after branch/save/discard and dirty Back.
Checkpoint/hash agreement and recorded terminal outcome are separate checks;
`Exact` identity, reconstructed prefix and full verification are distinct
claims. Hidden-information tests must exercise real secrets and event-existence
privacy. Add actual four-theme/compact/200%/keyboard/no-audio/reduced-motion
interaction and AT evidence when those consumers exist, then required repo
gates. Mock data, compile success and static designs cannot satisfy acceptance.
