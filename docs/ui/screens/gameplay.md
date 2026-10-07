# Gameplay contracts — screens 04–07

Stage A of [issue #51](https://github.com/loveoverflowcom/tabula/issues/51).
Source review base: `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188`.
Design provenance: `030da25d0098e240ab2cf36dacf9892e8b320a89`,
[`03-gameplay`](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/03-gameplay);
the pack itself audited `44f6b74e07648abc7191363d7582efc1fceab262`.
Rechecking source, rather than inheriting the pack's availability labels, is required.

Doc 00, doc 02 §7, doc 04 §3–§5/§7–§10, doc 05 and doc 07 govern these
specifications. [Foundation](foundation.md) owns shared component treatment;
[verification](gameplay-verification.md) separates specification from executed evidence.

## Ownership and gates

| Surface at the review base | Actual owner / availability | Gate for the remaining work |
|---|---|---|
| [04 — Chess](04-chess.md) | `games/chess/src/presentation/mod.rs`; real local pointer/keyboard presenter | Existing Phase-2 presenter may improve now; networking stays Phase 4 |
| [05 — Caro](05-caro.md) | `games/caro/src/lib.rs` is a design placeholder; no rules, module or presenter | Phase-2 exit, then Phase-3 game implementation and variant decision |
| [06 — Tiles](06-tiles.md) | `games/tiles/src/presentation.rs`; real local rules, presenter and bots | Existing Phase-3 presenter may improve now; network/async operations stay Phase 4+ |
| [07 — Werewolf](07-werewolf.md) | Validated config, canonical state/events and initial assignment only; no complete reducer, `View` or presenter | Complete Phase-3 rules/security before Phase 4; presentation/social/online Phase 7, voice Phase 8 |
| Runtime loader / connect / resume | `tabula-assets` supplies asset contracts; `tabula-net-client`, protocol, match and server remain Phase-4 scaffolds | Real delivery/session consumers and Phase-4 gate; no fabricated progress or connection |
| Board Reader DOM status and actions | Presenters expose descriptions, but game-client has no DOM mirror or action dispatcher | Phase 5 after Phase-4 exit; full region navigation is Phase 9 |

[ADR-0028](../../adr/0028-discovery-shell-ahead-of-phase-gate.md) authorizes
only discovery/setup. It does not authorize gameplay handoff, match creation,
networking, resume, asset delivery, or Board Reader ahead of their gates.
Caro's missing implementation and Werewolf's creation-only slice mean the
Phase-3 exit has not been demonstrated. These specs do not certify a phase exit.

## Entry, exit and authority

On web, `/play/:match_id` is a separate document booting the Macroquad gameplay
bundle (ADR-011). The Leptos catalog's rail, account menu and future result/history
routes in the artwork are shell context, not a request to recreate them inside
the game canvas or import Leptos into game-client (I-15). Native swaps a scene
with the same future `MatchContext`; it does not open an embedded web shell.

The future handoff pins match identity, game/package/rules version, compatible
pack and authorized viewer; a short-lived join token is read from the designated
session context, never printed into feedback. Direct entry validates context
before enabling commands. Missing/expired/incompatible context has an explicit
reason and recovery action. Cancel returns to the invoking shell context;
late loader/session completion after Cancel must not navigate or unlock input.
At this base, CLI local startup is the implemented entry; there is no successful
catalog-to-created-match handoff. Ended projection may show a compact result;
full result, history and rematch remain their owning shell/phase work.

The observable input path is:

```text
normalized InputEvent → GamePresentation::on_input(View, Local) → Intent<Command>
  → local authority: Input::Player { seat, command } → GameRules::apply
  → project + view_event → View / ViewEvent → presenter → RenderList → renderer
```

For online play the future session adapter sends the command through the
ordered/idempotent platform path; the server creates `Input::Player`.
Neither pixels nor an Intent constitute acceptance. Timer, seat and admin
inputs originate in the shell, never in a canvas action pretending to be one.

| Data | Owner and update rule | Forbidden substitution |
|---|---|---|
| Authoritative `View` | `project`, or a compatible projected update/fold; local driver re-projects after accepted rules input | Canonical `State`, guessed opponent secrets, merged optimistic state (I-5) |
| `ViewEvent` | Every event passes `view_event` for the authorized viewer, including omission of its existence (I-6) | Direct canonical events or an event-count cue from a hidden action |
| `Local` | Presenter: focus, hover, selection, drag, camera, preview rotation, modal choice, animation | Any upstream authoritative data or a rules mutation (I-10) |
| Pending command / preview | Separate client adapter value scoped to the command and view revision (I-12); optional preview only if declared safe | Writing predicted placement, turn, score, clock or outcome into `View` |
| Clock display | Derived from the authoritative checkpoint and presentation time, with saturation | Declaring timeout or ending a match because a local display reached zero |

The current local executor is synchronous; it has no network pending queue.
A bounded local busy/pending indicator may describe real command processing,
but an online acknowledgement, reconnect or resend must not be simulated.
Pending must be cleared by the actual acceptance/rejection adapter, never by
animation completion. Reject discards only that preview, preserves the last
projection and useful input context, shows a readable reason, and allows a
corrected action. Do not automatically retry a rejected game command.

## Shared runtime state matrix

This is a required future adapter contract unless the availability column says
local. Names follow doc 04 §4; the scaffold's names are not executable states.

| State / authoritative trigger | Board and command affordances | Feedback and allowed recovery | Availability at review base |
|---|---|---|---|
| Preparing module / required pack | No gameplay activation before compatible resources and initial projection | Stage name; real byte/resource progress with a known total, otherwise indeterminate text; Cancel | Delivery/boot adapter NOT_IMPLEMENTED |
| Required manifest, integrity or resource failure | Keep input locked; no substitute required pack | Persistent reason and safe error code; Retry the same compatible resource if meaningful, or leave | Asset contracts exist; visible loader binding NOT_IMPLEMENTED |
| Optional cosmetic unavailable | Compatible fallback art; gameplay remains usable only when required resources passed | Explain degraded art without a false success/failure banner | Consumer/fallback evidence required |
| Connecting / attach | Await authorized welcome; no speculative board ownership | Connecting status, Cancel/leave; no fabricated fraction | Phase 4 NOT_IMPLEMENTED |
| Ready / actual projected welcome | Commands gated by viewer, turn/phase, status and adapter readiness | Clear connection warning after readiness is real; restore reconciled focus | Local ready exists; online NOT_IMPLEMENTED |
| Pending command | Last authoritative board plus separately typed preview or sending label; prevent duplicate activation | Retain action label and context; acceptance/rejection from actual adapter | No network queue at this base |
| Local rejected / future `Reject` | Projection unchanged; transient preview removed; preserve selection when still valid | Inline/persistent nonmodal reason; semantic invalid feedback and readable no-audio equivalent | Local error returned; visible UI was stderr-only at this base |
| Reconnecting / socket close or error | Last authorized projection marked stale; freeze commands; keep safe camera/inspection controls | Persistent connection pill near turn; actual bounded retry/backoff, then Retry/leave; never convert to local | Phase 4 NOT_IMPLEMENTED |
| Resuming / `ResumeOk` | Stay locked while ordered redacted updates reconcile | Confirm actual cursor/ack reconciliation before readiness; same unacked command identity only | Phase 4 NOT_IMPLEMENTED |
| Resyncing / gap or `Resync` | Stay locked; discard pending/animations and stale drag/modal state; replace with authorized projection | Brief resynced status only after replacement; restore a valid focus target | Phase 4 NOT_IMPLEMENTED |
| Failed / retry exhausted or fatal local error | Frozen last projection when available; block commands; local fatal must remain visibly rendered | Persistent specific reason, Retry only when recovery exists, and leave; stderr is supporting diagnostics | Local failure type exists; visible binding was missing at this base |
| Ended / projected terminal status | No gameplay inputs; safe board inspection remains | Outcome from projection, labeled next action only when its adapter exists | Local terminal HUD exists; result/rematch routes gated |
| Unavailable / unsupported game, viewer, version or capability | No action that implies a successful match | Name unmet prerequisite and supported next action; no blank board or fake retry | Current discovery reasons exist; no online runtime |

After leave, cancel, blur or resync, release armed pointer/key activation.
Repeated Enter/Space or pointer-up without its matching press must not
submit twice. Stale asynchronous results are scoped to the active match/session
revision; a late result for a retired context is discarded. No modal appears
for each short network interruption. No toast is the only record of failure.

## Shared layout, focus and accessibility

Use `RenderList`, generated `Theme` and foundation widgets. Board is the first
large region; HUD uses tonal containment, purposeful shape and labeled primary
versus quieter contextual actions. Preserve board lines and functional focus,
selection, legal-target, threat and last-action markings; omit decorative
borders/shadows. Never copy prototype literals into production tokens.

At 320/390 logical dp, board and HUD reflow independently; commands retain
≥44 × 44 dp targets and remain reachable above safe-area insets. At 768 dp
use a board plus bounded HUD; at 1440 dp cap board size with readable ancillary
content. At 200% text/accessibility scaling, wrap/reflow HUD labels without
shrinking type or targets. Short landscape layouts require a reachable compact
HUD rather than silently omitting commands. These are acceptance targets,
not claims that the current layouts satisfy every width/scale.

Resolve `light`, `dark`, `hc-light` or `hc-dark` atomically. Theme/density and
reduced-motion changes preserve logical selection/focus and actual pending
identity. Turn/team/legality combine color with text, shape or glyph. Clocks
use mono tabular figures; their digits never roll or pulse. Event-driven
animation is interruptible, never gates input, and converges to the same final
view under sparse/dense sampling and reduced motion (I-10).

Canvas focus graph/input is the current keyboard mechanism. Each game's
`GamePresentation::a11y(View, Local)` is its transient description boundary;
the helper names `chess_a11y`/`describe` do not add a new public API. A future
DOM/native mirror consumes the same authorized facts and local controls.
Focus is separate from selection; modal entry/cancel returns to its invoker
or a valid successor, and blocked actions retain a readable reason.

Phase-5 Board Reader status/actions need a real `ActionId → Intent` dispatcher
with stale-view, disabled, invalid and once-only activation checks. Descriptions
and `ActionId` strings alone do not make keyboard/AT actions executable. Full
regions remain Phase 9 even when Chess currently emits region data. Never label
the catalog "accessible" from source descriptions or headless snapshots.
The current web bootstrap disables user zoom and has no DOM mirror; those
are explicit acceptance gaps, recorded in the verification ledger.
