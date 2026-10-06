# Current Tabula UI runtime review — 2026-10-06

16 original runtime PNGs selected from a fresh clean build of the actual Leptos shell and Macroquad Chess/Werewolf WASM. No image edits, design HTML, reconstructed screenshots, injected game state, or production source changes were used. Screenshots were hash/dimension-checked against the original CI manifests, then visually inspected with `view_image` after authorized artifact download.

## Exact source and execution

- UI source: [`59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6`](https://github.com/loveoverflowcom/tabula/commit/59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6), tree `fa4f55e81bc1bd860ccfb58cb1738a8e3aa2a176`
- Capture checkout: [`f88d8beba7e7db7e25d679a25f9d27bb48268dca`](https://github.com/loveoverflowcom/tabula/commit/f88d8beba7e7db7e25d679a25f9d27bb48268dca), tree `3b7f29a4a791f36e4fbd56ed9f023dce31cb37f1`
- [Compare](https://github.com/loveoverflowcom/tabula/compare/59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6...f88d8beba7e7db7e25d679a25f9d27bb48268dca): five **added** workflow/capture/receipt files only; all existing UI/game/assets byte-identical to the pinned develop source
- [Actions run 37428712006](https://github.com/loveoverflowcom/tabula/actions/runs/37428712006), artifact `11395749199`, capture window 07:21:43–07:23:23 UTC / 14:21:43–14:23:23 Vietnam, 6 October 2026
- Original ZIP: 16,561,060 B, SHA-256 `51150fd3ff1292c2fe059f1bb84ee8f3f73fbe9b756b1acabd5965eff3e069c1`
- Playwright 1.62.0, official Chromium 151.0.7922.34, disposable headless profiles, ANGLE/SwiftShader, DPR 1
- This snapshot uses source59's palette/logo. Unmerged PR90 and issue91 branding work are absent

The maintained Werewolf driver `games/werewolf/tests/verify-redesign.mjs` ran as a temporary copy. Changes were limited to official CI Chromium launch and **public pointer coordinates matching current `Layout::options`**. Old option-panel offsets otherwise hit the wrong button after the redesign. Existing real pointer/keyboard sequences and assertions were retained; no rules, renderers, canonical/private state or production code were changed. Original and executed-copy hashes are in `capture-provenance.json`.

## Cleanup and build

The requested bounded Cargo clean was performed first on seven owner-coordinated generated/inactive target directories. Source, unpushed edits, screenshots, independent bundles, registry/toolchains and receipts were retained. See `cargo-clean.json`. An optimizer started an old-baseline all-features job after an idle snapshot and collided with one cleanup; its output writes failed because generated `debug/deps` disappeared. This recoverable cache race is recorded separately from code failures. Fresh CI builds also began with Cargo clean.

Exact build commands and emitted asset hashes are in the unchanged original `capture-provenance.json`.

| Actual WASM | Bytes | SHA-256 |
|---|---:|---|
| Leptos shell | 947044 | `4ce9e169122426756f8446ff02bc202bf9f10d9b2a1f406caceca8699f419669` |
| Macroquad Chess | 926379 | `2972c09e3754875c621371d36a5fa7abb29a9e235d5b9b8e5c021a2c100031c9` |
| Macroquad Werewolf | 1001281 | `2a5d5c7fe1a55aeeaf774505c6fd56ff80c92f8b6c5a63a71e608e796113748a` |

## Findings and limits

- Fresh build/capture: PASS. Overall workflow: **FAIL** at the unchanged 900000 B shell gate (947044 B, 47044 B over). No budget waiver or continue-on-error
- Dashboard first Tab reaches visible skip link: PASS; Enter focusing main: **FAIL**
- Genuine 200% Chromium font preference: root 32px confirmed. English narrow Home still splits words (Account/Explore/discovery) and hero artwork crosses text. Empty DOM clipped-text/no-horizontal-overflow lists do not establish visual usability
- Werewolf selected real-input cases: **4 PASS / 0 FAIL**. Original pixels show upright living-seat Witch art/text, neutral public roster, real vote, Dawn, terminal draw and separate footer. Canvas/footer rectangles are 1200×824 + 56px and 390×788 + 56px
- Chess e2→e4: observed actual pawn e4/e2 empty, Black turn and public session log. Current presentation is still flat; in-game HUD is English despite VI host/setup. This run does not establish the unfinished #85 redesign acceptance
- Actual Library in this anonymous fixture shows Chess/Tiles only. Werewolf was launched via its separate real staged bundle

Fixtures are disposable: anonymous public shell with genuinely absent account adapter, local two-human Chess, and local Werewolf simulator seats “Người 1”–“Người 12”. A published role front belongs only to the intended living test seat; terminal role labels are game-authorized public. No private eliminated-player drawers are selected for this report. No real account data, credentials or canonical game dump is published.

Not run: physical mobile/touch; CMP/native GameHost; real login/accounts/avatar provider; Werewolf online/chat/voice; full fresh online Chess fault acceptance; full accessibility/frame-pacing acceptance. Narrow screenshots are desktop Chromium viewport tests.

## Receipts

- `selection-and-pixel-inspection.json`: supplementary per-selected-PNG original hash, Git blob hash, dimensions, theme/locale/viewport and visual finding
- `capture-provenance.json`, `capture-result.json`, `shell-budget.json`: **unchanged original CI receipts**, including their historical “inspection pending” wording. The separate selection receipt completes post-download inspection rather than rewriting them
- `werewolf-runs/`: unchanged maintained-driver results and action sequences. Werewolf records case starts and summary completion, not individual PNG timestamps
- `cargo-clean.json`: cleanup scope, retained source status and cache race

Old #82 remains the historical source for the prior online join-code Chess acceptance and earlier Werewolf captures. Superseding its screenshot report does not erase those receipts or resolve unrelated implementation/backlog issues.
