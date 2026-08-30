# Verification strategy catalog

Use this reference to choose a method after naming the invariant. Do not add all tools.

## Contents

1. Edge-case derivation
2. Property and metamorphic patterns
3. Stateful systems
4. Specialized tools
5. Selection matrix

## 1. Edge-case derivation

Derive cases from the representation and boundary:

| Source | Questions |
|---|---|
| Smart constructor | Which values sit adjacent to each predicate boundary? |
| Enum/state machine | Which transitions are legal, illegal, terminal, or repeatable? |
| Collection invariant | What do empty, singleton, duplicate, reordered, split, and merged inputs do? |
| Arithmetic | Can intermediate operations overflow even when the final value fits? |
| Parser | What happens for empty, truncated, invalid tag, overlong, unknown, and trailing input? |
| Authorization | Can evidence for actor/resource A be replayed for B? |
| Serialization | Which old/new versions, unknown fields/variants, and corrupt bytes matter? |
| Concurrency | Which ordering, cancellation, retry, duplicate, or lost-wakeup interleavings matter? |
| Projection | Can hidden data influence an unauthorized output, size, ordering, or error? |

Boundary triples (`n-1`, `n`, `n+1`) are useful only after checking overflow when constructing the
adjacent value.

## 2. Property and metamorphic patterns

### Round trip

```text
decode(encode(x)) == x
```

Specify whether equality is semantic or byte identity. For a canonical encoder, also test that
equivalent values produce one encoding.

### Idempotence

```text
normalize(normalize(x)) == normalize(x)
```

Useful for canonicalization, deduplication, and migrations designed to be rerunnable.

### Invariant preservation

```text
invariant(initial)
for action in actions:
    before = canonical(initial)
    result = step(initial, action)
    if accepted: invariant(initial)
    if rejected: canonical(initial) == before
```

### Symmetry

Relabeling equivalent seats, nodes, or IDs should relabel the result without changing semantics.
This exposes hidden dependence on numeric identity or iteration order.

### Metamorphic relation

When exact output is hard to calculate, transform input in a way with a known relation:

- add an unrelated node; existing reconciliation mappings stay unchanged;
- permute an unordered input; canonical output stays identical;
- split then concatenate chunks; streaming decode matches whole decode;
- replay from a snapshot; final state matches replay from genesis.

### Differential model

Write the smallest reference, even if slow. Keep it structurally different from the optimized
implementation. Compare results over generated inputs and retain mismatches as fixtures.

## 3. Stateful systems

Model commands and observations separately:

```text
ModelState --Command--> ModelState + ExpectedObservation
RealState  --Command--> RealState  + ActualObservation
```

After each command compare public observations and invariants. Generate invalid commands on
purpose; a generator that only emits legal actions cannot prove rejection behavior.

For deterministic games include all input variants that can mutate a match, not only player
commands. Verify:

- one ordered stream produces one state path;
- the same seed/context and inputs produce identical canonical bytes;
- invalid input does not mutate or emit accepted events;
- replay checkpoints match live state hashes;
- projections/events expose no unauthorized secret;
- terminal states reject or explicitly define subsequent inputs.

## 4. Specialized tools

### Fuzzing

Use for parser/decoder robustness, untrusted bytes, command sequences, and no-panic claims. Add
resource bounds or size limits; “does not panic” is insufficient if a tiny input causes enormous
allocation or time.

### Loom

Use for small synchronization primitives and schedule-sensitive code. Reduce the model: few
threads, bounded operations, no unrelated I/O. State the synchronization property before writing
the model.

### Miri and sanitizers

Use Miri for aliasing and undefined-behavior checks in supported code; sanitizers for memory,
thread, or address issues across larger native executions. They are complementary, not proofs of
domain behavior.

### Kani

Use for bounded exhaustive checks where the state/input bounds are small and meaningful. Record
the bounds in the property name/docs; a proof for length `<= 4` is not a proof for arbitrary
length.

### Flux, Verus, and Creusot

Use when function contracts, arithmetic, or inductive preservation are important enough to carry
annotations and toolchain cost. Separate verified kernels from unverified adapters.

### Aeneas and Lean

Use when the key value is a theorem maintained in Lean or connection to a larger formal model.
Keep the translation boundary small, avoid unsupported Rust features, and document correspondence
between executable code and theorem artifacts.

### Mutation testing and coverage

Mutation testing asks whether assertions kill plausible defects; coverage asks what executed.
Neither proves correctness. Use mutation testing selectively on stable pure cores. Use coverage to
find blind spots, not as a target percentage detached from risk.

## 5. Selection matrix

| Risk/shape | First choice | Escalate when |
|---|---|---|
| Small pure rule | table tests | input partition is large → property tests |
| Algebraic transform | property tests | no oracle → metamorphic/differential |
| Parser/decoder | tables + round trip | hostile bytes → fuzzing |
| Reducer/state machine | sequence properties/model | small critical state → Kani |
| Replay/canonical bytes | fixtures + differential runs | cross-target risk → CI matrix |
| Synchronization primitive | focused tests | schedule risk → Loom |
| Unsafe/FFI | safety invariants + Miri | broader native risk → sanitizers |
| Critical arithmetic/refinement | properties | assurance gap remains → Flux/Verus/Creusot |
| Formal domain theorem | executable checks | theorem reuse justifies → Aeneas+Lean |
