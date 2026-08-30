---
name: rust-ai-doc-contracts
description: Add sparse, machine-readable `@ai.*` contract metadata to Rust `///` and `//!` docs, validate the mini-schema, and emit source- or rustdoc-JSON graphs linking APIs to roles, purity, invariants, laws, related symbols, and evidence tests. Use when documenting high-value Rust modules, public domain transitions, trust boundaries, refined types, reducers, projections, canonicalization/replay code, or when building an AI/code-property-graph index. Do not annotate every helper or replace ordinary rustdoc, types, tests, or formal proofs.
---

# Rust AI doc contracts

Use compiler-recognized Rust docs as a thin semantic index. Keep ordinary rustdoc useful to humans;
append structured tags only where they reduce future discovery or verification cost.

Read the nearest `AGENTS.md` and architecture contract before annotating. Metadata must describe
actual behavior and evidence, never desired behavior.

## Workflow

1. Select only high-leverage items: module ownership points, public transitions, trust/projection
   boundaries, proof-bearing constructors, canonical encoders, replay logic, and critical ports.
2. Read the implementation and cited evidence. Do not infer purity or a law from a function name.
3. Write normal human rustdoc first: purpose, semantic inputs/output, errors, and safety/security
   boundary where relevant.
4. Append canonical `@ai.*` lines using the schema below. Use one tag/value per line.
5. Link ordinary prose with Rust intra-doc links such as ``[`Document`]``. Keep tag values stable,
   machine-friendly IDs or Rust paths.
6. Run the bundled checker over the narrow changed path.
7. When building an index, emit JSON from source immediately or consume rustdoc JSON for compiler
   item IDs, spans, visibility, docs, attributes, and links.
8. Review the diff for annotation noise and remove tags that do not alter what a future agent
   should read, assume, or verify.

## Canonical mini-schema

```rust
/// Reconciles node identity from `old` into `new` without mutating either document.
///
/// @ai.role domain-transition
/// @ai.domain document.reconcile
/// @ai.pure true
/// @ai.invariant node-id-uniqueness
/// @ai.law preserves-unrelated-nodes
/// @ai.evidence tests::reconcile_properties
/// @ai.read-first tests::reconcile_properties
/// @ai.related crate::Document
/// @ai.related crate::NodeMapping
pub fn reconcile_nodes(old: &Document, new: &Document) -> NodeMapping {
    // ...
}
```

Use exactly these keys:

| Tag | Cardinality | Value |
|---|---:|---|
| `@ai.role` | one | kebab-case semantic role |
| `@ai.domain` | zero/one | dotted lowercase domain ID |
| `@ai.pure` | zero/one | `true` or `false` |
| `@ai.invariant` | repeatable | kebab-case state predicate ID |
| `@ai.law` | repeatable | kebab-case behavioral law ID |
| `@ai.requires` | repeatable | kebab-case precondition/evidence ID |
| `@ai.ensures` | repeatable | kebab-case postcondition ID |
| `@ai.evidence` | repeatable | Rust path to a test/spec/proof item |
| `@ai.read-first` | repeatable | Rust path worth reading before edits |
| `@ai.related` | repeatable | related Rust symbol path |

Canonical form always includes a value; write `@ai.pure true`, not a flag shorthand. Repeat tags
instead of comma-separated lists. Do not put prose, Markdown links, JSON, line numbers, or multiple
values in a tag.

Read `references/schema.md` when defining new annotations, integrating another parser, or deciding
the graph edge semantics. Do not extend the schema ad hoc in source code.

## `///` versus `//!`

- Use `///` on the item that owns the contract.
- Use `//!` at a module/crate root for domain ownership and boundary-level laws.
- Do not copy the same contract onto a module and every contained function.
- Put evidence on the narrowest item whose behavior the test actually establishes.

Module example:

```rust
//! Deterministic rules for applying one ordered input.
//!
//! @ai.role functional-core
//! @ai.domain game.rules
//! @ai.pure true
//! @ai.invariant rejected-input-preserves-state
//! @ai.evidence tests::rejected_input_is_transactional
```

## Purity and proof honesty

`@ai.pure true` means output and observable state change depend only on explicit inputs under the
documented deterministic context. Interior mutation, caching, logging, clock reads, global state,
unordered observable iteration, or I/O can invalidate the claim.

`@ai.invariant` and `@ai.law` are claims, not proof. The checker requires at least one
`@ai.evidence` on the same item when either appears. Evidence may point to a unit/property/model
test or formal artifact; its strength comes from what ran, not from the tag.

Do not annotate an unchecked constructor as preserving an invariant. Use `@ai.requires` to name
its caller obligation and cite the producer evidence.

## Run the tools as black boxes

First inspect usage:

```bash
python3 .agents/skills/rust-ai-doc-contracts/scripts/ai_doc_contracts.py --help
```

Validate changed Rust paths:

```bash
python3 .agents/skills/rust-ai-doc-contracts/scripts/ai_doc_contracts.py check crates/example/src
```

Emit a source-derived graph:

```bash
python3 .agents/skills/rust-ai-doc-contracts/scripts/ai_doc_contracts.py index crates/example/src
```

Consume compiler-produced rustdoc JSON:

```bash
python3 .agents/skills/rust-ai-doc-contracts/scripts/ai_doc_contracts.py index-rustdoc path/to/crate.json
```

Use source indexing for fast local feedback. Use rustdoc JSON when exact compiler item IDs,
resolved doc links, spans, visibility, and attributes matter. Source indexing is deliberately
conservative and does not claim full Rust parsing.

## Graph meaning

The index maps tags to typed edges:

```text
item ─role────────► role
item ─domain──────► domain
item ─property────► pure=true|false
item ─preserves───► invariant
item ─satisfies───► law
item ─requires────► precondition
item ─ensures─────► postcondition
item ─evidenced_by► test/spec/proof
item ─read_first──► symbol
item ─related_to──► symbol
item ─rustdoc_link► compiler-resolved symbol
```

## Avoid annotation debt

Do not annotate private mechanical helpers, getters, obvious constructors, test fixtures, adapters
with no semantic rule, or generated code. Do not duplicate information already unambiguously
encoded in a strong type unless the tag creates a useful graph edge to evidence or ownership.

When code changes, update/remove stale tags in the same change. A false machine-readable contract
is worse than no tag because future agents will use it to skip context.

## Completion report

Report annotated symbols, checker result, index mode used (if any), and any claim whose evidence
could not be resolved or run.

