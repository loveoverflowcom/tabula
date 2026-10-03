# Xiangqi: shared mode, evidence and resource contract

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53), specification
slice A. Source inventory: `develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`,
2026-10-03. Screens [08 Play](08-xiangqi.md),
[11 analysis extension](11-xiangqi-analysis.md), [12 Learn](12-learn.md) and
[14 Resources](14-resources.md) share this boundary, the
[foundation](foundation.md) and [generic replay](11-replay.md).
Execution and acceptance are recorded in [the ledger](xiangqi-verification.md).

## Authority, source and current gates

[Doc 00](../../architecture/00-architecture-principles.md) wins over sketches.
I-1/I-15 keep pure rules below I/O and renderers; I-5/I-6 allow only authorized
`View`/`ViewEvent` to clients; I-9 keeps game-specific meaning behind module/
registry dispatch; I-10/I-12 separate presentation and drafts from authority.
The [#48 ownership comment](https://github.com/loveoverflowcom/tabula/issues/48#issuecomment-5904880767)
assigns rules/state/board/replay, engine adapter and tutor to Tabula. Savy is
an optional knowledge/evidence provider. Neither Savy nor an LLM owns a
position or makes a move legal.

| Boundary at inventory | Actual source | Disposition for #53 |
|---|---|---|
| Xiangqi rules, projection, presenter, bot and manifest | No `games/xiangqi`; [doc 07](../../architecture/07-phases-and-implementation-roadmap.md) Phase 1 explicitly defers Xiangqi to Phase 9 | NOT_IMPLEMENTED; a mock board is not local play |
| Vision/ADR and No ML reconciliation | #48 remains open; [doc 02 §6](../../architecture/02-game-module-and-sdk-design.md) retains No ML in MVP; [doc 09 §3.3](../../architecture/09-synthesis-and-decision-register.md) defers ML bots | BLOCKED: first reconcile #48 by explicit ADR/roadmap; this spec does not accept an exception |
| Discovery/setup | `tabula-registry`, `apps/web`; [ADR-0028](../../adr/0028-discovery-shell-ahead-of-phase-gate.md) | Implemented exception only for its named slice; no Xiangqi entry or analyzer/resources route is enabled |
| Existing local play | `apps/game-client/src/lib.rs` (`LocalMatch`), Chess/Tiles presenters | Useful architecture/example consumers; they provide no Xiangqi reconstruction or engine adapter |
| Replay | `tabula-testkit/src/replay/`, [#52 contract](results-replay.md) | Canonical offline support tools exist; projected viewer/seek/scrub/speed remain Phase 9 |
| Resources | `tabula-assets/src/lib.rs`, `manifest.rs`, `integrity.rs` | Pure size/hash/manifest verification exists; no engine provisioning, rights, process, browser cache or OS loader |
| Online fair-play | Phase-4 match/protocol/net-client banners | NOT_IMPLEMENTED; UI permission copy cannot enforce a multiplayer policy |
| Engine/tutor | No approved host adapter, pack or real evidence consumer | NOT_IMPLEMENTED; no request job, score, PV, motif, install progress or fallback service |

Pack [05-xiangqi at `030da25`](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi)
supplies editable SVGs, two desktop PNGs and a mobile/state PNG. Those are
sample positions, clocks, arrows, counters and text. Desktop Learn/Resources
have SVG source only. The foundation's pinned
[Material mapping](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/01-foundation/shared/material-component-map.md)
is a reference; `tokens.toml` and generated adapters govern all four schemes.
Keep separate-document gameplay handoff (ADR-011); do not port SVG gameplay
into a Leptos board or embed Leptos into game-client.

## Modes and assistance policy

One proposed module identity, with modes; the actual game ID, ruleset,
repetition/perpetual-check policy and rules version must be decided in the
rules slice. Do not invent three game IDs or infer Xiangqi rules from Chess.
Mode is a context fact, not a UI toggle that can grant a permission.

| Context | Human/bot/engine play | Analyze and tiered hints | Enforcement owner when implemented |
|---|---|---|---|
| Local practice | Human after rules/presenter gate; baseline bot only with linked factory; engine opponent only with ready real adapter | Explicitly permitted practice scope, if each provider/resource is ready | Local practice driver validates every executed command with `apply`; host checks assistance policy before jobs |
| Online casual | Only authority-advertised participants/modes | Denied by default; allowed only by explicit shared match policy | Match/session authority authorizes both request and output disclosure, beyond button visibility |
| Ranked/tournament active | Only authorized seat commands | Denied, including tutor, book move hints and engine output | Real authority refuses assistance APIs/dispatch; delayed spectator embargo also applies |
| Authorized postgame/replay | Original read-only; no new live command | Permitted exact viewer scope after policy/resource checks | Replay permission plus supported reconstruction/branch and host adapter |
| Analysis branch | Branch commands after reconstruction gate | Allowed within this branch and permission scope | Separate local branch driver; original/live command sink unreachable |

This policy governs Tabula's assistance surfaces; it does not claim to stop
an external engine on an adversarial device. Online enforcement requires
real authority tests. Missing/unknown/expired permission fails closed,
clears accessible cached assistance and retires jobs before mode changes.
A user-selected URL, `practice` label or hidden button cannot grant rights.
Entering Analyze/Learn from an active original opens a permitted separate
branch; it never silently changes the match's assistance policy.

## Proposed adapter boundary, not implemented APIs

The following responsibilities and paths are proposals for separate gated
changes. No module, crate, trait, route or service is created here. Choose
concrete locations only after an approved adapter design and dependency review.

| Owner / proposed location | Input and output | Required trust boundary |
|---|---|---|
| Rules: `games/xiangqi/src/rules/` | SDK inputs → state/events/effects; `project`/`view_event` → permitted data | Pure, synchronous, deterministic and transactional on error; no engine/process/LLM/time I/O |
| Presenter: `games/xiangqi/src/presentation/` | Permitted view/events plus `Local` → RenderList/intent; module notation/board description | No canonical state, process handles, direct state mutation or duplicate replay controller |
| Registry composition: existing `tabula-registry` module bridge | Actual module identity, modes and availability reasons | Capability declaration alone does not enable a missing consumer; no platform `if xiangqi` branch |
| Branch: authorized local driver, location TBD | Exact permitted replay/live origin plus supported reconstruction → separate rules instance | No Audit/seed escalation to reconstruct missing data; only rules admit branch commands |
| Host analyzer: client/platform adapter, location TBD | Policy-scoped public position encoding + request identity + verified resources + budget → bounded evidence or typed failure | I/O outside rules; native process and web worker are independently gated; parser does not execute output |
| Resource host: platform adapter, location TBD | Approved artifact records + explicit user provision action → verified/probed availability | Hash, license, compatibility, budget and offline capability are separate verified facts |
| Tutor: game contribution + optional provider, location TBD | Current evidence IDs + allowed question/goal → structured hint or bounded explanation with references | No rules writes; no automatic Savy/LLM/cloud dependency; provider receives only scoped authorized data |
| Shell: future Leptos/native documents | Typed sanitized context/resource/evidence summaries and navigation | No engine process in a DOM component; native/DOM semantics adapt the same screen contract |

External asynchronous engine search must not be forced into pure deterministic
`GameBot::choose`. Preserve that baseline contract. Engine commands still
enter the usual validated command path; engine protocol/notation is never a
second rules authority. Engine search may vary across budgets/hardware;
do not promise bit-identical evidence as deterministic rules replay.

## Position and evidence identity

Every request, progress event, candidate, hint and explanation binds to the
following exact context. Human-readable labels may shorten identifiers, but
an inspect/copy detail exposes the complete approved public identity.

| Identity component | Meaning / owner |
|---|---|
| Game/package + rules version/hash | Linked rules/module identity; never guessed from a game name or format string |
| Session/match or replay artifact + viewer scope | Authorized origin, including artifact version/digest and scope generation; not a bearer credential |
| Original cursor / accepted state version | Exact origin position; preserve original `InputIndex` gaps internally, separate accepted transitions and permitted replay cursor |
| Branch ID + branch revision | Original versus each independent draft; same board arrangement in another branch is a different context |
| Position digest | Supported rules/adapter-approved position identity bound to the above; any client value is public-safe, never a generic canonical-secret `StateHash`/seed export |
| Request ID + generation | Unique host-owned job attempt; position/mode/permission/provider/budget changes and every explicit retry retire the previous generation |
| Provider/revision + binary and network/weights digest | Exact search resources; engine build and NNUE are independently identified |
| Requested and applied budget | Threads, memory/hash limit, time/node/depth limits and actual supported limits; show unsupported limits rather than claiming enforcement |

The host can verify an internal canonical digest where authorized, but clients
receive only an approved opaque/public-scoped position reference. Perfect
information does not waive I-5 or justify passing `State`/canonical `.tbr`
into a presenter, LLM or browser. Re-authorization cannot relabel cached bytes.
Changed viewer scope discards previous assistance, including accessible
text/history/citations; harmless historical evidence may remain only when
still authorized, explicitly pinned as historical with its original identity.

## Analysis lifecycle and recovery

| State / event | Observable behavior | Host obligation |
|---|---|---|
| Unavailable / denied | Readable prerequisite/reason, no score/PV; link to relevant Resources/policy detail if supported | No launch/queue request; unknown gate is unavailable |
| Ready, no evidence | One explicit Analyze action and applied budget summary | Capture current context atomically; no implicit job on tab/focus/position entry |
| Pending | Persistent current-position/job label, real progress when available, Cancel | One job for this generation; duplicate presses do not create duplicate workers |
| Streaming candidates | Label provisional evidence and actual reached budget | Validate identity and parse/bounds before every publish; a final response is not the first identity check |
| Completed | Finite-budget evaluation/PV with provider, source, score perspective and actual metrics | Verify context/policy/resources again; malformed/illegal output cannot become usable candidate evidence |
| Cancelled | Clear status, no success label, explicit Retry | Retire generation before transport stop; late events discarded even if process cancellation fails |
| Timed out / crash / parser failure / budget failure | Distinct safe error; original board retained; Retry after capability recheck | Release/terminate bounded work; no stale output merged into the next request |
| Position, branch, mode, cursor or resources change | Retire current job; show still-authorized old evidence as historical only, never current arrows/hints | Equality includes generation, not just board digest; returning to the same position cannot revive an old job |
| Leave / blur / suspend / restart | Cancel armed gestures and pending request; retain a draft only through a real safe persistence path | No auto-resume job or download on reentry; real probe/context validation then explicit Analyze |

No artificial percent, elapsed time, depth, CPU value or status spinner stands
in for an absent worker. An adapter can publish only the metrics it supplies;
show “not reported” for optional unknown metrics. Required identity/budget/
source omissions make output unusable. Retry is a new ID; bounded restart
cleanup cannot kill unrelated user processes. Cancellation is local logical
retirement first, transport termination second.

Candidates show score unit, perspective/side, bound (exact/lower/upper),
terminal/mate semantics if genuinely supported, PV and finite search budget.
Do not invent a numeric mate conversion, “accuracy” or globally best verdict.
Parse protocol coordinates via the module, validate each proposed PV command
in order against the separate branch's real `apply`, and fail the candidate
on an invalid prefix. Never execute the PV directly against the original.

Branch creation checks exact origin identity and reconstruction capability.
Candidate activation explicitly opens/selects that branch; branch steps may
be inspected with the shared replay/controller seam when it actually exists.
Draft dirty/Save/Discard/Cancel follows [generic replay](11-replay.md): Save
requires a real adapter; otherwise offer Discard/Cancel with the unsaved limit.
Failed save preserves draft/original; stale completions cannot revive a
discarded branch. A branch has no original rating/outcome/verification label.
Replay uses recorded artifacts when available and authorized; fresh analysis
is labeled new work and never rewrites historical evidence as replay.

## Learn and evidence strength

Structured/template hints are the no-LLM path after rules/evidence consumers
exist. Tier 1 names a supported observation/goal; tier 2 identifies a relevant
piece/line with evidence; tier 3 reveals a validated candidate/branch sequence.
Advancing tiers is an explicit user action. Without adequate current evidence,
say “Not enough evidence for this position” and explain the missing source;
only source-backed rule facts may be shown, labeled as such. No-engine is not
no-LLM: an installed permitted engine may support analysis without an LLM,
while a chat model cannot substitute for missing engine analysis.

Keep engine evidence and book evidence in distinct labeled groups. An engine
reference names candidate/job/position/branch, applied budget and validated PV;
a book reference names source title/author, provider + pack revision/digest,
page/section/location and permitted excerpt/link. Book citations alone do not
prove a motif in this position. A supported motif requires explicitly named
board/PV evidence; a score alone cannot prove why a move loses a rook.
An optional explanation identifies each supporting evidence reference,
preserves uncertainty and abstains from unsupported tactical certainty.
Provider linkage/format checks cannot certify explanation truth; the later
tutor change needs a real quality oracle and unsupported-claim fixtures.

Pending/cancelled/failed/stale questions follow the analysis identity lifecycle.
Changing position does not silently retarget an old conversation. References
open the exact still-permitted historical context or explain why unavailable;
they never launch a job or apply a move. No question/context leaves the device
without a configured, explicitly chosen provider and permission. Offline or
missing optional provider retains usable engine/template capabilities; there
is no automatic cloud fallback.

## Resource readiness and platform matrix

Resource readiness is conjunctive: approved provenance and usage/distribution
rights for each exact artifact, matching digests/sizes, engine/weights pairing,
host/rules/protocol compatibility, supported enforceable budget and successful
probe on this platform. `tabula-assets` byte verification proves integrity
only. Its existing asset schema does not encode engine-license, CPU/RAM or
process lifecycle policy; no engine fields are grafted into it by this spec.
SDK `apply_budget` is rules observability, not engine resource enforcement.

| Capability | This Mac/native at pinned base | Web/WASM at pinned base | Gate for a future available claim |
|---|---|---|---|
| Xiangqi human local / baseline bot | Unavailable | Unavailable | Reconciled #48 + real rules/projection/presenter + actual factory/runtime tests |
| Xiangqi replay / branch | Unavailable | Unavailable | Authorized compatible projected playback/reconstruction consumer; #52/Phase 9 gate |
| External engine opponent / Analyze | Unavailable, no subprocess adapter | Unavailable, no supported worker/engine build | Separate approved adapter, exact licensed artifacts, limits + probe + legal/stale/failure integration tests |
| Template hints / Learn | Unavailable, no Xiangqi evidence consumer | Unavailable | Real allowed-position evidence and quality tests; no LLM requirement |
| Optional LLM / Savy books | Unavailable/unconfigured | Unavailable/unconfigured | Explicit provider, privacy/licensed resource scope, citations and failure/quality evidence |
| Install / remove / switch pack | Unavailable | Unavailable | Real provider/provisioner and verified rights/integrity/lifecycle/platform behavior |
| Native offline after provisioning | Not certified | No parity claim | Disconnect network and run each advertised capability with provisioned valid packs |
| Online fair-play / recovery | Not implemented | Not implemented | Real Phase-4 authority and adversarial policy/reconnect tests |

Mobile/other OS/CPU architectures are separately unavailable until tested;
native Mac compilation cannot enable them. Web cannot launch a native UCI
subprocess. Unknown worker/WASM/SIMD/thread/cross-origin prerequisites keep
the engine unavailable, never trigger a cloud switch. A configured remote
provider is online-only and explicitly labeled; it is not native offline parity.

Resources expose provider/source, engine revision, binary target/hash/size/
license and source obligation, separate weights revision/hash/size/license/
permission record, adapter/protocol/rules compatibility, offline scope and
CPU/thread/RAM/storage policy. No actual pack was chosen or approved here.
The [Pikafish README](https://github.com/official-pikafish/Pikafish#terms-of-use)
currently describes GPLv3 code; the separate
[Networks terms](https://github.com/official-pikafish/Networks#nnue-license)
restrict commercial use of Pikafish weights without permission (checked
2026-10-03). This source review is not a rights grant for any exact artifact.
The future pack must pin the exact binary/weights and reviewed terms/permission;
code licensing and subprocess placement cannot infer weights rights.

Missing, corrupt, incompatible, unlicensed, unsupported and unavailable remain
distinct resource states. Provision only on explicit supported user action;
show real bytes then integrity/rights/compatibility/probe stages. Cancel/failed
provision leaves the previous verified pack active or none; partial bytes are
never Ready. Switching/removing active resources retires jobs before replacement;
failed replacement cannot silently change provider. Retry creates a new bounded
attempt. No automatic download, bundle, re-provision or cloud fallback occurs.

## Implementation acceptance after gates

Slice B first needs #48's recorded resolution and a real SDK Xiangqi slice:
maintained rule/information model, legal/rejected/terminal cases, transactional
context/state, conformance, projection/security, deterministic replay/goldens,
notation/description and native/WASM consumers. Select the ruleset before UI
tests claim move legality. Then test one authority across Play/Analyze/Learn,
branch isolation and mode permissions, with genuinely unavailable engine/LLM.

Slice C adds one approved adapter per reviewed change. Keep doubles separate
from real engine integration: same/different position and branch, reversed
cursor to same digest, resource/permission/provider/budget changes, duplicate
activation, late progress/final response, cancel/timeout/crash/parser errors,
restart/leave/blur and successful explicit retry. Assert original bytes/events/
outcome unchanged; illegal output rejected; no denied worker/output/cached
hint disclosure; all wrong-identity results ignored before rendering. Test
every resource failure and old-pack preservation with real integrity/probe
controls, and advertised offline behavior with network removed.

Future UI QA needs actual four-scheme rendered states, 320/390/768/1440 dp,
short landscape, 200% text/zoom, keyboard/focus/restoration, repeat/interrupt,
pointer/touch cancellation, reduced motion/no audio and Board Reader/AT. The
mobile artwork's small board spacing and missing third mode do not waive
44 dp targets or accessible Learn/Analyze navigation. Keep focus/selection/
error/grid strokes, remove decorative borders/shadows and use the existing
foundation components/tokens. i18n keys cover en/vi reasons/status/plurals;
notation and score perspective come from the module, not UI string assembly.
Required gates follow focused tests; no goldens are regenerated to hide failure.
