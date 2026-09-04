# Goal

Ten reviewable Phase-3 PRs, with complete prerequisites and acceptance evidence. Every PR keeps
the local core gate green. None activates an incomplete game or fills Phase-7/8 skeletons.

## Current state

Only Cargo/lint/doc skeletons exist. The private-kernel approach below keeps early PRs buildable:
W1–W6 add real types and independently tested helpers; W7 connects GameRules/GameModule only when
all paths and redaction exist. No fake successful apply arm or State-as-View is needed.

## Decisions

**Recommended first implementation PR across both games: W1.** Settling Werewolf's information
policy, Hunter/Doctor/Witch interactions and simulation separation removes the largest uncertainty
before the runtime/protocol contract freezes. Caro C1 can follow independently once Phase 2 exits.
An ADR request recorded by W1 is not a normative ADR amendment or a platform implementation.

```text
W1 decisions/types → W2 create/identity → W3 phases/timers → W4 night → W5 vote/death/win
                                                                       ↓
W10 security/docs ← W9 simulation/replay/demo ← W8 deeper evidence ← W7 adapter/events/scan
                                                                       ↑
                                                          W6 projections/knowledge
```

## W1 — Ruleset, knowledge decisions and validated primitive types

| Field | Plan |
|---|---|
| Objective | Record W-D1..18 and a concrete knowledge policy; make accepted config/role values constructible |
| Prerequisites | Phase 2 exit evidence; doc 00/02/07/08; this plan reviewed |
| Scope | Preset table, vote mode, duration/round bounds, Role/Alignment, Hunter precommit and Witch blind-heal decisions; metadata/voice ADR requests; simulation separation |
| Out of scope | State transitions, GameRules/GameModule, build.rs before rules tree exists, platform changes |
| Expected files | `docs/games/werewolf.md`, `docs/games/README.md`; `games/werewolf/src/rules/{mod,config,role}.rs`, `src/lib.rs`, Cargo.toml; constructor tests |
| Invariants | Named types preserve intrinsic bounds; serialized raw DTOs cannot bypass validation; phase gates explicit |
| Verification/evidence | Exact 15-row count checks, config boundary/deserialize tests: example-tested; information/ADR decisions: documented |
| Acceptance criteria | All decisions accepted or dependent PR explicitly blocked; roles/counts coherent; no early UI/bot substitution; `cargo xtask check` green |
| What it unlocks next | W2; independent Caro C1; generic architecture decisions available before Phase 4 |

## W2 — State, deterministic creation and build identity

| Field | Plan |
|---|---|
| Objective | Create valid initial Night state from config, roster and deterministic RNG |
| Prerequisites | W1 accepted count/role/knowledge choices |
| Scope | State/RawState, initial role assignment helper, initial event data, metadata/capabilities statics, disabled manifest, rules version constant and source hash |
| Out of scope | Public GameRules adapter, live phase reducer, presentation, simulation |
| Expected files | `games/werewolf/src/rules/{state,event,mod}.rs`, `src/lib.rs`, `game.toml`, `build.rs`, Cargo.toml; `tests/assignment.rs`, `tests/config.rs` |
| Invariants | Roster stable and sorted; roles assigned once via named DetRng stream; source hash covers canonical subtree; no root package dependency from rules |
| Verification/evidence | Fixed assignment vectors, noncontiguous seats, invalid rosters, seeded determinism and state round trips: example/property-tested; manifest/hash checks: statically checked |
| Acceptance criteria | Real `src/rules/mod.rs` version marker exists before enabling copied build.rs; nonzero hash; manifest version matches; rollout disabled; gate green |
| What it unlocks next | Reachable initial fixtures for W3–W6 |

## W3 — Phase/timer/lifecycle and scope kernels

| Field | Plan |
|---|---|
| Objective | Define fixed public phase windows and lifecycle meaning with no clock or transport |
| Prerequisites | W2; W-D8/14/16/17 |
| Scope | Pure phase-entry/expiry helpers, fresh bounded TimerIds, checked deadlines, stale/early timer policy, reconnect/absence handling, absolute chat/voice-membership values |
| Out of scope | Applying missing night/vote resolution as successful stubs; scope enforcement or a voice permission redesign |
| Expected files | `games/werewolf/src/rules/{mod,state,scopes}.rs`, `tests/phases.rs`; information-model scope table |
| Invariants | W-I2/3/6; private submissions cannot change deadline/scopes; role/seat assignment immutable |
| Verification/evidence | Full phase×input partitions; exact deadline and overflow/stale-id tables; absolute scope expected bytes: example-tested |
| Acceptance criteria | No ID reuse within ≤100 rounds; abandoned seats cannot reconnect as replacements; no zero-progress overflow timer; gate green |
| What it unlocks next | W4 action windows and W5 terminal scheduling |

