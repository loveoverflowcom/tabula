# Goal

Map Caro claims to the cheapest adequate oracle. These are planned evidence levels; no Caro rules
have been implemented or tested by this documentation task.

## Current state

No tests exist. `conformance!` supplies 11 fixture-driven tests, not automatic property generation,
selfplay, golden replay or budget enforcement. `selfplay::run` measures synthetic apply latency and
checks repeated-run outputs; it does not prove gameplay correctness or run SecretModel. Caro has
no hidden information and needs no SecretModel.

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| L1 Placement legality | Occupied overwrite, invalid seat/coordinate, wrong turn | Valid/invalid boundary table over independent predicate, including all SeatIds and raw coordinate bounds | example-tested | Every PR | More complex future variants need new partitions |
| L2 Four-direction geometry | Missed diagonal/edge/corner or wrapped row | Enumerate all length-five windows at every supported size; remove each stone and add blockers/overlines | example-tested (exhaustive over stated geometry) | Every PR | Not exhaustive board-state verification |
| L3 Detector agreement | False positive/negative in mixed board | Whole-board reference vs last-move detector over reachable nonterminal prefixes and final move | differentially-tested | Every PR | Oracle shares no production scan helper; returned line checked separately |
| L4 Turn switching | Double turn or terminal turn inconsistencies | Generated accepted placements: switch iff play continues, retain last active seat if terminal | property-tested | Every PR | Resign/timeout are separate terminal paths |
| L5 Draw and win precedence | Full board hangs; final win misreported draw | Explicit draw pattern from C-D7 at all sizes, alternating legal trace; final-cell-win fixture | example-tested | Every PR | Fixture construction must itself assert no earlier win |
| L6 Terminal behavior | Double EndMatch or post-end mutation | Line/draw/resign/timeout/admin-cancel cases, every Input variant after end | example-tested | Every PR | No human-input liveness claim |
| L7 legal_commands consistency | Missing off-turn resign or illegal hint | For reached states and every roster/unknown seat, compare full typed alphabet to apply at open logical time; stable order/no duplicates | property-tested | Every PR | Due timer can make stored hints stale; apply remains authority |
| L8 Transactionality/R8 | Late bound/deadline mutation | Canonical bytes before/after hostile typed input plus legal follow-up; baseline invalid/probe fixture | property-tested | Every PR | Actual transport-byte decoder excluded |
| L9 Current-build determinism | Unstable iteration or RNG use | Fixed conformance scenarios + generated double runs, events/effects/bytes/index assertions and snapshot continuation | example-tested + property-tested | Every PR | Single target only |
| L10 Historical replay | Behavior silently changes old games | Three committed reviewed scripts, literal checkpoint/final hashes, Exact verdict and terminal outcome | differentially-tested (self-differential) | Every PR | First generation is a reviewed baseline, not an independent proof of rules |
| L11 Bot/selfplay progress | Always resign, missing move, infinite campaign | Trivial/Easy legal projection tests, bounded 64-match crate campaign; 100k at 9/15/19 nightly | property-tested | Nightly | PR campaign is smaller; synthetic policy not human play |
| L12 Assertion strength | Deleted win/legality guard stays green | cargo-mutants list first, scoped kernel/validators, classify every survivor | mutation-tested | Phase exit | Cannot repair an incorrect specification |
| L13 Size/budget | Wrong StateSizeClass or burst budget | Encoded-state size at every size/terminal reason; event count assertion and synthetic selfplay latency | example-tested (measurement) | Phase exit | Hardware-sensitive timing; not production-observed |
| L14 Timer policy | Early/disabled/stale timer forfeits a player | Matching-id before/at/after deadline, rearm, duplicate old id, overflow tables | example-tested | Every PR | Delivery/persistence Phase 4 |
| L15 Native/WASM equality | WASM builds but hashes differ | Execute same input vectors across native debug/release/WASM and compare checkpoints | cross-target-tested (when executed) | Phase exit | Current WASM CI is compile-only; generic runner is a separate tooling dependency if absent |
| L16 SDK boundaries | Adding a game changes platform rules | check-deps/check-no-game-ids plus diff accounting against starting develop | statically checked | Every PR | Manual review required for changes without game-id literals |

