# Draft Skill: Rust Functional / Haskell-Inspired Engineering

> Status: exploratory draft.  
> Goal: collect ideas for a future coding skill that makes Rust code more explicit, compositional, testable, AI-readable, and friendly to incremental compilation.
>
> This document intentionally contains more ideas than rules. Verify and prune before turning it into a strict agent skill.

---

## 1. Core philosophy

Prefer Rust that can be understood as a sequence of explicit transformations:

```text
Raw Input
  ↓
Parse
  ↓
Typed Input
  ↓
Validate
  ↓
Domain Value
  ↓
Decide
  ↓
Decision / Events
  ↓
Apply
  ↓
New State
```

Primary heuristic:

```text
data → pure function → data
```

Effects should be pushed outward.

```text
Imperative Shell
    ↓ plain values
Functional Core
    ↓ decisions / values
Imperative Shell
```

The shell owns:

- filesystem
- network
- database
- async runtime
- clock
- randomness
- UUID generation
- logging
- environment variables
- UI
- Tauri commands
- Leptos signals
- HTTP
- Matrix
- OS APIs

The core owns:

- parsing decisions
- validation
- business rules
- state transitions
- conflict resolution
- authorization decisions
- scoring
- document transformations
- game rules
- capability matching
- deterministic algorithms

---

## 2. Haskell ideas worth translating to Rust

Do not imitate Haskell syntax. Translate its design principles into idiomatic Rust.

### Algebraic data types

Haskell:

```haskell
data Result a
  = Success a
  | Failure Error
```

Rust:

```rust
enum Outcome<T> {
    Success(T),
    Failure(DomainError),
}
```

Prefer enums over unrelated booleans and nullable fields.

Bad:

```rust
struct Plugin {
    installed: bool,
    enabled: bool,
    runtime: Option<Runtime>,
    error: Option<String>,
}
```

Potentially better:

```rust
enum PluginState {
    Discovered {
        manifest: Manifest,
    },
    Installed {
        manifest: Manifest,
        runtime: Runtime,
    },
    Disabled {
        manifest: Manifest,
    },
    Failed {
        manifest: Manifest,
        error: InstallError,
    },
}
```

Question for the agent:

> Can impossible combinations of flags be replaced by an enum?

---

## 3. Make illegal states unrepresentable

Possible rule:

> If the same invariant is checked repeatedly, consider encoding it in a type.

Examples:

```rust
struct UserId(String);
struct RoomId(String);
struct DocumentId(Uuid);
struct NonEmptyName(String);
struct PositiveAmount(Decimal);
```

Prefer:

```rust
fn send_message(room: RoomId, body: MessageBody)
```

over:

```rust
fn send_message(room: String, body: String)
```

when the domain distinction matters.

Do not make a newtype for every primitive mechanically.

Use a newtype when it adds at least one of:

- domain meaning
- invariant
- unit safety
- privacy boundary
- serialization boundary
- accidental-mixing prevention
- stable public API

---

## 4. Smart constructors

Prefer:

```rust
impl Username {
    pub fn parse(raw: &str) -> Result<Self, UsernameError> {
        // normalize + validate
    }
}
```

Then downstream code receives:

```rust
Username
```

instead of repeatedly validating `String`.

Potential rule:

> Parse/validate at boundaries; operate on trusted domain types internally.

Related ideas:

- Parse, don't validate
- Constructor returns proof-bearing value
- Private fields preserve invariants
- `TryFrom` for fallible domain conversion
- `From` only for infallible conversion

---

## 5. Functional Core / Imperative Shell

Example:

```rust
pub fn decide(
    state: &GameState,
    command: Command,
    facts: Facts,
) -> Result<Decision, RuleError> {
    // no I/O
}
```

Where `Facts` contains already-resolved external facts:

```rust
pub struct Facts {
    pub now: Timestamp,
    pub actor: PlayerId,
    pub random_roll: DiceRoll,
}
```

Avoid this inside the core:

```rust
clock.now()
repository.load()
rand::random()
env::var()
```

The shell resolves them first.

---

## 6. Prefer values over service dependencies in domain logic

Instead of:

```rust
fn decide<C: Clock>(
    state: &State,
    clock: &C,
) -> Decision
```

consider:

```rust
fn decide(
    state: &State,
    now: Timestamp,
) -> Decision
```

Instead of:

```rust
fn evaluate<R: Repository>(
    doc: &Document,
    repo: &R,
) -> Result<Outcome, Error>
```

consider:

```rust
fn evaluate(
    doc: &Document,
    policy: &Policy,
    known_links: &[ResolvedLink],
) -> Result<Outcome, Error>
```

Possible skill phrase:

> Prefer value injection over dependency injection inside the functional core.

