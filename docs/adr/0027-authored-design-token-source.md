# ADR-0027: Authored design-token source and generated adapters

- **Status:** accepted
- **Date:** 2026-08-31
- **Supersedes:** ADR-018 (representation only; the semantic-authority intent remains)
- **Invariants touched:** I-1, I-10

## Context

ADR-018 correctly requires one semantic design language across DOM and canvas, but
its wording says that tokens are defined in Rust. The implementation has an authored
`tokens.toml` contract and a generated `tabula-design` Rust runtime, with CSS and JSON
as generated adapters. Leaving the old wording unchanged makes the source-of-truth
boundary ambiguous and invites hand-edited generated artifacts.

## Decision

`tokens.toml` is the one authored semantic design-token contract. `cargo xtask
gen-tokens` parses and validates it, resolves the renderer-neutral `tabula-design`
runtime model, and emits the committed Rust, CSS, and JSON adapters. Generated files
are never edited by hand; CI checks freshness and the no-raw-colors rule. Rust remains
the typed runtime API consumed by presentation and renderers, not the authored source.

This supersedes ADR-018 only on the representation of the source. Its locked intent —
one semantic authority shared by DOM and canvas — is unchanged.

## Consequences

The authored contract can be reviewed and diffed independently of generated output,
while consumers retain compile-time Rust types and platform-neutral semantic names.
Generation must remain deterministic and complete: every committed adapter must be
reproducible from `tokens.toml`.

## Revisit when

Adopt a different authored format only if it can preserve the same typed validation,
deterministic multi-adapter generation, and reviewable semantic contract; record that
change in a new superseding ADR.
