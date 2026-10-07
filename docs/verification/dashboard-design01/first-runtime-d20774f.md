# First authentic Design 01 browser evidence

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/dashboard-design01).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


## Verdict and exact source

This is an intermediate, **failed** acceptance run. It does not mark the PR ready.
The genuine Chromium/Leptos run captured 77 PNGs and executed nine selected claims:
seven passed, the new shell raw-WASM budget failed, and the intended 200% browser
font preference did not activate. The latter leaves actual 200% behavior unproved.

- [Run 37418266286](https://github.com/loveoverflowcom/tabula/actions/runs/37418266286)
- [Public evidence artifact 11392074958](https://github.com/loveoverflowcom/tabula/actions/runs/37418266286/artifacts/11392074958), 6839158 bytes, expires 20 October 2026
- Downloaded ZIP SHA-256 verified: `e2e3d6ea3a4d4329514b9c6765b15b56e0b1225ca1a31917e01f34d3f1b64116`
- PR head: `d20774f90caaf27bcd333a28e1010bf20563da03`
- Actual Actions checkout: `ea684aa19dde35deb6bc6abfc14c225ea1686ba5`, GitHub's verified test merge of that exact head into the unchanged baseline
- Before runtime: `ab0983e92135bc42399066aa891c1e5a27e7a37e`
- Original byte-preserved Design 01: `fe6a6bac1037ea28355bd1f1192acdca6f2ce123`; all 16 archived byte/hash entries verified
- Browser actions/captures: 6 October 2026, 05:33:34–05:34:23 UTC

The artifact's `before-*` and `after-*` images are actual compiled Leptos runtimes.
`prototype-*` is the archived standalone prototype's combined `#library` screen.
It is not a product route or runtime. Home and Catalog before/after share the same
normal-size Chromium process, CSS viewport, DPR1 and Vietnamese selection as their
paired prototype captures. Counts, names, clocks and profile fixtures remain only
in the original prototype.

## Executed checks

| Claim | Result on this run | Evidence / limits |
|---|---|---|
| Before emitted shell uses the established raw-WASM limit | PASS | Existing `shell_budget`, 893533 bytes, limit 900000 |
| After emitted shell uses the established raw-WASM limit | FAIL, introduced | 989770 bytes: +96237 versus before, 89770 above the unchanged limit |
| Same-viewport Home/Catalog/prototype and responsive geometry | PASS | All six selected viewports, including 1440/full, 1100×850, 390×844, 320×640, 768×900 and 844×390; no measured horizontal overflow or text glyph clipping |
| Four schemes, vi/en, reduced motion and genuine result states | PASS | Eight preference/locale partitions; own filtered-empty announcement/recovery and both invalid query axes checked |
| Native drawer keyboard and route dismissal | PASS | Complete Tab/Shift+Tab cycles; Escape/toggle focus restoration; repeated open; nav, Brand mouse/Enter, Browser Back/Forward dismiss the modal |
| Search→detail→setup→Back/Forward and lazy landing | PASS | Genuine router/registry path; online create/join controls present; actual match authority/play remains separate |
| Multiword/native-select focus and URL contract | PASS | In-progress trailing space and search focus retained; native ArrowDown retains select focus; latest constraints combine; reset and valid absent-inventory values work |
| Fixed mobile navigation's last-card slot | PASS | 320px last card fully reachable above the nav; 96px main slot, measured 72px nav |
| Genuine 200% default-font preference | NOT ESTABLISHED | Receipt reports FAIL because both configured scales measured body 14px/title 34px; these are normal-size images, not 200% proof |

The initial default Playwright headless launch used the separate headless-shell
embedder. The preference keys are Chromium's documented browser preferences, but
this run did not establish their effect. The next harness selects full Chromium's
new headless channel and independently probes the blank-document root font at
16/32px before judging application reflow. Unsupported setup will be BLOCKED.
This is an evidence prerequisite correction, not a diagnosis that application rem
styles reject user scaling. [Official Playwright browser guidance](https://playwright.dev/python/docs/browsers#chromium-new-headless-mode).

## Named pixel inspection

Inspected actual runtime and same-viewport prototype PNGs include:

- `after-desktop-1440x1000-full.png`, `prototype-desktop-1440x1000-full.png`: restored Tabula mark, quiet 224px rail/80px topbar, paper canvas, serif hierarchy, lavender split hero and token artwork; both real Chess/Tiles landscape covers are populated, with compact metadata
- `after-review-1100x850.png`: desktop rail/context and three-column grid remain coherent; hero/continue copy is legible and does not clip
- `after-mobile-390x844.png`, `after-small-mobile-320x640.png`: clear mobile brand/drawer control, serif hero and CTA, fixed three-destination navigation; catalog/continue below the first fold are scrollable rather than squeezed into it
- `after-tablet-768x900.png`, `after-low-landscape-844x390.png`: compact navigation and reflow, no horizontal clipping; a short landscape viewport naturally requires vertical scrolling
- `after-catalog-desktop-1440x1000-full.png`, `after-catalog-mobile-390x844.png`: distinct focused Catalog route, native labeled search/filter controls, truthful unavailable continue, accurate two-game count and artwork cards
- `after-dark-vi-1100x850.png`, `after-hc-light-en-1100x850.png`, `after-hc-dark-vi-1100x850.png`: serif hierarchy and actionable states remain legible; HC boundaries survive flattened colors; measured solid-pair minimum contrast is 7.98 light, 8.55 dark, 15.24 HC-light, 12.90 HC-dark
- `after-mobile-drawer-keyboard.png`: actual purple focus ring around Close, modal scrim and visible navigation/unavailable reasons
- `after-small-mobile-catalog-bottom-slot.png`: last Tiles card, metadata and its detail action end visibly above fixed navigation

Intentional differences from the prototype are the supported two-game registry,
neutral avatar/account area, real metadata and supported Home/Catalog routes; no
sample opponent, clock, favorite, profile or Tutor/AI is advertised as runtime
fact. The desktop hero's first line currently includes “một” whereas the original
forces its break after the comma. The 95px desktop/390px continue region and
135px 320px variant are larger than the fixture strip because they explain the
true unavailable state; this is not a pixel-identical restoration claim.

No skip-link visual acceptance is included yet: the new desktop rail covers the
old low-z-index skip link until the subsequent fix/check. The PNG filenames ending
`200percent` in this first artifact are explicitly excluded from scaled-text
visual evidence, because their measured font sizes never changed.

## Actual landing bytes

The fresh browser requested only `/`, shell CSS, generated token CSS, shell JS and
one shell WASM. Encoded transferred bytes were **1134871**; no gameplay document,
second/game WASM, atlas, role pack or model request occurred. This actual waterfall
is separate from static emitted-file inventory and gzip calculations; it does not
claim production/CDN timing or cache behavior.

After WASM SHA-256: `c279b5141d3437902906432648a7c1b1bf980e3f9b0a6d37e7777da42712cdcb`
Before WASM SHA-256: `df414f2e15b45c0dead235b7db26a35efa10231f16f07502cf4490fd311cb8ab`

## Remaining acceptance

Reduce the actual shell to its existing byte limit without weakening the cap,
establish genuine 200% fonts in the real browser, verify the corrected desktop
skip link, and rerun/inspect the final code tree. Genuine create/join/play Chess is
owned by the existing `online-match` workflow on the final PR revision. This
record proves neither provider login nor gameplay, authenticated avatar, native/
CMP execution, actual assistive technology or wider phase completion.
