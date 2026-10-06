# Actual UI after PR93 — source 7716b2ef, 2026-10-06

16 selected original runtime PNGs: 8 Dashboard/Library/font-reflow, 5 local Werewolf, 3 local Chess. These were freshly built and executed after PR93's ordinary merge. The preceding source59 report remains untouched for history. This folder is artifact storage, not a claim that the evidence branch's production source is the capture checkout.

- UI source: [7716b2ef91c8c2e79e49db6879de19a5742ee76a](https://github.com/loveoverflowcom/tabula/commit/7716b2ef91c8c2e79e49db6879de19a5742ee76a), tree `1f851b5fc3be2916db2408d52554777b5ebdc489`
- Capture checkout: [1d5d0666d261b098efe7ab221938fc7591e06526](https://github.com/loveoverflowcom/tabula/commit/1d5d0666d261b098efe7ab221938fc7591e06526), tree `3b474c8c9f2d5172fb3927ce8745aa851c5652ca`
- [Source comparison](https://github.com/loveoverflowcom/tabula/compare/7716b2ef91c8c2e79e49db6879de19a5742ee76a...1d5d0666d261b098efe7ab221938fc7591e06526): only four added capture/workflow files, no production source/assets changed
- [Screenshot-verification run 37444732095](https://github.com/loveoverflowcom/tabula/actions/runs/37444732095), artifact `11402344998`; 09:49:32–09:51:09 UTC / 16:49:32–16:51:09 Vietnam
- ZIP 18,243,592 B; SHA-256 `6113dbe97e871610fdd2155709f3c82e43a5352723d06091bea96935ef08e4da`, matches the GitHub artifact digest
- Official Chromium 151.0.7922.34 / Playwright 1.62.0, headless ANGLE/SwiftShader, DPR 1

## Actual results

- Fresh build/capture and the unchanged shell budget: **PASS**, actual CI WASM **737400 / 900000 B**. This differs from the separate locally emitted 739144 B; the CI artifact is independently measured, not copied from a local result
- First Tab reaches the skip link and Enter focuses main: **PASS**
- Maintained genuine 320/390px, VI/EN, 100/200% font-preference driver: **8 partitions PASS**, zero recorded failures. Root font is independently probed at 16/32px. CTA/navigation words remain intact, narrow hero copy is full-width with artwork hidden, Continue is stacked
- **Visual review remains PARTIAL:** original `after-small-mobile-en-default-font-200percent.png` shows a purple skip-link tail “task” at the top before intentional keyboard focus. The unchanged `apps/web/style/app.scss:38–51` fixed negative offset is a candidate cause when the label wraps taller; the actual unfocused link rectangle/focus state was not recorded. No cross-platform claim. The passing driver deliberately excludes skip-link bounds, so it did not detect this
- Werewolf selected real-input cases: **4 PASS / 0 FAIL**. Selected pixels show public neutral roster, own living Witch card, actual keyboard vote and public terminal draw. Only the intended living seat's role front is selected; terminal role labels are game-authorized public
- Chess e2→e4 is visible at the actual pawn position, Black turn and public session log. Flat board presentation and English in-game HUD remain; #85 redesign acceptance is not established

The report reuses merged `tools/dashboard-acceptance/run.py::text_scale`; only the screenshot sink is adapted to attach original-PNG metadata. No layout CSS, mocked HTTP, private/canonical game state or rendering replacement is injected. The Werewolf maintained-driver temporary copy retains the scoped official-browser/public Options-coordinate adaptation documented in original provenance. Public missing-adapter shell and disposable local game seats only.

Original full-page rasters retain fixed navigation painted at its initial viewport position. No cropping, retouching, stitching, flipping or screenshot edits were used. Named pixel inspection is in the separate selection receipt; original CI receipts retain their original “inspection pending” wording.

Not run: physical mobile/touch, CMP/native GameHost, real login/account/avatar provider, online Werewolf/chat/voice, fresh full online-Chess fault acceptance, full accessibility/frame pacing. Source90 theme and issue91 unified logo are absent from this pinned source.

## Receipts

`selection-and-pixel-inspection.json` contains each selected original SHA-256/Git blob hash/byte size/dimensions, viewport/theme/locale/time scope and findings. `capture-provenance.json`, `capture-result.json`, `shell-budget.json`, `font-scaling.json`, `werewolf-driver-output.log` and `werewolf-runs/` are unchanged originals. Font profiles' independent 16/32px probes and all eight Home/Library/drawer observations are preserved in `font-scaling.json`.
