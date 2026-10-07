# Approved original Werewolf artwork

Source supplied in the approved design review on 2026-10-04. The six portrait
panels are original built-in image generation, not third-party publisher pixels.
[`design-provenance.json`](design-provenance.json) retains the exact creation prompt, panel ordering, source
hash references and commercially referenced *visual conventions*. The retained
WebP has SHA-256 `83455e8181f4e211feb9d13772e841cce2d3e169c7a7ed763e9bbdd222fae757`.
The original PNG was not in the materialized handoff; the WebP is the exact
approved supplied source. No unavailable or unpublished Mac implementation was
read or copied; Rust gameplay presentation is recreated in this checkout.

The generator crops individual 512-square panels in row-major order, downsamples
1x with Lanczos, and applies deterministic 256-color median-cut quantization.
The shared opaque moon/diamond back is re-authored code-native artwork matching
the supplied generic composition and carries no role-dependent pixels. Front
card borders, faction, Vietnamese title/rules and all interactive labels are
live RenderList commands. No publisher logos, fonts, photographs or card fronts
are imported, and no licensed use of publisher artwork is claimed.

Public availability of role portraits conveys no match assignment. Gameplay
projections and RenderLists disclose only authorized choices and facts; the
all-role source atlas never enters runtime resources or projections. These
artwork exports do not change the repository software license or claim a new
third-party license grant.

## Village scenes supplied for #84

The owner supplied the two `Downloads/tabula-redesign 3` and
`Downloads/tabula-redesign 2` directories for this redesign. The scene source
pixels are copied byte-for-byte from the former's `werewolf/assets/` on
2026-10-06. Its `ASSETS.md` declares original generated village artwork; these
are design inputs, not third-party game screenshots. The originals are retained
here under runtime-independent authoring filenames:

| Supplied source | Retained source | Dimensions | Bytes | SHA-256 |
|---|---|---|---:|---|
| `tabula-redesign 3/werewolf/assets/village.png` | `village-night.png` | 1672×941 | 2,753,512 | `9bd72067406f625900a88548bab2721e48ec90d7c4dcc68d096a31988df6212d` |
| `tabula-redesign 3/werewolf/assets/village-dawn-original.png` | `village-dawn.png` | 1672×941 | 3,051,337 | `d49fb3223bb7cc99bfb7d71ad6d821d1f87b0a2d7f0251ccf1942a0e51da5733` |

`generate.py` converts each original to RGB, downsamples to 640×360 and
1280×720 with Lanczos, applies 256-color median-cut/Floyd–Steinberg quantization,
and emits PNGs at compression level 9. `budgets.json` pins their exported hashes,
dimensions and bytes; pack 0.2.0 pins BLAKE3 resource identity. The original scene
PNGs and role atlas are authoring inputs and are not served, embedded in WASM or
loaded into gameplay. The eighteen independent exports are the runtime images.

Neither reference's sample account avatars or labels are promoted to real
account/profile facts. The current simulator uses neutral host-provided fallback
geometry. Sources, byte budgets and mockup images do not establish runtime
performance, account integration or multiplayer acceptance.
