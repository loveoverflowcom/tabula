# Goal

Make every authorized observation explicit, including **whether an event exists**. This is the
Phase-3 security acceptance contract for project, view_event, describe and legal affordances.
It does not claim that a future socket, replay download or voice provider is safe.

## Current state

There is no Werewolf projection code or `docs/games/werewolf.md`. The testkit already provides
SecretModel (including `event_secrets`), containment scans, pairwise View/Option<ViewEvent>
noninterference assertions, positive controls and a reachable-trace security fixture.

## Decisions: knowledge matrix

Assumes the [W-D defaults](00-decisions-and-phases.md). Read rows at the **current phase**.
`V` = visible; `D` = degraded/redacted but the fact/event exists; `N` = non-existent for this
viewer (no field/event revealing the fact, not a null placeholder); `R` = revealed later by a
named public transition. Qualifiers scope a cell completely. "Own" includes only the viewer's
seat; all other actors are N unless the cell says otherwise. Seer/Doctor/Witch columns are living.

| Fact | Living villager | Living wolf | Seer | Doctor | Witch | Dead player | Outside spectator | Server/audit |
|---|---|---|---|---|---|---|---|---|
| Roles | V own; R others on death/end | V own + wolf roles; R nonwolves on death/end | V own; R others on death/end (investigation is alignment only) | V own; R others on death/end | V own; R others on death/end | V all current roles | R on death/end | V all |
| Wolf teammates | N until role reveal | V exact team | N except investigated Wolf results/reveals | N until role reveal | N until role reveal | V exact team | N until role reveal | V |
| Night action existence | N | V own; N teammates/others | V own; N others | V own; N others | V own; N others | V current/retained | N | V |
| Night actor | N | V own; N others | V own; N others | V own; N others | V own; N others | V | N | V |
| Night target | N | V own; N others | V own; N others | V own; N others | V own; N others | V | N | V |
| Seer result | N | N | R own at Dawn → V retained alignment | N | N | V retained | N | V |
| Doctor protection/save | N | N (including wolf attack failure cause) | N | D own selected target; N success attribution | N | V choice and resolved cause | N | V |
| Witch action/resources | N | N | N | N | V own choice/charges; N attack victim/success attribution | V | N | V |
| Votes / abstention | V cast votes; D missing → abstain at close | V / D as villager | V / D as villager | V / D as villager | V / D as villager | V / D as villager | V / D as villager | V actual ballots |
| Death | R at Dawn/Dusk → D seat+role, hidden night cause | R → D same | R → D same | R → D same | R → D same | V full current cause | R → D public seat+role | V |
| Revealed roles | V public map | V | V | V | V | V | V | V |
| Phase | V public phase+round | V | V | V | V | V | V | V |
| Timer | D public phase deadline only | D same | D same | D same | D same | D same | D same | V deadline/id |
| Alive/dead set | V | V | V | V | V | V | V | V |

Living Hunter follows villager cells, plus V own precommitted mark and N others' private actions.
At death the mark was already fixed, so full vision cannot influence retaliation. Dead players
remain Viewer::Seat; never convert them into Viewer::Audit. Invalid seat viewers use outsider
policy. Delayed spectators use the same knowledge policy on the historical state supplied by the
shell; project does not implement a delay buffer or read a clock.

Public preset counts, revealed roles, known wolf teammates, one's own role, and Seer alignment
results permit logical deductions. These are **intentionally derivable**. Zero deaths at Dawn
does not certify a save: wolves may have passed or tied. Do not publish successful-save flags,
number of pending/submitted actions, remaining wolves, other players' potion counts or investigator
counts. At Ended all roles become public; outsiders still do not get the action history or seed.

## Proposed architecture

Use a distinct View with public facts and a closed knowledge enum:

```text
View = public seats/alive/revealed/phase/deadline/ballots/result
       + Knowledge::{PublicOnly, Living(LivingKnowledge), Dead(DeadKnowledge), Audit(AuditKnowledge)}
LivingKnowledge = own role-specific enum (Villager/Wolf/Seer/Doctor/Witch/Hunter)
                  + own action/ballot status + authorized targets/results/resources
```

No `Option<Role>` fields used to blank another player's secret. Public revealed roles are a
positive knowledge map; absent entries mean unrevealed. Optional target choices are legitimate
domain option values, not substitutes for authorization. Dead/Audit knowledge is constructed by
separate branches; public view types never embed State. Describe reads the already projected
View, with the same knowledge constraints. ViewEvent is a distinct enum, never an Event alias.

### Event visibility decisions