## W4 — Night authorization and independent resolution inputs

| Field | Plan |
|---|---|
| Objective | Validate each role's choice and resolve simultaneous night effects deterministically |
| Prerequisites | W3 and accepted Doctor/Witch/Hunter semantics |
| Scope | Private action witnesses; one submission per Night; potion/history updates; wolf consensus, Seer results, protection/poison, precommitted Hunter mark; pure draft resolution |
| Out of scope | Vote implementation, production bot, any night UI, final public adapter |
| Expected files | `games/werewolf/src/rules/{command,resolution,state,event}.rs`; `tests/night.rs`, independent test resolver |
| Invariants | W-I2/4/6; choices effective from actors killed in same batch; duplicate causes do not duplicate deaths |
| Verification/evidence | Role×action/target/resource table, byte-preserving errors, independent set-based night model: example/property/differentially-tested |
| Acceptance criteria | Self-save/repeat-save/potion exhaustion/tied wolf vote/poison-vs-heal cases explicit; private errors reveal no other actor; gate green |
| What it unlocks next | W5 death/outcome finalization and W6 private knowledge |

## W5 — Public vote, death, retaliation and terminal rules

| Field | Plan |
|---|---|
| Objective | Complete pure transition behavior, including guaranteed finite all-pass scenario under delivered timers |
| Prerequisites | W4; W-D6/7/9/10/11/17 |
| Scope | Replaceable votes/Unvote, plurality/absolute majority, no-elimination ties, sorted death/reveal batch, Hunter trigger, parity/empty-alive results, round-cap draw, admin abort, terminal probes |
| Out of scope | Enabling a game with incomplete project/view_event; online cancellation/ratings |
| Expected files | `games/werewolf/src/rules/{mod,resolution,state,event}.rs`; `tests/voting.rs`, phase/terminal tests |
| Invariants | W-I5/6/8; one EndMatch; all roster seats covered; no live actions after full dead knowledge |
| Verification/evidence | Small exhaustive ballot models, all feasible wolf/nonwolf counts ≤20, batch-order cases, all-pass round cap: example/differentially-tested |
| Acceptance criteria | No interim parity result; poison plus wolf same target dies once; Hunter cannot choose after death; no missing Input arm in completed reducer design; gate green |
| What it unlocks next | W6 full before/after death/terminal projection fixtures |

## W6 — View knowledge boundary and safe affordances

| Field | Plan |
|---|---|
| Objective | Implement the knowledge matrix as distinct serializable View types |
| Prerequisites | W5 complete transition helpers and reviewed information model |
| Scope | PublicOnly/Living role variants/Dead/Audit knowledge; project helper, describe from View, Enumerated legal commands; matrix tests and paired-role/action laws |
| Out of scope | State blanking, Event aliasing, event transport, reveal animation |
| Expected files | `games/werewolf/src/rules/projection.rs`, command helpers, projection test support, `docs/games/werewolf.md` |
| Invariants | W-I7; dead seat differs from outsider; invalid Seat viewer public-only; legal affordances reveal only authorized facts |
| Verification/evidence | All matrix cells, canonical-byte noninterference and authorized positive controls: example/property-tested |
| Acceptance criteria | No role/resource/action-count leak; no public canonical counters/hashes; paired generator preserves authorized facts and actually varies secrets; gate green |
| What it unlocks next | W7 complete public adapter and event security |

## W7 — ViewEvent, SecretModel and complete module activation

| Field | Plan |
|---|---|
| Objective | Connect GameRules/GameModule with full redaction and mandatory baseline suites |
| Prerequisites | W5 complete reducer + W6 project/affordances; transient event model accepted |
| Scope | Exhaustive view_event including None, both SecretModel hooks, token/negative controls; wire pure helpers through complete trait impls; baseline GameTestFixture + HiddenInformationFixture |
| Out of scope | Runtime registration, production bots, partial-success TODO arms, shared scanner refactor |
| Expected files | `games/werewolf/src/rules/{mod,event,secret}.rs`, `src/lib.rs`; `tests/conformance.rs`, in-crate security tests; metadata/manifest parity assertions |
| Invariants | I-5/I-6, W-I2/7/8; canonical assignment remains server-only; accepted dead expansion evaluated from exact state_after |
| Verification/evidence | Explicit None and no-action/action stream cases; baseline conformance and projection_security; transient event leak controls: example/property-tested |
| Acceptance criteria | 11 baseline conformance tests plus security suite green and non-vacuous; runtime bot None in every feature; no unredacted fallback; manifest stays disabled for product release; gate green |
| What it unlocks next | W8 deep evidence, W9 actual module replay/simulation |

