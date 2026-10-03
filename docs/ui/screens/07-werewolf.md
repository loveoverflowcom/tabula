# 07 — Werewolf gameplay (future-gated)

Issue #51; shared [gameplay contract](gameplay.md) and [foundation](foundation.md).
Reviewed `develop @ 1d8fab294931750f45ef5d7b498f34b5b0417188` and pinned
`03-gameplay/screens/07-werewolf.svg` source. This companion sheet includes
the same Caro/private-role/recovery regions as screen 05. Its private role
card and public phase/vote panel describe intended UI, not supplied gameplay.

## Gate and source authority

[`games/werewolf/src/rules`](../../../games/werewolf/src/rules/mod.rs) supplies
validated configuration, role/state/event types and deterministic initial
assignment. It has no complete `GameRules`/`GameModule`, command reducer,
per-viewer `View`/projection, security/conformance fixture or presenter at
this base. W2 initial state is canonical and must never be given to a UI as
a temporary projection. The artwork's "Rules/UI stub" label must not imply
that a complete headless game or redaction already exists.

Complete Phase-3 rules/projection/security validation before Phase 4 freezes
the protocol. Werewolf presentation, social and online UX wait for Phase 7
after Phases 4/5; voice waits for Phase 8. A test-only terminal projection
viewer is Phase-3 verification tooling, not a gameplay presenter. The
maintained [Werewolf decisions and knowledge matrix](../../games/werewolf.md)
own role, phase, vote, lifecycle and secrecy semantics; this document adds
no game-rule variant or host power.

## Private view and local interaction

The future presenter receives only authorized `View` and `ViewEvent`.
Private role display, selected target, open role card and reveal animation
are local UI; the role itself comes from the viewer's permitted projection.
Public roster/liveness, phase/deadline, revealed roles and ballots come from
that same boundary. The UI never asks canonical state for another role.

| Viewer / fact | Future disclosure contract |
|---|---|
| Living seated player | Own role plus authorized role knowledge; public roster/phase/ballots; own private choices/reports only where W-D decisions permit |
| Living wolf | Authorized teammate identity; no live teammate target or private-submission-progress indicator |
| Dead `Viewer::Seat` | Permitted full vision from completed death transition; no gameplay commands |
| Outside `Viewer::Spectator` | Public-only facts, with roles revealed only by permitted death/end transitions |
| Internal `Viewer::Audit` | Never a reachable gameplay viewer |
| Unauthorized private night action | `view_event → None`, including its existence; no generic "someone acted" cue |

Do not use absence of a death to announce a save; do not show private ready
counts, pending checks or timing cues for other players. Role hide/reveal
changes local visibility only and cannot broaden knowledge. Clear private
UI when the authorized viewer/session changes; a stale role card must not
remain behind a public reconnect or spectator screen.

## Intended action mapping

The following rows use maintained game concepts, not current command APIs.

| Intended UI action | Future local / authority path | Current status |
|---|---|---|
| View/hide own role | Local card expansion and focus restoration from permitted role fact | Presenter NOT_IMPLEMENTED |
| Select night target or pass | Local target, then game-owned night command in `Intent` → `Input::Player`; role/phase/alive/deadline gates | Reducer/projection NOT_IMPLEMENTED |
| Cast/replace public ballot, abstain or unvote | Game-owned ballot intent; authority confirms resulting public view | Reducer/projection NOT_IMPLEMENTED |
| Inspect public roster/revealed roles | Focus/description only | Presenter/mirror NOT_IMPLEMENTED |
| Chat send | Platform transport with game-provided read/write scopes, socket enforcement | Phase 7 NOT_IMPLEMENTED |
| Voice join/mute/listen | Platform/SFU enforcement of authorized voice scopes | Phase 8 NOT_IMPLEMENTED |
| Reconnect/leave | Session adapter; phase timers continue per rules, no local pause or substitute occupant | Phase 4/7 NOT_IMPLEMENTED |

No speech/Ready/host command is added solely because a doc-02 illustrative
sketch mentions it. Match controls must match the eventual implemented
module contract. Substitution remains forbidden; a disconnected seat's
missing choice defaults under W-D8, not a client-chosen automatic death.

## Layout, descriptions and acceptance

Use a compact phase/status region, public seat roster with readable living/
dead/ballot labels and a separately contained private role/target region.
Avoid a persistent large hero that displaces voting or role actions. Compact
portrait places legal primary actions within reach with ≥44 dp targets;
20-seat rosters wrap or scroll without exposing hidden data. Phase motion is
skippable and never gates command submission; reduced motion keeps phase,
deadline and last public outcome clear. A displayed countdown estimates an
authoritative deadline and never advances the phase by itself.

Future `a11y(View, Local)` must name allowed phase, roster, own role/selected
target, enabled legal actions and scope reasons without private cross-seat
data. Native/DOM privacy must be reviewed as carefully as the canvas: hidden
role text is still exposed if it remains in an unauthorized accessibility
tree. Board Reader status/actions need the real dispatcher; full regions
remain Phase 9 and voice is not required for an accessible text path.

Before presentation/online acceptance, exercise living seats by role, dead
seats and outsiders through reachable phase transitions, explicit private
event omission, viewer replacement, late pending/reconnect/resync, vote
replacement and deadline races. Socket-level chat tests and a second-engineer
leak review are separate Phase-7 obligations. The canonical version/ack
metadata side channel and asymmetric voice permissions in the maintained
model require platform ADR resolution at their respective gates. A private
card screenshot cannot settle either. Today all Werewolf UI execution is
NOT_IMPLEMENTED; see [the ledger](gameplay-verification.md).
