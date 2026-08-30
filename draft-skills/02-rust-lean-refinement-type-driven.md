# Draft Skill: Lean-Inspired / Refinement / Type-Driven Rust

> Status: exploratory idea bank.
>
> Goal: design a future AI coding skill that borrows useful reasoning habits from Lean, dependent types, refinement types, Liquid Haskell, and proof-oriented programming without trying to turn ordinary Rust into a theorem prover.

---

## 1. Central idea

Use types as lightweight proofs.

Instead of:

```rust
fn save(name: String) {
    assert!(!name.is_empty());
}
```

prefer:

```rust
fn save(name: NonEmptyName) {
}
```

Then construction of `NonEmptyName` is the proof boundary.

Mental model:

```text
raw value
   +
validation
   ↓
proof-bearing domain value
```

Lean-ish interpretation:

```text
NonEmptyName ≈ String together with evidence that it is non-empty
```

Rust does not provide full dependent types, but private constructors, enums, newtypes, const generics, phantom types, and verifier tools can encode many useful invariants.

---

## 2. “Proof-producing functions”

Prefer:

```rust
fn validate(
    raw: RawDocument,
) -> Result<ValidDocument, ValidationError>;
```

over:

```rust
fn is_valid(
    doc: &Document,
) -> bool;
```

A boolean loses evidence.

A validated type preserves evidence.

Potential rule:

> Validation functions should preferably return a stronger type, not merely `bool`.

Other examples:

```rust
fn authenticate(token: RawToken)
    -> Result<AuthenticatedUser, AuthError>;

fn parse_plugin_id(raw: &str)
    -> Result<PluginId, PluginIdError>;

fn resolve_surface(req: SurfaceRequest)
    -> Result<ResolvedSurface, ResolveError>;
```

---

## 3. Private fields as proof barriers

Example:

```rust
pub struct Percentage(u8);

impl Percentage {
    pub fn new(value: u8) -> Result<Self, PercentageError> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(PercentageError::OutOfRange)
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}
```

Do not expose:

```rust
pub struct Percentage(pub u8);
```

if the invariant matters.

Possible skill rule:

> If a type represents an invariant, its raw representation should usually be private.

---

## 4. Newtypes as propositions

Potential domain types:

```text
PositiveAmount
NonZeroQuantity
NormalizedPath
CanonicalUrl
VerifiedSignature
AuthenticatedUser
InstalledPlugin
ResolvedSurface
ValidatedManifest
ParsedDocument
NonEmptyTitle
SortedRange
UniqueNodeId
```

Each communicates a fact.

The agent should ask:

> What has already been proven about this value?

---

## 5. Refinement type spectrum

Possible spectrum:

```text
plain primitive
    ↓
newtype
    ↓
smart constructor
    ↓
nutype / validator macro
    ↓
generic refinement wrapper
    ↓
typestate
    ↓
Flux
    ↓
Creusot / Kani contracts
    ↓
Verus
    ↓
Aeneas → Lean
    ↓
manual Lean specification/proof
```

Do not jump to the bottom unless the problem warrants it.

---

## 6. Candidate libraries/tools to investigate

### Lightweight domain types

- `nutype`
- `refined`
- `validator`
- `garde`
- `derive_more`
- `thiserror`
- `nonempty`
- `bounded-integers` / bounded integer crates — verify ecosystem status
- `typed-index-collections` / typed index crates
- `slotmap` key types
- `compact_str` for representation concerns, not proofs

### Typestate

- `typestate`
- `state_machine_future` historically — verify maintenance
- custom `PhantomData<State>`
- enum state machines instead of generic typestate where simpler

### Static/refinement verification

- Flux
- Verus
- Creusot
- Prusti — verify current maintenance/status
- Kani
- MIRAI — verify current maintenance/status

### Rust → theorem prover

- Aeneas / Charon
- Lean backend
- F*
- Coq
- HOL4

### Property/spec tools

- `proptest`
- `quickcheck`
- `bolero`
- `kani`
- `loom` for concurrency
- `miri`
- fuzzing with `cargo-fuzz`

Do not hard-code tool choices until verifying current maturity and maintenance.

---

## 7. `nutype` idea

Potential use:

```rust
#[nutype(
    sanitize(trim),
    validate(not_empty, len_char_max = 100)
)]
pub struct DisplayName(String);
```

Advantages to investigate:

- concise declaration
- validation
- sanitization
- serde support
- generated traits
- less boilerplate

Potential costs:

- proc macro compile cost
- generated API hidden from reader
- macro errors
- AI must know macro semantics
- migration risk

