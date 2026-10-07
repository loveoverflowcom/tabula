# Verification selection and test design

Supporting reference for [Tabula engineering](../SKILL.md). The foundation owns the overall
contract-to-implementation workflow, capability discovery, check status, and evidence ledger.
Use this file to choose an adequate oracle and design meaningful assertions; it is not another
workflow entrypoint. [Strategy patterns](strategy-catalog.md) provide edge derivation and law shapes.

## Select an oracle for the named claim

Different methods detect different failure classes; a strong result for one claim does not
establish another. Compute the finite space before adding a heavier tool. Choose each method
only when it contributes a needed oracle, rather than following every row.

| Claim / shape | Cheapest adequate evidence | Technique detail |
|---|---|---|
| invalid states can be prevented | private construction barrier / stronger input type, plus boundary checks | [types as proofs](types-as-proofs.md) |
| small pure rule with few input partitions | table tests with independently stated expected results | semantic assertions below |
| manageable finite input / reachable state domain | enumerate it completely against a domain predicate or model | [exhaustive enumeration](replay-differential-testing.md#3-exhaustive-enumeration-of-finite-domains) |
| external specification or published vectors | committed independently sourced expected values | [published data](replay-differential-testing.md#2-published-reference-data) |
| optimized implementation | slow, structurally different reference model | [reference models](replay-differential-testing.md#1-reference-models) |
| law over a large samplable space | generated inputs against a law, with useful shrinking | [property testing](property-testing.md) |
| reducer / state machine | sequences over reachable states, including deliberate invalid commands | [state machines](property-testing.md#state-machines) |
| output must not depend on a secret | secret scrambling under authorized public equivalence; compare unauthorized output | [noninterference](property-testing.md#noninterference-the-property-that-catches-derived-leaks) |
| live/replay, migration, target or build equality | recorded checkpoints, compatibility fixtures, executed target comparisons | [replay / target comparison](replay-differential-testing.md#4-self-differential-replay-targets-and-builds) |
| hostile bytes; panic, hang, allocation risk | raw-byte target, realistic corpus, explicit bounds | [fuzzing](fuzzing.md) |
| arithmetic / small-state proposition too large to enumerate | nonvacuous bounded model check with explicit independent postcondition | [Kani](kani.md) |
| assertion strength in a stable pure module | scoped mutation campaign with classified survivors | [mutation testing](mutation-testing.md) |
| existing synchronization primitive | small model of relevant interleavings | tool considerations below |

## Design semantic assertions

- Assert structured outputs and exact domain error variants where the contract distinguishes them.
- Name tests as claims, such as `legal_move_preserves_piece_count`.
- Use pure `#[test]` for rules. Keep mocks for adapter contracts; assert call counts only when
  ordering or count is itself the contract.
- Use snapshots for stable reviewable output, not a domain truth that deserves an explicit assertion.
- A rejected operation must preserve canonical bytes, context obligations, and all other observations
  required by its contract. An advertised legal command must be accepted; do not discard its error.
- Failed setup must fail the check. `let Ok(x) = setup() else { return };` can hide the very
  invariant the check claims to exercise.
- Confirm selected tests and relevant guarded cases actually execute. The foundation's status
  rules apply to examples, corpora, model harnesses, and campaigns alike.

## Derive the edge partition

Select relevant classes from constructors, state enums, versions, and trust boundaries:

- empty, singleton, minimal valid, typical, maximal valid;
- immediately below / at / above numeric, size, time, and version limits, without overflowing
  while constructing the test case;
- duplicates, permutations, ties, and order-sensitive inputs;
- malformed, truncated, overlong, trailing, unknown, and corrupt encodings;
- overflow, underflow, and allocation limits;
- every input variant and legal/illegal transition, terminal states, and repeated commands;
- unauthorized viewers/actors, wrong-resource or stale witness reuse;
- retry, duplicate delivery, cancellation, timeout, and recovery;
- supported old/new schema versions and failed migrations;
- byte/UTF-8/UTF-16 offsets and normalization when text is involved;
- identical seeded inputs across the targets and build modes in the changed claim.

Use [strategy patterns](strategy-catalog.md) when these partitions or a law's shape remain unclear.
Do not add hypothetical cases unrelated to the contract.

## Keep expected results independent

Never calculate the expected result by calling the implementation through a second wrapper.
Adequate alternatives include a simpler reference model, an algebraic relation, a separately
reviewed canonical fixture, published external data, or a prior compatible version. A domain
predicate can check preservation without copying the transition algorithm.

Live-versus-replay comparison establishes agreement between paths, not rule legality. Same-target
repeatability establishes only that scope. Mutation results establish assertion strength, not
specification correctness. Combine evidence only where the changed claim needs both.

Preserve a generated failing seed and minimized input as an ordinary committed regression before
refactoring. Corpus regeneration is an explicit reviewed operation, never ordinary test output.

## Tool considerations

Do not infer current repository support from this list; use the foundation's capability discovery.

- **Loom** models small synchronization primitives under enumerated interleavings. Use it when
  concurrent code and a synchronization claim exist. Model the real ownership/drain/cache
  interaction with bounded threads and operations; it does not test unrelated actor behavior.
- **Miri** detects undefined behavior on executed paths. Safe domain logic in this workspace,
  which forbids unsafe code, usually has a cheaper relevant oracle. Revisit when an approved
  unsafe exception, FFI, or dependency behavior creates an actual UB question; state execution
  and tool limitations rather than claiming all transitive code is safe.
- **Flux / Verus / Creusot / Aeneas+Lean** require a maintained function contract or inductive
  invariant important enough to justify annotations and toolchain cost. Record the proposition,
  modeled/excluded behavior, trusted base, assumptions, reproduction artifacts, ownership, and
  why a simpler method is inadequate before adopting one.
- For bounded symbolic checks, read [Kani](kani.md), including reachability, stubs, and unwind
  assertions. A planned harness contributes no evidence.

## Cost and placement

Match actual repository wiring and task scope; these are placement criteria, not instructions to
create CI jobs or start remote campaigns.

| Placement | Suitable checks | Constraint |
|---|---|---|
| ordinary change / PR | fmt, lint, semantic tables, conformance, architecture gates, small pinned property suites, fast external vectors, compile-fail checks, target builds | cheap, deterministic, attributable |
| bounded deeper run | larger property/self-play suites, mutation/fuzz campaigns, model harnesses, deeper external data, executed target hash comparisons | explicit scope, reproducible output, time/resource budget |
| phase exit / release | full corpus and compatibility checks, classified mutation findings, security/projection audits, supported-target comparisons, relevant load scenarios | explicit phase/release gate and owner |

Keep a fast oracle in the ordinary suite when it is the only detector for a critical defect
class. Measure before deferring it. Do not turn every implementation task into a verification
campaign; use the foundation to report missing or unrun evidence honestly.
