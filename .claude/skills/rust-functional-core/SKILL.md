---
name: rust-functional-core
description: >
  Keep Rust business rules in a pure, deterministic core and push I/O (clock, RNG, network, DB,
  filesystem, async, UI, framework types) into a thin shell. Covers functional core / imperative
  shell, effects-as-data, replacing flag soup with enums, error ADTs, and crate dependency
  direction. Use this whenever writing or reviewing Rust domain logic, deciding which module or
  crate a function belongs in, untangling rules from Axum/Tauri/Leptos/sqlx/tokio handlers,
  designing a state machine or reducer, laying out a Cargo workspace, or when you hit code that
  is hard to test, tests that need many mocks, or `Arc<Mutex<_>>` sprawl. Applies even when the
  user never says "functional", "pure", or "architecture" — the trigger is the shape of the code,
  not the vocabulary.
---

# Rust functional core

The goal is not functional programming aesthetics. It is that **the rules of the system can be
read, tested, and changed without booting anything.** Everything below serves that.

## Triage first (30 seconds)

Before applying anything here, decide whether this code has a core worth separating:

| Signal | Verdict |
|---|---|
| Rules that outlive the framework (pricing, game rules, parsing, permissions, sync/merge, scheduling) | Separate a core |
| Tests need a DB, clock, runtime, or >2 mocks to assert one rule | Separate a core |
| The same decision is re-derived in a handler, a job, and the UI | Separate a core |
| Thin CRUD passthrough, glue script, one-shot CLI, adapter that only maps types | **Do not.** A pure core here is ceremony |
| The "rule" is one `if` and will never grow | **Do not** |

If the answer is "do not", say so and stop. Inventing a core for glue code makes it worse, and
that judgment is the most valuable thing this skill offers.

## The shape

```text
Raw input → parse → Typed input → validate → Domain value
                                                  ↓
                                    decide (pure, total, deterministic)
                                                  ↓
                                     Decision { next_state, effects }
                                                  ↓
                                       shell interprets effects
```

Shell owns: filesystem, network, DB, async runtime, clock, OS randomness, UUID generation,
env vars, logging, UI, HTTP, IPC.
Core owns: parsing, validation, business rules, state transitions, conflict resolution,
authorization decisions, scoring, deterministic algorithms.

## Rule 1 — pass values, not services

The core should not be generic over the things it needs. It should already have them.

```rust
// Avoid: the core can now only run where a Clock and Repository exist.
fn decide<C: Clock, R: Repository>(state: &State, clock: &C, repo: &R) -> Decision

// Prefer: the shell resolved these before calling.
fn decide(state: &State, now: Timestamp, links: &[ResolvedLink]) -> Decision
```

Why this specific move matters more than it looks: a `&dyn Clock` parameter is still an
unresolved effect. It forces every test to construct a fake, it makes the function's real
dependencies invisible in the signature (you must read the body to learn it reads the clock
twice), and it blocks replay — you cannot re-run the decision later and get the same answer.
A `Timestamp` parameter fixes all three at once.

**The test:** can you call this function from a `#[test]` with no `async`, no fixtures, and no
mocks? If not, an effect is still inside.

Trait parameters are still right at the *shell's* boundaries — `trait DocumentStore` for
swapping Postgres and in-memory is a real abstraction. The rule is about the core, not the app.

## Rule 2 — return effects as data

When a rule needs something to happen in the world, describe it; do not do it.

```rust
pub enum Effect {
    PublishEvent(DomainEvent),
    Persist(Document),
    Notify(Notification),
}

pub struct Decision {
    pub next_state: State,
    pub effects: Vec<Effect>,
}
```

This buys dry-run, replay, audit trails, deterministic tests, and simulation — but the reason to
reach for it is narrower than that list suggests. Use it when the *set* of effects is itself part
of the rule (i.e. a reviewer would ask "and what else does this trigger?"). If a function has
exactly one obvious side effect and always will, returning `Effect::Persist(doc)` is indirection
for its own sake — let the shell just save the returned value.

Do not make `Effect` generic on the first day. A concrete per-domain enum is easier to read,
easier to exhaustively match in the interpreter, and cheaper to compile.

## Rule 3 — enums for facts that cannot co-occur

Flag soup is the most common way illegal states get in.

```rust
// Every field combination is representable; most are nonsense.
struct Plugin {
    installed: bool,
    enabled: bool,
    runtime: Option<Runtime>,
    error: Option<String>,
}

// The lifecycle is now the type. `installed && error.is_some()` cannot be written.
enum Plugin {
    Discovered { manifest: Manifest },
    Installed  { manifest: Manifest, runtime: Runtime },
    Disabled   { manifest: Manifest },
    Failed     { manifest: Manifest, error: InstallError },
}
```

