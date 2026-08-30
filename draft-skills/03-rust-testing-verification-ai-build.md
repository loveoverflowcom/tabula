# Draft Skill: Rust Testing / Verification / AI-Readable Build Discipline

> Status: exploratory.
>
> Goal: collect testing, property-based testing, model checking, formal verification, compile-time, and AI-review ideas that complement functional and refinement-oriented Rust.

---

## 1. Testing philosophy

Testing becomes easiest when architecture is:

```text
pure deterministic core
        +
thin effectful shell
```

Target:

```text
most domain tests:
input → output
```

without:

- mocks
- network
- DB
- filesystem
- Tokio runtime
- UI
- sleep
- current clock

Integration tests cover the shell.

---

## 2. Testing pyramid for Rust domain code

Potential hierarchy:

```text
                 theorem proof
                     /\
                    /  \
             model checking
                  /      \
         property-based tests
              /            \
         unit / table tests
            /                \
     integration / adapter tests
          /                    \
            end-to-end tests
```

Not every project needs every layer.

Use stronger techniques only where the correctness value justifies cost.

---

## 3. Table-driven tests

Prefer explicit cases:

```rust
#[test]
fn transition_cases() {
    let cases = [
        // ...
    ];

    for case in cases {
        assert_eq!(
            apply(case.state, case.action),
            case.expected
        );
    }
}
```

Could define:

```rust
struct Case {
    state: State,
    action: Action,
    expected: Result<State, RuleError>,
}
```

Benefits:

- easy for AI to extend
- reads like specification
- highlights state matrix
- catches missing cases

---

## 4. Tests as executable specification

Potential skill rule:

> Domain tests should describe rules, not implementation details.

Prefer:

```rust
fn player_cannot_move_after_game_finished()
```

over:

```rust
fn test_handle_action_branch_4()
```

Avoid asserting private intermediate calls.

Test observable semantic behavior.

---

## 5. Pure tests should avoid mocks

Bad signal:

```text
domain unit test requires 6 mocks
```

Possibly architecture has effects inside the core.

Prefer values:

```rust
let facts = Facts {
    now,
    actor,
    permission,
};
```

Then:

```rust
decide(state, command, facts)
```

Mocks belong more naturally in application/integration layers.

---

## 6. Deterministic time

Never call real clock in domain tests.

Use:

```rust
Timestamp
```

as input.

Potential types:

```text
CurrentTime
Deadline
Duration
```

Functions:

```rust
fn is_expired(
    expiration: Timestamp,
    now: Timestamp,
) -> bool;
```

---

## 7. Deterministic randomness

Instead of:

```rust
thread_rng()
```

inside game logic, separate:

```text
random source
    ↓
RandomFacts
    ↓
pure transition
```

Options:

- seeded RNG passed to shell/core carefully
- pre-generated random values
- deterministic PRNG state as part of game state
- return `NeedRandom` effect

Example:

```rust
enum Decision {
    Complete(State),
    NeedRandom(RandomRequest),
}
```

Potentially useful for replay and multiplayer determinism.

---

## 8. Snapshot tests

Candidate crates:

- `insta`

Useful for:

- AST
- diagnostics
- serialization
- generated UI-independent text
- parser output
- document diffs

Risks:

- accepting snapshots blindly
- huge snapshots
- noisy formatting
- testing implementation instead of semantics

Potential rule:

> Snapshot structural outputs, but assert critical semantics separately.

---

## 9. Property-based testing

Candidate crates:

- `proptest`
- `quickcheck`
- `bolero`

Best for algebraic/domain laws.

Examples:

### Round-trip

```text
decode(encode(x)) == x
```

### Idempotence

```text
normalize(normalize(x)) == normalize(x)
```

### Symmetry

```text
distance(a, b) == distance(b, a)
```

### Associativity

```text
merge(merge(a,b),c) == merge(a,merge(b,c))
```

when required.

### Invariant preservation

