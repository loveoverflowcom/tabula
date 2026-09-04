# Goal

Record Phase-7/8 obligations without using them to expand Phase 3. No work in this document is
part of the current planning task's implementation scope.

## Current state

Werewolf has no presentation. Registry/match/protocol/voice are future-phase skeletons. ChatScopes
has distinct speak/listen participant sets; VoiceRoom has only members. The latter cannot express
the dead listening to living wolves without speaking to them.

## Decisions and scope

| Stage | Prerequisites | Work | Explicit exclusions |
|---|---|---|---|
| Phase-4 generic prerequisite | Phase-3 contract validation and ADR for metadata noninterference | Resolve canonical version versus observer cursor/ack/resume semantics, redact scope grants, secure audit/replay access, enforce Forbidden | No Werewolf-specific platform branch; no social UI |
| Phase 7 | Phases 4 and 5 exits; Werewolf Phase-3 security model accepted | Role-only presentation, seat list, phase banner, legal action UI, public voting, death/reconnect knowledge refresh, text chat, moderation, parties/rooms | Voice provider, speaking indicators, spectator replay product |
| Phase 8 | Phase 7 exit; generic voice permission ADR/design approved | Provider adapters, scoped publish/subscribe grants, token revocation, SFU enforcement, TURN, permission/device UX, speaking indicators | Custom SFU, recording/transcription, rules driven by audio arrival |
| Phase 9+ | Separate replay/privacy and ruleset review | Projected spectator replay, advanced roles, moderator mode | Canonical replay download to ordinary users |

Future presentation reads View/ViewEvent only. Do not cache dead/Audit knowledge into a living
viewer after a seat change or reconnect; substitution remains Forbidden. Animations cannot delay
input authority or phase timers. Night UI and per-seat scope updates may not reveal other actors'
participation. Existing public vote markers are intentionally public; actor-specific night markers
are not.

## Proposed architecture and generic contract requests

Chat: game emits absolute scope values; generic server validates sender/readers, transports,
moderates and projects per-recipient channel permissions. No platform code learns role names.
Dead can listen to wolves but only speak in dead channel; public clients never receive the full
wolves membership map. Define outsider text access separately because Participants names seats only.

Voice: before coding, propose a reusable room grant with explicit publish/subscribe authority and
multiple-room listening. State how overlapping subscriptions, mute precedence and revocation work.
This is insufficient in today's VoiceRoom membership-only contract and deserves a separate generic
design/ADR and PR, with Werewolf as the motivating consumer. Phase 3 may test existing membership
values; it must not label them evidence of the future permission semantics.

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| No unauthorized game frames | Hidden actions leak via empty frames/version/ack/resync | Paired multi-client traces at every transition, including probes and reconnect | example-tested (future) | Phase exit | Public deductions and physical traffic analysis require stated threat model |
| Text scopes enforced | Wolf message delivered to villager socket | 20-seat socket assertions, permission revocation and queued-message tests | example-tested (future) | Phase exit | Out-of-band human communication |
| Knowledge transition UX is safe | Stale private view or wrong viewer after death/reconnect | UI interaction tests + screenshot/RenderList cases per role/phase | example-tested (future) | Every PR | Human review still required |
| Social game is usable | Protocol tests pass but humans cannot finish | Recorded 12-person session with disconnect/return and second-person leak review | example-tested (manual, future) | Manual/security review | One session is limited evidence |
| Voice grants enforce policy | Muted seat still publishes at SFU | SFU API/media assertions and per-participant recordings, revocation <500ms | example-tested (future) | Phase exit | Provider behavior and network conditions |
| Voice failure does not stop game | Media outage blocks ordered game inputs | Disable SFU/TURN; complete text-only match | example-tested (future) | Phase exit | Broader operational incidents |
| 20-seat traffic meets budget | Vote bursts overload fan-out | L2 load scenario on actual runtime/hardware | example-tested (synthetic measurement, future) | Phase exit | Not production-observed |

## Expected file changes

Phase 7: Werewolf presentation/assets/tests, generic server chat handlers, lobby, client shell and
integration tests. Phase 8: voice contracts/adapters, generic server grants, client audio UI and
deployment configuration. Enumerate precise files only after those phases have real code; current
paths are ownership boundaries, not permission to fill skeletons early.

## Acceptance criteria

- [ ] Separate future plans name phase exits and approved generic contracts before implementation.
- [ ] Phase-3 closure does not require UI, chat transport, SFU, human online playtest or load results.
- [ ] Phase-7/8 release gates include actual socket/media enforcement, not only game-level scans.
- [ ] No canonical State, events, seed or full scope map reaches an ordinary client.

## Residual risks

Dead full vision makes social collusion possible by design. Moderation and privacy tradeoffs need
their own product decisions. None authorizes weakening the headless information boundary.

## Next dependency

Finish [W1–W10](05-pr-sequence.md); then satisfy the roadmap gates before opening social work.
