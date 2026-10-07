# Boundary hardening for refined types

Supporting reference for [Tabula engineering](../SKILL.md) and
[types as proofs](types-as-proofs.md). Read when a validated value crosses serde, storage,
fixtures, FFI, or migration boundaries. SQL examples belong to storage adapters, not game rules.

## Contents

1. Decide whether schema and domain are the same type
2. Route serde through validation
3. Convert database rows explicitly
4. Audit bypasses
5. Test boundary preservation
6. Schema compatibility

## 1. Decide whether schema and domain are the same type

Use one type only when all of these are true:

- the invariant is intrinsic and stable across versions;
- decode failure is the correct boundary behavior;
- no migration needs access to pre-invariant raw data;
- error reporting from a single conversion is sufficient.

Use separate DTO/domain types when API callers need field-level diagnostics, persisted data may
predate current rules, compatibility is versioned independently, or migrations must inspect raw
values.

```rust
#[derive(serde::Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub age: u32,
}

pub struct CreateUser {
    pub username: Username,
    pub age: AdultAge,
}

impl TryFrom<CreateUserRequest> for CreateUser {
    type Error = ValidationErrors;

    fn try_from(raw: CreateUserRequest) -> Result<Self, Self::Error> {
        // Accumulate user-facing field errors here.
        todo!()
    }
}
```

Inside the domain, fail-fast errors are usually right. At a human-facing input boundary, reporting
all independent field failures in one response is often better.

## 2. Route serde through validation

Deriving `Deserialize` directly on a tuple newtype may construct its field without calling `new`.
Route through `TryFrom` when the encoded representation is intentionally the same:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Percentage(u8);

impl TryFrom<u8> for Percentage {
    type Error = PercentageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Percentage> for u8 {
    fn from(value: Percentage) -> Self {
        value.0
    }
}
```

Write conversion errors for their boundary context: include the invalid class and expected range,
but do not leak secrets.

Serde `try_from` requires the conversion error to implement `Display`. The `into` serialization
side requires the refined type to implement `Clone`, because serialization receives a reference
and clones before converting. Add only the direction the format needs. Keep diagnostics useful
at the field path where deserialization reports them.

Canonical state may need a deliberately pinned encoding rather than a generic serde format. Follow
the repository's hashing/versioning contract instead of assuming a JSON round trip is sufficient.

## 3. Convert database rows explicitly

Decode into a raw row and then validate:

```rust
#[derive(sqlx::FromRow)]
struct AccountRow {
    id: uuid::Uuid,
    balance: i64,
    status: String,
}

impl TryFrom<AccountRow> for Account {
    type Error = CorruptAccountRow;

    fn try_from(row: AccountRow) -> Result<Self, Self::Error> {
        todo!()
    }
}
```

An invalid stored row is corruption or version skew, not user validation failure. Include the
stable row identity in diagnostics/metrics. Never silently `filter_map` invalid rows unless data
loss is an explicit policy and counted.

For a stable one-column type, a centralized database decoder can be appropriate, but its tests
must prove that all decoded values pass the same constructor.

## 4. Audit bypasses

Search for:

- public tuple/struct fields;
- `Default`, especially for non-empty and non-zero values;
- `DerefMut`, `AsMut`, `BorrowMut`, mutable raw collection access;
- `Deserialize`, `FromRow`, binary/archive decoders, FFI constructors;
- `Arbitrary` strategies that build fields directly;
- `pub(crate)` constructors whose reachable scope grew;
- migration code and old fixtures;
- macros that expand to struct literals;
- `mem::zeroed` or unsafe initialization (normally forbidden).

For property generators, build valid values through the public constructor. Generate raw invalid
values separately for rejection properties.

## 5. Test boundary preservation

Minimum tests:

```rust
#[test]
fn deserialization_rejects_value_above_maximum() {
    assert!(serde_json::from_str::<Percentage>("101").is_err());
}

#[test]
fn serialization_round_trip_preserves_valid_value() {
    let original = Percentage::new(42).unwrap();
    let encoded = serde_json::to_string(&original).unwrap();
    let decoded = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, original);
}
```

Also test:

- the exact limit and adjacent values;
- old fixtures for each supported schema version;
- unknown/new enum variants according to compatibility policy;
- corrupt rows fail loudly and identify the record;
- migration followed by validation yields a valid current domain value;
- canonical encoding is stable where hashes/replays depend on it.

## 6. Schema compatibility

Domain values mean valid under the current rules; wire and stored values may come from older
code. Keep strict domain validation in the conversion and make raw compatibility deliberate:

- A new field may use `#[serde(default)]` only when that default has the intended old-version
  meaning. Do not silently invent valid current domain data from corrupt or incomplete records.
- Unknown fields/variants may require versioned rejection or an explicit fallback according to
  the protocol contract. Config typo rejection and forward-compatible service messages have
  different needs. Inspect actual serde behavior when using flattening and unknown-field rules.
- Long-lived formats need an explicit version and a fixture per supported version. Current code
  must replay exactly, migrate explicitly, or mark an artifact unreplayable (I-13/I-16).
- Changing validation must leave a way to read old raw rows for migration. A stricter domain
  constructor should not make the only storage decoder lose access to data needing repair.

Use [replay / differential testing](replay-differential-testing.md) for compatibility fixtures.
