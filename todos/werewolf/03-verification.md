# Goal

Choose evidence per Werewolf invariant. All rows below describe **planned evidence**, not results
already achieved by this planning-only change. Start with the router skill, then use properties,
independent reference models and scoped mutation testing only for their named claims.

## Current state

No Werewolf tests exist. Available reusable surfaces:

- `conformance!` expands 11 fixture-driven example tests, not automatic generated properties.
- `projection_security!` drives SecretModel containment on reachable steps, separately from conformance.
- `assert_projection_noninterference` and `assert_view_event_noninterference` compare canonical
  bytes, including Option existence; `*_differs` supplies authorized positive controls.
- `selfplay::run` repeats matches and compares outputs; it does not check secrecy authorization.
- ReplayRunner reads committed `.tbr` and reports graded identity/evidence. Explicitly assert Exact.
- `Scenario` fixtures assign `now = input_index * 1000ms`; they cannot accept arbitrary timestamps.
  Golden replay frames can carry actual logical times. Do not weaken timer rules for fixture convenience.

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| V1 Preset/assignment is valid | Wrong counts, duplicate/missing seats | Enumerate 15 preset rows, sparse SeatIds, invalid configs; permutation/count property | example-tested + property-tested | Every PR | Seeded shuffle fairness is not game balance |
| V2 Assignment is deterministic | Unstable seat iteration or RNG domain | Fixed assignment vectors; same seed with permuted roster order; alternate seed | example-tested + property-tested | Every PR | Alternate seeds need not always yield distinct assignment |
| V3 Phases/timers obey contract | Early, duplicate or stale closure | All phase×input partitions, before/at/after deadline, unknown timer, checked overflow | example-tested | Every PR | Actual delivery and timer persistence Phase 4 |
| V4 Rejection is transactional | Potion/ballot/phase partially mutated | Full canonical bytes and legal follow-up around every error class over reachable prefixes | property-tested | Every PR | Driver counter/RNG trace needs separate assertions |
| V5 Night authorization | Villager heals, wolf attacks teammate, second potion action | Exhaustive role×action tables plus target/resource boundary partitions | example-tested | Every PR | Unauthorized error/affordance equivalence covered in V10 |
| V6 Simultaneous resolution | Arrival order changes death/save/poison/outcome | Independent slow set-based resolver; permuted commuting actions | differentially-tested | Every PR | Neither resolver decides whether recommended rules are desirable |
| V7 Vote and ties | Abstain counts as candidate, false majority, unstable tie | Exhaustive short ballot vectors for small rosters; table models at 6/12/20 seats | differentially-tested | Every PR | Full 20-seat ballot state space is not enumerated |
| V8 Victory/death/standings | Early parity check; duplicate death; missing dead seat | All wolf/nonwolf count pairs with total ≤20; simultaneous empty-alive case; roster-rank invariants | example-tested + property-tested | Every PR | Reachable game distribution is separate from count partition |
| V9 View secrecy | Revealed role, private counts or resource leaks | Matrix examples plus authorized-equivalence paired properties | property-tested | Every PR | Generator coverage and wire boundary |
| V10 Event non-existence/errors | Some(Redacted), length/order or error variant leak | Explicit None cases, Option comparison, hidden-action/no-action visible-stream comparison, same-probe error equality | property-tested | Every PR | Timing of real frames not executed |
| V11 Transient secret containment | Cleared night facts leak through events | Both SecretModel hooks, actual nested-byte tokens, injected leak controls | example-tested | Every PR | Scalar/short token coverage explicitly excluded |
| V12 View authorization expands correctly | Dead treated as outsider, outsider as Audit | Before/after death/end/reconnect tables and positive controls | example-tested | Every PR | Cached UI/fan-out transitions Phase 7 |
| V13 Scope values are exact | Wrong wolf/dead membership or mutable deltas | Entire expected table, stable order and repeated absolute effect bytes | example-tested | Every PR | Does not prove chat/SFU enforcement; voice listen-only absent |
| V14 Replay is exact | RNG/time/resource state diverges after restore | Literal golden hashes/checkpoints + snapshot continuation at every phase | differentially-tested | Every PR | Same-build reconstruction alone cannot prove historical correctness |
| V15 Runtime substitution remains forbidden | Simulation wrapper escapes into runtime | Bot None under all features; capability equality; OccupantChanged rejection | example-tested | Every PR | Real replacement authorization belongs to future runtime |
| V16 Liveness under timer delivery | All-pass infinite game; duplicate EndMatch | All-pass max-round scenario plus bounded simulation with terminal reason counts | example-tested + property-tested | Every PR | Unbounded hostile traffic/no timers excluded; distinguish draw-cap from decisive games |
| V17 Rare sequences remain deterministic/secret | Deep trace bug never exercised | 100k simulation runs across seat/preset matrix + expanded property/secrecy runs | property-tested | Nightly | Synthetic policy is neither human gameplay nor production evidence |
| V18 Security/win guards are asserted | Removing guard stays green | List then narrow cargo-mutants campaign over projection/event/vote/win | mutation-tested | Phase exit | Assertion strength, not proof of ruleset correctness |
| V19 Declared resource class/budget is honest | State history or resolution burst exceeds estimate | Canonical byte measurement at 6/12/20 including max rounds; xtask/selfplay timing and event counts | example-tested (synthetic measurement) | Phase exit | Hardware-sensitive latency; no production load claim |
| V20 Cross-target determinism | Native build passes but WASM differs | Execute same accepted-input vectors in native debug/release and a WASM execution harness; compare checkpoints | cross-target-tested (when executed) | Phase exit | Existing CI only compiles WASM; a generic runner may need a separate tooling PR |

