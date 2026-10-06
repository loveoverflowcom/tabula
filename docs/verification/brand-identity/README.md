# T Portal integration evidence

Current integration and merge evidence: [2026-10-06 completion](completion-20261006/README.md).
The original draft record below is retained as historical evidence.

Source baseline: remote develop7716b2ef91c8c2e79e49db6879de19a5742ee76a,
after PR93 normal merge. Approved asset source:
ae40d8efda28c141127116b3342d80843aba4185, issue #91. This record concerns the
new draft logo PR only; it does not reinterpret the handoff HTML as runtime.
PR94's later baseline5e66202cc6b7bb64ec96dea15d8f10b58f53744f is integrated
without changing its skip-link correction.

## Claims and local evidence

- PASS: all canonical SVGs/palette byte-preserved; vectors contain real paths,
  no embedded raster, font-dependent text, gradient, glow or filter
- PASS: source PNG256 materialized and visually inspected on a solid backing;
  original indexed transparency verified, recognizable open T Portal
- PASS: Inkscape1.4 exports rasterized from SVG at each exact native dimension;
  alpha marks/micro16/24 and opaque square launcher outputs checked
- PASS: six additive semantic brand roles, all24 scheme mappings, existing
  primary/on-primary/selected and all other pre-existing authored tokens unchanged
- PASS: normal cargo xtask gen-tokens, repeat-generation equality,15 token
  generator tests and12 design tests; token-scoped Clippy with warnings denied
- PASS:17 existing staging tests after adding required brand files/directory
- PASS: six source/consumer/provenance/pixel/launcher assertions; ICO16 matches
  micro16 pixels, portal stays open at16/24/32/48/64
- PASS: actual Trunk online/wasm-release build, source-derived identity files in
  dist/brand; shell WASM739,440B, below unchanged900,000B cap
- PASS: native game-client all-target/all-feature compilation before final
  setup-header integration; final check status will be recorded below

## Existing consumers

Shared Leptos Brand() supplies sidebar/topbar/mobile menu with one accessible
Tabula link name and decorative SVG; standalone headers and game loading
screens use the same generated lockup and semantic colors. Brand exports
supply favicon/manifest, CMP Home, Android launcher/adaptive icons, iOS AppIcon
catalog and native Macroquad window icons. Native setup header uses the existing managed Sprite pipeline. Its combined
resource declaration preserves the original gameplay group and warm textures
across setup/gameplay/restart cycles. No logo motion is applied.

## Honest limits

- Local browser rendering is NOT_RUN: this environment's browser socket boundary
  is unavailable. No local browser bypass is attempted. Dedicated authorized
  screenshot QA is separate, source-pinned work; append actual receipts only
- Mobile build/UI test execution BLOCKED before configuration: pinned Gradle9.7.0
  cannot download from services.gradle.org in this environment; JDK17 and Android
  SDK absent. Existing OpenJDK21 does not establish a compatible Android build
- Xcode/iOS/installed launcher device execution NOT_RUN on this Linux environment
- apps/desktop remains the gated optional no-op experiment; About/app-splash
  screens do not exist and were not created for a logo
- No game art/palettes, avatars, rules, gameplay layout or phase gates changed
- PR90 remains unmerged; action palette changes are not pulled into this PR
- No GitHub CI checks, CI waiting, auto-merge or deployment

## Final exact-source checks

- PASS: first portable cargo xtask check, all gates completed locally
- PASS:80 native tests and native/helper Clippy after warm-cache refinement
- PASS: native resource tests prove zero re-decodes/releases across warm cycles
  at densities1/2/3/1 and no mask/cover load during gameplay-only preparation
- PASS: source review identified drawer SVG overflow at320px/200% root font;
  drawer-only max-width/auto-height/canonical aspect ratio correction applied
- PASS: integrated source cargo xtask check after preserving PR94, all portable gates
- PASS: integrated Trunk online/wasm-release build and17 dashboard helper tests
- Pending: remote source commit/branch and draft PR verification
- Separate authorized runtime screenshot QA remains pending and is not a
  prerequisite for publishing this honest draft