```text
valid(state)
  ⇒
valid(apply(state, action))
```

### Parser never panics

```text
for all byte strings:
parse(input) returns Result and never panics
```

---

## 10. Haskell/QuickCheck-inspired law testing

Potential laws to encode for Rust types:

### Semigroup-like merge

```text
(a <> b) <> c == a <> (b <> c)
```

### Monoid-like identity

```text
empty <> a == a
a <> empty == a
```

### Ordering

```text
cmp(a,b) == reverse(cmp(b,a))
```

### Normalization

```text
normalize(x) is canonical
normalize(normalize(x)) == normalize(x)
```

### Diff/apply

```text
apply(old, diff(old,new)) == new
```

### Serialize/deserialize

```text
decode(encode(x)) == x
```

This is a powerful place to import Haskell reasoning without importing Haskell abstractions.

---

## 11. Metamorphic testing

Useful when exact expected outputs are hard to enumerate.

Examples:

```text
sorting twice == sorting once
adding whitespace does not change semantic AST
renaming unrelated node does not change another node's identity
reordering independent operations commutes
```

Potentially excellent for document engines and compilers.

---

## 12. Differential testing

Compare two implementations:

```text
old algorithm
     vs
new algorithm
```

or:

```text
Rust parser
     vs
reference parser
```

Potential use:

- migration
- parser rewrite
- optimization
- Rust port of another implementation

Agent should consider differential testing before deleting a legacy implementation.

---

## 13. Fuzzing

Candidate:

- `cargo-fuzz`
- libFuzzer ecosystem
- `afl`-based tooling if relevant

Good targets:

- parser
- binary protocols
- Markdown
- file formats
- AST transforms
- decompression
- network decoders

Properties:

- no panic
- no UB
- bounded resource use where feasible
- successful parse satisfies invariant
- serialize/parse roundtrip

Potential skill rule:

> Parsers accepting untrusted input should have a fuzz target when practical.

---

## 14. Model checking with Kani

Kani can explore bounded values exhaustively/symbolically.

Potential targets:

- integer overflow
- state transitions
- protocol state
- edge-case indexing
- panic freedom
- unsafe wrappers

Possible example concept:

```rust
#[kani::proof]
fn every_legal_move_preserves_board_invariants() {
    let state: SmallState = kani::any();
    let action: Action = kani::any();

    kani::assume(state.is_valid());

    if let Ok(next) = apply(&state, action) {
        assert!(next.is_valid());
    }
}
```

Verify current syntax before final skill.

---

## 15. Concurrency testing with Loom

Candidate:

- `loom`

Use when code contains:

- atomics
- mutex/lock protocols
- channels
- custom concurrent state

Loom explores possible interleavings.

Potential rule:

> If correctness depends on concurrency interleavings, normal unit tests are insufficient.

Do not use Loom for ordinary async application code indiscriminately.

---

## 16. Miri

Potential checks:

- unsafe Rust
- aliasing
- pointer validity
- UB
- some data races / stacked/tree borrows behavior

Possible command:

```text
cargo +nightly miri test
```

Verify exact setup before final skill.

For pure safe Rust crates, Miri may add limited value.

---

## 17. Sanitizers

Research current Rust sanitizer workflow:

- AddressSanitizer
- ThreadSanitizer
- MemorySanitizer
- LeakSanitizer

Useful for:

- FFI
- unsafe
- native dependencies
- multithreaded components

Could be a separate CI job.

---

## 18. Formal verification ladder

Potential escalation:

```text
unit tests
  ↓
property tests
  ↓
fuzzing
  ↓
model checking
  ↓
refinement verification
  ↓
deductive verification
  ↓
Lean proof
```

Skill should teach selection, not maximalism.

Question:

> What is the cheapest technique that meaningfully increases confidence for this property?

---

## 19. Flux testing/verification role

Potentially use Flux for properties already naturally expressed as types:

- positive values
- array/vector bounds
- arithmetic relations
- mutation invariants
- refined return values