## Proposed architecture and generator policy

Use `proptest` already permitted by deps.toml, 128 cases/property per PR and 2,048 nightly.
Generate legal prefixes with independent hostile-command branches; weight samples toward late Night,
Vote, deaths, exhausted potions and round cap. Cover every role and both vote modes at 6, 12 and
20 seats plus all 15 count constructors. Shrink the seed/config/choice sequence and rebuild from
create; do not shrink State fields into impossible histories. Commit minimized failures as named
ordinary tests. No proptest-state-machine dependency until ordinary sequence shrinking proves inadequate.

The independent resolver lives in `tests/support/reference_resolution.rs` (or a cfg(test) sibling
when private access is needed), uses explicit sets/tally arrays, and shares no production legality,
death, vote, outcome or projection helpers. Compare accepted/rejected decisions, deaths, resources,
results and final outcomes; assert canonical event order separately. Short exhaustive partitions
do not imply the complete Werewolf state space was enumerated.

Conformance: supply valid/invalid/terminal/randomness scenarios; end the main script in an active
Night or Vote so legal_commands sanity actually sees commands. For the harness's 1-second ticks,
use valid long-enough fixture windows and pad with documented harmless known-seat lifecycle inputs
when a future deadline must be reached. Assert the intended phase at each checkpoint. Direct
timestamp boundary tests use Ctx explicitly; replays use recorded logical times.

Projection security has its own fixtures, including roles/actions **and** public Some events while
secrets remain. Call scanners on every accepted state of security traces, including create; add
explicit missing-event class coverage outside the macro. Never call a deterministic-output comparison
a secrecy test. Audit positive controls are separate from the client viewer universe.

## Simulation and canonical replay

Follow W-D18's test-only SimulationModule; no new production bot API. Simulated choices stop after
one vote/night action per window so the lowest seat does not starve other roles. Add targeted
replacement-vote scripts separately. Test both ordinary play and all-pass policy; `max_inputs` is
a harness failure bound, never an implicit successful termination. Report decisive/draw-cap/abort
counts; reject a campaign that silently ends every match via hostile Admin(Cancel).

SimulationPolicy chooses from its projected eligible actions using the supplied DetRng: each active
role selects a legal target or pass; a living voter selects a legal public ballot once per window.
It never looks up canonical role maps or predicts another actor's choice. Include fixed policies
that pass, spend both potion charges on different nights, and exercise a Hunter mark so random
selection cannot leave those paths uncovered.

W9 adds `xtask selfplay werewolf --seats N --matches M` by dispatching to SimulationModule, plus
xtask dependency and replay dispatch. These commands are **proposed**, currently unsupported.
Run 100k per chosen nightly roster 6/12/20; report elapsed time before choosing CI shard budgets.
Keep a bounded 64-match crate test for PRs; properties supply broader semantic coverage.

Commit at least three synthetic canonical replays under root `tests/replays/`:

| File | Required scenario and assertions |
|---|---|
| werewolf-normal-golden.tbr | Multi-round play with night kill, public vote and decisive outcome |
| werewolf-edge-golden.tbr | Doctor/heal vs poison, Hunter retaliation, tied vote and batch parity |
| werewolf-timeout-golden.tbr | Missing night choices/ballots, disconnect/reconnect and due timers; finite terminal outcome |

