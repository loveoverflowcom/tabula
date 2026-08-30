# Serde and storage boundaries

Where invariants go to die. Read this before making a validated type `Deserialize`, or before
deciding a domain type can double as a wire/row type.

## Contents

1. [The failure mode](#1-the-failure-mode)
2. [Fix A: route through the constructor](#2-fix-a-route-through-the-constructor)
3. [Fix B: separate DTO and domain type](#3-fix-b-separate-dto-and-domain-type)
4. [Choosing between A and B](#4-choosing-between-a-and-b)
5. [Databases](#5-databases)
6. [Other silent bypasses](#6-other-silent-bypasses)
7. [Schema evolution](#7-schema-evolution)
8. [Tests worth writing](#8-tests-worth-writing)

---

## 1. The failure mode

```rust
pub struct Percentage(u8);

impl Percentage {
    pub fn new(v: u8) -> Result<Self, Error> {
        (v <= 100).then_some(Self(v)).ok_or(Error::OutOfRange)
    }
}

#[derive(Deserialize)]      // <- constructs the field directly
pub struct Settings { pub opacity: Percentage }
```

`derive(Deserialize)` generates code that builds `Percentage` from its fields. `new` is never
called. `{"opacity": 250}` parses fine, and from that point the rest of the program holds a
`Percentage` that is not a percentage — with every downstream function reasonably assuming it is.

This is worse than having no newtype at all, because the type now advertises a guarantee it is
not providing. Reviewers see `Percentage` and stop checking.

The same applies to `sqlx::FromRow`, `bincode`, `rkyv`, `#[derive(Default)]`, and any `pub` field.

---

## 2. Fix A: route through the constructor

Serde's container attributes let you insist that deserialization go through a conversion.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Percentage(u8);

impl TryFrom<u8> for Percentage {
    type Error = PercentageError;
    fn try_from(v: u8) -> Result<Self, Self::Error> { Percentage::new(v) }
}

impl From<Percentage> for u8 {
    fn from(p: Percentage) -> u8 { p.0 }
}
```

Requirements, all of which the compiler will tell you about:

- `try_from = "T"` needs `TryFrom<T> for Self`, and the associated `Error` must implement
  `Display` (serde renders it into the deserialization error).
- `into = "T"` needs `From<Self> for T` **and** `Self: Clone`, because serialization takes the
  value by reference and must clone to convert.
- If you only need one direction, use only the one attribute.

For a `String` newtype the shape is identical with `#[serde(try_from = "String", into =
"String")]`.

Error quality: a failure now surfaces as a serde error mentioning your `Display` text and the
field path, e.g. `opacity: value out of range at line 3 column 18`. Write the `Display` message
so it is useful in that position — say what was wrong, not just "invalid".

---

## 3. Fix B: separate DTO and domain type

The stronger option: the wire type and the domain type are different types, and validation is an
explicit step you can see in the code.

```rust
// wire — mirrors the payload exactly, no rules
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub age: u32,
    #[serde(default)]
    pub nickname: Option<String>,
}

// domain — every field carries its guarantee
pub struct CreateUser {
    pub username: Username,
    pub age: AdultAge,
    pub nickname: Option<DisplayName>,
}

impl TryFrom<CreateUserRequest> for CreateUser {
    type Error = ValidationErrors;

    fn try_from(r: CreateUserRequest) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::default();

        let username = Username::parse(&r.username)
            .map_err(|e| errors.push("username", e)).ok();
        let age = AdultAge::new(r.age)
            .map_err(|e| errors.push("age", e)).ok();
        let nickname = r.nickname.as_deref().map(DisplayName::parse).transpose()
            .map_err(|e| errors.push("nickname", e)).ok().flatten();

        if !errors.is_empty() { return Err(errors); }
        Ok(CreateUser { username: username.unwrap(), age: age.unwrap(), nickname })
    }
}
```

The reason to accumulate errors rather than `?` on the first one: an API that reports one
validation failure per round-trip is a bad API, and this is the layer where users are watching.
Inside the domain, fail-fast `?` is the right default — that code is not talking to a human.

---

## 4. Choosing between A and B

| Situation | Use |
|---|---|
| Leaf value type, invariant is intrinsic and permanent (`Percentage`, `Uuid` wrapper) | A |
| Public API request/response bodies | B |
| You must report *which* fields failed and why | B |
| Persisted data may predate the current rules | B |
| Config files where a clear parse error is the whole UX | A is usually enough |
| Internal service-to-service messages you version together | A |

Default to A for small value types and B at every trust boundary. They compose: the DTO in B can
hold plain `String`s while individual value types elsewhere use A.

The general principle is that a domain type asserts "this is valid **now**, by today's rules".
Storage and wire formats hold data written by past code under past rules. Equating them means a
single rule change can make old data unloadable — you find out in production, during a rollout,
with no way to read the rows to fix them.

---

## 5. Databases

`sqlx::FromRow` has the same problem as `Deserialize` and no `try_from` container attribute.
Options:

**Implement `sqlx::Type` / `Decode` on the newtype** so the conversion is centralized. This is
the tidiest when the type maps to a single column.

**Or query into a row struct, then convert** — the B pattern again:

```rust
#[derive(sqlx::FromRow)]
struct AccountRow { id: Uuid, balance: i64, status: String }

impl TryFrom<AccountRow> for Account {
    type Error = CorruptRow;
    fn try_from(r: AccountRow) -> Result<Self, Self::Error> { /* ... */ }
}
```

Decide explicitly what a row that fails conversion means. It is not a user error — it is
corruption or version skew, and it usually deserves a distinct error type, a log line with the
primary key, and a metric. Silently `filter_map`-ing bad rows out of a list query hides data loss;
if you do it deliberately, count it.

---

## 6. Other silent bypasses

Audit refined types for all of these:

- `#[derive(Default)]` — `Default` for `Percentage` gives `0`, which is fine, but `Default` for
  `NonEmptyName` gives `""`, which is a lie. Implement `Default` by hand or not at all.
- `pub` fields, and tuple-struct fields that are `pub` by accident.
- `DerefMut`, `AsMut`, `BorrowMut`, `as_mut_vec`, `iter_mut` on a collection whose ordering or
  uniqueness is the invariant.
- `#[derive(Arbitrary)]` for proptest/fuzzing — it will generate invalid values and produce
  failures that are not real bugs. Write the strategy so it builds values through the constructor.
- Any `pub(crate)` constructor that has quietly become reachable from more of the crate than
  intended as the crate grew.
- `serde(flatten)` and `#[serde(deny_unknown_fields)]` interactions — flatten silently disables
  unknown-field rejection for the flattened part.

---

## 7. Schema evolution

Keep raw types permissive so old data keeps loading, and put strictness in the conversion:

- Add fields with `#[serde(default)]` so previously written data still decodes.
- Adding an enum variant breaks old readers. If external parties or persisted data are involved,
  include an unknown/other fallback from day one, or version the payload explicitly.
- `#[serde(deny_unknown_fields)]` is a good default for config the user writes by hand (typos
  become errors) and a bad default for messages from a service that may be newer than you.
- Store a schema version when the format is long-lived, and keep one fixture file per version in
  the repo — see `rust-verification-testing` for the corresponding regression test.

---

## 8. Tests worth writing

These are cheap, and each one catches a real class of the failures above:

```rust
#[test]
fn deserialization_rejects_out_of_range() {
    assert!(serde_json::from_str::<Percentage>("250").is_err());
}

#[test]
fn json_roundtrip_preserves_value() {
    let p = Percentage::new(42).unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<Percentage>(&json).unwrap(), p);
}

#[test]
fn v1_fixtures_still_decode() {
    for entry in std::fs::read_dir("tests/fixtures/v1").unwrap() {
        let raw = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        serde_json::from_str::<StoredDocument>(&raw).expect("v1 fixture must still decode");
    }
}
```

The first is the one people skip. It is also the one that fails the day someone adds
`#[derive(Deserialize)]` back onto a validated type in a hurry.
