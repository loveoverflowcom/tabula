# Approved original Werewolf artwork

Source supplied in the approved design review on 2026-10-04. The six portrait
panels are original built-in image generation, not third-party publisher pixels.
[`design-provenance.json`](../../../../docs/ui/werewolf-approved/design-provenance.json) retains the exact creation prompt, panel ordering, source
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
