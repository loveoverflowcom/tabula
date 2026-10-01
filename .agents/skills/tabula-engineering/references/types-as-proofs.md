# Rust types as proofs

Supporting technique for [Tabula engineering](../SKILL.md). The foundation owns the overall
workflow, check status, and evidence report; use this reference only for the named technique.

Make each type state exactly what has already been established about a value. Read the nearest
`AGENTS.md` and normative architecture docs before changing a public type or crate boundary.

## Translate the Lean mindset honestly

Use these ideas, without pretending Rust is a theorem prover:

| Lean/refinement idea | Practical Rust form |
|---|---|
| Proposition about a value | Invariant stated for a named domain type |
| Proof term | Value constructible only after the check |
| Subtype `{x // P x}` | Newtype with private representation and fallible constructor |
| Proof-producing theorem | `Raw -> Result<Validated, Error>` |
| Impossible case elimination | Enum variants / strengthened parameters |
| Indexed state | Separate state types; typestate only when justified |
| Capability proof | Unforgeable witness scoped to the authorized resource |
| Theorem | Property/model/formal verification result, not a comment |

A private newtype is runtime-checked evidence with compiler-enforced provenance. Call it a proof
*barrier*, not a formal proof. Reserve “proved” for a verifier that actually ran under a stated
model.

## Design procedure

1. State the proposition in one sentence: `Percentage is <= 100`, `LegalMove is valid for this
   position`, or `CanEdit authorizes actor A for document D`.
2. Locate every construction path: public fields, constructors, deserialization, database rows,
   defaults, tests, migrations, FFI, macros, and unchecked helpers.
3. Choose the cheapest representation that removes the target invalid states.
4. Make the representation private and expose only invariant-preserving operations.
5. Strengthen consumers to accept the refined type or scoped witness, so they cannot forget the
   check.
6. Keep wire/storage DTOs raw unless their compatibility contract truly equals the domain
   invariant. Convert at the trust boundary.
7. Test constructor partitions, bypass paths, preservation under every mutator, and round trips.
8. Escalate to property/model/formal verification only when the risk and state space justify it.

## Choose the smallest sufficient encoding

Use this order:

1. **Named primitive/newtype** for one intrinsic invariant, unit, identity, or coordinate system.
2. **Struct of refined fields** when invariants compose independently.
3. **Enum** when several fields are conditionally valid or states are mutually exclusive.
4. **Separate input/output types** for `Raw -> Validated -> Resolved` pipelines.
5. **Scoped witness** when a check authorizes a later operation on specific resources.
6. **Typestate** for one strong, mostly compile-time lifecycle axis shared by one object.
7. **Const generics/static verifier** only for high-value invariants the earlier forms cannot
   express economically.

Prefer a boring named type over a generic refinement expression in public APIs. Compiler errors,
rustdoc, and agent context remain short and local.

## Newtype proof barrier

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percentage(u8);

impl Percentage {
    pub fn new(value: u8) -> Result<Self, PercentageError> {
        (value <= 100)
            .then_some(Self(value))
            .ok_or(PercentageError::OutOfRange { value })
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}
```

The private field is the mechanism. `pub struct Percentage(pub u8)` is only documentation.
Implement `TryFrom` for fallible conversion and `From` only for infallible conversion. Never hide
panic, truncation, or lossy normalization in `From`.

A newtype earns its cost when it establishes an invariant, prevents accidental mixing, records a
unit/coordinate system, creates a privacy boundary, decouples serialization, or stabilizes a
public API. Do not wrap primitives mechanically.

## Produce evidence instead of booleans

```rust
fn is_valid(raw: &RawDocument) -> bool;

fn validate(raw: RawDocument) -> Result<ValidDocument, ValidationError>;
```

The first discards evidence after the branch. The second makes downstream functions require it.
Apply the same shape to authentication, authorization, resolution, canonicalization, and command
legality.

Scope witnesses to the facts they prove:

```rust
pub struct CanEdit {
    actor: UserId,
    document: DocumentId,
}

pub fn authorize_edit(
    actor: &Actor,
    document: &Document,
) -> Result<CanEdit, PermissionError>;

pub fn edit(proof: CanEdit, document: Document, change: Change) -> Document;
```

Carrying resource IDs alone does not bind them to `document`: the consumer must compare them,
or use a borrow/lifetime or resource-specific wrapper that makes mismatch impossible. Likewise,
bind state-dependent evidence to a version or scoped borrow so it cannot survive mutation.
An unscoped or unchecked witness can authorize the wrong resource and is not useful evidence.
Keep ordinary predicates returning `bool` when no later operation relies on their provenance.

## Model states, not flag combinations

Replace fields such as `installed: bool`, `runtime: Option<_>`, and `error: Option<_>` with an
enum whose variants carry only valid data. Prefer exhaustive matching to `unreachable!`.

Start lifecycles with separate types:

```text
RawManifest → ValidatedManifest → ResolvedManifest
```

Use typestate only when one object genuinely shares behavior across a stable, compile-time state
axis. Avoid it for heterogeneous collections, data-driven transitions, public APIs users must
name frequently, or two independent `PhantomData` axes. An enum is usually clearer there.

## Preserve the proposition

Audit every method on a refined type. `DerefMut`, `AsMut`, public fields, raw mutable iterators,
unchecked setters, and derived `Default` can invalidate the guarantee. Expose semantic mutators
that return a refined value or `Result`.

For indexed or non-empty collections, check operations that remove, reorder, split, merge, and
deserialize—not only construction.

## Harden trust boundaries

Serde, row decoders, persistence migrations, and FFI can bypass a smart constructor. Do not derive
`Deserialize`, `FromRow`, `Default`, or `Arbitrary` on a validated type until you have verified the
generated construction path preserves the invariant.

Prefer:

```text
wire/storage DTO → parse/decode → validate/migrate → domain value
```

Read [boundary-hardening](boundary-hardening.md) before adding serialization, row mapping, fixtures, or an
unchecked constructor to a refined type.

## Escape hatches

If trusted recovery or a hot path must bypass validation:

- keep visibility as narrow as possible;
- include `_unchecked`, `assume_valid`, or `trusted_from_parts` in the name;
- document the precise proof obligation and the external evidence that establishes it;
- add a debug assertion when cheap;
- test the trusted producer and consumer together.

Do not use `unsafe fn` for a logical invariant unless violating it can cause memory unsafety.

## Verification obligations

For every introduced or changed refined type, verify:

- each valid boundary representative constructs successfully;
- values just below/at/above every limit behave correctly;
- malformed, empty, maximal, duplicate, and overflow cases are covered where applicable;
- deserialization/database/fixture paths cannot forge invalid values;
- every public mutator preserves the invariant;
- equality, ordering, hashing, and canonical serialization match the domain meaning;
- unchecked constructors have a documented and tested producer obligation.

Use the foundation's evidence step and [verification selection](verification-testing.md) when
boundary examples cannot cover the changed claim. A naive reference validator can support
[properties](property-testing.md); a scoped [mutation campaign](mutation-testing.md) can probe
whether a critical authorization check is asserted. Neither is an automatic requirement.
Use [AI doc contracts](ai-doc-contracts.md) when a high-value invariant or witness benefits from
a durable link to its actual evidence.

## Design evidence

Use the foundation report; include the design facts below.

State: proposition encoded, construction barrier, boundary conversions, preservation evidence,
and any remaining runtime assumption. Do not claim stronger verification than was executed.
