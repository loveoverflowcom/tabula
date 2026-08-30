---
name: rust-verification-testing
description: Turn Rust invariants and failure modes into reproducible evidence using semantic unit tests, edge partitions, property and state-machine tests, replay/determinism checks, fuzzing, differential or metamorphic tests, concurrency tools, and justified formal verification. Use when implementing or reviewing nontrivial Rust behavior, fixing regressions, testing hostile input, enumerating edge cases, proving rejected operations are transactional, validating parsers/serialization/migrations, checking deterministic state machines, replacing mock-heavy tests, or deciding whether proptest, fuzzing, Loom, Miri, Kani, Flux, Verus, Creusot, or Lean is warranted.
---

# Rust verification testing

Produce evidence for named claims, not a large count of tests. Read the nearest `AGENTS.md`, the
repository's normative architecture, and its required commands before editing tests or manifests.

## Start with a verification ledger

Before implementation, write a small working table:

| Claim/invariant | Failure mode | Cheapest oracle | Test level |
|---|---|---|---|
| rejected command preserves state | partial mutation | canonical bytes before/after | property |
| normalization is stable | repeated changes | `normalize(normalize(x)) == normalize(x)` | property |
| projection hides secrets | unauthorized derivation | per-viewer secret scan | property/security |

Keep this table in working notes unless the repository has a durable location for it. Every added
test must map to a claim. Every high-impact changed claim must have evidence or an explicit gap.

## Workflow

1. Read public types, invariant comments/module docs, and existing tests before implementation.
   Search by symbol and theorem-like test names; do not load an entire workspace by default.
2. State the changed invariant and define the oracle independently of the new implementation.
3. Partition inputs into valid, invalid, and boundary classes. Include hostile and degenerate
   cases relevant to the type.
4. Add the smallest deterministic regression/example test. For a bug, make it fail first when
   practical and preserve the minimized counterexample.
5. Add one property, model, replay, or differential check when examples cannot cover the space.
6. Implement or fix the behavior.
7. Run the single test, then the module/crate suite, then repository checks in required order.
8. Report exact commands/results and classify what remains unverified.

Do not add a test dependency or verification tool unless repository policy and phase gates allow
it. Prefer existing harnesses. A planned future tool is not evidence.

## Choose the cheapest adequate evidence

Escalate only as needed:

1. **Type checking and exhaustive matches** — impossible states and missing cases.
2. **Semantic unit/table tests** — examples, exact errors, and boundary values.
3. **Property/state-machine tests** — laws and long transition spaces.
4. **Metamorphic/differential tests** — no simple oracle, but relations/reference exist.
5. **Fuzzing** — hostile bytes, parsers, decoders, command sequences, no-panic claims.
6. **Replay/cross-build checks** — determinism and compatibility.
7. **Loom/Miri/sanitizers** — concurrency schedules or semantic/UB bugs.
8. **Kani/Flux/Verus/Creusot/Aeneas+Lean** — critical bounded or deductive proof obligations.

Read `references/strategy-catalog.md` only when choosing beyond ordinary unit tests or when the
edge space is unclear.

## Design semantic tests

- Assert structured outputs or exact domain error variants, not only `is_err()`.
- Name tests like theorem statements: `legal_move_preserves_piece_count`.
- Keep arrange/act/assert data small enough to inspect in one screen.
- Prefer pure `#[test]` for rules. Use mocks only for adapter contracts.
- Avoid asserting internal call counts unless ordering/call count is itself the contract.
- Snapshot stable, reviewable output—not business truth that deserves structural assertions.
- Treat human-readable error text separately from structured error semantics.

## Edge-case partition

Select only relevant classes, but inspect each category:

- empty, singleton, minimal valid, typical, maximal valid;
- just below/at/just above each numeric, size, time, or version boundary;
- duplicates, permutations, unstable ordering, ties;
- malformed/truncated/extra/unknown fields and invalid encodings;
- integer overflow/underflow and allocation/length limits;
- every enum/state transition, including terminal and repeated commands;
- unauthorized viewers/actors and cross-resource witness reuse;
- retries, duplicate delivery, cancellation, timeout, and recovery;
- old/new schema versions and migration failure;
- Unicode normalization, byte/UTF-8/UTF-16 offsets when text is involved;
- identical seed/input replay across relevant targets/build modes.

Do not manufacture irrelevant cases. Derive partitions from constructors, state enums, protocol
versions, and security boundaries.

## Test laws, not generated noise

High-value property shapes:

| Law | Shape |
|---|---|
| Round trip | `decode(encode(x)) == x` for supported values |
| Idempotence | `f(f(x)) == f(x)` |
| Invariant preservation | `valid(s) && ok(step) => valid(s')` |
| Transactional rejection | `Err(step) => bytes(s') == bytes(s)` |
| Determinism | same state/input/context gives identical outcome and bytes |
| Replay equivalence | live evolution equals replayed ordered facts |
| Projection noninterference | changing hidden data does not change unauthorized view |
| Symmetry | relabeling equivalent actors transforms results consistently |
| Model agreement | optimized implementation equals a small reference model |

Build valid generators through public constructors. Generate raw invalid inputs separately. Ensure
shrinkers preserve validity; otherwise minimized failures may be meaningless.

## State transitions and hostile input

For reducers/game rules, generate action sequences, not just isolated states. At every accepted
step assert invariants; at every rejected step assert state byte identity and absence of emitted
facts/effects unless the contract explicitly says otherwise. Include repeated terminal actions,
out-of-turn actors, invalid identifiers/indices, duplicate sequence numbers, and timer/admin/seat
inputs where the state machine supports them.

For parsers/decoders, combine:

- table tests for grammar and diagnostics;
- round-trip properties for canonical values;
- arbitrary-byte fuzzing for no panic and bounded resource behavior;
- fixture tests for every supported schema/protocol version.

## Keep the oracle independent

Do not compute expected output by calling the implementation under test through a second path.
Use one of:

- a simpler obviously-correct reference model;
- an algebraic relation;
- a canonical fixture generated by a separately reviewed process;
- a previous compatible version for differential tests;
- a domain invariant checked without duplicating the transition algorithm.

When a property fails, record seed/input and preserve the minimized counterexample as a focused
regression test before broad refactoring.

## Verification commands

Run targeted-to-broad and keep output attributable:

```text
single test → module/crate tests → property/replay/fuzz check → crate lint/check → workspace gate
```

For Tabula, use the commands required by `AGENTS.md`; game changes also require the conformance
and secret projection checks. Do not reorder or skip a mandated gate silently. If a slow or
unavailable tool was not run, say so precisely.

## Formal verification discipline

Formal tools need a written proof target, model boundary, and assumptions. Before adopting one,
record:

- proposition and impact if false;
- modeled inputs/state size and excluded behavior;
- trusted code/toolchain assumptions;
- reproduction command and expected artifact;
- CI or scheduled ownership;
- simpler techniques that were insufficient.

Kani is useful for bounded state exploration; Flux for refinement contracts; Verus/Creusot for
deductive Rust-like proofs; Aeneas+Lean when Lean-level reasoning is worth the translation cost.
Tool names alone do not strengthen confidence.

## Completion report

Return a concise ledger:

```text
Invariant: ...
Evidence: test/property/tool → command → pass/fail
Edge classes covered: ...
Not run / residual risk: ...
```

Use `rust-ai-doc-contracts` only when a durable law-to-test link will materially reduce future
discovery cost. Keep ordinary test explanations in theorem-like names and assertions.

