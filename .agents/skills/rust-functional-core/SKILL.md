---
name: rust-functional-core
description: Keep Rust domain rules pure, deterministic, total, and isolated from I/O by designing functional cores with imperative shells, effects-as-data, explicit state transitions, transactional error paths, and downward-only dependencies. Use when adding or reviewing Rust business/game rules, reducers, authorization or reconciliation logic, state machines, crate/module boundaries, async handlers with embedded decisions, mock-heavy tests, nondeterministic code, or framework types leaking into domain code. Do not use for trivial glue that contains no durable rule.
---

# Rust functional core

Make rules readable and testable without booting a runtime. Treat architecture constraints in
the nearest `AGENTS.md` and repository docs as the contract; read them before editing.

## Work in this order

1. Read the module's public types and module docs, then the tests that state its behavior. Open
   implementations only as needed. Do not crawl unrelated crates.
2. Write a compact boundary map before changing code:

   ```text
   trusted inputs → pure decision/state transition → returned values/effects → shell interpreters
   ```

3. Name the affected invariant, the function that owns it, and the observable evidence that will
   verify it. If ownership is unclear, fix ownership before adding behavior.
4. Move world access to the shell. Resolve clocks, randomness, identity, storage, network, UI,
   environment, and configuration into plain typed values before calling the core.
5. Implement the smallest pure transformation. Validate completely before mutating. Prefer a
   proof-bearing intermediate when it makes the commit phase infallible.
6. Add targeted semantic tests and the relevant law/property tests. Use
   `rust-verification-testing` — the router — when the edge space or invariant is nontrivial. A pure
   core is what makes the strong oracles affordable: reference models, exhaustive enumeration of a
   finite state space (`rust-replay-differential-testing`), sequence properties over reachable
   states (`rust-property-testing`), and bounded model checking (`rust-kani`) all require the
   determinism the core provides.
7. Run the narrowest useful checks first, then repository-required broad checks.
8. Report the invariant changed, boundary preserved, evidence run, and any unverified risk.

## Core shape

```text
Raw input → parse → validate → Domain input
                                  │
                       decide/reduce/apply
                                  │
                    Outcome { state, events, effects }
                                  │
                         imperative shell
```

The core owns parsing decisions, validation, business rules, state transitions, conflict
resolution, authorization decisions, scoring, projections, and deterministic algorithms.

The shell owns I/O, async runtimes, clocks, OS randomness, UUID creation, databases, sockets,
filesystems, logging, environment variables, UI/framework state, retries, and effect execution.

## Pass facts, not services

Do not hide unresolved effects behind traits inside a rule.

```rust
// Avoid in the core.
fn decide<C: Clock, R: Repository>(state: &State, clock: &C, repo: &R) -> Decision;

// Prefer: the shell already resolved the facts.
fn decide(state: &State, now: LogicalTime, links: &[ResolvedLink]) -> Decision;
```

Traits remain appropriate at shell ports and for genuinely open implementation sets. The test is
simple: a domain rule should be callable from an ordinary `#[test]` without async, fixtures,
services, or mocks.

## Return effects as data

When the *set of consequences* is part of the rule, describe it explicitly.

```rust
pub struct Decision {
    pub events: Vec<DomainEvent>,
    pub effects: Vec<Effect>,
}
```

Keep effects concrete and domain-local until more than one real consumer proves a generic
abstraction useful. Make shell interpreters idempotent when recovery can replay effects.

Do not wrap one obvious side effect in a ceremonial enum. A returned domain value that the shell
always persists may be clearer.

## Make rejection transactional

For `fn apply(&mut State, ...) -> Result<_, _>`, an `Err` must not leave partial mutation.
Choose one of these shapes:

- Validate every fallible condition, construct a `LegalCommand`, then mutate infallibly.
- Decide immutable events first, then evolve state from accepted events.
- Build a candidate state and replace the original only after validation succeeds.

Never use "mutate and roll back" unless rollback itself is mechanically proven and tested. Add a
test that compares canonical bytes or the full state before and after every rejected input class.

## Prefer total, deterministic transformations

- Use exhaustive `match` for closed state spaces.
- Return structured errors for hostile or caller-controlled input.
- Question every `unwrap`, `expect`, unchecked index, `unreachable!`, and panic in the core.
- Pass logical time and deterministic RNG explicitly; never read the wall clock or OS entropy.
- Avoid output-affecting unordered iteration, floats in canonical decisions, pointer-derived data,
  hidden global state, and parallel reductions whose order changes results.
- Sort or canonicalize at the boundary when order is semantically irrelevant but observable.

Local mutation is fine when it is encapsulated, deterministic, and not observable until the
function returns. Purity is about effects and referential behavior, not avoiding `mut`.

## Choose closed representations first

- Use enums for states or facts that cannot coexist.
- Use enums for closed implementation sets; use traits for open extension points.
- Replace flag/`Option` combinations with explicit states when invalid combinations exist.
- Keep domain errors as enums. Map them to opaque application/transport errors only at the edge.
- Avoid generic parameters that propagate through the dependency graph without buying a real
  abstraction.

## Keep dependencies pointing outward

```text
stable domain values → pure algorithms → orchestration → ports/adapters → frameworks
```

A core crate must not depend on runtime, transport, persistence, renderer, or UI crates. Map their
types at the edge. Do not create a `common` crate that becomes a dependency magnet; split by
semantic ownership and reason to change.

For Tabula specifically, doc 00 and `deps.toml` are normative. Game-specific behavior belongs
behind the game API/registry, never in platform branches. Respect phase gates before implementing
an otherwise sensible abstraction.

## Optimize for narrow context

Use transformation names that expose data flow: `parse`, `validate`, `resolve`, `decide`,
`reduce`, `apply`, `evolve`, `project`, `encode`. Keep module roots as maps: public types,
invariants, and ownership; keep details in leaf modules.

Add structured Rust doc contracts only to high-leverage boundaries and algorithms. Invoke
`rust-ai-doc-contracts` for the schema; do not scatter free-form "important for AI" comments.

## Stop conditions

Do not invent a functional core for thin CRUD, one-shot glue, or an adapter that only maps types.
Do not introduce a new crate, dependency, trait, generic framework, or formal tool unless the
repository permits it and the task demonstrates the need.

## Completion checklist

- The changed rule has one clear owner and an explicit invariant.
- Core signatures accept values rather than effectful services.
- Rejected inputs preserve state exactly.
- Effects cross the boundary as returned data or obvious return values.
- No framework/runtime dependency points into the core.
- Tests describe behavior and laws, not mock call counts.
- Verification commands and remaining gaps are reported.

Read `references/extraction-recipes.md` only when extracting mixed I/O/rule code or splitting a
fallible mutation into decide/commit stages.