Use explicit reviewed scripts and recorded times independent of bot heuristics. Pin Exact identity,
literal final hash, terminal outcome and per-input checkpoints. Generation uses the named
`xtask replay-goldens` writer; tests only read. Semantic changes require rules_version review;
source-only/test/comment edits inside rules also change RULES_HASH and may require intentional header
regeneration without a semantic version bump. Avoid unrelated corpus rewrites.

Current nightly `replay --all` is unsupported (gap G11). W9 adds per-file replay dispatch;
repair batch discovery as separate generic tooling work or configure CI to invoke supported file
paths. Do not claim new files are automatically verified by that broken nightly command.
Selfplay currently reports seeds but does not auto-commit failure replays: explicitly plan an xtask
failure artifact/report path, distinct from committed goldens, or retain seed/index for manual replay.

## Headless projection viewer (Phase-3 demo)

W9 adds proposed `cargo xtask inspect-projections werewolf --fixture normal --step N`.
Use deterministic synthetic fixtures only. Show public phase/deadline and side-by-side projected
columns for each role, a real dead seat, live/delayed outsider and explicitly labelled Audit.
Show the authorized visible event stream, including an explanatory `no visible event` comparison
for private submissions in this **audit test tool**. Do not make that indicator a client payload.
Never render State Debug as a substitute for project; separate audit-only canonical input/hash labels.
No Macroquad, RenderList, web server, accounts, socket or chat transport. CLI output snapshot plus
structural assertions checks columns against project/view_event. W10 records a reviewed demo transcript.

## Kani and fuzzing decisions

**Kani: not required.** Count partitions, role authorization and the chosen set resolver have cheap
enumeration/reference oracles. Upstream LogicalTime proofs do not eliminate deadline policy tests.
Only reconsider after a concrete escaped bug needs symbolic evidence. A candidate scope would be:

| Field | Candidate (inactive, not evidence) |
|---|---|
| Proposition | For a pure fixed-width viewer-role/dead authorization predicate, toggling an unauthorized role bit never changes allowed disclosure |
| Bounds | At most 20 seats, u32 masks restricted to those seats, six role discriminants |
| Assumptions | Valid disjoint role/dead masks, existing viewer, fixed phase and public facts |
| Trusted code | Predicate translation/model, valid-state assumptions, Rust frontend/CBMC and pinned Kani toolchain |
| Excluded behavior | BTreeMap State, serde, RNG, complete phase traces, output bytes, network timing and scope enforcement |
| Trigger | Noninterference/mutation evidence leaves a named high-impact gap that finite tables cannot cheaply cover |

If this trigger fires, load rust-kani, name the harness/bounds/reproduction command and explain why
the simpler oracle failed before adding it. No claim of formal verification in this plan.

**Fuzzing: not applicable to these typed rule inputs.** Hostile typed commands use tables/properties.
There is no new game-specific byte decoder. Decoder fuzzing belongs to the actual erased codec in
Phase 4. State deserialization gets constructor/round-trip/invalid DTO tests; introduce a fuzz target
only when a real exposed byte/resource boundary warrants it.

## Expected file changes

`games/werewolf/tests/{assignment,phases,night,voting,determinism,conformance,replay,state_size}.rs`,
`tests/support/` models; in-crate projection/event/SecretModel tests; `src/simulation.rs`;
`xtask/{Cargo.toml,src/selfplay_cmd.rs,src/replay_cmd.rs,src/replay_goldens_cmd.rs,src/main.rs}`,
new `xtask/src/projections_cmd.rs`, xtask README, root replay artifacts/index, nightly jobs and
`docs/games/werewolf.md`. No platform API edits are prerequisites.

## Acceptance criteria

- [ ] Each V1..20 row has an owned test/artifact or explicit unexecuted phase gate.
- [ ] 6/12/20 secrecy coverage includes every preset-supported role and before/after death.
- [ ] Every event's None/Some behavior is asserted; no empty token-model or constant-view pass.
- [ ] Three reviewed goldens, synthetic projection demo and bounded simulation terminate correctly.
- [ ] Mutation survivors classified; genuine secrecy survivors become regressions before approval.
- [ ] Core gate green; no falsely claimed WASM execution, production measurement or socket security.

## Residual risks

Separate shipped-rule evidence from source inspection, synthetic observations and scheduled work.
Nightly runtime/corpus retention needs explicit CI ownership; stale workflow comments do not run tests.

## Next dependency

[W8–W10](05-pr-sequence.md); [future social/voice gates](04-future-social-voice.md).