## W8 — Stronger state-machine, secrecy and differential evidence

| Field | Plan |
|---|---|
| Objective | Close gaps left by fixed fixtures and confirm the independent oracles catch plausible defects |
| Prerequisites | W7 |
| Scope | V1..13 generated sequence laws, all 15 count configurations, 6/12/20 secrecy traces, snapshot continuation, independent vote/night resolver, minimized regression corpus |
| Out of scope | Mandatory Kani, typed-command fuzzing, whole-workspace mutation campaign |
| Expected files | `games/werewolf/tests/{determinism,night,voting,phases}.rs`, `tests/support/` reference models; in-crate secret/projection tests |
| Invariants | W-I1..8 across accepted/rejected/timer/seat/admin prefixes |
| Verification/evidence | 128 cases/property, independent models, explicit coverage counters and positive controls: property/differentially-tested |
| Acceptance criteria | No vacuous skips; replayed shrinking retains valid setup; error/affordance and event-existence laws included; gate green |
| What it unlocks next | W9 larger automation and committed replay/demo witnesses |

## W9 — Simulation, canonical replays and headless projection demo

| Field | Plan |
|---|---|
| Objective | Demonstrate security locally and replay complete synthetic games without enabling runtime substitution |
| Prerequisites | W8; test-only wrapper boundary accepted |
| Scope | SimulationModule/Policy, bounded crate campaign, selfplay/replay CLI dispatch, three reviewed goldens, synthetic projection viewer, docs and actionable failure seed/index output |
| Out of scope | GamePresentation/RenderList, account/network/SFU access, spectator replay downloads, treating selfplay repeatability as secrecy |
| Expected files | `games/werewolf/src/simulation.rs`, `src/lib.rs`, `tests/replay.rs`; root `tests/replays/werewolf-*.tbr` and README; `xtask/Cargo.toml`, selfplay/replay/golden/main command files, new projections_cmd.rs, xtask README/tests |
| Invariants | Runtime module bot=None/Forbidden; wrapper same rules/capabilities/hash; only View reaches policy; canonical replay remains audit-only |
| Verification/evidence | Golden Exact identity/literal hashes/terminal outcomes; 64 synthetic PR matches; positive/negative viewer demo assertions: example/self-differential evidence |
| Acceptance criteria | No starvation from repeated ballots; timeout golden includes real times; side-by-side living/dead/outsider/audit columns; per-file CLI supported; no false `replay --all` claim; gate green |
| What it unlocks next | W10 Phase-3 security exit and nightly rollout |

## W10 — Security hardening, resource measurements and Phase-3 closure

| Field | Plan |
|---|---|
| Objective | Produce auditable evidence and record remaining platform/security gates |
| Prerequisites | W9 |
| Scope | Targeted mutation campaign and regression fixes; 100k simulations per 6/12/20 roster; larger nightly secrecy properties; state-size/event-count/apply measurements; human review of synthetic viewer transcript; document completed vs deferred acceptance |
| Out of scope | Fixing unrelated nightly load/fuzz placeholders, Phase-7 human network test, Phase-8 permission implementation |
| Expected files | `games/werewolf/tests/state_size.rs`, real regression tests, `docs/games/werewolf.md`, `.github/workflows/nightly.yml`, relevant xtask benchmark/report code only if measured data needs it |
| Invariants | All W invariants unchanged; no report calls unexecuted work evidence |
| Verification/evidence | List-first scoped mutants and survivor classification; synthetic timing/size; security review; cross-target checkpoint comparison if generic runner exists, otherwise explicit phase-exit dependency |
| Acceptance criteria | No unclassified security survivor; every real gap fixed with regression; declared size follows measurement; terminal reason counts meaningful; metadata/voice ADR requests retain future owner; core gate green |
| What it unlocks next | Werewolf Phase-3 rules/security acceptance when its evidence is complete; separate Phase-4 generic contract work. Phase 7/8 remain gated |

## Verification ledger

Per-claim tiers and evidence are [V1–V20](03-verification.md). A green intermediate PR establishes
only its scope. Compile-ready helpers are not a playable Werewolf implementation; W7 is the first
complete module. Source-only changes in hashed rules may change Exact identity; subsequent goldens
must be intentionally refreshed with a reviewed semantic-version decision.

## Residual risks

Preset balance and reactive Hunter alternatives need explicit product review in W1. Kani is not
a substitute for this decision. Scope and version metadata insufficiencies are recorded before
Phase 4 but are not silently repaired by game PRs. Full Phase-3 exit remains subject to the
roadmap's wider portfolio gates, not just these ten PRs.

## Next dependency

Start W1 after confirming Phase 2 exit; use [future-social-voice](04-future-social-voice.md) only
after the named later gates.
