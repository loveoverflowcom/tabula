# Goal

Fix a reviewable Phase-3 headless ruleset before implementing Werewolf. These are **recommended
defaults**, to be recorded as accepted or replaced in W1; they do not amend normative architecture.
The unresolved product choices below block their dependent PRs, not this planning task.

## Current state

`games/werewolf/` contains only a documented skeleton, Cargo feature declarations and lints.
There is no module, manifest, rules hash, rules, information model or test suite.
See [the inspected baseline and drift register](../reference-games-gap-analysis.md).

## Scope

| Phase | Deliverables | Gate |
|---|---|---|
| 3 | Rules, deterministic timers/RNG, projections, event non-existence, scope **values**, SecretModel, tests, simulation, canonical replay, terminal projection viewer | Phase 2 exit; doc 07 Phase 3 |
| 4 | Generic network envelope/routing/security decisions required before protocol freeze | Separate platform work; no Werewolf-specific branches |
| 7 | Werewolf presentation, online social UX, chat enforcement, moderation, human playtest | Phases 4 and 5; doc 07 Phase 7 |
| 8 | Voice provider, publish/listen enforcement, SFU/TURN and device UX | Phase 7 exit; ADR-016 |
| 9+ | Spectator replay product, advanced roles, custom rulesets, moderator mode | Separate scope decisions |

No Phase-7/8 implementation is authorized by this plan. A headless text projection viewer is
Phase-3 verification tooling, not `GamePresentation` or a local social client.

## Decisions

Each row supplies a recommended default, alternatives, impact and dependent work. W1 records
explicit outcomes, including rejected alternatives. All six base roles remain in Phase-3 scope.

| ID / question | Recommended default | Alternatives | Why it matters / dependencies |
|---|---|---|---|
| W-D1 Role presets | One named `ClassicV1` preset family, selected by seat count; table below | Smaller beginner preset; multiple named presets | Pins role distribution and reproducible fixtures; W1/W2, manifest and all goldens |
| W-D2 Assignment/counts | 6–20 unique occupied seats; sort by SeatId, shuffle the exact role multiset using `ctx.rng.stream(DOMAIN_ROLES)` once in create | Reject selected counts; user-authored counts (future) | Stable seat binding, no role-count ambiguity; W2 assignment and alternate-seed tests |
| W-D3 Doctor | One protection choice or pass each Night; self-save allowed; cannot protect the same seat on consecutive nights; blocks wolf attack only | No self-save; repeated saves; protect against poison too | Needs previous-night target, independent poison precedence; W4 resolver/tests |
| W-D4 Witch | One heal and one poison charge per match; choose **at most one potion per Night**, or pass. Heal any living seat, including self, against wolf attack only. No private preview of the wolves' victim | Victim-informed heal in a fixed second night window; both potions in one Night | Avoids leaking wolf action existence through Witch UI; W4, legal commands, W6/W7 knowledge model. Victim-informed variant requires redesigned fixed windows before coding |
| W-D5 Hunter trigger | Precommit an optional retaliation target during Night; keep it through that round's Vote. If killed by wolves, poison or vote, kill the precommitted target if still living; trigger once, before checking victory | Reactive last shot; no shot after poison | Reactive shot plus immediate dead full vision is a cheating oracle. Precommit avoids a new phase and this conflict; W4/W5, secrecy and chain-death tests |
| W-D6 Day vote/ties | Public last submitted ballot, replaceable; `Plurality` or `AbsoluteMajority` preset option; tied maximum → no elimination in both modes | Runoff; random tie-break | Count all living seats for majority threshold, including abstainers; W5 tables/reference vote model. Other tie policies require a later rules decision |
| W-D7 Abstention | `Vote(Abstain)` and `Unvote`; missing ballot at expiry counts as abstain; no early closure | Mandatory ballot; early majority close | Timer-only closure keeps public phase duration fixed; W3/W5, simulation fairness |
| W-D8 Disconnect/lifecycle | Phase continues. Disconnected/idle seats retain submitted choices, missing choices default to pass/abstain. Reconnect may act before deadline. Abandoned/Vacated marks seat permanently absent, with missing actions defaulted; no reassignment or automatic death | Forfeit death at next public boundary | Roster/roles never change; W3/W5 and replay lifecycle traces |
| W-D9 Death reveal | Publish seat and role on death; public night death conceals cause/actor. At Ended reveal all roles, never the seed or full action log to outsiders | Hide roles until end | Matches doc 08 §5.1 and §5.3; W5/W6/W7 |
| W-D10 Victory/parity | After the whole death/retaliation batch: no living seats → draw; zero wolves → village win; wolves ≥ living nonwolves → wolf win; otherwise continue | Parity after each individual death | Order must not change the winner; W5 differential/table tests. Dead team members share team outcome |
| W-D11 Resolution order | Freeze choices; compute wolf consensus; apply Doctor/heal against wolf attack; poison independently; union initial deaths; trigger Hunter; reveal sorted deaths; determine victory | Sequential instant kills | Simultaneous actions remain effective when their actor dies in the same batch; W4/W5 reference resolver |
| W-D12 Dead-player knowledge | Dead `Viewer::Seat` sees all current roles, pending actions and retained results from the completed transition onward; cannot issue gameplay commands | Public-only dead spectators | Required by doc 08 §5.1. Old discarded history is not reconstructed. New full vision explains W-D5; W6/W7 boundary tests |
| W-D13 Outside spectators | Both live and delayed tiers use public knowledge; no role-specific affordances; all roles only at Ended | No outside spectators | `SpectatorPolicy::GameControlled`; delay buffering belongs to the shell. Invalid Seat viewer also gets public-only knowledge; W6/W7 |
| W-D14 Scope values | Absolute `ChatScopes { channels }`, stable keys/order and sorted participants; `VoiceScopes { rooms }` can exercise membership only | Add builders; redesign voice permissions | Existing structs construct without platform changes. Voice cannot yet encode asymmetric speak/listen (see architecture issues); W3 and future gates |
| W-D15 Secret tokens | Actual serializable compound fragments with explicit authorized viewers; noninterference covers small scalar secrets | Single role-byte scan; tagged synthetic tokens | Tiny tokens collide, invented tags miss real leaks; W7 must validate granularity and negative controls before claiming scan coverage |
| W-D16 Timers/closure | Five fixed-duration phases; close only on matching due timer; missing actions default once; no early close based on participation or surviving roles | Close when all roles acted | Early close reveals roles/action counts. Config durations 1,000..=600,000 ms, default Night 30s, Dawn 2s, Day 120s, Vote 30s, Dusk 2s; W3 deadline boundary tests |
| W-D17 Liveness bound | `max_rounds` 1..=100, default 100; after final Dusk resolution without victory → stalemate draw | Unbounded social game; operator abort only | All-pass games otherwise never terminate. Deadline progress remains a shell assumption; W3/W5/W9 |
| W-D18 Automation | Runtime module returns no bots; separate verification-only module supplies projection-driven simulation policies; keep substitution forbidden in both | Runtime bot behind bots feature; bespoke simulation driver | Reuses existing selfplay without exposing takeover. W9 and the module separation tests below |

