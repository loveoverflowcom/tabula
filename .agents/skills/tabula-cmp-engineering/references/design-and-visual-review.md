# CMP design and visual review

Use for styling, responsive geometry, localization, semantics and Web parity.
Read [doc 04 §7–§10](../../../../docs/architecture/04-frontend-and-design-system.md),
the [compact foundation](../../../../docs/ui/screens/foundation.md) and the
affected shell/account contract. They own Tabula's identity and geometry.

## Trace the mounted consumer

`tokens.toml` is authored; `cargo xtask gen-tokens` generates Rust, CSS, JSON and
Kotlin adapters (ADR-027/0032). Trace semantic token → generator →
`design/TabulaTokens.kt` → `TabulaTheme`/component → production caller before
changing a role. Edit the authored source or generator when appropriate, then
regenerate all adapters. Never hand-edit generated Kotlin, add a second palette,
or assume an outer Material theme restyles a custom control. Shared token/brand
changes require the affected Web and Rust consumers' checks/evidence too.

Use the T Portal artwork and generated brand adapter at their existing owner;
see [brand assets](../../../../assets/brand/README.md). Transfer hierarchy,
readable type, quiet surfaces and state feedback from the relevant Web surface;
do not copy VOT's palette or shrink desktop geometry into a phone. Screenshot
pixels are not the authored color values or a px→dp specification. Trace copy
through the exhaustive vi/en keys in `localization/` and supplied catalog
translations; an identifier or test tag is not user-facing copy.

## Select representative states

For affected layout, start with 320/390 dp phones and 100%/200% text, vi/en and the
relevant light/dark/high-contrast schemes. Add short landscape, rail/tablet,
reduced motion, long names, overlays or loading/error/empty states according to
the changed risk, avoiding an unbounded cross-product. Use real shared
production composables with explicit fixture boundaries.

Preserve 44 dp minimum targets, readable text, wrapping actions, scroll reach,
focus visibility, contrast and non-color state cues. Safe insets belong to the
outer shell once. The current shell uses compact 16 dp page/card insets, labeled
phone bottom navigation and a rail from 600 dp; check the screen contract for
the actual control geometry. Do not lower text/hit sizes to resolve overflow.

Give interactive nodes accessible role/name/state and put a stable `shell-*`,
`discovery-*` or `account-*` tag once on the action owner. A test tag identifies
a control for inspection; it is not its accessible label. Review merged/child
semantics and popup roots before adding redundant metadata. Semantics must not
expose facts that the current user/identity is not permitted to see.

## Inspect pixels and close the finding

Open the current CMP capture and an attributable reference when claiming parity.
Record reference/build/state, viewport, density, font scale, theme, locale and
what to transfer. Equivalent states support parity; an unrelated signed-out
image may inform identity but does not prove a form matches.

Use the [MCP loop](preview-inspection.md) or existing preview test captures:
capture before → implement at owner → test → capture/open after → record concrete
findings → fix if needed → recapture/open the same scenario. Keep Web reference,
CMP before and CMP after for parity work. Preserve scenario, source/build and
fixture identity across rechecks; if a fixture was wrong, disclose the correction.
Never approve or regenerate a baseline just to hide a failure, and do not
pixel-diff unrelated renderers as a quality oracle.

Inspect text clipping, scrolling, hierarchy, wrapping, target/focus geometry,
state feedback and unavailable explanations. Record image paths actually opened
and findings, not just a capture count. Static pixels and host callbacks prove
no real IME, TalkBack/VoiceOver, safe-area behavior, device touch, animation
smoothness or native lifecycle; select separate native evidence for those claims.
