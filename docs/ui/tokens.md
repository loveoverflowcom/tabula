# Semantic token contract

`tokens.toml` is the authored source for Tabula's design system. `cargo xtask
gen-tokens` parses it into a typed source, validates it, resolves the compact
renderer-neutral `Theme`, and emits the committed Rust, CSS, and JSON adapters.
The transformation is deterministic; it reads no system theme, clock, or
renderer state.

## Authored-token audit

| Authored family | Parsed and validated | Runtime `Theme` | CSS | JSON |
|---|---|---|---|---|
| Reference palette | Typed design metadata | No; guidance only | No | Yes |
| Resolved schemes and semantic color | Typed `SchemeSource` | `color` | Yes | Yes |
| Typography and font-stack identity | Typed role/size metrics | `type_` | Yes | Yes |
| Spacing | Named typed scale | `space` | Yes | Yes |
| Reference and semantic shape | Typed non-negative radii | `shape` | Yes | Yes |
| State layers | Typed fractions | `state` | Yes | Yes |
| Base motion, springs, profiles, reduced-motion policy | Typed and finite/positive | `motion` | Yes | Yes |
| Density, focus, elevation | Typed and bounded | `density`, `focus`, `elevation` | Yes | Yes |
| Component tier | Deliberately open additive metadata | No, until a reusable component needs it | No | Yes |

Resolved scheme colors are explicitly authored. `[ref.palette]` records source
colors and design provenance; it does not claim an automatic tonal derivation.

Typography uses `TextStyleToken`'s closed semantic names such as `BodyMd` and
`MonoMd`; renderers resolve those names through `Theme::text_style`. `Mono*`
always requests tabular figures. The runtime has no strings, font files,
browser objects, or GPU handles.

`Density::min_target` is a logical accessibility unit (dp-like), not a physical
pixel. Elevation levels are abstract: CSS may map them to shadows while canvas
renderers may use authored assets or another suitable visual treatment.

## Future game accents and components

A future game manifest may provide a source accent and a mood. A build-time
resolver may derive compatible game-scoped accent tokens from those inputs. It
must not permit games to override platform semantic roles such as danger,
legal-target, focus, or accessibility-critical on-colors. This boundary does
not implement game assets or a game-theme pipeline.

Component tokens remain empty by design. Add one only when a reusable component
has a written reason to deviate from a system semantic role.

Team and seat-marker colors are semantic identity aids, not a claim that color
alone is colorblind-safe. Presenters must pair them with a label, glyph, or
position; contrast tests cover the roles that are actually placed on surfaces.

## Verification ledger

| Claim | Evidence |
|---|---|
| Malformed sources fail before generation | `tokens_cmd::tests::malformed_sources_fail_at_the_typed_boundary` |
| All token families reach adapters | `tokens_cmd::tests::all_major_token_families_reach_the_intended_artifacts` |
| Generation is deterministic | `tokens_cmd::tests::generation_is_idempotent`; `cargo xtask gen-tokens` + freshness check |
| Rust/CSS names correspond | `tokens_cmd::tests::rust_and_css_use_the_same_semantic_motion_name` |
| Runtime value bounds are preserved | `tabula_design::tests::bounded_token_values_reject_invalid_boundaries` |
| Accessibility pairs and HC strength hold | named design-crate contrast tests |
| Presentation uses closed semantic typography | design/presentation crate compilation and `mono_styles_require_tabular_figures` |
