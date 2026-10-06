# Dashboard resource, keyboard and large-text correction

Fresh baseline: `59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6`, tree
`fa4f55e81bc1bd860ccfb58cb1738a8e3aa2a176`. This preserves the newly merged
public-display/avatar and Werewolf/renderer work. Earlier `6637c66` captures are
historical regression evidence, not current-develop acceptance.

## Changed claims

| Claim | Owner / failure | Evidence | Status |
|---|---|---|---|
| Shell remains within unchanged 900,000-byte raw-WASM cap | discovery's full-game vtable retained unused canonical factories | actual official Trunk online wasm-release output and existing shell_budget | PASS, fresh local output; exact-final receipt below |
| Existing full game APIs/config/handoffs are preserved | generic erasure could change constructors, coercions or validation | descriptor/query/normalization/API-compatibility differentials, feature/native/WASM consumers | Focused PASS; aggregate/feature completion pending |
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

The portable `cargo xtask check` and remaining feature/target checks are running
for exact-final local verification. No automatic GitHub CI status is inspected or
claimed here. No real post-fix keyboard/font/pixel PASS is inferred from compilation.