Keep proof annotations near critical algorithms.

Potential CI separation:

```text
Fast CI:
cargo check
cargo test

Verification CI:
Flux
Kani
Miri
```

---

## 20. Verus role

Potentially use Verus when:

- postconditions matter
- loop invariants matter
- algorithm correctness is critical
- mathematical specs are useful

Avoid requiring it for ordinary orchestration.

Possible strategy:

```text
normal Rust API
        ↓
small verified algorithm crate
        ↓
adapter around verified code
```

Research interoperability constraints.

---

## 21. Aeneas + Lean role

Potential workflow:

```text
Rust algorithm
     ↓ translate
Lean function
     ↓ theorem
Proof
```

Could be ideal for:

- document tree algorithms
- merge algorithms
- CRDT properties
- board game rules
- state machines
- deterministic transforms

Research whether generated Lean is stable enough for long-lived proofs.

---

## 22. Golden/reference implementations

For optimized code, keep a simple reference implementation:

```rust
fn resolve_reference(...)
fn resolve_optimized(...)
```

Property test:

```text
optimized(x) == reference(x)
```

Excellent for AI optimization work.

The reference may be slower but intentionally simple.

Potential rule:

> Prefer preserving a simple oracle during risky performance rewrites.

---

## 23. Mutation testing

Investigate Rust mutation testing tools:

- `cargo-mutants`
- alternatives/current ecosystem status

Purpose:

```text
Would tests fail if behavior were subtly changed?
```

Useful to evaluate test quality.

Potential CI:
- periodic, not every PR
- target critical core crates

---

## 24. Coverage

Candidates:

- `cargo-llvm-cov`

Do not optimize for raw percentage.

Use coverage to find:

- untested branches
- untested error states
- untested state transitions

Potential rule:

> Coverage is a diagnostic, not a correctness metric.

---

## 25. State-transition coverage

For state machines, build a matrix:

```text
State × Command → Result
```

Test all meaningful pairs.

Example:

| State | Start | Stop | Reset |
|---|---:|---:|---:|
| Idle | allowed | denied | allowed |
| Running | denied | allowed | denied |
| Finished | denied | denied | allowed |

Could generate tests from a table.

AI can reason over this representation easily.

---

## 26. Invariant functions in test builds

Sometimes define:

```rust
fn invariant(&self) -> bool
```

or:

```rust
fn validate_invariants(&self) -> Result<(), InvariantError>
```

Use in:

- tests
- debug assertions
- Kani
- proptest
- fuzzing

Avoid shipping expensive checks in hot paths unless valuable.

Possible:

```rust
debug_assert!(state.invariant());
```

---

## 27. Debug assertions vs type invariants

Use `debug_assert!` for:

- internal consistency
- developer mistakes
- properties already guaranteed by private APIs

Use `Result` for:

- expected invalid user input
- runtime failures
- recoverable conditions

Use type invariants for:

- reusable facts that many functions depend upon

---

## 28. Compile-time discipline

The testing skill should consider feedback-loop cost.

Commands:

```text
cargo check -p crate
cargo test -p crate
cargo test -p crate test_name
cargo build --timings
```

Potential rule:

> Default to the smallest package/test target that proves the change.

Full workspace checks belong in CI or before integration when possible.

---

## 29. Separate fast and slow verification

Possible CI:

```text
PR fast:
  cargo fmt --check
  cargo clippy -p changed-crate
  cargo test -p changed-crate

PR integration:
  cargo test --workspace

specialized:
  cargo miri test
  kani
  flux
  fuzz smoke
  mutation test

nightly:
  full fuzz
  expensive verification
  mutation testing
```

Exact design depends on project size and CI resources.

---

## 30. Build graph awareness

Testing/verification crates should not accidentally become dependencies of production crates.

Bad:

```text
domain-core
  ↓ depends on
verification-utils
  ↓ depends on
large solver ecosystem
```

Prefer:

```text
domain-core
     ↑
verification harness / dev-dependencies / separate package
```

Potential rule:

> Verification tooling should not increase the runtime dependency graph unless required.

---

## 31. Dev-dependencies can still affect test builds

Remember:

- dev-dependencies are compiled for tests/examples/benchmarks
- heavy test libraries can slow test builds
- proc macros in tests can still cost compile time

Potential response:

- central test support crate carefully
- avoid giant shared test helper crate with high fan-out
- separate slow integration suites

---

## 32. Test fixture design

Prefer semantic builders:

```rust
GameFixture::new()
    .player(alice)
    .player(bob)
    .turn(alice)
    .build()
```

But avoid enormous builders hiding important setup.

Alternative:

```rust
fn running_game() -> GameState
```

Small named fixtures can be more readable.

Potential rule:

> Test setup should expose facts relevant to the rule being tested.

---

## 33. Avoid fragile mock expectations

Bad:

```text
repository.load called exactly once
clock.now called exactly once
publisher.publish called before save
```

unless order itself is the behavior.

Prefer asserting:

```text
given inputs
→ decision/effect list
```

Then test adapter execution separately.

---

## 34. Contract testing for adapters

For repositories/plugins/providers, consider shared test suites.

Example trait:

```rust
trait DocumentStore {
    ...
}
```

Every implementation runs the same semantic contract tests:

```text
memory store
sqlite store
postgres store
```

This is similar to typeclass laws.

Potential helper:

```rust
fn document_store_contract<S: DocumentStore>(store: S)
```

Watch generic compile cost; could use a macro or per-implementation calls.

---

## 35. Parser testing stack

Potential recommended layers:

1. hand-written examples
2. golden fixtures
3. round-trip properties
4. malformed-input cases
5. fuzzing
6. differential tests against reference parser
7. formal proof only for selected invariants

Very applicable to Markdown/AST/document work.

---

## 36. Document engine properties

Ideas:

```text
parse(render(parse(x))) preserves semantic structure
normalize is idempotent
NodeId uniqueness is preserved
unrelated edit preserves unaffected identities
diff/apply roundtrip
merge deterministic
UTF-16 ↔ byte conversion preserves valid boundaries
range never has start > end
```

Potential Kani/Flux targets:

- small range arithmetic
- offset conversion
- index bounds

Potential proptest targets:

- arbitrary document trees
- move/edit/rename sequences

---

## 37. Board-game engine properties

Ideas:

```text
exactly one active player during normal turn state
turn order never becomes empty
legal moves preserve board validity
piece count conservation where applicable
score is never negative if rules require it
finished game rejects further moves
replay(actions) is deterministic
server and client rule engines produce same result
```

Property-based generated game traces could be very powerful.

---

## 38. Plugin system properties

Ideas:

```text
validated manifests always have a resolvable ID format
resolution never returns a plugin that does not provide requested capability
ambiguous capability returns explicit ambiguity
incompatible interface versions never resolve
installed state requires deployment/runtime metadata
resolved entrypoint satisfies origin/base URL constraints
```

Potential use of refined types:

```text
ValidatedManifest
CompatiblePlugin
ResolvedSurface
```

Then tests can be organized around transitions.

---

## 39. AI-friendly assertions

Prefer meaningful comparisons:

```rust
assert_eq!(
    result,
    Err(RuleError::NotYourTurn)
);
```

over:

```rust
assert!(result.is_err());
```

when exact error semantics matter.

Prefer structured types to string matching.

---

## 40. Error snapshot caution

Do not rely heavily on exact human-readable error strings.

Prefer:

```rust
enum ValidationError {
    MissingField(Field),
    InvalidRange { ... },
}
```

Test enum structure.

Snapshot rendered diagnostics separately if needed.

---

## 41. Failing tests as counterexamples

Lean/model-checking mindset:

A failure should ideally give a minimal counterexample.

Property-test shrinkers already do this.

Potential agent rule:

