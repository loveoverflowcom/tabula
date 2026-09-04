---
name: rust-verification-testing
description: Router and strategy skill for Rust verification. Turn a named invariant into the cheapest adequate evidence — semantic unit tests, edge partitions, property or state-machine tests, differential and replay oracles, fuzzing, mutation testing, bounded model checking, or concurrency tools — and report what remains unverified using precise evidence levels instead of the word "verified". Use when implementing or reviewing nontrivial Rust behavior, fixing a regression, testing hostile input, enumerating edge cases, proving rejected operations are transactional, validating parsers/serialization/migrations, checking deterministic state machines, replacing mock-heavy tests, or deciding which verification tool is warranted. Start here, then follow the router to a specialized skill.
---

# Rust verification testing — the router

Produce evidence for named claims, not a large count of tests. Read the nearest `AGENTS.md`, the
repository's normative architecture, and its required commands before editing tests or manifests.

This skill decides **what evidence a claim needs**. Five sibling skills say **how to produce it**:

```text
rust-property-testing              laws, generators, state machines, noninterference
rust-replay-differential-testing   reference models, published data, exhaustive enumeration,
                                   replay equivalence, cross-target comparison
rust-mutation-testing              whether your assertions actually kill defects
rust-kani                          bounded model checking, with an honest scope statement
rust-fuzzing                       untrusted bytes: panics, hangs, resource exhaustion
```

and two prevention skills: `rust-types-as-proofs` (make the invalid state unrepresentable) and
`rust-functional-core` (architecture that makes all of the above cheap).

## Start with a verification ledger

Before implementation, write a small working table:

| Claim/invariant | Failure mode | Cheapest oracle | Evidence level |
|---|---|---|---|
| rejected command preserves state | partial mutation | canonical bytes before/after | property over reachable states |
| normalization is stable | repeated changes | `normalize(normalize(x)) == normalize(x)` | property |
| move generation is legal | illegal games, divergence | published node counts | differential |
| projection hides secrets | unauthorized derivation | noninterference under secret scrambling | property |

Every added test maps to a claim. Every high-impact changed claim has evidence or a stated gap.

## Never collapse evidence levels into "verified"

These detect different failure classes and are **not** substitutes for one another. Name the level
you actually have:

```text
documented              a sentence in a doc or comment
type-enforced           the compiler refuses the alternative
statically checked      a lint, a grep, or a repo gate refuses it
example-tested          fixed inputs, hand-written expectations
property-tested         generated inputs against a law, with shrinking
differentially-tested   compared against an INDEPENDENT implementation or published data
mutation-tested         plausible defects are demonstrably killed by the assertions
bounded-model-checked   exhaustive over a symbolic domain, under stated assumptions and bounds
cross-target-tested     identical results on another architecture or target
production-observed     seen to hold on real traffic
```

Two rules that follow:

- A claim with a *strong* level in one column and nothing elsewhere is not broadly verified. Say
  which column.
- Never write "crate X is formally verified". Write *"harness H proves proposition P over domain D
  under assumptions A with bound B"*.

## The router

Answer in order; take the first row that matches.

| If the claim is about… | and… | go to |
|---|---|---|
| an impossible state | you can change the type | `rust-types-as-proofs` — prevention beats detection |
| a small pure rule | the input partition is small | plain table tests (below); no skill needed |
| a **finite** state or input space | you can enumerate all of it | `rust-replay-differential-testing` §exhaustive — walk it; do not sample, do not model-check |
| a specification that lives **outside** the code | published data or a standard exists | `rust-replay-differential-testing` §published data |
| an optimised implementation | a slow obvious twin is writable | `rust-replay-differential-testing` §reference model |
| a universal law over a large space | you can state the law without calling the code under test | `rust-property-testing` |
| a reducer or state machine | sequences matter more than single states | `rust-property-testing` §state machines |
| what an output does **not** depend on (secrecy) | — | `rust-property-testing` §noninterference |
| deterministic replay, migration, or cross-platform equality | — | `rust-replay-differential-testing` §self-differential |
| untrusted **bytes** and the risk is panic/hang/allocation | — | `rust-fuzzing` |
| unbounded arithmetic or a domain too large to enumerate | an independent oracle exists inside the harness | `rust-kani` |
| whether the tests you already have are strong | the module is pure and stable | `rust-mutation-testing` |
| interleavings of concurrent operations | the concurrent code exists | Loom — see *Not yet* below |
| undefined behaviour | there is `unsafe` or FFI | Miri — see *Not applicable* below |

### Not applicable / not yet

- **Miri** detects UB — out-of-bounds access, use-after-free, invalid values, aliasing violations,
  data races — but only on executed paths, with no FFI, and its own documentation notes that for
  pure safe Rust the compiler's type system already provides the guarantee. In a workspace that
  forbids `unsafe` and has no FFI, Miri is ceremony. **Trigger to revisit:** an approved `unsafe`
  exception, or a C/system dependency entering the graph.
