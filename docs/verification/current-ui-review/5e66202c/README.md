# PR94 narrow actual skip-link regression — 2026-10-06

Three untouched actual Leptos screenshots verify 320×640, EN/light, DPR1, genuine Chromium 32px default font (200%). UI source [5e66202c](https://github.com/loveoverflowcom/tabula/commit/5e66202cc6b7bb64ec96dea15d8f10b58f53744f), tree32705eb93a814297df5505d12b50bbc6b74ee97e; capture checkout56b5c276bac6082195ac7dbcc25dab4b7d65c7b8/tree05de8975121b72b79660a60d3218e5f4b9ae2cff adds only two harness files. Artifact storage branch is not the production build checkout.

[Run37450749806](https://github.com/loveoverflowcom/tabula/actions/runs/37450749806), artifact11405199351, 10:44:05–10:44:07 UTC /17:44 Vietnam. ZIP SHA-2567dbb2f53747b25a0a2dcdae50a0769e135fb0be27cce56a73804faff3886a94a matches GitHub digest. Official Chromium151.0.7922.34/Playwright1.62.0, public anonymous shell, no real account data/credentials/page or CSS mocks.

PASS: unfocused link rect x8/y-112/w312/h104/bottom-8; first real Tab rect x8/y8/w312/h104, fully visible; Enter transfers native focus to main and reconceals link. Original pixels independently inspected confirm no tail, a full focused label and concealed link after main focus. Fresh shell WASM737400 B, SHA-2560a1e52ac980307b574afcff0381b6d990ba845cb89170ccd356e9a3cb8e48314, identical to the preceding CI artifact; no source/budget waiver.

First run37448882018 falsely failed its focused-rectangle check because the adapter sampled immediately after Tab, before the screenshot frame synchronized style. Its original PNG was already correct. The revised harness measures after screenshot paint, as the maintained desktop helper does; no production CSS/DOM changes were introduced. The original failed receipt is retained, not relabeled PASS.

This is a narrow screenshot review, not full UI/a11y/gameplay/network/physical-mobile/native acceptance or a merge gate. No unrelated CI was inspected. Current logo/theme follow-ups are absent from this pinned source. Original hashes/dimensions and explicit scope are in pixel-inspection.json; original CI provenance is unchanged.
