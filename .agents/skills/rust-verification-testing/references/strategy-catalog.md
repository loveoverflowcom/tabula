# Verification strategy catalog

Use this reference to choose a method after naming the invariant. Do not add all tools. Sections 1–3
are shared technique; sections 4–5 route to the specialized skills rather than duplicating them.

## Contents

1. Edge-case derivation
2. Property and metamorphic patterns
3. Stateful systems
4. Specialized tools — where the detail lives
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

## 4. Specialized tools — where the detail lives

This catalog no longer duplicates per-tool guidance. Each tool has its own skill; open the one the
router sent you to, and use the table below only to confirm you are in the right place.

| Tool | Skill | One-line scope | Cannot tell you |
|---|---|---|---|
| proptest (+ state machine) | `rust-property-testing` | a law holds over sampled inputs, with shrinking | anything about inputs the generator never reaches |
| reference models, published data, exhaustive enumeration, replay, cross-target | `rust-replay-differential-testing` | the implementation agrees with an independent oracle | anything the oracle also gets wrong |
| cargo-mutants | `rust-mutation-testing` | your assertions kill plausible defects | whether the specification you asserted is right |
| Kani (CBMC) | `rust-kani` | a proposition holds over a symbolic domain, under stated assumptions and bounds | anything outside the domain, the assumptions, or the bound |
| cargo-fuzz / libFuzzer | `rust-fuzzing` | arbitrary bytes cause no panic, hang, or unbounded allocation | whether correct input is accepted, or output is right |
| Loom | not yet — see the router's *Not yet* section | a synchronization property holds over enumerated interleavings | anything without concurrent code to model |
| Miri | not applicable under `forbid(unsafe_code)` with no FFI — see the router | UB on executed paths | domain behaviour; unexecuted paths; FFI |
| Flux / Verus / Creusot / Aeneas+Lean | none yet | deductive contracts and inductive invariants | anything outside the annotated boundary |

Coverage is not on this list. Coverage says what executed; it does not say what was checked. Use it
to find blind spots, never as a target.

## 5. Selection matrix

Compute the size of the space **before** choosing. Most escalations are avoidable.

| Risk / shape | First choice | Escalate when |
|---|---|---|
| Small pure rule | table tests | input partition is large → property tests |
| **Finite reachable space** (thousands of states) | **enumerate all of it** | it stops being finite |
| Algebraic transform | property tests | no oracle → metamorphic or reference model |
| A standard exists (a published corpus, an RFC vector set) | **use the published data** | it does not cover the hard cases → add a reference model |
| Parser / decoder of trusted input | tables + round trip | — |
| Parser / decoder of **untrusted bytes** | tables + round trip | hostile bytes, hangs, allocation → fuzzing |
| Reducer / state machine | sequence properties over **reachable** states | small critical state and unbounded domain → Kani |
| Unbounded arithmetic | property tests | must hold for *every* value → Kani |
| Replay / canonical bytes | committed fixtures + live-vs-replay | cross-target risk → cross-target hash comparison |
| Secrecy / redaction | containment scan | derived leaks → noninterference property |
| Synchronization primitive | focused tests | schedule risk → Loom |
| Unsafe / FFI | safety invariants + Miri | broader native risk → sanitizers |
| "Are our tests any good?" | — | pure, stable module → mutation testing |