### ClassicV1 counts

`V = n − W − S − D − H − T`; T means Witch. All other counts are exact. This is a
Tabula preset recommendation, not a claim of tournament-standard Werewolf balance.

| n | W | S | D | H | T | V |
|---|---|---|---|---|---|---|
| 6 | 1 | 1 | 1 | 0 | 0 | 3 |
| 7 | 1 | 1 | 1 | 0 | 0 | 4 |
| 8 | 2 | 1 | 1 | 1 | 0 | 3 |
| 9 | 2 | 1 | 1 | 1 | 0 | 4 |
| 10 | 2 | 1 | 1 | 1 | 1 | 4 |
| 11 | 2 | 1 | 1 | 1 | 1 | 5 |
| 12 | 3 | 1 | 1 | 1 | 1 | 5 |
| 13 | 3 | 1 | 1 | 1 | 1 | 6 |
| 14 | 3 | 1 | 1 | 1 | 1 | 7 |
| 15 | 3 | 1 | 1 | 1 | 1 | 8 |
| 16 | 4 | 1 | 1 | 1 | 1 | 8 |
| 17 | 4 | 1 | 1 | 1 | 1 | 9 |
| 18 | 4 | 1 | 1 | 1 | 1 | 10 |
| 19 | 4 | 1 | 1 | 1 | 1 | 11 |
| 20 | 5 | 1 | 1 | 1 | 1 | 11 |

Additional pinned semantics: Seer investigates one other living seat, receives Wolf/NonWolf at
Dawn, even if the target dies that batch. Each living wolf submits one nonwolf target or pass;
unique positive plurality of wolf choices attacks, ties/all passes do not. Choices are immutable
once submitted that Night; Doctor's previous target clears after a pass. Hunter chooses another
living seat or passes; its mark resets on entry to each Night. Dead actors' valid submitted
actions resolve because eligibility was frozen before the batch. Potions are consumed at submission
even when ineffective, and all night submissions are private to their actor (not wolf teammates).

## Proposed architecture

`Config/roster/seed/ordered Input/Ctx → pure rules → events/effects/projections → shell`.
Use private validated config/state, closed phase and knowledge enums; detailed transitions are in
[01-rules.md](01-rules.md). `GameModule::validate_config` and `create` share a game-local validator.

