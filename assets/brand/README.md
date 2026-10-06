# Shared Tabula identity: T Portal

The canonical vector sources and palette are byte-preserved from the approved
[issue #91 handoff](https://github.com/loveoverflowcom/tabula/issues/91),
commit [`ae40d8ef`](https://github.com/loveoverflowcom/tabula/tree/ae40d8efda28c141127116b3342d80843aba4185/docs/ui/tabula-logo-lavender).
The original handoff was based on develop59ec8c6; implementation starts on
merged develop7716b2e. `generated/exports.json` records exact source and output
SHA-256 hashes; `provenance.json` separately pins every original handoff input. The PNG256 handoff has real transparency; it is not the export
source for larger assets.

T Portal is an original normalized vector inspired by a shared playing table and
an open portal. There is no game-specific decoration or copied Wonderous asset.
The lower-case wordmark is the handed-over Inter Display ExtraBold outline;
it does not depend on a font installation or require a runtime font request.

## Ownership and roles

- The SVGs and `palette.json` here own geometry, proportions and design provenance
- `tokens.toml` owns semantic colors; run `cargo xtask gen-tokens` to update
  Rust/CSS/JSON/Kotlin. `brand-mark`, `brand-wordmark`, `brand-canvas`,
  `brand-panel`, `brand-ink`, `brand-on-ink` are separate from action primary
- Ordinary light mark uses #7C63ED and wordmark uses the more readable #5E4B8B;
  dark mark uses #B9A7F3 and wordmark #F7F4FF. HC adapts to monochrome
- Source SVGs remain valid fixed-color downloadable artwork. Generated shell
  SVG and CMP paths use semantic roles, not scattered application hex values
- Artwork for games and account avatars is independently owned and unchanged

The source mark uses a256×256 viewport. The lockup is705.276×256 with the
outlined wordmark positioned at(294,77). Keep that ratio and transparent
viewport padding; no independent fonts, stretching, mirroring, crop, shadow,
glow or added dots. Micro is the optical16–24px outline; larger sizes use the
normal path. No logo motion is applied, so there is no delay, loop or
reduced-motion exception to manage.

## Reproducible exports

Requires the repository’s pinned Rust toolchain, Python3 + Pillow and Inkscape1.4. These are development tools only,
not new application dependencies.

    python3 tools/export-brand.py
    python3 tools/export-brand.py --check
    python3 -m unittest discover -s tools/tests -p test_brand_identity.py -v

Every PNG is rasterized from SVG at its exact target size. The exporter
normalizes PNG metadata, produces opaque RGB iOS icons, and preserves real
alpha for transparent marks/adaptive Android foregrounds. `--check`
re-rasterizes and compares all outputs without modifying files. Rasterizer
changes can alter edge antialiasing; treat them as a reviewed export update.

Outputs include the shared semantic inline lockup, generated CMP paths,
web favicon16/24 micro plus32/48/64 normal exports, multi-size ICO,
Apple-touch icon180, manifest192/512, Miniquad decoded RGBA16/32/64,
five Android launcher/adaptive density sets, and iPhone/iPad/App Store icons.
The app-icon source is square and has no baked rounded corners: the OS owns
its mask. Android adaptive foreground uses a target-specific centered safe
inset for its108dp layer/66dp safe circle and a separate generated ink background.
The canonical shape and original square artwork remain unchanged. Native
setup masks keep the same shape/wordmark and use the existing verified Sprite
pipeline; the existing xtask BLAKE3 helper derives their manifest fragment.

`web/` is copied into the Trunk distribution's `brand/` directory.
`apps/game-client/web/brand` is a generated target mirror, not a second source.
The standalone HTML slot between `tabula-brand` markers is updated from the
same lockup by this exporter; surrounding game layouts are hand-authored.

## Existing surfaces and limits

Implemented: Leptos sidebar/topbar/mobile menu; standalone entry header and
game loader; web favicon/manifest; native Macroquad window icons; CMP Home
identity; Android launcher/adaptive identity; iOS AppIcon resource catalog.
Existing game toolbar titles remain game titles.

No independent app About/splash screen exists. `apps/desktop` is the documented
gated optional experiment that prints guidance and exits; no Tauri bundle or
launcher configuration exists to update. This does not create one.

Export pixels, source wiring and local compilation are distinct from installed
launcher/device evidence. Android/iOS execution is NOT_RUN unless reported in
the accompanying verification record; the HTML handoff preview is design
provenance, never evidence of application integration.
