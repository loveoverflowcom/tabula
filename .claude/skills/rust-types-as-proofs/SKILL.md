---
name: rust-types-as-proofs
description: >
  Encode Rust domain invariants in types so they are checked once at a boundary instead of
  re-asserted everywhere. Covers parse-don't-validate, newtypes with private fields and smart
  constructors, when a newtype earns its keep, avoiding boolean-blind APIs, typestate vs plain
  separate types, keeping serde and database deserialization from bypassing validation, invariant-
  preserving APIs, and safe escape hatches. Use this when modeling domain types, writing or
  reviewing validation code, naming things `validate_*`/`is_valid`/`check_*`, adding a
  `#[derive(Deserialize)]` to a type with rules, seeing repeated `assert!`/`unwrap`/re-checks of
  the same condition, passing raw `String`/`u32`/`Uuid` through several layers, or designing a
  lifecycle (Raw → Validated → Resolved, Disconnected → Connected). Applies even when the user
  says only "add validation" or "this keeps breaking".
---

# Types as proofs

The idea in one line: **a value's type should tell you what has already been checked about it.**

```rust
fn save(name: String)      { assert!(!name.is_empty()); }  // checked here, and at 9 other call sites
fn save(name: NonEmptyName) { }                            // checked once, where it entered
```

`NonEmptyName` is a `String` plus evidence it is non-empty. Rust cannot carry a proof term the
way Lean can, but a private field plus a fallible constructor gets the same practical effect:
**there is exactly one place the invariant can be established, and the compiler enforces that it
was.**

## When a newtype earns its keep

Do not wrap primitives mechanically — `struct Count(usize)` with no rules and no distinct meaning
is pure friction. Introduce one when it buys at least one of these:

| Reason | Example |
|---|---|
| An invariant to establish once | `Percentage`, `NonEmptyName`, `ValidRange` |
| Preventing accidental mixing | `UserId` vs `RoomId` — both `Uuid`, never interchangeable |
| Unit / coordinate-system safety | `ByteOffset` vs `Utf16Offset` vs `LineNumber` |
| A privacy boundary | callers cannot reach in and corrupt the representation |
| A serialization boundary | wire format decoupled from in-memory form |
| A stable public API | you can change the inside without breaking callers |

The strongest of these in practice is coordinate-system safety. `fn slice(text: &str, start: u32,
end: u32)` has silently shipped in every editor ever written with the wrong offset kind; `fn
slice(text: &str, range: Utf16Range)` cannot.

## The core pattern

```rust
pub struct Percentage(u8);   // private field is the whole mechanism

impl Percentage {
    pub fn new(value: u8) -> Result<Self, PercentageError> {
        (value <= 100).then_some(Self(value)).ok_or(PercentageError::OutOfRange)
    }
    pub fn get(self) -> u8 { self.0 }
}
```

`pub struct Percentage(pub u8)` is not a refinement — it is a comment. If the invariant matters,
the field is private and lives in a module that keeps it that way.

Conventions worth following: `TryFrom` for fallible conversion, `From` only for infallible ones,
`parse` for string input. A `From` impl that can panic or truncate is a trap, because callers
reasonably assume `From` is total.

## Prefer proof-producing over boolean-returning

```rust
fn is_valid(doc: &Document) -> bool;                                // evidence discarded
fn validate(doc: RawDocument) -> Result<ValidDocument, Error>;      // evidence preserved
```

A `bool` is forgotten the moment the `if` closes. Everything downstream must either re-check or
trust a comment. The same move applies broadly:

```rust
fn authenticate(token: RawToken)  -> Result<AuthenticatedUser, AuthError>;
fn authorize(actor: &Actor, doc: &Document) -> Result<CanEdit, PermissionError>;
fn resolve(req: SurfaceRequest)   -> Result<ResolvedSurface, ResolveError>;
```

Now `fn edit(proof: CanEdit, doc: Document, change: Change) -> Document` cannot be called by code
that skipped the permission check — the argument is unforgeable if `CanEdit`'s constructor is
private. Make such a witness carry *what* was authorized (`CanEdit { actor: UserId, document:
DocumentId }`), otherwise a proof obtained for one document authorizes editing another.

Booleans stay correct when the caller genuinely only needs a yes/no and nothing downstream
depends on it: `if list.is_empty()`, `if flag_enabled(...)`. The smell is a `bool` that licenses
a later operation.

## Serde and storage will bypass your constructor

This is the single most common way a carefully built invariant silently dies.

```rust
#[derive(Deserialize)]
pub struct Percentage(u8);   // deserializes 250 without complaint
```

