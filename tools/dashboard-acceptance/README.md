# Actual Design 01 dashboard acceptance

This is the bounded, authorized GitHub Actions route for issue #87. It is not a
local browser fallback around a blocked loopback origin. `run.py` refuses to run
unless GitHub Actions and the explicit disposable acceptance opt-in are present.
Local helper tests do not launch a browser or listener.

The workflow builds real Trunk/Leptos applications from:

- before: `ab0983e92135bc42399066aa891c1e5a27e7a37e` (the issue implementation base)
- after: the exact checked-out PR verification revision, recorded in provenance
- prototype: unmodified Design 01 at `fe6a6bac1037ea28355bd1f1192acdca6f2ce123`

It uses the repository's existing static shell route handler. Missing APIs remain
missing; it does not simulate an account, resume list, catalog response, game
runtime, network or server authority. No browser route interception, injected
layout CSS, ignored TLS error, tunnel or external listener is used. The dashboard
is public static HTTP on CI loopback; it never logs in or establishes authority.
Profiles containing browser font preferences are temporary and are not artifacts.

## Claims and oracles

- Same-process official Playwright Chromium captures before, after and original
  prototype at identical CSS viewport dimensions, DPR1 and Vietnamese locale
- Required views: 1440×1000 and full page, 1100×850, 390×844 and 320×640
- Additional responsive partitions: 768×900 and 844×390 low landscape
- Actual DOM geometry checks overflow, the 224px desktop rail/80px topbar, hero,
  compact truthful continue state, nonempty artwork and three-column grid
- Mobile controls must have both hit dimensions at least 44 CSS px. A fixed
  bottom nav needs actual main padding, and the last catalog card must be fully
  reachable above it at scroll end
- Four genuine browser media-preference combinations select light/dark/hc-light/
  hc-dark. Both vi/en run with reduced motion. Solid computed semantic text pairs
  use independent WCAG luminance; gradients and artwork require pixel inspection
- The native dialog is opened by keyboard. Tab/Shift+Tab are exercised across a
  complete cycle, then Escape, toggle-focus restoration, reopening and navigation
- Real search/query, detail, registry setup, Back and Forward run through actual
  routes. Online create/join controls must remain present in the opted-in build
- Actual landing request paths and encoded transferred bytes are recorded. Only
  the shell WASM may load; gameplay documents/WASM, atlases and model files fail
- Genuine Chromium default font preference 16 → 32 checks actual 200% type
  scaling and reflow in Home and Library. It does not insert CSS to fake scaling
- Reachable filtered-empty and invalid-query states are announced. Continue is
  unavailable because this slice has no resume-list adapter. The synchronous
  embedded registry does not pretend to expose asynchronous loading/server-error
  states

Create/join/play Chess is owned by the separate existing `online-match` workflow
on the same PR revision. This harness does not duplicate that workflow's backend
or network-fault matrix, and a dashboard pass cannot substitute for its verdict.

## Reproduce helper validation

```sh
python3 -m pip install -r tests/online-match/requirements.txt
python3 -m unittest discover -s tools/dashboard-acceptance -p 'test_*.py' -v
python3 -m py_compile tools/dashboard-acceptance/run.py
```

Actual browser execution uses `.github/workflows/dashboard-design01.yml` on the
PR. The downloadable `dashboard-design01-actual-browser-evidence` artifact
contains public PNGs, per-view geometry, exact build/reference file hashes,
request-byte receipt and closed claim results. A failed layout still preserves
all same-viewport comparison captures. Inspect actual PNG pixels before citing
`screenshot-inspected`; successful capture alone is `screenshot-captured`.

No merge enforcement, production deployment, authenticated avatar, native/CMP
execution or broader phase completion is implied.