Potential recommendation:

> Use for repetitive leaf-domain invariants; keep critical foundational types plain Rust if long-term stability matters.

---

## 8. Generic refinement wrapper idea

Conceptually:

```rust
Refinement<T, Predicate>
```

Examples:

```text
Refinement<u16, PortRange>
Refinement<String, NonEmpty>
Refinement<Vec<T>, AtLeastOne>
```

Advantages:

- reusable predicates
- compositional validation
- mathematically elegant

Risks:

- nested generic types
- monomorphization
- noisy error messages
- public APIs become type-level expressions
- AI readability may decrease
- compile-time cost

Potential rule:

> If the refinement expression is harder to read than a named domain type, create a named type.

Instead of exposing:

```rust
Refinement<
    String,
    And<Trimmed, And<NonEmpty, MaxLength<100>>>
>
```

expose:

```rust
DisplayName
```

even if internally implemented with a refinement crate.

---

## 9. Liquid Haskell concepts to translate

Liquid Haskell style:

```text
{x : Int | x > 0}
```

Rust approximations:

### Runtime-preserved invariant

```rust
struct PositiveI32(i32);
```

### Refinement library

```text
Refinement<i32, GreaterThan<0>>
```

### Flux

```text
i32{v: v > 0}
```

Potential ideas from Liquid Haskell:

- bounds
- non-empty lists
- sorted lists
- length relations
- arithmetic relations
- state invariants
- map key membership
- indexed collections
- protocol state

Research which are practical in Flux today.

---

## 10. Lean subtype thinking

Lean conceptually:

```lean
{x : Nat // x > 0}
```

Rust approximation:

```rust
struct PositiveU32(u32);
```

Constructor:

```rust
impl TryFrom<u32> for PositiveU32 {
    type Error = PositiveError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(PositiveError)
        }
    }
}
```

Important distinction:

Rust constructor performs a runtime proof.

Lean can carry an actual proposition/proof term.

Flux/Verus may statically prove some cases.

---

## 11. Typestate as indexed state

Lean/dependent type intuition:

```text
Connection state parameter determines legal operations.
```

Rust:

```rust
struct Connection<S> {
    inner: Inner,
    _state: PhantomData<S>,
}

struct Disconnected;
struct Connected;
```

Only:

```rust
impl Connection<Connected> {
    fn send(&mut self, msg: Message) { ... }
}
```

Potential use cases:

- plugin lifecycle
- document parse pipeline
- authorization
- transaction builder
- cryptographic handshake
- upload sessions
- game setup
- synchronization protocol

Potential non-use cases:

- state changes frequently at runtime and callers need heterogenous storage
- state graph is large
- many independent state dimensions

In those cases, enum state machines may be better.

---

## 12. Proof states via enums

Often simpler than typestate:

```rust
enum Manifest {
    Raw(RawManifest),
    Validated(ValidatedManifest),
}
```

or simply separate types:

```rust
RawManifest
ValidatedManifest
ResolvedManifest
```

This preserves explicit proof stages without generic state parameters.

Potential recommendation:

> Prefer separate named types before generic typestate unless generic reuse is valuable.

---

## 13. Branded IDs / typed indices

Avoid accidental mixing:

```rust
struct UserId(Uuid);
struct RoomId(Uuid);
struct DocumentId(Uuid);
```

Potential indexed collections:

```rust
struct NodeId(u32);
struct PlayerIndex(usize);
```

Libraries to investigate:

- `typed-index-collections`
- `index_vec`
- `slotmap`
- `generational-arena`

Questions:

- Can typed index prevent cross-collection indexing?
- Does the library preserve compile-time distinction?
- Compile-time cost?
- serialization story?

---

## 14. Const generics as lightweight dependent typing

Examples:

```rust
struct Matrix<T, const ROWS: usize, const COLS: usize>;
struct FixedDeck<const N: usize>;
```

Potential use:

```rust
fn transpose<T, const R: usize, const C: usize>(
    input: Matrix<T, R, C>,
) -> Matrix<T, C, R>;
```

Useful invariants:

- fixed board dimensions
- packet sizes
- vector lengths
- cryptographic sizes
- image channels
- protocol fields

Limitations:

- Rust const generic expressiveness is still limited compared with dependent types
- generic-const-expr capabilities need verification for current stable Rust
- can increase type complexity

Potential rule:

> Use const generics for genuinely compile-time dimensions, not ordinary business configuration.

---

## 15. Type-level units

Potential crates:

- `uom`
- other units-of-measure crates

Useful idea:

```text
Meters ≠ Seconds
Bytes ≠ Kilobytes
Milliseconds ≠ Timestamp
```

Domain-specific newtypes may be simpler.

Could inspire:

```rust
struct Revision(u64);
struct ByteOffset(u32);
struct Utf16Offset(u32);
struct LineNumber(u32);
```

Very useful in editor/document systems.

---

## 16. Proof-carrying ranges

Instead of:

```rust
struct Range {
    start: usize,
    end: usize,
}
```

with repeated:

```rust
assert!(start <= end);
```

consider:

```rust
struct ValidRange {
    start: usize,
    end: usize,
}

impl ValidRange {
    pub fn new(start: usize, end: usize)
        -> Result<Self, RangeError>
    {
        (start <= end)
            .then_some(Self { start, end })
            .ok_or(RangeError)
    }
}
```

Potential stronger types:

```text
ByteRange
Utf16Range
LineRange
NodeRange
```

This helps prevent coordinate-system mixing.

---

## 17. State-dependent capabilities

Instead of:

```rust
if user.is_authenticated {
    ...
}
```

everywhere, produce:

```rust
AuthenticatedUser
```

Possible pipeline:

```text
SessionToken
  ↓ authenticate
AuthenticatedSession
  ↓ authorize
Authorized<EditDocument>
```

Rust approximation:

```rust
struct Authorized<A> {
    actor: UserId,
    _capability: PhantomData<A>,
}

struct EditDocument;
```

Use carefully: could become over-engineered.

Potentially useful for highly sensitive APIs.

---

## 18. Capability tokens as proof witnesses

Idea:

```rust
fn check_permission(
    actor: &Actor,
    doc: &Document,
) -> Result<CanEditDocument, PermissionError>;
```

Then:

```rust
fn edit_document(
    proof: CanEditDocument,
    doc: Document,
    change: Change,
) -> Document;
```

Benefit:

- separates authorization from mutation
- operation requires explicit witness
- tests can target permission logic separately

Concern:

- proof object forgery if constructors public
- ownership/lifetime design
- whether witness should tie to specific document ID

Possible stronger witness:

```rust
struct CanEditDocument {
    actor: UserId,
    document: DocumentId,
}
```

or lifetime branding.

---

## 19. Lifetime branding / generativity

Research advanced techniques:

- generative lifetimes
- branded indices
- `GhostCell`
- session-local IDs

Potential use:

- ensuring handles belong to the correct graph/document/world
- safe graph mutation
- preventing ID mixing across sessions

Libraries/concepts to investigate:

- `ghost-cell`
- generativity crates
- branded arenas

This may be powerful but should likely remain an advanced skill appendix.

---

## 20. Refinement across serialization boundaries

Important invariant question:

```text
Does deserialization bypass the invariant?
```

Bad:

```rust
#[derive(Deserialize)]
struct Percentage(u8);
```

if serde can construct invalid values directly.

Possible approaches:

- custom `Deserialize`
- `try_from`
- macro crates that preserve validation
- raw DTO → domain conversion

Preferred architectural pattern:

```text
JSON DTO
   ↓ deserialize
RawRequest
   ↓ TryFrom
DomainCommand
```

This makes boundary validation explicit.

---

## 21. Refinements should usually stop at boundaries

Potential rule:

> Do not make transport/storage schemas equal to domain proof types by accident.

Reason:

- DB may contain legacy invalid data
- API must report validation errors
- migrations may need raw access
- version skew

Use:

```text
RawPluginManifest
  ↓ validate
ValidatedPluginManifest
```

instead of directly deserializing into a type that assumes everything is valid, unless the serialization layer correctly enforces the invariant.

---

## 22. Flux ideas to investigate

Flux appears especially relevant to this skill.

Possible properties to experiment with:

- positive values
- bounded indices
- vector length
- loop invariants
- sortedness?
- ownership/state invariants
- arithmetic postconditions
- mutation refinement

Potential skill policy:

> Flux annotations are optional and should target small, critical pure crates.

Do not make normal development depend on verifying the whole workspace unless evidence shows it is practical.

Possible CI:

```text
cargo test
cargo clippy
cargo flux / flux check
```

as separate jobs.

---

## 23. Verus ideas to investigate

Verus may support:

- `requires`
- `ensures`
- `invariant`
- spec functions
- proof functions
- ghost state
- quantified properties

Potential uses:

- parsers
- allocators
- trees
- CRDT invariants
- board-game rules
- document transforms
- security-sensitive protocol state