## Decisions

Property testing is primary for reachable sequence laws, rejection and legal-command equality;
use 128 cases/property per PR, 2,048 nightly, shrink choice/input sequences and reconstruct from
create. Include late-game prefixes and hostile cases separately. Do not discard failing generator
setup with `.ok()` or early returns; commit minimized regressions.

Differential testing is primary for the fast win detector versus the slow board oracle. Enumerating
572 windows is strong geometry evidence, not evidence for all 3^225 states. Mutations are useful
once win/legality kernels stabilize; run only the package/file scope and classify true gaps,
equivalents, unreachable paths, verifier-only noise, limitations and low-value survivors.

Kani is not required: the chosen predicates have cheap boundary/enumeration/reference oracles.
The full board state space is huge, so “finite” alone is not the justification. Upstream logical-time
proofs do not prove this game's deadline policy. Reconsider only a named arithmetic/pattern defect
that cheaper tests cannot adequately cover, with proposition/bounds/assumptions/trusted code/exclusions.

Fuzzing is not applicable to this planning scope: no new exposed byte decoder is introduced.
Typed hostile inputs use tables/properties. A future erased codec should be fuzzed at that boundary.

## Proposed architecture and replay plan

Baseline GameTestFixture lands in C2; use an active nonterminal deterministic_script with at least
one legal placement. invalid_command has a genuinely legal probe from that same state; terminal
scenario uses resignation; randomness() may be None because rules never draw RNG. Keep broader
laws outside the fixture macro. Time-aware tests use explicit Ctx; Scenario uses 1-second ticks.

C4 adds root artifacts `tests/replays/caro-{normal,draw,timeout}-golden.tbr`, game tests/replay.rs,
xtask dependency/selfplay/replay dispatch and golden writers. Normal is an explicit winning script;
draw uses the alternating no-five pattern; timeout contains a due Input::Timer with recorded time.
Do not generate all three by “first legal cell” and assume their claimed coverage holds.

Tests only read goldens. Assert ReplayVerdict::Exact (not only is_verified), literal final hash,
checkpoint evidence and expected outcome. A behavior change needs rules_version review; source/test
edits inside the hashed rules subtree can change identity without changing semantics. Regeneration
is explicit and reviewed, never automatic acceptance of a new expected result.

Selfplay bot module is enabled for cfg(test) or bots; put ordinary default-build campaign in an
in-crate test or feature-gate an integration test and explicitly run that feature. Do not claim an
integration test enables its own crate's features. Rules no-RNG invariant excludes bot RNG.

## Expected file changes

`games/caro/tests/{rules,conformance,determinism,replay,state_size}.rs`, independent test oracle;
`src/bot.rs`, in-crate campaign; Cargo.toml; root replay files/README;
`xtask/{Cargo.toml,src/selfplay_cmd.rs,src/replay_cmd.rs,src/replay_goldens_cmd.rs}` and README;
nightly workflow; docs/games/caro.md. CLI must add proposed `--board-size` support explicitly.

## Acceptance criteria

- [ ] L1..16 have tests/evidence or named unexecuted phase gates; no conformance overclaims.
- [ ] Every size's draw trace and line geometry pass independent checks.
- [ ] Three Exact goldens have visibly different intended coverage and literal hashes.
- [ ] Selfplay at 9/15/19 is executable via the documented proposed board-size flag.
- [ ] Mutation survivors classified; no Kani/fuzzing added without a new named need.
- [ ] `cargo xtask check` green; cross-target execution is not inferred from compilation.

## Residual risks

Current nightly `replay --all` is unsupported; per-file replay dispatch and explicit file invocations
are needed until generic batch tooling is repaired. Its fuzz/load jobs also reference absent targets.
These are separate pre-existing workflow gaps, not reasons to expand Caro scope. Tests and external
oracles can catch changes too; a golden is not the only evidence that survives a code change.

## Next dependency

[C3/C4](04-pr-sequence.md), then [presentation](03-presentation-integration.md).