> When a property fails, preserve the minimized counterexample as a regression unit test.

---

## 42. Verification comments

If formal verification is not used, comments can still state contracts:

```rust
/// Requires:
/// - `range` is within the UTF-16 length of `text`.
///
/// Guarantees:
/// - returned byte offsets are UTF-8 boundaries
/// - start <= end
```

Better if encoded in types/tests.

Potential style:

```text
Requires
Ensures
Invariant
```

sparingly for critical algorithms.

---

## 43. Test naming as theorem names

Lean-inspired names:

```text
normalization_is_idempotent
legal_move_preserves_piece_count
resolved_surface_satisfies_requested_capability
utf16_roundtrip_preserves_offsets
```

Think of tests as small theorems backed by examples/property generation.

---

## 44. AI change workflow

Potential skill workflow for a coding agent:

1. Read public types.
2. Identify invariant changed by the task.
3. Locate pure function responsible for the rule.
4. Add/modify smallest semantic test first.
5. Implement transformation.
6. Run targeted test.
7. Run property/model checks relevant to changed invariant.
8. Run package-level clippy/check.
9. Only then broaden to workspace checks.

This keeps feedback fast and context focused.

---

## 45. AI architecture review questions

Before editing:

- Is the behavior deterministic?
- Which invariant is affected?
- What is the smallest pure unit?
- Is there already a proof-bearing type?
- Could the change weaken an invariant?
- Which property test can detect regressions?
- Does changing this public type force downstream rebuilds?

After editing:

- Did a framework concern leak into core?
- Did a new generic parameter propagate?
- Did test setup become more complex?
- Is there a new panic path?
- Can invalid state now be constructed?
- Are all enum cases handled?

---

## 46. Build-time benchmarking ideas

Create repeatable measurements:

```text
clean build
incremental no-op build
edit leaf function
edit public function body
edit public signature
edit foundational type
test-only build
```

Measure with:

```text
cargo build --timings
hyperfine
```

Potential experiment:

Compare:

```text
plain newtype
nutype
generic refined wrapper
typestate
Flux annotations
```

This can turn vague compile-time concerns into project-specific evidence.

---

## 47. Compiler tooling to investigate

- `cargo build --timings`
- `cargo llvm-lines`
- `cargo bloat`
- `cargo tree`
- `cargo tree -d`
- `cargo machete`
- `cargo udeps` — verify compatibility
- `cargo expand`
- `-Z timings` / self-profile tooling if needed
- rustc self-profile
- `samply` for runtime profiling, not compile-time
- `hyperfine`

Potential goals:

- identify monomorphization
- macro expansion
- duplicate dependency versions
- high-fan-out dependencies
- heavy test-only dependencies

---

## 48. Proc-macro inspection

Use:

```text
cargo expand
```

when a macro-generated abstraction becomes difficult to understand.

Potential skill rule:

> If correctness depends on generated code, inspect expansion before making assumptions.

Relevant for:

- `nutype`
- serde derives
- builder derives
- Leptos components
- Tauri commands
- SQL macros

---

## 49. Generic compile-cost review

Potential warnings:

```rust
fn f<T, U, V, E, P>(...)
```

Ask:

- How many concrete instantiations?
- Is generic code large?
- Can generic wrapper normalize into a concrete inner representation?
- Can `dyn Trait` be used at a cold boundary?
- Can domain code use concrete types?

Potential pattern:

```rust
fn generic_wrapper<T: Into<Input>>(value: T) -> Output {
    core(value.into())
}

fn core(value: Input) -> Output {
    // large non-generic implementation
}
```

---

## 50. Test crate topology

Possible split:

```text
domain-core/
domain-test-support/
integration-tests/
verification-harness/
```

But be careful: a shared test support crate can itself become high fan-out.

Alternative:

Keep small fixtures close to each crate.

Use dedicated integration package only when cross-crate behavior needs it.

---

## 51. Feature flags for verification