Potential concerns:

- dialect/toolchain divergence
- learning curve
- proof maintenance
- editor integration
- unsupported Rust features
- interoperability with normal crates
- CI cost

---

## 24. Creusot ideas to investigate

Potential use:

```rust
#[requires(...)]
#[ensures(...)]
```

Questions to verify:

- supported stable/nightly compiler requirements
- current trait/iterator support
- async support
- unsafe support
- library model coverage
- Why3 workflow
- CI ergonomics

Possible place:

```text
algorithm-core
document-tree
game-rules
```

not:

```text
Tauri shell
Axum handler
Leptos component
```

---

## 25. Kani as exhaustive bounded proof

Kani mindset:

```rust
#[kani::proof]
fn property() {
    let x: T = kani::any();
    ...
    assert!(invariant);
}
```

Useful for:

- bit-level code
- integer boundaries
- protocol edge cases
- state machines with bounded state
- panic freedom
- arithmetic overflow
- unsafe abstractions

Possible skill guidance:

> Use model checking for small state spaces and boundary-heavy correctness, not ordinary UI/business orchestration.

---

## 26. Miri as semantic validation

Miri is not a refinement checker.

But include it in the verification skill for:

- undefined behavior
- aliasing violations
- unsafe code
- invalid pointer usage

Possible CI for unsafe-heavy crates:

```text
cargo miri test
```

---

## 27. Prusti / MIRAI / other tools

Keep an investigation list:

- Prusti
- MIRAI
- Crux-MIR
- RustHorn / CHC-based projects
- Rust verification research
- deductive verification over MIR
- symbolic execution tools

Status must be verified before recommending.

---

## 28. Aeneas: Rust → Lean

Especially interesting for proof-oriented core.

Possible workflow:

```text
production Rust
      ↓
Charon / LLBC
      ↓
Aeneas
      ↓
pure functional model
      ↓
Lean
      ↓
proof
```

Research questions:

- Which Rust subset is supported?
- How does mutation translate?
- What happens to traits?
- What happens to references?
- How stable is Lean backend?
- How much proof automation exists?
- Can proofs survive Rust refactors?
- Can generated Lean be kept out of normal dev loop?

Potential use:

```text
core algorithm
   +
Lean proof
```

while keeping Rust as implementation source of truth.

---

## 29. Lean patterns worth teaching an AI even without Lean

### Separate data and proposition

Ask:

> What fact must be true before this function runs?

Try to encode that fact in an input type.

### Strengthen output types

Ask:

> What property is guaranteed after this operation?

Return a stronger type.

### Eliminate impossible cases

Ask:

> Can the type system remove a branch entirely?

### Exhaustive cases

Use `match`.

### Structural recursion / decreasing measures

Relevant for algorithms where termination matters.

Rust itself does not prove termination, but AI can still prefer visibly decreasing recursive logic.

### Invariants around recursion and loops

State the invariant in comments/tests/contracts.

---

## 30. Avoid boolean blindness

Bad:

```rust
fn validate(x: &X) -> bool;
fn permission(x: &X) -> bool;
fn ready(x: &X) -> bool;
```

Potentially better:

```rust
fn validate(x: X)
    -> Result<ValidX, ValidationError>;

fn authorize(actor: Actor, resource: Resource)
    -> Result<CanEdit, PermissionError>;

fn prepare(x: X)
    -> Result<PreparedX, PrepareError>;
```

Boolean predicates remain useful when the caller genuinely only needs a yes/no decision.

---

## 31. Refined collections

Potential types:

```text
NonEmpty<T>
Unique<T>
Sorted<T>
AtLeast<N, T>
Exactly<N, T>
BoundedVec<T, MIN, MAX>
```

Questions:

- library support?
- proof preservation after operations?
- compile cost?
- serialization?
- ergonomics?

Maybe create domain-specific wrappers rather than universal generic refinements.

---

## 32. Proof preservation methods

If:

```rust
SortedVec<T>
```

is sorted, methods should preserve proof:

```rust
fn insert(self, value: T) -> SortedVec<T>;
```

Avoid:

```rust
fn as_mut_vec(&mut self) -> &mut Vec<T>;
```

because it destroys the invariant.

Potential rule:

> APIs on refined types should preserve their invariant by construction.

---

## 33. Escape hatches

Every strong invariant abstraction may need deliberate escape hatches.

Examples:

```rust
unsafe fn new_unchecked(...)
pub(crate) fn from_storage_unchecked(...)
fn into_inner(...)
```

