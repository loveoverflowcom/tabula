# Dashboard resource, keyboard and large-text correction

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/dashboard-design01); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


Fresh baseline: `59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6`, tree
`fa4f55e81bc1bd860ccfb58cb1738a8e3aa2a176`. This preserves the newly merged
public-display/avatar and Werewolf/renderer work. Earlier `6637c66` captures are
historical regression evidence, not current-develop acceptance.

## Changed claims

| Claim | Owner / failure | Evidence | Status |
|---|---|---|---|
| Shell remains within unchanged 900,000-byte raw-WASM cap | discovery's full-game vtable retained unused canonical factories | actual official Trunk online wasm-release output and existing shell_budget | PASS, fresh local output; exact-final receipt below |
| Existing full game APIs/config/handoffs are preserved | generic erasure could change constructors, coercions or validation | descriptor/query/normalization/API-compatibility differentials, feature/native/WASM consumers | PASS, bounded local selection recorded below |
| Skip activation uses native fragment focus | router intercepted #main and only scrolled | explicit target=_self bypass on existing tabindex=-1 main; unchanged genuine Enter assertion | Source-reviewed; post-fix browser NOT_RUN |
| 200% text gets usable width without shrinking fonts | narrow percentage hero/nav and side-by-side Continue forced arbitrary word splits | font-relative <=16rem full-width hero, yielded art, stacked Continue, intact-label oracle and original-copy assertion | Source-reviewed; post-fix browser NOT_RUN |

The read-only facade statically forwards every fact/configuration method to the
unchanged existing adapter. It adds no match authority, factory stub, rule/auth
semantics, asset exclusion or backend change. Legacy catalog aliases, wildcard
imports and full-signature launch helpers remain source-compatible.

Normal 320/390px composition and semantic colors stay unchanged. English soft
hyphens provide optional correct syllable breaks; stripping them yields the exact
authored sentence. Body/main-title root-relative type is not reduced to make it fit.

## Merge and evidence boundary

The owner explicitly requests local verification and ordinary self-merge for this
corrective PR without waiting/checking GitHub CI. Workflows, budgets and server
protections remain unchanged. Exact-final local results will be recorded below;
automatic CI is not claimed as passed. Genuine after screenshots are separately
requested post-merge and must pin the actual merged source/build hashes.

Local browser navigation remains denied; no alternate port, shell Chromium,
tunnel, ignored TLS error or fabricated screenshot is used. Native/CMP/device,
assistive-technology and actual online gameplay acceptance are not established by
these bounded source/build/unit checks.

## Coherent local checkpoint

- Official Rust 1.96.1, Trunk 0.21.14, wasm-bindgen 0.2.129 and Binaryen 123;
  `TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online`,
  `RUSTFLAGS=-D warnings`, jobs 2, incremental off
- Actual emitted shell WASM: 739,144 bytes; unchanged 900,000-byte cap PASS with
  160,856 bytes of headroom. Five shell resources; zero gameplay references
- Focused registry: 42 unit + 6 recovery + 11 runtime tests PASS, including the
  three new inventory/config/handoff/API-compatibility differentials
- Online web: 78 tests PASS, including the authored-word/soft-hyphen assertion
- Registry doctests: five compile-fail tests PASS; three pre-existing examples ignored
- Browser-oracle helpers: 16 tests PASS; Python compilation PASS. These are pure
  sensitivity/example tests, not execution of the browser itself
- Independent fresh-source review: no open API/security/source defect. Current
  public-display/avatar, full erased factories/runtime, game and theme code preserved

## Completed local verification

The production/test code tree is `35ec467fd66678d75f2faf2a815d87c68e2529d3`
(remote `ada5d982d03cad04e30ba14780d092d28024c90f`). The final documentation
checkpoint repeats the portable gate; its result is reported in the PR.

- `cargo xtask check`: PASS, every ordered gate including strict all-feature
  Clippy, workspace tests, dependency/game-id/manifest/token/color checks and
  cargo-deny. 1,260 tests passed, zero failed; 18 existing documentation examples
  were ignored. No lint or budget suppression added
- `cargo check --workspace --no-default-features` and `--all-features`: PASS
- Registry zero-game: one compatibility unit + five compile-fail doctests PASS
- Registry Chess-only: four units + six recovery + eleven runtime tests + five
  compile-fail doctests PASS; Tiles-only: four units + five doctests PASS. Both
  feature selections execute the new descriptor/config/handoff differentials
- Zero-game's unchanged private parse helpers produce inherited dead-code
  warnings. The new test import warning was corrected; strict warning-clean
  zero-game is not claimed. Three legacy illustrative doctests are ignored in
  each registry selection; zero selected integration targets are not called tests
- WASM compilation: web default/online; protocol/registry/match all-features;
  game-client web and web+online PASS. These are compilation, not WASM execution
- Separate native match-postgres and online-match acceptance manifests compile
  successfully, including online-match all-features/continuity/body-publication;
  this does not execute PostgreSQL, faults or browser gameplay
- Final emitted shell: 739,144 bytes, SHA-256
  `3cf77b4726c22c1befaa42f15b1bf5697ce2b7edfb003611bed2a40811cd3462`.
  Existing shell_budget PASS with five resources, 884,192 raw bytes total and
  zero gameplay references. This inventory is not a request waterfall/CDN result
- Final CSS contains the 16rem large-text and manual-hyphenation rules, SHA-256
  `9301f00b9738ee99804a1b25b3ca330279b0d239afcd6634932dcc238d737ec3`
- Independent final source/API/security and test-refactor review: PASS within
  this scope; no lost cases or changed validation/handoff behavior

[Historical Exact local emitted loading receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/dashboard-design01/corrective-loading-receipt.json) records
shipping source hashes, tool/profile settings and every selected artifact's byte
count/hash. The unchanged budget owner, protocol, canonical full factories/runtime,
current public-display/avatar, gameplay, tokens/theme and CI workflows remain
byte-identical to the freshly fetched baseline.

No automatic GitHub CI status is inspected or claimed here. No real post-fix
keyboard/font/pixel PASS is inferred from compilation. The extended genuine
eight-partition font and unchanged first-Tab/Enter scenarios remain implemented
for the separately requested post-merge capture.