- **Loom** enumerates interleavings for small synchronization primitives. It needs concurrent code
  to exist. **Trigger to revisit:** the first real actor/mailbox/cache with shared state — model
  the cache and the drain interaction, with two threads and bounded operations, and state the
  synchronization property before writing the model.
- **Flux / Verus / Creusot / Aeneas+Lean** are worth their annotation and toolchain cost only when
  a function contract or an inductive invariant is important enough to maintain in a proof
  language. Record the proposition, the trusted base, and what simpler techniques failed first.

## Workflow

1. Read public types, invariant comments/module docs, and existing tests before implementing.
   Search by symbol and by theorem-like test name; do not load a whole workspace.
2. State the changed invariant. Define the oracle **independently of the implementation**.
3. Partition inputs: valid, invalid, boundary, hostile, degenerate.
4. Add the smallest deterministic regression test. For a bug, make it fail first and keep the
   minimized counterexample.
5. Add one property, model, replay, or differential check where examples cannot cover the space.
6. Implement or fix the behavior.
7. Run targeted → module/crate → the repository's required gate, in the required order.
8. Report exact commands and results, and classify what remains unverified.

Do not add a test dependency or verification tool unless repository policy and phase gates allow
it. Prefer existing harnesses. A planned future tool is not evidence.

## Design semantic tests

- Assert structured outputs or exact domain error variants, not only `is_err()`.
- Name tests like theorem statements: `legal_move_preserves_piece_count`.
- Keep arrange/act/assert small enough to inspect on one screen.
- Prefer pure `#[test]` for rules. Use mocks only for adapter contracts.
- Do not assert internal call counts unless ordering or call count *is* the contract.
- Snapshot stable reviewable output — never business truth that deserves a structural assertion.
- **A check that can silently no-op is worse than no check.** `let Ok(x) = setup() else { return };`
  in a conformance helper turns a failing invariant into a green tick. Panic instead.

## Edge-case partition

Select the relevant classes; inspect each category:

- empty, singleton, minimal valid, typical, maximal valid;
- just below / at / just above every numeric, size, time, or version boundary;
- duplicates, permutations, unstable ordering, ties;
- malformed, truncated, extra, unknown fields; invalid encodings;
- integer overflow/underflow; allocation and length limits;
- every enum and state transition, including terminal and repeated commands;
- unauthorized viewers/actors; cross-resource witness reuse;
- retries, duplicate delivery, cancellation, timeout, recovery;
- old/new schema versions and migration failure;
- Unicode normalization and byte/UTF-8 offsets when text is involved;
- identical seed/input replay across relevant targets and build modes.

Derive partitions from constructors, state enums, protocol versions, and security boundaries — not
from imagination.

Read `references/strategy-catalog.md` when the edge space is unclear or when choosing between
oracle shapes.

## Keep the oracle independent

Never compute the expected output by calling the implementation through a second path. Use:

- a simpler obviously-correct reference model;
- an algebraic relation;
- a canonical fixture produced by a separately reviewed process;
- published external data;
- a previous compatible version;
- a domain invariant checked without duplicating the transition algorithm.

When a property fails, record the seed and preserve the minimized counterexample **as a committed
regression test** before refactoring.

## Formal verification discipline

Before adopting any formal tool, record: the proposition and the impact if false; the modeled
input/state size and the excluded behavior; trusted code and toolchain assumptions; the
reproduction command and expected artifact; CI or scheduled ownership; and which simpler techniques
were insufficient. Tool names alone do not strengthen confidence. See `rust-kani` for the bounded
case.

## Where each check belongs

| Tier | Contains | Rule |
|---|---|---|
| **Every PR** | fmt, lints, unit + table tests, conformance, architecture gates, compile-fail tests, small property suites with pinned case counts, fast published-data oracles, cross-target *builds* | cheap, deterministic, attributable |
| **Nightly** | large property runs, long randomized/self-play campaigns, mutation campaigns, fuzz runs, model-checking harnesses, deep reference-data levels, cross-target *hash comparison* | bounded, scheduled, owned |
| **Phase exit / release** | manual scenario scripts, full corpus re-verification, full mutation campaign with every survivor classified, security/projection audit, migration checks, load tests | gates, not habits |

Do not put research-grade verification in the per-PR tier. Equally, do not demote a fast oracle
that is the *only* detector for an important defect class — measure before moving anything.

## Completion report

```text
Invariant:            ...
Evidence level:       <from the list above>  (never the bare word "verified")
Evidence:             test/property/tool → exact command → pass/fail
Edge classes covered: ...
Not run / residual:   ...
```

Use `rust-ai-doc-contracts` only when a durable law-to-test link will materially reduce future
discovery cost. Keep ordinary explanations in theorem-like test names and assertions.