Potential feature:

```toml
[features]
verification = []
```

But investigate whether feature-gating annotations/tooling is actually beneficial.

Potential problem:

Feature unification can alter builds.

Separate verifier packages or cfg attributes may be cleaner.

Do not finalize until trying real tools.

---

## 52. Slow tests

Marking/organizing slow tests:

- separate integration test target
- ignored tests
- dedicated CI workflow
- package-specific commands

Avoid making `cargo test` unexpectedly take minutes.

Fast default tests encourage frequent testing.

---

## 53. Deterministic replay

For event-driven/game/document systems:

```text
InitialState + EventLog → State
```

Property:

```text
replay(log) == live_state
```

Could test:

- persistence restore
- network sync
- migrations
- crash recovery
- multiplayer determinism

---

## 54. Serialization versioning tests

Potential properties:

- old fixtures still decode
- new encode/decode roundtrip
- unknown fields handled as designed
- enum additions don't silently reinterpret old data

Use fixture directories per schema version.

This matters for stable plugin/document protocols.

---

## 55. Backward compatibility as a property

For public crates/protocols:

Research tools:

- `cargo-semver-checks`

Potential CI:

```text
public interface crate
    ↓
semver check against previous release
```

Useful because foundational contract changes cause both API risk and rebuild blast radius.

---

## 56. Determinism checks

Critical for:

- board games
- collaborative documents
- sync engines

Potential test:

Run same command sequence with:

- different hash seeds
- different iteration order where possible
- repeated processes
- WASM/native implementations

Assert same semantic result.

Avoid relying on `HashMap` iteration order.

Potentially prefer `BTreeMap`/`IndexMap` where ordering is semantic.

---

## 57. Cross-platform differential tests

For Rust compiled to native + WASM:

```text
same fixture
    ↓
native core
    vs
WASM core
```

compare serialized output.

This can detect platform-specific assumptions.

Useful for board-game rules or document core shared between server/client.

---

## 58. No-panic contracts

Potential policy for pure libraries:

> Public domain operations should return `Result` rather than panic on caller-controlled input.

Test with:

- property generation
- fuzzing
- Kani

Possibly document APIs with:

```text
# Panics
```

when panic is intentional.

---

## 59. Testing unsafe abstractions

For any `unsafe` block:

- document safety invariant
- unit tests
- Miri
- fuzzing
- sanitizers if appropriate
- Kani/model checking where possible

Potential skill rule:

> Unsafe code without a written safety invariant is incomplete.

---

## 60. Suggested future skill split

Could end up as three actual skills:

```text
rust-functional-core
    architecture + Haskell patterns

rust-types-as-proofs
    Lean/refinement/typestate

rust-verification-testing
    proptest/fuzz/Kani/Flux/Lean + compile discipline
```

Or four:

```text
rust-functional-core
rust-type-driven-design
rust-testing-laws
rust-formal-verification
```

Keeping formal verification separate may avoid making the ordinary Rust skill too heavy.

---

## 61. Candidate “levels” the AI can select

### Level 0 — ordinary Rust

```text
struct
enum
Result
unit tests
```

### Level 1 — domain invariants

```text
newtypes
smart constructors
nonempty
explicit state machines
```

### Level 2 — strong testing

```text
property tests
fuzzing
mutation testing
```

### Level 3 — lightweight proof

```text
typestate
Flux
Kani
```

### Level 4 — formal proof

```text
Verus
Creusot
Aeneas + Lean
```

Potential rule:

> Escalate only when the failure impact and invariant complexity justify it.

---

## 62. Final idea

The long-term goal is not “more abstractions”.

The goal is:

```text
Types explain what values mean.
Functions explain how values transform.
Tests explain the laws.
Verifiers explain the critical invariants.
Crate boundaries explain what can change independently.
```

That combination should make Rust code easier for:

- humans
- compiler diagnostics
- testing
- formal tools
- AI coding agents
- incremental builds
- refactoring

