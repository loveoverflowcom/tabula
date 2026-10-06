# Design 01 dashboard restoration — issue #87

## Source and oracle

- Issue: [#87](https://github.com/loveoverflowcom/tabula/issues/87)
- Runtime baseline: `ab0983e92135bc42399066aa891c1e5a27e7a37e`
- Original Design 01: `fe6a6bac1037ea28355bd1f1192acdca6f2ce123`, byte-preserved
  `docs/ui/dashboard-original-design-01/design-01/standalone.html#library`
- Original standalone: 230108 bytes,
  SHA-256 `6c6bfce11fd85b5e15bd6b00c8e3b0e1c1bfff6967493dad2cd78f88ee0edb9a`
- Actual after revision and built-file hashes: the CI artifact's `provenance.json`

The supplied 1440×1408 desktop and 390×844 mobile PNG pixels were inspected for
identity, serif heading, 224px rail, context topbar, split lavender hero, white
continue strip, artwork hierarchy and fixed mobile navigation. This is reference
inspection, not execution of the restored application. Prototype sample names,
clocks, profile, favorites/counts and AI are fixtures, not runtime product facts.

## Evidence ledger

| Claim | Owner / failure mode | Oracle and selected domain | Status | Residual scope |
|---|---|---|---|---|
| Layout/artwork hierarchy restores Design 01 | shell/Home/card DOM and semantic styles | actual same-viewport before/after/prototype PNGs, DPR1; 1440/full, 1100×850, 390×844, 320×640 | NOT_RUN | CI artifact must be captured and pixels inspected before visual verdict |
| Compact reflow and reachable navigation | shell CSS/dialog and catalog controls | 768×900, 844×390; geometry, 44px targets, last-card bottom slot | NOT_RUN | actual Chromium acceptance pending |
| Keyboard drawer dismissal is safe | native modal dialog | full Tab/Shift+Tab cycle, Escape, restored toggle focus, repeat open and route close | NOT_RUN | actual Chromium acceptance pending |
| Locale/system settings retain legibility | locale/media preference adapter and semantic schemes | vi/en × four schemes, reduced motion, computed solid contrast pairs | NOT_RUN | gradient/artwork contrast is pixel inspection; token tests remain separate |
| Type scale respects user settings | semantic rem typography and responsive layout | isolated Chromium default-font 16→32, measured doubling and Home/Library reflow | NOT_RUN | real browser preference execution pending; no CSS-injected substitute |
| Continue and catalog states are truthful | current resume adapter boundary and registry | unavailable continue, empty unmatched filter, invalid query recovery; fixture absence | NOT_RUN | ready/empty/error session adapter is absent; asynchronous catalog loading/server-error is NOT_APPLICABLE |
| Deep links/search/Back/setup preserved | router/query and registry descriptors | real search/filter→detail→setup→Back/Forward; online-control presence | NOT_RUN | actual create/join/play owned by existing online-match job on same revision |
| Landing remains lightweight | separate-document boundary ADR-011 | actual request paths + encoded bytes; no gameplay WASM/atlas/model requests | NOT_RUN | no production/CDN/cache performance claim |
| Acceptance assertions detect plausible violations | independent Python helpers | contrast, both target dimensions, missing nav slot, overflow/clipped text, empty selection, resource classifier, byte hashes and implemented route handler | PASS, example-tested | nine local tests, no browser execution inferred |

## Exact local helper checks

```sh
python3 -m unittest discover -s tools/dashboard-acceptance -p 'test_*.py' -v
python3 -m py_compile tools/dashboard-acceptance/run.py
```

Both passed while preparing this harness; the helper suite selected and executed
9 tests. They prove the scoped assertion helpers, not runtime Leptos UI.

## Actual execution and screenshots

[Workflow source](../../../.github/workflows/dashboard-design01.yml) builds the
before and after applications and renders the original prototype using the same
pinned official Playwright Chromium as the existing actual-browser harness.
Local loopback browser navigation is blocked in the available cloud environment,
so no alternate port, shell Chromium, tunnel or ignored TLS error is used here.
Actual GitHub CI execution is the legitimate verification route.

The job emits `dashboard-design01-actual-browser-evidence` with:

- `provenance.json`: actual checked-out source identities and full build hashes
- `comparison.json`: each identical-viewport source capture and measured geometry
- `acceptance.json`: nonempty closed claim selection, PASS/FAIL and residual limits
- PNGs prefixed `before-`, `after-`, `prototype-`, plus settings/state/drawer/route captures

After the first actual run, record its URL, exact revision, claim outcomes and
named pixel-inspection findings here. Until those artifacts are inspected, all
runtime rows remain NOT_RUN. Compilation/native/headless assertions are never
promoted to real Web pixel proof. Repository `just check`, normal feature/WASM CI
and the inherited actual create/join Chess verdict are reported independently.
