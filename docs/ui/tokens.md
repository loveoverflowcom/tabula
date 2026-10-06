# Semantic token contract

`tokens.toml` is the authored source for Tabula's design system (ADR-027). `cargo xtask
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
| State layers | Typed exact whole percentages | `state` | Yes | Yes |
| Base motion, springs, profiles, reduced-motion policy | Typed and finite/positive | `motion` | Yes | Yes |
| Density, focus, elevation | Typed and bounded | `density`, `focus`, `elevation` | Yes | Yes |
| Component tier | Deliberately open additive metadata | No, until a reusable component needs it | No | Yes |

Resolved scheme colors are explicitly authored. `[ref.palette]` records source
colors and design provenance; it does not claim an automatic tonal derivation.

### Shared primary violet

The owner's button reference uses a filled-control violet around `#745FA1`
(`ref.palette.primary-source`). The resolved light primary `#573B83` deepens
that hue; the dark primary `#B093DD` replaces the pale lavender while retaining
readable primary text and focus on dark surfaces. HC variants are `#32164F`
and `#DFCCF9`. `selected` follows each scheme's primary and focus resolves to
the same role. Shell surfaces, tonal actions, feedback/seat colors, game art,
and state-layer/disabled opacities keep their existing contracts.

Filled controls already consume `primary` / `on-primary` in shared web SCSS,
`tabula-presentation::button`, and CMP `ShellButton`; changing individual
button styles would bypass this shared contract. Functional primary accents
and selection/focus marks also inherit the palette. Decorative game art does
not derive from it.

For a build-free color-only check, run `python3 tools/tests/check-primary-theme.py`.
It compares all scheme colors with the committed Rust/CSS/Kotlin and JSON
adapters and checks primary label/state/surface contrast. It does not replace
the typed generator, full token freshness, Rust tests, or the portable core gate.

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
| Generation stays collision-free under parallel workers | OS-unique temporary rustfmt files in `format_rust`; concurrent token-generation tests (CI nextest gate) |
| Exact motion mapping reaches every adapter | `tokens_cmd::tests::motion_medium_has_an_exact_cross_artifact_oracle` (`system.motion.medium`, scoped Rust, exact CSS var) |
| Exact typography mapping reaches every adapter | `tokens_cmd::tests::body_medium_weight_has_an_exact_cross_artifact_oracle` (`system.type.body.md.weight`, scoped Rust, exact CSS var) |
| Fractions are representable without rounding | `tokens_cmd::tests::fractions_require_exact_whole_percentages`; `exact_fractions_have_identical_cross_artifact_semantics` |
| Reference palette values are validated | `tokens_cmd::tests::malformed_sources_fail_at_the_typed_boundary` (`ref.palette.primary-source`) |
| Runtime value bounds are preserved, including finite proof | `tabula_design::tests::bounded_token_values_reject_invalid_boundaries`; `generated_measurements_reject_non_finite_values` |
| Accessibility pairs and HC strength hold | named design-crate contrast tests |
| Foundation text, supporting text, actions, errors, success, control boundaries, and focus work on three tonal surfaces in all four schemes | `foundation_tonal_surface_pairs_meet_their_thresholds` |
| Filled/tonal action labels remain readable under hover/focus/press layers in all four schemes | `foundation_action_state_layers_preserve_text_contrast` |
| Presentation uses closed semantic typography | design/presentation crate compilation and `mono_styles_require_tabular_figures` |

## Phase status

This contract does not certify a phase exit. Current Chess/Tiles presentations
consume the generated themes; the [foundation specification](screens/foundation.md)
records the compact component mapping. DOM shell/settings implementation still
requires the Phase-4 exit and opened Phase-5 gate (doc 07).
