# Results, history and replay — shared boundary

Stage A of [issue #52](https://github.com/loveoverflowcom/tabula/issues/52).
Source review base: `develop @ eb7803b8a304f2be4ae9890ea6484dab02688540`.
Design provenance: `030da25d0098e240ab2cf36dacf9892e8b320a89`,
[`04-replay`](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/04-replay).
The pack's implementation notes audited `44f6b74e07648abc7191363d7582efc1fceab262`;
its sample results, people, times, move counts and positions are not runtime facts.
All four SVG sources and the actual result, history and mobile PNGs were reviewed.
The pack has no desktop replay PNG; its mobile PNG includes the replay view.

Doc 00 I-5/I-6/I-8/I-9/I-10/I-12/I-16, doc 04 §2–§4/§7–§10,
doc 05 §7–§10 and doc 07 govern this contract. [Foundation](foundation.md)
owns shared components/tokens and [gameplay](gameplay.md) owns the live input
boundary. Screen contracts are [09 results](09-results.md),
[10 history](10-history.md) and [11 replay](11-replay.md).
This specification records intended behavior; it is not executed UI evidence.

## Ownership and phase gates

| Surface at the review base | Actual owner / availability | Remaining gate |
|---|---|---|
| Local terminal result | `LocalMatch::ended()` observes rules `Effect::EndMatch`; game presenters receive projected terminal status | A small honest local result/next-game consumer may improve the existing driver; no persistence or online claim |
| Local failure / restart | `runtime_ui::LocalFeedback` freezes a failed session and can request a fresh local match | Failure is separate from rules outcome; restarting is never resume or rematch |
| Accepted canonical evidence | `replay_capture::LocalReplayTrace` records accepted inputs and checkpoint hashes in memory | No file, user export, history or projected playback consumer exists |
| Canonical `.tbr` tooling | `tabula-testkit::replay`, typed `ReplayRunner<R>` and `xtask replay` for Chess/Tiles | Offline verifier/support tooling; not a client replay viewer or authorized user download |
| Online result persistence / replay generation | Protocol, match runtime, storage and server are Phase-4 scaffolds | Phase 3 exit, then real ordered/persisted authority and projection/security integration evidence |
| Result/history shell | `/matches/:id` and history at `/u/:handle` are future doc-04 routes | Phase 4 exit before Phase 5; real session, permissions and typed result/history service |
| Projected replay viewer | No normal presenter playback/fold/seek controller | Phase 9: projected replay, scrub, speed, viewer redaction and replay golden evidence |
| Full Board Reader / branching extensions | No replay AT dispatcher, complete regions or analysis branch consumer | Board Reader status/actions Phase 5; full regions and replay viewer Phase 9; branch needs its own compatible authorized adapter |

[ADR-0028](../../adr/0028-discovery-shell-ahead-of-phase-gate.md) opens only
discovery/setup. It explicitly leaves `/matches`, `/u`, protocol, storage and
online operations gated. Neither the `.tbr` verifier nor a static history
preview proves those gates. Phase 4's replay/reconstruction tests establish
authority; they do not advance the Phase-9 client scrub controller.

## Outcome is a rules fact; failure is a shell fact

The rules own end meaning. The platform owns persistence, authorization,
ratings and delivery. A result consumer receives a public-safe terminal
summary plus permitted roster/standing facts from the authority adapter.
Online clients consume projected `View`/`ViewEvent` or a permission-checked
result response, never canonical state or an unredacted rules event. Local
authority may derive the same safe summary in-process, while the presenter
continues to receive only projected data (I-5/I-6).

[`MatchOutcome`](../../../crates/tabula-core/src/outcome.rs) has `Decisive`,
`Draw` and `Aborted` kinds, standings and a public-safe summary/i18n key.
For non-aborted results, standings cover the authoritative roster once with
contiguous ranks starting at zero; tied ranks and game-defined scores are
valid. Deserialization checks intrinsic structure, but a persisted outcome
must also `validate_against` the separately authoritative roster. A snapshot
cannot prove its own roster. Aborted outcomes have no standings and grant no
winner/rating claim. Checkmate, resignation and timeout are game reason
labels; the UI never infers them from board appearance or its displayed clock.

Keep these independent axes:

| Observed fact | Display / recovery rule |
|---|---|
| Valid terminal outcome, successful delivery | Show that outcome and its supported next actions |
| Valid terminal outcome, later save/export/network failure | Retain outcome; show the separate failed operation and real Retry when available |
| Fatal local timer, renderer, input limit or conflicting end effects | Show “Local session stopped” with the safe reason and fresh-game/leave action; never synthesize `Aborted`, defeat or completed history |
| Known valid outcome before an unrelated later shell failure | Preserve the known outcome separately; do not let the failure overwrite it or claim persistence |
| Conflicting end effects / otherwise ambiguous authority | Do not choose a winner from the first effect or a stale projection; report authority failure until a trusted outcome exists |
| Last accepted checkpoint but no terminal outcome | Evidence of a transition, not proof that the match finished |

`LocalMatch::apply_canonical` records acceptance before view-event dispatch
and effect interpretation. Therefore an accepted trace may survive a later
shell error. Result completion must not be inferred from trace length,
renderer success, animation completion or a restart request. A new local game
creates fresh canonical state and resets old feedback/presenter state; it
does not revise the previous result or silently retry its rejected input.

## Authority facts and client-local state

The following names describe a future adapter contract, not new wire structs.
Implementing a wire representation still requires Phase-4 ownership, codec
limits/versioning and I-13 evidence.

| Facts supplied by their owner | Required scope / use |
|---|---|
| Identity | Match or local-session identity, full registry game identifier, recorded game/rules version/hash and format version; display names/notation arrive as module data |
| Terminal summary | Outcome kind, permitted reason, validated standings and safe seat labels; distinguish local volatile receipt from stored result |
| Time / counters | Recorded start/duration and expressly named unit; missing values are absent, not zero or estimated sample values |
| Replay availability | Real artifact identity, kind, authorized viewer, compatibility/resource disposition, integrity state and supported operations with reason |
| Presentation data | Permitted initial `View`, viewer-scoped `ViewEvent`s and projected checkpoints, plus an authorized timeline/notation adapter |
| Local UI state | Search/filter/sort, focus/selection, scroll, playback cursor/speed, request generation, camera, animations and separate branch draft; never original authority |

No result/history consumer computes Elo, accuracy, rewards, streaks or engine
quality from standings or moves. Ratings require their real platform provider.
No generic shell/controller branches on a game identifier. Registry/module
data supplies game names, board geometry, seat/team labels, notation and
available extensions (I-9).

## Canonical evidence and projected replay are different boundaries

Canonical `.tbr` contains seed and full accepted inputs, including timer,
seat/admin and hidden-information commands. It stays in authorized server
audit/support tooling. `Viewer::Audit` is internal and never selectable by a
game client, imported file, URL parameter or spectator control. Canonical
local capture is not automatically safe to download because it was produced
on a user's machine. Removing a seed alone is insufficient: inputs, config,
roster, snapshots and metadata can disclose secrets too.

A user replay contains the permitted initial `View` and that viewer's
`ViewEvent` stream with no seed (doc 05 §8.2). Every event is redacted through
`view_event` for the authorized viewer at its state/phase. Hidden event
existence remains hidden: no placeholder rows, canonical event counts,
private phase markers, gaps or timing cues that reveal omitted actions.
Viewer changes require a newly authorized artifact; relabeling cached bytes
cannot widen access. Ending a match grants no automatic reveal permission.
Seat names/identifiers are pseudonymized unless the owner policy grants them.

Structural parsing, CRC/checkpoints, a `Projected { viewer }` header and a
rules-hash match establish different facts. None alone proves who may read
the file or that its opaque event bytes are truly redacted. In particular,
`ValidatedReplay` supports a projected container but `ReplayRunner<R>` rejects
projected playback; no implemented client fold or viewer authorization follows
from that container support.

## Cursor and accepted-index discipline

Keep cursor domains explicit. Canonical `InputIndex` is the original attempt
ordinal and RNG-domain input. Accepted-transition `StateVersion` counts only
accepted transitions. A user replay cursor counts its permitted playback
positions; a game's “ply”, turn, round or tile action is a separate module
notation/counter mapping. These are not interchangeable integers.

`RecordedInput` logs every attempt with contiguous indices; `LocalReplayTrace`
records only accepted inputs and must retain their original, possibly gapped
`InputIndex`. For example, rejected attempt 1 followed by accepted attempts 2
and 3 yields two accepted transitions at indices `[2, 3]`. Re-running with
`[1, 2]` changes `DetRng::for_input(seed, index)`. Never fill gaps with invented
frames, compact indices or use accepted count as the last canonical index.

**Canonical tooling correction in #52:** the reviewed base's
`tabula-testkit/src/replay.rs::validate_structure` required `.tbr` frame indices
to be contiguous from one, which rejected a valid gapped local trace. The
correct contract accepts strictly increasing nonzero original `InputIndex`
values, including a first accepted index greater than one; accepted-frame
count remains the `StateVersion`/seek/trailer domain. Runner RNG uses each
stored original index, and diagnostics locate real input IDs while using
accepted-frame adjacency for evidence strength. This corrects existing
canonical tooling without implementing trace export, projected playback or
history. Reader/writer/version compatibility and gap/RNG/diagnostic evidence
belong to doc 05 and ADR-026; contiguous golden fixtures alone cannot establish
the corrected cases. Never work around the mismatch by renumbering.

For future playback, cursor zero is the permitted initial view and end is the
last permitted position. Seek validates range, restores the nearest compatible
**projected** checkpoint at or before the target, folds the exact permitted
events in order and atomically publishes the resulting view/cursor. No
checkpoint means re-fold from the initial view. Seeking neither enters the
live command path nor mutates the recorded original. Sparse evidence supports
only its actual checkpoint/window coverage: reconstructed position is not
automatically verified prefix or verified full match (doc 05 §7.3/§8.3).

## Compatibility, resources and failure matrix

| State / source | Result/history behavior | Replay and recovery |
|---|---|---|
| Loading real metadata/artifact | Named stage; known-shape skeleton; preserve navigation/context | No fabricated percent; cancellable work; no board until validated permitted initial data |
| Empty eligible history | Explain actual storage source; distinguish filters from no stored matches | Start supported new game, reset filters, or import only if a real importer exists |
| Busy save/export/import/seek | Retain prior result/view; label pending operation | Once-only submission; disable dependent actions; Cancel where adapter supports it |
| Save/export fails | Keep known outcome and volatile/stored distinction | Retry the same operation only through its real adapter; no success toast before completion |
| Offline / stale cache | Label cached result/history and last refresh if known | Cached projected playback only when supported and still authorized; no implicit resume/new authority |
| Unauthorized / expired grant | Safe denial; no private metadata/board leak | Re-authenticate/re-request access when meaningful; clear now-forbidden cached data and retire pending work |
| Match missing / replay expired | Distinguish missing result from unavailable replay if policy allows | Retain authorized result; explain retention/unavailable replay; return to history |
| Corrupt / truncated / oversized / invalid frame order | Do not promote imported metadata to a stored result | Fail closed before allocation/fold/activation; show safe error and choose a different file |
| Exact linked rules identity | May display recorded disposition | Integrity/projection checks still required; matching hash alone is no reconstruction proof |
| Same rules version, different hash | Label compatibility disposition | Verify against recorded evidence; divergence stops playback; never label `Exact` |
| Migration required | Preserve authorized stored outcome | Use only a real supported migration; mark migrated output, preserve original; no approximate reconstruction |
| Unknown game/rules/format or absent migration | Show safe metadata/stored outcome when permitted | Explicit unsupported reason; update client/return; no AI-generated or best-guess board |
| Required pack incompatible/missing/integrity failure | Metadata/result remain readable | Retry compatible resource or leave; required artwork cannot be replaced with a false successful board |
| Optional cosmetic unavailable | Explain degraded visuals if useful | Compatible semantic fallback only; preserves board labels, focus and permissions |
| Reconstruction/checkpoint/terminal mismatch | Keep result as separately sourced; display replay failure | Stop, retain last verified permitted position if safe; diagnostic identifies only evidence-supported location |
| Cancel / leave / changed match, viewer or rules | Release armed input and retire request generation | Late responses cannot navigate, replace the board, reopen dialogs or broaden permissions |

Current `.tbr` tools bound compressed/decompressed data at 4 MiB, header at
128 KiB, frame/config at 64 KiB and frames at 100,000. These are source facts
about that parser, not authorization for a browser importer or a new wire
limit. A future importer must use a supported real parser, enforce relevant
compressed/decompressed/allocation limits before playback, validate checksum,
identity/kind/ordering and permissions, and expose only sanitized metadata.
Export names its actual format/version and projected viewer scope; cancel,
failure and persistence completion have real adapter outcomes.

## Shared presentation and verification targets

Use generated `Theme`/CSS from `tokens.toml`: four atomic schemes `light`,
`dark`, `hc-light`, `hc-dark`; tonal containers, purposeful shapes, sans task
titles, stable mono counters and filled primary versus tonal/quiet routine
actions. Functional board lines, focus rings, off-control boundaries and
validation strokes remain. Artwork palette proposals and sample purple fills
do not become a parallel token source. No decorative card borders/shadows.

Verify 320/390, 768 and 1440 logical dp plus short landscape and 200% text/zoom.
Reflow controls/labels; every activation target remains at least 44 × 44 dp
with safe-area/focus clearance. All result kinds and errors have text/glyph
identity, never color or audio alone. Reduced motion preserves current
cursor, last action, status and once-only input; animation completion never
commits a result, controls seek order or grants authority (I-10).

The future supported slice must compare real reconstruction/checkpoints and
terminal outcome with an authority fixture, exercise cursor zero/end,
repeated/reversed seek, cancellation/stale completion, malformed/oversized
data, incompatible resources and forbidden viewers. Hidden-information
coverage needs actual secret changes, unauthorized outputs/events and an
observability control; a containment scan alone is insufficient. Confirm
non-empty test selection, then run required repository gates. Record
`PASS`, `FAIL`, `BLOCKED`, `NOT_RUN` or `NOT_IMPLEMENTED` with evidence kind.
Source review, mock UI, RenderList assertions and compilation do not establish
real rendered UI, screen-reader interaction or Phase-4/5/9 exit.