Rules:

- keep visibility narrow
- document proof obligation
- use only at verified boundaries
- test callers

Potential Lean-like naming:

```text
assume_valid
new_unchecked
trusted_from_parts
```

---

## 34. Runtime refinement vs compile-time refinement

Distinguish clearly:

### Runtime checked

```text
newtype + constructor
nutype
refined crate
validator
```

### Compile-time/static proof

```text
Flux
Verus
Creusot
Kani (model checking)
Lean/Aeneas
```

Do not claim runtime refinement gives theorem-prover guarantees.

---

## 35. Compile-time cost discipline

Refinement features can increase build cost through:

- proc macros
- generic predicates
- nested type-level expressions
- monomorphization
- verifier passes
- solver time

Potential rule:

> Keep foundational public types simple and named.

Example:

Prefer public:

```rust
pub struct Username(...);
```

over public:

```rust
pub type Username =
    Refinement<
        String,
        And<Trimmed, And<NonEmpty, MaxLen<64>>>
    >;
```

The implementation may use a library internally.

---

## 36. Refinement crate boundaries

Possible layout:

```text
domain-types/
    simple plain Rust invariant types

document-core/
    algorithms

document-verify/
    Flux / Verus / proof harnesses

adapters/
    ...
```

Alternative:

Keep Flux annotations close to source if tooling requires it.

Research which layout works with each verifier.

---

## 37. AI review prompts

Ask the coding agent:

### Invariants

- What invariants are currently comments or assertions?
- Are they checked repeatedly?
- Can they become private-constructor types?
- Can an enum remove illegal combinations?

### Inputs

- Does this function accept values weaker than necessary?
- Can a stronger input type eliminate validation branches?

### Outputs

- Can the output encode a guarantee?

### Lifecycles

- Can invalid operation ordering be represented by separate states?

### Verification

- Is this property important enough for property tests?
- Is it bounded enough for Kani?
- Is it arithmetic/refinement-heavy enough for Flux?
- Is it critical enough for Verus/Lean?

---

## 38. Candidate strict rules

Possible future skill rules:

1. Repeated invariant checks should trigger consideration of a type.
2. Fallible validation should return a stronger type.
3. Domain invariant fields should usually be private.
4. Prefer named refined types over exposing deeply nested generic predicate types.
5. Prefer enums/separate types before complex typestate.
6. Keep formal verification isolated to correctness-critical core logic.
7. Do not claim a property is statically proven unless a verifier actually proves it.
8. Serialization/storage must not bypass invariants silently.
9. Refined APIs should preserve their invariant.
10. Use unsafe/unchecked constructors only with explicit proof obligations.

---

## 39. Potential experiments

Create tiny benchmark/example crates:

### Experiment A — plain newtype

```text
PositiveU32
NonEmptyString
Percentage
```

Measure compile time.

### Experiment B — nutype

Same invariants with `nutype`.

Measure:

- clean build
- incremental build
- macro expansion
- binary size

### Experiment C — generic refined

Use composed predicates.

Measure monomorphization and diagnostics.

### Experiment D — Flux

Prove:

- positive addition
- bounded index
- vector access
- state transition

### Experiment E — Kani

Check a small board-game state machine.

### Experiment F — Aeneas

Translate a pure Rust algorithm into Lean and prove a simple property.

---

## 40. Possible project-specific applications

### Document engine

Potential refined concepts:

```text
NodeId
Revision
Utf16Offset
ByteOffset
ValidRange
CanonicalPath
ParsedDocument
NormalizedDocument
ResolvedDocument
```

### Plugin system

```text
PluginId
ValidatedManifest
InstalledPlugin
ResolvedEntrypoint
CompatibleInterfaceVersion
ResolvedSurface
```

### Board-game engine

```text
PlayerCount
TurnOrder
NonEmptyPlayers
LegalMove
InitializedGame
RunningGame
FinishedGame
Deck<Shuffled>
```

### Auth

```text
RawToken
VerifiedToken
AuthenticatedIdentity
AuthorizedCapability
```

These are idea candidates, not prescriptions.

---

## 41. Possible skill names

- `rust-refinement-design`
- `rust-lean-patterns`
- `rust-proof-oriented`
- `rust-type-driven`
- `rust-invariant-modeling`
- `rust-types-as-proofs`
- `rust-correct-by-construction`

Potentially use:

```text
rust-types-as-proofs
```

for ordinary code, and a separate:

```text
rust-formal-verification
```

for Flux/Verus/Lean tooling.