---

## 7. Effects as data

Instead of executing an effect:

```rust
send_email(...)
publish_event(...)
write_file(...)
```

return a value describing it:

```rust
enum Effect {
    PublishEvent(DomainEvent),
    PersistDocument(Document),
    SendNotification(Notification),
}
```

Example:

```rust
struct Decision {
    next_state: State,
    effects: Vec<Effect>,
}
```

The shell interprets effects.

Benefits:

- deterministic tests
- replay
- auditing
- easier AI reasoning
- dry-run support
- simulation
- event sourcing compatibility
- easier migration between runtimes

Open question:

- Should `Effect` be generic?
- Should effects be domain events only?
- Should persistence be outside domain entirely?

Do not over-generalize too early.

---

## 8. Reducer/state-transition style

Simple form:

```rust
fn apply(
    state: &State,
    action: Action,
) -> Result<State, RuleError>;
```

Event-oriented form:

```rust
fn decide(
    state: &State,
    command: Command,
) -> Result<Vec<Event>, RuleError>;

fn evolve(
    state: State,
    event: &Event,
) -> State;
```

Useful for:

- board games
- editors
- plugin lifecycle
- auth/session state
- sync state
- workflows
- document versioning
- collaborative operations
- retry machines
- background jobs

Potential skill rule:

> For domain state machines, make transitions explicit and exhaustively matched.

---

## 9. Totality mindset

Rust is not a total language, but code can move toward total functions.

Prefer:

```rust
match value {
    A => ...
    B => ...
    C => ...
}
```

over:

```rust
if ...
else if ...
else {
    unreachable!()
}
```

Avoid unchecked assumptions in domain logic:

- `unwrap()`
- `expect()`
- indexing without proof
- `unreachable!()`
- `todo!()`

Exceptions may be allowed for:

- tests
- impossible states established by API invariants
- initialization code where failure is intentionally fatal

Possible skill heuristic:

> Every panic in core logic should trigger a review: could the precondition become a type or a `Result`?

---

## 10. Pattern matching as executable specification

Use exhaustive `match` to expose behavior.

Example:

```rust
match (state, command) {
    (State::Waiting, Command::Start) => ...
    (State::Running(run), Command::Stop) => ...
    (State::Finished(_), Command::Start) => Err(...),
    ...
}
```

This can be easier for AI to inspect than distributed polymorphic behavior.

Potential guideline:

> Prefer visible transition tables over clever trait hierarchies when the state space is finite.

---

## 11. Haskell `Either`, `Maybe`, and composition

Rust equivalents:

```text
Maybe a   → Option<T>
Either e a → Result<T, E>
```

Use combinators when they improve readability:

```rust
map
map_err
and_then
filter
ok_or
transpose
collect::<Result<Vec<_>, _>>()
```

But avoid iterator/combinator chains that become harder to debug than a simple loop.

Possible rule:

> Functional style is about explicit semantics, not maximizing combinator density.

Local `mut` is fine.

---

## 12. Local mutation is allowed

Good:

```rust
fn normalize(items: &[Item]) -> Vec<Item> {
    let mut out = Vec::with_capacity(items.len());

    for item in items {
        if let Some(item) = normalize_one(item) {
            out.push(item);
        }
    }

    out
}
```

Still semantically pure if:

- no external state is mutated
- same input produces same output
- mutation is local and unobservable

Avoid ideological “no mut” rules.

---

## 13. Non-empty structures

Haskell concept:

```haskell
NonEmpty a
```

Rust candidates:

- `nonempty`
- custom `NonEmptyVec<T>`
- `smallvec` with constructor invariant
- custom domain collection

Prefer:

```rust
struct TurnOrder(NonEmpty<PlayerId>);
```

over:

```rust
Vec<PlayerId>
```

when empty is invalid.

Potential rule:

> Collection cardinality constraints should be represented structurally when important.

Other useful structures:

- `AtLeastOne<T>`
- `ExactlyOne<T>`
- `BoundedVec<T, N>` if available / appropriate
- fixed arrays `[T; N]`
- `IndexMap` when stable ordering is semantic

---

## 14. Phantom types / type-state

Possible modeling:

```rust
struct Document<State> {
    data: DocumentData,
    _state: PhantomData<State>,
}

struct Raw;
struct Parsed;
struct Validated;
struct Resolved;
```

Transitions:

```rust
fn parse(doc: Document<Raw>)
    -> Result<Document<Parsed>, ParseError>;

fn validate(doc: Document<Parsed>)
    -> Result<Document<Validated>, ValidationError>;

fn resolve(doc: Document<Validated>)
    -> Document<Resolved>;
```

Good when:

- transitions are few
- states are meaningful
- invalid operation ordering is a real source of bugs

Bad when:

- generic state explosion
- dozens of orthogonal axes
- public APIs become unreadable
- compilation cost grows from combinatorial monomorphization

Potential skill constraint:

> Typestate should model one strong lifecycle axis, not every boolean in the system.

---

## 15. Traits as Haskell typeclass-like contracts

Good uses:

```rust
trait Canonicalize {
    fn canonicalize(self) -> Self;
}
```

Potentially good at abstraction boundaries:

```rust
trait Plugin {
    fn manifest(&self) -> &PluginManifest;
}
```

But avoid generic propagation through the entire domain:

```rust
Core<A, B, C, D, E>
where
    A: ...
    B: ...
```

Question:

> Is this trait expressing a real abstraction or merely avoiding a concrete type?

---

## 16. Prefer closed-world enums when implementations are known

If the domain has exactly:

```text
Markdown
AsciiDoc
LaTeX
```

then:

```rust
enum DocumentFormat {
    Markdown,
    AsciiDoc,
    Latex,
}
```

may be better than dynamic traits.

If third parties can extend it:

```rust
dyn DocumentParser
```

may be appropriate.

Potential rule:

> Use enums for closed sets; traits for open sets.

---

## 17. Separate semantic functions

Good functional vocabulary:

```text
parse
decode
normalize
canonicalize
validate
classify
resolve
decide
reduce
apply
evolve
merge
diff
project
render
encode
```

These names expose data transformation direction.

Avoid vague names:

```text
handle
process
do_work
manage
execute_logic
helper
util
```

unless context makes them precise.

---

## 18. Stable pipelines

A domain pipeline should be visible from signatures.

Example:

```rust
fn parse(raw: RawDocument) -> Result<ParsedDocument, ParseError>;

fn normalize(doc: ParsedDocument) -> NormalizedDocument;

fn analyze(doc: &NormalizedDocument) -> Analysis;

fn resolve(
    doc: NormalizedDocument,
    analysis: Analysis,
) -> Result<ResolvedDocument, ResolveError>;
```

AI should be able to infer ordering without reading bodies.

---

## 19. Push framework types to the edges

Core should not require:

- `axum::extract::*`
- `tauri::State`
- `leptos::*`
- `sqlx::*`
- `reqwest::*`
- `tokio::*`
- `wasm_bindgen::*`

Map boundary types into domain types early.

Example:

```text
HTTP request
  ↓ adapter
CreateDocumentRequest
  ↓
CreateDocumentCommand
  ↓ core
Decision
  ↓ adapter
HTTP response
```

---

## 20. Crate boundaries for functional architecture

Possible workspace shape:

```text
crates/
  domain-types/
  document-core/
  document-markdown/
  document-diff/
  game-core/
  plugin-interface/

adapters/
  storage-sqlite/
  matrix-client/
  browser-runtime/

apps/
  desktop/
  web/
  server/
```

Potential rule:

> Pure crates should depend downward on stable data/contracts, not upward on frameworks.

---

## 21. Avoid giant `common` crates

Anti-pattern:

```text
common/
  errors
  ids
  logging
  parsing
  networking
  serde
  date
  everything
```

If it changes often and every crate depends on it, incremental rebuild suffers.

Prefer small stable packages:

```text
vot-id
vot-plugin-interface
vot-document-types
```

only when justified.

---

## 22. AI-readable code heuristics

The agent should prefer code where behavior can be learned in this order:

1. types
2. signatures
3. enums
4. match expressions
5. tests
6. implementation details

Potential skill rule:

> If the reader must inspect hidden mutable state or framework callbacks before understanding the rule, consider restructuring it.

---

## 23. Possible libraries to investigate

Do not require these blindly.

Functional/data libraries:

- `itertools`
- `either`
- `nonempty`
- `im` — persistent immutable collections
- `rpds` — persistent data structures
- `smallvec`
- `arrayvec`
- `indexmap`

Type/domain helpers:

- `derive_more`
- `thiserror`
- `strum`
- `enum_dispatch`
- `typed-builder`
- `bon`

Functional programming experiments:

- `frunk`
- `higher`
- crates emulating HKT / optics — research before use
- lens-like crates — likely niche

Potential guidance:

> Prefer the standard library and simple domain types first. Add FP libraries only if they simplify actual code.

---

## 24. Persistent immutable collections

Haskell commonly uses persistent immutable data.

Rust candidates to investigate:

- `im`
- `rpds`

Potential benefit:

```text
State_n
  ↓ action
State_n+1
```

without destructive updates.

Useful for:

- undo/redo
- time travel
- version trees
- game state
- collaborative document state
- speculative execution

Tradeoffs to verify:

- allocation
- cache behavior
- cloning semantics
- WASM size
- compile time

Do not automatically replace `Vec`/`HashMap`.

---

## 25. Lenses / optics — investigate carefully

Haskell ecosystems often use lenses.

Potential Rust analogues exist, but ergonomics may be poor.

Before adopting:

- compare with explicit methods
- compare with pattern matching
- check compile cost
- check generic error messages
- check AI readability

Possible conclusion after research:

> Rust's ownership + pattern matching may make explicit transforms preferable to generic lens abstractions.

---

## 26. Error design as ADTs

Avoid:

```rust
Result<T, anyhow::Error>
```

deep inside the domain when callers need to reason about failure classes.

Prefer:

```rust
enum MoveError {
    NotYourTurn,
    Occupied,
    InvalidCoordinate,
}
```

Use `anyhow` / `eyre` at application boundaries if desired.

Potential layering:

```text
DomainError
  ↓ mapped into
ApplicationError
  ↓ mapped into
Transport/UI Error
```

---

## 27. Functional dependency graph

Another interpretation of “functional programming”:

Dependencies should flow one direction.

```text
stable domain types
        ↓
pure domain algorithms
        ↓
application orchestration
        ↓
adapters
        ↓
framework/runtime
```

No cycle.

No adapter imported into domain.

No UI type in core.

---

## 28. Anti-pattern list

Potential skill warnings:

- Boolean blindness
- Primitive obsession
- `String` for every ID
- giant service objects
- methods that mutate six fields indirectly
- hidden globals
- core calling clock/network/random
- blanket `Arc<Mutex<_>>`
- domain logic inside async handlers
- business rules in ORM hooks
- domain rules spread across UI callbacks
- pervasive `dyn Any`
- generic abstraction without multiple meaningful implementations
- giant `utils.rs`
- `unwrap()` in core
- excessive iterator cleverness
- excessive typestate
- excessive macro magic
- builder pattern for trivial structs

---

## 29. Example review checklist

When reviewing Rust:

### Data model

- Can invalid field combinations exist?
- Should this be an enum?
- Is a primitive carrying domain meaning?
- Can repeated validation become a constructor invariant?

### Functions

- Is this deterministic?
- Does it read hidden state?
- Can external facts be passed as values?
- Does the function return an explicit decision?

### Effects

- Is I/O mixed with business logic?
- Could the effect be returned as data?

### State

- Is a transition explicit?
- Are all states exhaustively handled?
- Can operation order be represented by types?

### Architecture

- Is a framework dependency leaking into the core?
- Does this dependency direction increase rebuild blast radius?
- Is a generic abstraction worth its compile cost?

### AI readability

- Can a coding agent infer the rule from types + tests?
- Does behavior depend on callback order or implicit global state?
- Are names semantic?

---

## 30. Potential strict rules for a future SKILL.md

Possible rules to test:

1. Core domain functions must not directly perform I/O.
2. External facts should be passed as immutable values where practical.
3. Repeated runtime validation should trigger consideration of a domain type.
4. Finite business states should prefer enums over flag combinations.
5. Domain transitions should be explicit and exhaustively matched.
6. Avoid `unwrap` in domain logic.
7. Keep framework-specific types outside pure domain crates.
8. Prefer concrete types in core; use generics only when abstraction is real.
9. Prefer enums for closed extension points and traits for open extension points.
10. Local mutation is allowed if externally pure.
11. Tests for pure core should not require mocks or async runtimes.
12. Avoid introducing abstractions solely to appear functional.

---

## 31. Ideas to research further

- “Parse, don’t validate” in Rust
- Functional Core / Imperative Shell
- Railway-oriented programming
- Tagless-final patterns and whether they are worth using in Rust
- Free monad / effect systems: likely too abstract, but study lessons
- Persistent data structures
- Optics/lenses
- Haskell `NonEmpty`, `Validated`, `These`
- Applicative validation vs fail-fast `Result`
- Semigroup/Monoid concepts for deterministic merge logic
- CRDT algebra
- State machines as ADTs
- Property-based testing for algebraic laws
- Newtype pattern
- PhantomData / typestate
- “Make illegal states unrepresentable”
- Total function design
- denotational vs operational modeling
- parser combinators
- algebraic effects translated to explicit Rust enums

---

## 32. Possible skill name ideas

- `rust-functional-core`
- `rust-functional-design`
- `rust-haskell-patterns`
- `rust-explicit-dataflow`
- `rust-domain-modeling`
- `rust-pure-core`
- `rust-algebraic-design`
- `rust-type-driven-core`

Potential final split:

```text
rust-functional-core
rust-type-driven-domain
rust-formal-verification
```

rather than one giant skill.