`derive(Deserialize)` constructs the value field-by-field. It never calls `new`. Same for
`sqlx::FromRow`, and for any `Default` impl you derived without thinking. Route deserialization
through the constructor:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Percentage(u8);
```

This requires `TryFrom<u8> for Percentage` whose `Error: Display`, plus `From<Percentage> for u8`
and `Clone` for the `into` side.

Separately: decide deliberately whether the stored/wire schema *should* equal the domain type.
Usually it should not. Databases hold rows written by older code, APIs must report validation
errors rather than fail to parse, and migrations need raw access. The durable pattern is
`RawManifest → validate → ValidatedManifest`, with the raw type owning the derives. See
`references/serde-and-storage-boundaries.md` before making a validated type `Deserialize`.

## Lifecycles: reach for the simplest form that works

Three ways to encode "this must happen before that", cheapest first:

**Separate named types** — start here.

```rust
RawManifest → ValidatedManifest → ResolvedManifest
```

Readable in signatures, no generics, no `PhantomData`, works with `dyn`, serializes normally.
Covers the large majority of real lifecycles.

**An enum** when a value must be storable in any stage: `enum Manifest { Raw(..), Validated(..) }`.

**Typestate** only when you need one struct with shared behaviour across states:

```rust
pub struct Connection<S> { inner: Inner, _state: PhantomData<S> }
pub struct Disconnected;
pub struct Connected;

impl Connection<Connected> { pub fn send(&mut self, msg: Message) { /* ... */ } }
```

Typestate fits **one strong lifecycle axis** — handshake, upload session, transaction builder.
It fights you when states change at runtime based on data, when callers need a `Vec` of mixed
states (you now need an enum wrapper anyway, so start there), when there are several independent
axes (combinations multiply), or when the parameter starts appearing in public signatures that
users must spell out. Two orthogonal `PhantomData` axes is the usual point where readers stop
being able to follow the API.

## Refined APIs must preserve their own invariant

An invariant is only as strong as the weakest method on the type.

```rust
impl SortedVec<T> {
    pub fn insert(self, value: T) -> SortedVec<T>;      // preserves sortedness
    pub fn as_mut_vec(&mut self) -> &mut Vec<T>;        // destroys it, silently
}
```

Any method handing out `&mut` to the raw representation, or a `pub` field, or a `DerefMut`, is an
escape hatch you did not mean to create. Audit for these whenever you add a method to a refined
type. Read-only exposure (`Deref` to `&[T]`, `fn get`, `fn into_inner`) is fine — it cannot
violate anything.

## Escape hatches, done deliberately

Real systems need to skip validation sometimes: trusted storage, hot loops, migration code.
Provide the hatch explicitly rather than making people reach for the public constructor with
made-up data.

```rust
impl Percentage {
    /// # Safety obligation (not memory safety)
    /// `value` must be <= 100. Callers must have established this by other means —
    /// e.g. it was validated before being written to the database.
    pub(crate) fn from_storage_unchecked(value: u8) -> Self { Self(value) }
}
```

Keep visibility as narrow as it will go, name it so the risk is visible at the call site
(`_unchecked`, `assume_valid`, `trusted_from_parts`), and write down the obligation. Do not mark
it `unsafe fn` unless violating it causes *memory* unsafety — misusing `unsafe` for logical
invariants trains people to ignore it where it counts.

## Be honest about what is proven

| Mechanism | Guarantee |
|---|---|
| Newtype + private field + constructor | Checked **at runtime**, once, at a known place |
| Typestate / `PhantomData` | Operation **ordering** checked at compile time |
| Const generics | **Dimensions** checked at compile time |
| Property tests / fuzzing | Evidence, not proof — see `rust-verification-testing` |
| Kani, Flux, Verus, Creusot | Actual static proof, within stated bounds |

A smart constructor is a runtime check with a good filing system, and that is genuinely valuable.
Do not describe it as "provably correct" — reserve that language for a verifier that actually ran.
For static tools, see `rust-verification-testing`; they are experimental to varying degrees and
their status should be checked before adopting.

## Keep the type name simpler than its predicate

Generic refinement combinators are elegant and unreadable in signatures:

```rust
pub type Username = Refinement<String, And<Trimmed, And<NonEmpty, MaxLen<64>>>>;  // avoid in public API
pub struct Username(String);                                                       // prefer
```

Error messages, rustdoc, and IDE hovers all render the first form in full. Implement with
whatever machinery you like — including a macro crate such as `nutype` for repetitive leaf types
— but expose a named type. Foundational types that many crates depend on are also where proc
macro cost and opacity hurt most, so those are the ones to keep as plain Rust.

## Review checklist

- Is the same condition asserted or re-checked in more than one place? → constructor invariant.
- Does a function take a weaker type than it actually requires? → strengthen the parameter.
- Does a function return `bool` where the caller then does something privileged? → return a witness.
- Can `derive(Deserialize)`, `FromRow`, `Default`, or a `pub` field bypass the constructor?
- Does any method hand out `&mut` to the inner representation?
- Are two `PhantomData` parameters accumulating on one type? → probably switch to enums.
- Does an `Option` field mean "not applicable in this state"? → the state should be an enum.
- Is an unchecked constructor's proof obligation written down, and is its visibility minimal?

## Deeper material

- `references/serde-and-storage-boundaries.md` — DTO/domain split, `try_from`, `FromRow`,
  schema evolution, and why validated types usually should not be your wire types.

## Related skills

- `rust-functional-core` — where these types live and which layer may construct them.
- `rust-verification-testing` — proving the invariants actually hold.