| Canonical event | Client result | Why |
|---|---|---|
| RolesAssigned | None for **all** clients; Audit receives audit form | Its payload is server-only; own roles come from project, public roles from explicit reveal events |
| NightActionSubmitted (including pass/mark/potion) | Some(OwnActionRecorded) to actor; Some(ObservedNightAction) to already-authorized dead; None to every other client | Some(RedactedNightAction) still leaks existence and is forbidden |
| Investigated | Some(InvestigationResult) to Seer at Dawn and dead; None elsewhere | Result and existence are private |
| Protection/HealingResolved | Emit once at closure for every submitted protection/heal, including ineffective choices. Some(OwnProtectionRecorded) to actor, identical regardless of success; full detail to dead/audit; None elsewhere | Conditioning event existence on a successful save would leak the attack even with redacted payload |
| Night death with canonical causes | Some(DeathRevealed { seat, role }) to living/outsiders; full observation to dead/audit | The death exists publicly; cause is degraded |
| VoteCast / VoteRemoved / VoteResolved | Public Some with public ballot/tally facts | These are intentionally observable, including ordering |
| PhaseChanged | Public Some at fixed boundary | No early close or participant-count payload |
| Ended / roles revealed | Public result and role map; no canonical log/seed | End-game declassification is explicit and limited |

`view_event(state_after, event, viewer)` receives the **complete post-input state**, not a state
after each element of events. New dead viewers may receive the batch's permitted full details;
this expansion is intentional. Use the state captured at that input when replaying security tests.
Never project historical events using the final match state and call that evidence of live secrecy.
RolesAssigned remains None even if a later caller passes an ended state; later reveal does not
retroactively resend the assignment event. If historical viewer replay is built later, give it a
separate review (Phase 9).

## Invariants and noninterference properties

Define `s ≈V t` as equality of every fact V is authorized to know **now**, including its own role,
resources/results, public config/phase/time/alive/revealed/ballots, and authorized wolf knowledge.
This relation is specified from the matrix, independently of project; defining it by project
equality would make the property circular.

1. **Role noninterference:** for reachable s and valid paired t with s ≈V t, changing unauthorized
   role assignments leaves canonical(project(V)) equal. Preserve the preset multiset, revealed roles,
   V's own role/results and (for a wolf) team membership. Swap only genuinely unauthorized roles.
2. **Private-action noninterference:** from one reachable prefix before Night closure, submit two
   different authorized private choices by another actor. Assert V's View and encoded
   `Option<ViewEvent>` are identical; compare None itself, not just Some payloads.
3. **Event-absence noninterference:** compare a prefix with no submission against the same public
   checkpoint after an unauthorized submission. Compare flattened **visible** event streams and
   projected Views: both observations must remain equal. Do not fabricate a canonical dummy event
   on the no-action side. Pairwise event helper alone cannot test unequal event counts.
4. **Ordering/count noninterference:** permute commuting submissions by different actors, add/remove
   hidden submissions, and compare public visible stream before resolution. Compare authorized wolf
   knowledge as well: knowing team identity does not authorize teammates' submission timing.
5. **Error/affordance noninterference:** the same hostile command issued by V against s ≈V t returns
   the same public error code/detail; V's legal-command set and describe output agree. Self-role and
   target-public-state errors may differ only when the matrix authorizes those differing facts.
   Include same-protection/changed-wolf-action pairs with the same public deaths: the Doctor/Witch
   receives the same acknowledgement existence and bytes whether the protection was effective or not.
6. **Positive controls/declassification:** own action/result changes its owner's View; changed roles
   affect Audit and dead View; public death/role reveal changes every observer's public View. Check
   these beside each applicable equal-output property to catch constant-output implementations.

Build reachable prefixes via create/apply, with targeted seeds covering each role and actual death
transitions. Role-pair helper is crate-private and cfg(test/testkit); assert the invariant validator
accepts both states, preserve authorized histories, and retain a reachability justification (e.g.
permuted assignment before any private result). Later-game properties prefer paired accepted scripts
instead of arbitrary mutation of past roles. A validity check alone does not establish reachability.
Require nonzero changed-secret and unauthorized-viewer counts; no broad `prop_assume` that discards
every difficult case. After public resolution, outcomes may diverge: do not assert equality across
intentional death/result declassification. Add same-public-outcome pairs to probe hidden cause leaks.

## SecretModel and containment coverage

Implement `secrets(state)` **and** `event_secrets(state_after, event)` in `rules/secret.rs` under
cfg(test/testkit). The latter covers transient resolved actions cleared from State. A secret's
authorized set varies by role, time and death; `RolesAssigned` event authorization is stricter
than current role knowledge. Do not reuse production authorization helpers as the sole test oracle.

- Prefer actual complete role-map or action/result record fragments when long enough to be specific.
  A nested postcard field does not contain the canonical encoding version prefix: derive tokens
  from its actual nested payload, and test a deliberately leaked field to establish a match.
- Never pad tokens with invented tags or salts absent from the real output. A token that cannot
  occur in an injected leak is no evidence. Never whitelist accidental collisions as authorization.
