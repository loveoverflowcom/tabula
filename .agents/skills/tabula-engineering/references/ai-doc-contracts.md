# Rust AI doc contracts

Supporting technique for [Tabula engineering](../SKILL.md). The foundation owns the overall
workflow, check status, and evidence report; use this reference only for the named technique.

Use compiler-recognized Rust docs as a thin semantic index. Keep ordinary rustdoc useful to humans;
append structured tags only where they reduce future discovery or verification cost.

Read the nearest `AGENTS.md` and architecture contract before annotating. Metadata must describe
actual behavior and evidence, never desired behavior.

## Annotation procedure

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

The [tag schema](ai-doc-schema.md) owns supported keys, cardinalities, identifiers, validation,
and graph edges. Read it before defining annotations or integrating another parser. Use one
value per line and repeat multi-valued tags; do not extend the schema ad hoc in Rust source.

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
python3 .agents/skills/tabula-engineering/scripts/ai_doc_contracts.py --help
```

Validate changed Rust paths:

```bash
python3 .agents/skills/tabula-engineering/scripts/ai_doc_contracts.py check crates/example/src
```

Emit a source-derived graph:

```bash
python3 .agents/skills/tabula-engineering/scripts/ai_doc_contracts.py index crates/example/src
```

Consume compiler-produced rustdoc JSON:

```bash
python3 .agents/skills/tabula-engineering/scripts/ai_doc_contracts.py index-rustdoc path/to/crate.json
```

Use source indexing for fast local feedback. Use rustdoc JSON when exact compiler item IDs,
resolved doc links, spans, visibility, and attributes matter. Source indexing is deliberately
conservative and does not claim full Rust parsing.

## Avoid annotation debt

Do not annotate private mechanical helpers, getters, obvious constructors, test fixtures, adapters
with no semantic rule, or generated code. Do not duplicate information already unambiguously
encoded in a strong type unless the tag creates a useful graph edge to evidence or ownership.

When code changes, update/remove stale tags in the same change. A false machine-readable contract
is worse than no tag because future agents will use it to skip context.

## Technique evidence details

Add annotated symbols, checker result, and index mode (if any) to the foundation report. Name
claims whose evidence could not be resolved or run. The checker verifies tag syntax and local
consistency; it does not resolve evidence paths, run their tests, or prove their claims.

