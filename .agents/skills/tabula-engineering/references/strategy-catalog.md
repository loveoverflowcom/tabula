# Verification strategy catalog

Supporting reference for [Tabula engineering](../SKILL.md). Use these patterns after naming the
invariant. Method selection lives in [verification selection](verification-testing.md); detailed
generators and shrinking live in [property testing](property-testing.md).

## Contents

1. Edge-case derivation
2. Property and metamorphic patterns
3. Stateful systems

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