- Single role/seat/bool encodings are too short for trustworthy substring scanning. Explicitly
  list scalar and short-record gaps; independent typed visibility tables and noninterference own
  them. No production serialization changes purely to make a test scanner convenient.
- Token choices must pass positive injected-wholesale-leak controls and clean-output collision
  checks at 6, 12 and 20 seats. If no useful fragment exists for a category, record zero token
  coverage for it; do not claim containment covers the category. A generic scanner redesign, if
  necessary, is a separate proposed testkit PR, not a silent game dependency.
- `HiddenInformationFixture::game_controlled_spectators()` must return Live and a Delayed tier;
  fixture roster supplies all seats. Audit is a positive control, excluded from unauthorized scans.
  Include a public Some event while secrets exist so the suite's non-vacuity checks are meaningful.
- Add explicit event-class/viewer coverage counters. The existing security suite counts Some
  outputs and token comparisons; it does not establish that every expected None case was tested.

## Metadata, scope and transport leak register

| Vector | Phase-3 obligation | Later owner/gate |
|---|---|---|
| Counts/collection ordering | Exclude private counts and ordering from View/describe/affordances; test paired states and streams | Phase 4 projected envelope sizes/order, Phase 7 socket assertions |
| Event existence/timing | None for unauthorized events; fixed public windows, unchanged scopes per private action | Phase 4: suppress empty batches and distinguish observer-visible emissions from canonical accepts |
| `state_version`, ack.at, resync_at | Do not put canonical index/hash in game View. Document unavoidable envelope risk | **ADR before Phase-4 protocol freeze**: a public counter gap, reconnect response or probe ack can reveal hidden input count |
| Errors | Stable existing codes with public-safe details; no private target/resource diagnostics | Phase 4 codec/auth/rate-limit errors must preserve observation policy too |
| Scope changes | Return server-only absolute permission values; no private action-driven updates | Phase 7 per-recipient scope views must hide wolf roster/channel membership; never broadcast full effect maps |
| Hashes/replay/seed | Canonical replay and seed are audit artifacts only; no canonical hash in client View | Phase 4 download/access controls; Phase 9 projected replay redaction |
| New dead knowledge | End-to-end batch declassification tables; Hunter choice already frozen | Phase 7 reset cached UI, permissions, reconnect view and queued events atomically |

The counter issue is an **architecture finding**, not permission to violate I-7. Internal
StateVersion must still increment once per accepted input. A reusable observer cursor/ack/resume
design is one candidate; it needs an ADR and coherent changes to doc 05 and generic consumers.
An empty-frame suppression alone does not solve gaps visible in the next public frame or resync.
Phase 3 can demonstrate game-output noninterference while explicitly leaving wire secrecy open.

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| Matrix holds | Wrong role/dead/outsider branch | Table tests for all knowledge cells and event variants | example-tested (planned) | Every PR | Derived combinations need pair properties |
| Unauthorized secrets do not influence output | Private count/order/checksum leak | Paired reachable-state properties 1, 2, 5 | property-tested (planned) | Every PR | Generator preserves authorized facts; wire excluded |
| Events do not exist for unauthorized viewers | Some(Redacted), different list length | Property 3 plus explicit None table | property-tested (planned) | Every PR | No socket scheduler exists yet |
| Whole structures cannot leak unnoticed | Role map forwarded wholesale | SecretModel scan with injected-leak controls | example-tested (planned) | Every PR | Small scalars and alternate encodings excluded |
| Guards are asserted | Deleted viewer check stays green | Scoped project/view_event mutation campaign, every survivor classified | mutation-tested (planned) | Phase exit | Not proof of the policy itself |
| Metadata contract is safe | Hidden accepted count leaks on ack/resume | ADR review + later paired wire traces | documented | Manual/security review | Phase-4 gate remains open |

## Expected file changes

`games/werewolf/src/rules/{projection,event,secret}.rs`, tests for projection tables and paired
traces, `docs/games/werewolf.md` information model, future ADR request recorded in W1/W10 notes.
No changes to platform/protocol/voice code in Phase 3.

## Acceptance criteria

- [ ] Every matrix cell and event variant maps to a test; roles include Hunter, dead and invalid seat.
- [ ] Explicit None assertions plus no-action/action visible-stream equality pass.
- [ ] Positive controls, transient event secrets and collision coverage are recorded.
- [ ] At 6/12/20 seats all supported presets and GameControlled spectator tiers are exercised.
- [ ] Normative information model is written with intentional deductions and residual wire risks.
- [ ] No claim of socket/voice security based only on headless tests.

## Residual risks

Authorized public deductions, full dead vision and out-of-band human communication remain possible.
Low-entropy token scans are a coarse supplement. Paired models can be wrong if their assumed
authorization relation is copied from the code under test; manual security review is a separate gate.

## Next dependency

[03-verification.md](03-verification.md) and W6/W7/W8 in the PR sequence.
