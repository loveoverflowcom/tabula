# Rust AI doc contract schema

This file is the normative schema for `@ai.*` tags in this skill.

## Contents

1. Grammar
2. Keys and semantics
3. Identifier conventions
4. Validation rules
5. Graph mapping
6. Versioning guidance

## 1. Grammar

An annotation occupies one Rust doc-comment line:

```text
annotation = doc-prefix, "@ai.", key, " ", value ;
doc-prefix = "/// " | "//! " ;
key        = "role" | "domain" | "pure" | "invariant" | "law"
           | "requires" | "ensures" | "evidence" | "read-first" | "related" ;
```

Rules:

- Use one annotation per line.
- Trim leading/trailing whitespace around the full value.
- Do not use flag-only tags, comma-separated lists, quoting, or inline comments.
- Repeat multi-valued keys.
- Keep ordinary Markdown outside annotation values.
- Annotations attach to the item receiving the `///` block or the module receiving `//!` docs.

## 2. Keys and semantics

### `role`

One semantic responsibility, such as `domain-transition`, `projection-boundary`,
`smart-constructor`, `canonical-encoder`, `effect-port`, or `adapter`. It is not the Rust item kind.

### `domain`

Stable ownership path such as `game.rules`, `document.reconcile`, or `protocol.codec`. This is not
necessarily the module path; use a domain concept that survives file moves.

### `pure`

`true` means referential behavior depends only on explicit arguments and documented deterministic
context. `false` is useful only for a boundary that might otherwise be mistaken for pure. Omit it
when purity is irrelevant.

### `invariant`

A state predicate that the item establishes or preserves, for example
`node-id-uniqueness` or `rejected-input-preserves-state`.

### `law`

An equation/relationship across calls or transformations, for example `normalization-idempotent`,
`codec-round-trip`, or `replay-equivalent-to-live`.

### `requires` and `ensures`

Named logical pre/postconditions. Prefer proof-bearing type names in the Rust signature. Use these
tags when a condition cannot be encoded economically, for unchecked/trusted paths, or to connect
an item to a shared predicate in the graph.

### `evidence`

Rust path to a test, specification item, model check, or formal proof artifact. The path is an
index key; the checker does not pretend to resolve/build it. The verification workflow must still
run the cited evidence.

### `read-first`

Rust path to the smallest high-value context a future editor should inspect before changing this
item, commonly a property test or state enum. Do not build long reading lists.

### `related`

Rust path to a semantically related symbol when signature/doc links do not already make the
relationship clear.

## 3. Identifier conventions

- `role`, `invariant`, `law`, `requires`, `ensures`: lowercase kebab-case.
- `domain`: lowercase dotted segments; each segment may contain hyphens.
- `pure`: exactly `true` or `false`.
- `evidence`, `read-first`, `related`: Rust-style symbol path using identifiers and `::`; optional
  generic punctuation, file paths, URLs, and line numbers are forbidden.

Prefer semantic IDs over implementation names. Renaming a function should not rename
`rejected-input-preserves-state` unless the proposition changed.

## 4. Validation rules

- Unknown keys are errors.
- Missing or malformed values are errors.
- `role`, `domain`, and `pure` may appear at most once per item.
- Duplicate identical repeatable tags are errors; they add no information.
- Any item with `invariant` or `law` must also have at least one `evidence`.
- An annotated item should have `role`; the checker warns rather than errors so module/domain-only
  migrations can be incremental.
- Source annotations not attached to a recognizable item produce a warning.

The validator checks syntax and local consistency. It does not prove purity, resolve Rust names,
run tests, or establish that evidence proves the claim.

## 5. Graph mapping

| Key | Edge | Target node namespace |
|---|---|---|
| `role` | `role` | `role:<value>` |
| `domain` | `domain` | `domain:<value>` |
| `pure` | `property` | `property:pure=<value>` |
| `invariant` | `preserves` | `invariant:<value>` |
| `law` | `satisfies` | `law:<value>` |
| `requires` | `requires` | `predicate:<value>` |
| `ensures` | `ensures` | `predicate:<value>` |
| `evidence` | `evidenced_by` | `symbol:<value>` |
| `read-first` | `read_first` | `symbol:<value>` |
| `related` | `related_to` | `symbol:<value>` |

Rustdoc JSON `links` add `rustdoc_link` edges to compiler IDs when present.

Source-derived item IDs contain the repository-relative source path, line, kind, and name. They
are navigation IDs, not stable public identities. Rustdoc-derived nodes retain compiler item IDs
for the generated artifact; those IDs may change between compiler/rules builds.

## 6. Versioning guidance

Treat the key set and graph edges as version 1 while the skill remains repository-local. Before
external consumers depend on it:

1. add an explicit schema version to index output;
2. freeze key and edge meanings;
3. add fixture compatibility tests;
4. define how renamed symbols and evidence paths are migrated;
5. generate rustdoc JSON with a pinned toolchain because its schema is not a stable Rust language
   interface.

Never change the meaning of an existing tag silently. Add a new tag/edge or version the index.
