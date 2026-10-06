# Actual T Portal logo runtime evidence — PR95 candidate

The 12 PNGs are unchanged original screenshots from authorized, source-separated Chromium execution in GitHub Actions run [37460100042](https://github.com/loveoverflowcom/tabula/actions/runs/37460100042).

- Before source: `5e66202cc6b7bb64ec96dea15d8f10b58f53744f`, tree `32705eb93a814297df5505d12b50bbc6b74ee97e`
- Candidate source: `291fc9d067999aa44f5425f7ab7a64d9378a55ff`, tree `246b5e27843728bf198da60a5395b7e5a488a66b`; unmerged Draft [PR95](https://github.com/loveoverflowcom/tabula/pull/95) at publication
- Capture-only harness: `ce07c4f7558a0e3081505817e299e3ebd9067754`
- Browser: Chromium 151.0.7922.34, Playwright 1.62.0, DPR1
- Before capture: 2026-10-06 12:03:26–12:03:47 UTC; candidate: 12:04:36–12:05:03 UTC
- Viewports: 1440×1000, 390×844, 1200×880; special 320×640 English drawer with independently established 32px default font (200%)
- Four actual themes: light, dark, hc-light, hc-dark. Public anonymous shell and disposable untimed same-device Chess only
- Candidate actual emitted shell WASM: 739221 B, SHA256 `65c1bff3fd161ee762982049fb722c771986d29e97ffa61a2240ee7ea346cb95`; measured below the unchanged 900000 B cap
- Candidate actual Chess WASM: 944010 B, SHA256 `2c69e82806060e63ef7637702a3c4b5a4625a0abc805200de5868cc862d0e4fb`

The final run preserves 42 original PNGs in its two Actions artifacts. This folder selects two before and ten candidate frames for durable review. Original provenance manifests contain every image hash, size, dimension, source build hash, theme, locale, action and bounded assertion. Candidate 41 and before 20 assertions pass; these assert theme, SVG presence, actual WASM completion and candidate narrow-logo containment, not general UI correctness.

## Inspection and limitations

Every selected original was inspected for visible logo, clipping of the logo, blank/error rendering and visible private data. The 320px English 200% logo fits inside the drawer; its long navigation labels remain visually tight, so no broader drawer typography acceptance is claimed.

The genuine loader was observed by temporarily throttling the real compiled WASM download to 160 KiB/s and 100ms latency in a disposable browser, then restoring normal transfer and verifying actual canvas readiness. No response or UI state was replaced. Loader DOM measurements precede screenshot paint: the light loader has pre-stylesheet geometry/fills in its unchanged original provenance and cannot support a geometry/color claim for the PNG. The inspected screenshot is properly styled.

An earlier capture photographed some loaders before bootstrap applied the requested theme. Its actual theme metadata is retained separately in `earlier-capture-timing.json`; those frames are not used as final theme evidence. Only a read-only capture wait was corrected, then the exact unchanged source pair was rebuilt.

Native OS window icons/Dock/titlebar, physical mobile devices, CMP embedding, touch, real accounts, online services, full gameplay and frame pacing remain **NOT_RUN**. This is bounded screenshot QA, not a merge gate; unrelated CI was not inspected.