Trigger to watch for: two or more `bool`/`Option` fields in one struct whose valid combinations
you can't enumerate on request, or a `None` that "can't happen here". For making a *single*
value carry an invariant (`NonEmptyName`, `Percentage`), see the `rust-types-as-proofs` skill.

## Rule 4 — enums for closed sets, traits for open sets

If you know every variant and third parties cannot add one, an enum beats a trait object: it is
exhaustively matched (the compiler finds every site when you add a variant), it needs no vtable,
and an agent or reviewer can see the whole space in one place. Reach for `dyn Trait` when the set
is genuinely open — plugins, user-supplied backends, a boundary you intend to keep stable for
external implementers.

Ask of any trait in the core: **is this expressing an abstraction, or avoiding a concrete type?**
A trait with exactly one impl and no plan for a second is usually the latter, and it costs you
generic propagation through every caller.

## Rule 5 — errors are ADTs inside, opaque outside

```rust
// core
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MoveError {
    #[error("not your turn")] NotYourTurn,
    #[error("square occupied")] Occupied,
    #[error("coordinate out of range")] InvalidCoordinate,
}
```

`anyhow::Error` / `eyre` in the core destroys the caller's ability to branch on failure and makes
tests assert on strings. Keep them for the application boundary, where "log it and return 500" is
genuinely the whole story. Map deliberately: `DomainError → ApplicationError → Transport error`.

## Rule 6 — move toward total functions

Prefer exhaustive `match` over `if / else if / unreachable!()`. In core logic, treat every
`unwrap`, `expect`, unchecked index, `unreachable!` and `todo!` as a question: *could this
precondition be a type or a `Result` instead?* Sometimes the honest answer is no — a genuinely
impossible state established by a private constructor, or fail-fast startup code — and then a
comment explaining why it cannot happen is the deliverable. Tests may unwrap freely.

## Dependency direction

```text
stable domain types → pure domain algorithms → orchestration → adapters → framework/runtime
```

Arrows never reverse and never cycle. Concretely, a pure crate must not depend on `axum`,
`tauri`, `leptos`, `sqlx`, `reqwest`, `tokio`, or `wasm_bindgen`. Map framework types into domain
types in the adapter, at the edge.

Two workspace hazards worth naming:

- **The `common` crate.** A grab-bag of errors + ids + logging + serde + date helpers that every
  crate depends on becomes a rebuild amplifier: touching it recompiles the world, and it tends to
  accumulate framework deps that then leak everywhere. Split by reason-to-change instead.
- **Generic propagation.** One `<T: Store>` on a core type spreads outward through every caller
  and every test. Prefer a concrete type in the core and a thin generic wrapper at the edge:

```rust
pub fn handle<T: Into<Command>>(input: T) -> Outcome { decide(input.into()) }
fn decide(cmd: Command) -> Outcome { /* large, non-generic, monomorphized once */ }
```

## Name functions for the transformation

`parse` `decode` `normalize` `canonicalize` `validate` `classify` `resolve` `decide` `reduce`
`apply` `evolve` `merge` `diff` `project` `render` `encode`

These make the data direction legible from the signature alone. Avoid `handle`, `process`,
`manage`, `do_work`, `execute_logic`, `helper`, `util` unless the surrounding context already
pins the meaning down. A reader — human or agent — should be able to infer the pipeline order
from signatures without opening a single body.

## Review checklist

Run through this when reviewing Rust that contains rules:

- Can invalid field combinations be constructed? Should this struct be an enum?
- Does a core function read a clock, RNG, env var, DB, or network — directly or through a trait?
- Is I/O interleaved with the decision, or does the function return the decision?
- Are all states of the transition exhaustively matched?
- Does the domain test need mocks, `#[tokio::test]`, or a fixture DB? (Symptom, not the disease.)
- Is a framework type present in a pure module?
- Is a trait here earning its keep, or is it a concrete type in disguise?
- Any `unwrap`/`unreachable!` on caller-controlled input?
- Would a new generic parameter propagate outward?

## Deeper material

- `references/extraction-recipes.md` — worked before/after: pulling a core out of an Axum
  handler, turning a mutating method into `decide`/`apply`, and replacing mock-heavy tests.
  Read it when performing an actual extraction rather than a review.

## Related skills

- `rust-types-as-proofs` — making individual values carry their invariants.
- `rust-verification-testing` — what to test once the core is pure, and how far to escalate.