Capabilities: Phased, hidden=true, GameControlled spectators, game-scoped table/wolves/dead chat,
Recommended voice (future consumer), ranked=No, async=Disabled, pausable=false,
client_preview=false, AckAfterApply (doc 02 §12.3), fill_with_bots=false,
substitution=Forbidden, reconnect.notify_rules=true/grace=60s, teams=None, symmetric=false.
Measure state size before declaring final `StateSizeClass`; Small is an estimate. Budget initially
2,000 μs/64 events per input, then measure max-seat resolution. Scope effects are not client payloads.

## Bot/self-play contract investigation

Current `selfplay::validate_config` requires **every occupant to be Bot**, then delegates to
`M::validate_config`. `execute_match` unconditionally calls `M::bot(level)`; missing bots fail.
It does **not** inspect `SubstitutionPolicy`. The scheduler polls each seat's `project`, orders by
`(ready_at, SeatId)`, and gives due timers priority at equal time; it does not require RequestBotMove.
`check_projections` compares repeated-run outputs; it does not run SecretModel or noninterference.

Recommended W9 implementation:

1. `WerewolfModule::bot` stays None under every feature, including `bots` and `testkit`.
2. `src/simulation.rs`, under `cfg(any(test, feature = "testkit"))`, defines
   `SimulationModule` with `Rules = WerewolfRules`. Delegate metadata, capabilities, hash and config
   validation unchanged; override only `bot` to return `SimulationPolicy`.
3. Config validation checks count, unique occupied seats, no roster teams; it accepts initial Bot
   occupants for the headless harness. It does not implement lobby authorization. Product autofill
   is disabled and there is no runtime bot provider. Every OccupantChanged input remains rejected.
4. The policy receives only View. Return None after this seat's night choice or first vote,
   and while dead/inactive; otherwise the lowest SeatId can monopolize the poller by replacing votes.
5. Test runtime None, identical Forbidden capabilities, replacement rejection and simulation
   completion explicitly. No RequestBotMove is emitted by Werewolf rules.

The abstraction couples simulation policy discovery to the runtime module's bot hook, but a
test-only wrapper resolves it without changing `tabula-game-api` or `tabula-testkit`. If reuse later
justifies an injected generic policy provider, propose a separate tooling/testkit PR; never relax
Forbidden. A test policy is not evidence that the production runtime enforces substitution.

## Architecture issues requiring separate decisions

| Issue | Insufficiency / reusable contract | Disposition |
|---|---|---|
| Version/ack/resync metadata | A canonical input counter reveals hidden accepted activity even if view_event=None; applies to any hidden-action game | W1 records an ADR request; Phase 4 must settle observer cursor and emission semantics before wire freeze; preserve I-7 internally |
| Voice permission asymmetry | VoiceRoom has only members, cannot express dead listening to wolves while forbidden to publish there | Document now; separate generic ADR/design before Phase-8 implementation. Do not pretend membership values enforce doc 07 Phase 8's full policy |
| Uniform TeamSpec | Cannot represent unequal wolf/village counts | teams=None; rules own alignment, ranked=No. No new team API for this game |
| Audience on canonical events | GameRules Event has no audience envelope; only Notify contains Audience | RolesAssigned is server-only through exhaustive view_event handling. Do not assume an Audience annotation routes it |

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| Presets are internally coherent | Impossible roster or parity at creation | All 15 count rows and constructor partitions | example-tested (planned) | Every PR | Balance needs Phase-7 playtests |
| Runtime substitution stays forbidden | Simulation accidentally enables takeover | Capability/bot feature tests and lifecycle rejection | example-tested (planned) | Every PR | Actual lobby/bot-runner enforcement Phase 4+ |
| Scope plan is representable | Membership described as publish/listen security | Read current effect structs against permission table | documented | Manual/security review | Voice asymmetry is an open generic contract |
| No phase crossing | Headless work pulls in UI/network | Feature/dependency checks and diff review | statically checked (planned) | Every PR | Phase exit requires separate product evidence |

## Expected file changes

W1: `docs/games/werewolf.md`, its index, `games/werewolf/src/rules/{mod,config,role}.rs`,
`games/werewolf/src/lib.rs`, Cargo.toml. Identity/build.rs arrives in W2 with a real rules tree.
No normative doc or platform source is changed by this planning task.

## Acceptance criteria

- [ ] W-D1..18 and exact preset counts recorded; Hunter/Witch defaults explicitly accepted.
- [ ] Phase-3 and Phase-7/8 gates remain distinct; generic issues have owner phase and gate.
- [ ] No product claims inferred from synthetic selfplay or successful compilation.

## Residual risks

Full dead vision permits out-of-band collusion; it is an intentional information policy, not
something `project` can prevent. Human balance, live timing traffic and provider enforcement
remain outside Phase-3 evidence. See [the secrecy plan](02-secrecy-and-projections.md).

## Next dependency

[W1 → W2](05-pr-sequence.md), then phase/timer and resolution kernels.
