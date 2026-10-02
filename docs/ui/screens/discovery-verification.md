# Discovery/setup verification ledger

Stage A of [issue #50](https://github.com/loveoverflowcom/tabula/issues/50).
The [shared contract](discovery.md) pins source and reference versions; the
[availability inventory](discovery-availability.md) links inspected owners.
Evidence vocabulary follows [Tabula engineering](../../../.agents/skills/tabula-engineering/SKILL.md).
Documentation was the deliverable of Stage A. A specified layout or check is not
an executed runtime result.

**Stage B has since landed**, ahead of the Phase 4 and Phase 5 gates, as a
recorded owner decision: [ADR-0028](../../adr/0028-discovery-shell-ahead-of-phase-gate.md).
Its evidence is in [Stage B evidence](#stage-b-evidence) below; the
[Stage A evidence](#stage-a-evidence) and acceptance table are kept as the
record of what the specification alone established.

**Stage C** closes the one acceptance line Stage B left measured at two widths
and one scheme: reflow, zoom, pointer targets, focus and the four generated
schemes. It is in [Stage C evidence](#stage-c-evidence), with the two defects it
found and the three that stay open under a named owner.

## Stage A evidence

| Claim / invariant | Owner and failure mode | Oracle / scope | Check and status | Residual scope |
|---|---|---|---|---|
| Current-base inventory is reproducible | Source audit; stale baseline could advertise unimplemented options | Local `HEAD` and live `develop`, metadata/capability/config/launcher sources | `git rev-parse HEAD`; `gh api repos/loveoverflowcom/tabula/branches/develop` — PASS, both `cdff554de327cf8d23893c7b606fc09670371b74` before edits; source-read | Facts are scoped to this base, not a later deployment/build |
| No phase gate is crossed | Registry/shell; mock could become a fake service | Doc 07 Phase 3/4/5, banners and executable bodies | Source-read PASS; change scope is Markdown specifications only | Runtime Stage B BLOCKED by phase exits and adapter dependencies |
| Configuration authority and data owners are explicit (I-9) | Module → registry → shell; guessed options/game branches | Actual accessors, game config, immutable validator and named missing adapters | Source-read/document review PASS; declared, linked and launchable facts distinguished | Forms, normalized adapter values, transport and service policy NOT_IMPLEMENTED |
| State and UI preferences stay separate (I-5/I-6/I-10) | Forms/resume; state caches or preferences could cross trust boundary | Doc 00 and doc 04 projection/presentation contracts | Document review PASS; summaries/resume require public adapter or projection data only | Real wire, projection and persistence paths require runtime checks |
| M3 Expressive tasks consume the foundation | Screen specs; mock palette/radii could become another theme | Foundation, generated role names, pack component map and screen source | Documented/source-read; tonal sections, contained lists, connected selectors, action emphasis and functional outlines specified | No new preview, token change, pixel render or measured usability result |
| All states have an action/result | Screens 01–03; unsupported/pending operation could appear successful | Routes, state machine, failure matrix and keyboard sections | Document review PASS, including disabled new creation versus eligible resume and room seat plan versus full roster | Response races, deduplication, navigation and authorization NOT_IMPLEMENTED |
| Documentation is navigable | Screen/index links; missing targets/headings | Repository-local Markdown links/anchors and whitespace in changed documents | `python3 -` stdlib target/heading/whitespace check — PASS, 93 local links across all eight changed Markdown files; `git diff --cached --check` — PASS | No permanent repository screen validator exists; external reference links are pinned provenance |
| Existing workspace remains green | Repository; regressions or stale generated outputs | Authoritative ordered local gate | `just check` — PASS, all gates including cargo-deny; first attempt could not lock its read-only advisory database, permitted rerun passed | Existing workspace checks do not test these unimplemented shell flows |

The pinned reference pack's mobile PNG was visually inspected as static artwork.
Setup SVG text/source and screen/component notes were inspected for intended
hierarchy. No new artifact was rendered and no browser/native UI, assistive
technology, network or interactive game session was exercised for this change.
No feature-matrix or target-specific build was run separately: this change has
no code, dependency, feature or token-schema effect. Core-gate test execution
does not establish the runtime claims below.

## Acceptance traceability

| Issue requirement | Specification / evidence | Current status |
|---|---|---|
| Tonal hierarchy, purposeful shape/groups, strong primary and labeled secondary actions | Foundation and component tables in [01](01-library.md), [02](02-game-detail.md), [03](03-new-match.md) | Documented; runtime visuals NOT_IMPLEMENTED |
| Decorative border/shadow removed; focus/selection/error retained | Foundation treatment and per-screen component/state sections | Documented; four-scheme focus and contrast measured in [Stage C](#stage-c-evidence) |
| Route → typed data owner → action → result for 01–03 | [Routes and typed mapping](discovery.md#routes-aliases-and-navigation), shared state machine and failure matrix | Documented/source-read; adapters and running routes NOT_IMPLEMENTED |
| Empty/loading/filter-no-result/unavailable/rejected UI | Library/detail tables, setup feedback, [shared matrix](discovery.md#shared-empty-loading-and-failure-matrix) | Documented; execution NOT_IMPLEMENTED |
| No unsupported rating/engine-ready/accessibility/offline badge | [Inventory](discovery-availability.md), typed mapping and claim restrictions | Source-read; future badges require scoped evidence |
| Normalized config/time/seat/mode summary and supported actions | [Setup form and summary](03-new-match.md#form-and-normalized-summary), immutable-validator dependency | Documented; form/normalization/creation adapters NOT_IMPLEMENTED |
| Local flow avoids inappropriate auth gating | Route/action contract and mode readiness; existing local driver | Source-read/documented; shell auth-return flow NOT_IMPLEMENTED |
| 320/390/768/1440, 200%, targets ≥44 dp, useful keyboard/focus | Per-screen breakpoint and keyboard sections; foundation | Measured in [Stage C](#stage-c-evidence): reflow, zoom, targets and focus PASS; keyboard activation and AT BLOCKED |

## Stage B evidence

Implemented at `feat/issue-50-discovery-shell`, on top of `develop` merge
`a4ec52c`. Scope: a runtime catalog in `tabula-registry` and the `/`, `/games`,
`/games/:id` (+ `?setup=1`) routes in `apps/web`. The gate crossing, and the
part of Phase 4/5 that is deliberately still missing, are in ADR-0028.

| Claim / invariant | Owner and failure mode | Oracle / scope | Check and status | Residual scope |
|---|---|---|---|---|
| No platform or shell crate names a game (I-9) | Registry is the only game-naming zone; a shell branch on a game id, or a shell-held game message key, would be the defect | `xtask check-no-game-ids` zone policy over the whole workspace | `cargo xtask check-no-game-ids` — PASS (331 files, 4 game ids). Game copy reaches the shell only as data from `Catalog::messages` | A future locale file placed in `apps/web` would reintroduce the risk; it must stay in the adapters |
| Dependency direction and I-15 hold | `deps.toml` matrix over the resolved graph | `cargo xtask check-deps` | PASS, 25 crates. `leptos` is in `apps/web` only; `apps/game-client` still forbids it | `deps.toml` is unchanged by this slice |
| A game's own validator decides every configuration | `Adapter<S>`; a form that accepted what the rules reject, or clamped silently, is the defect | The games' own `validate_config`, clock validator and deadline floor | `cargo nextest run -p tabula-registry` — PASS, 24 tests: zero initial clock rejected through the module; 4,999 ms deadline rejected and mapped to the seconds field; 0/1/3/6-seat plans; `5.5`, `-1`, `five`, `5 min`, empty and `601` rejected without clamping | Only the two linked games are covered; a third adapter is unproven |
| Increment and delay never summarize identically | Summary rendering; flattening both into one label is the named failure | Distinct `SummaryValue::TimeControl` variants per control | `example-tested` PASS (`chess_untimed_and_timed_configurations_normalize_distinctly`) plus `screenshot-inspected` on the running shell | Other games' summary wording is unwritten |
| Readiness never outlives its draft | `SetupState`; a start carrying a configuration the player changed is the defect | Revision-scoped validation results | `cargo nextest run -p tabula-web` — PASS, 17 tests: late result for a retired revision discarded, edit retires readiness, pending refuses a second submission and every field edit, rejection preserves input | Asynchronous validation is not exercised: the registry answers in-process |
| An unreadable filter is reported, never silently reinterpreted | `query::parse`; a wrong but plausible result set is the defect | Every axis, with invalid, zero and empty values | `example-tested` PASS (6 tests), including `players=0` as an error rather than a removal | — |
| Every key the catalog hands the shell exists in en and vi | Message tables; an untranslated key shown as product copy | Keys collected from metadata, forms, choices, modes, reasons, bot levels and rejections | `example-tested` PASS (`every_key_the_catalog_hands_the_shell_exists_in_both_locales`) | The shell's own table is not covered by that test; a missing key renders as a labeled fallback, not as a bare key |
| No raw colour outside `tabula-design` | `app.scss`; a literal would break theming | `xtask check-no-raw-colors` over `.rs`/`.css`/`.scss` | PASS | — |
| The shell builds and runs in a browser | Trunk/wasm build and the rendered routes | `trunk build` then `trunk serve` at `http://localhost:8080` | `compiled` PASS (`wasm32-unknown-unknown`, 3.6 MB dev wasm) and `interaction-tested` PASS: home, library, detail, setup; clock choice revealing its fields; validation to Ready; a rejected deadline; the locale switch changing platform *and* game copy | A dev-profile bundle; no production size, Lighthouse, or performance measurement |
| A start that cannot happen says so | `launch::resolve`; a claimed match that does not exist is the defect | Unbound runtime binding | `example-tested` and `interaction-tested` PASS: Start moves to Unavailable with reason, recovery, and the exact refused summary retained | The bound path builds a URL and navigates; **no gameplay document has been handed off end to end** |
| The workspace stays green | Repository | The authoritative ordered local gate | `just check` — PASS (fmt, clippy `-D warnings`, 832 tests, check-deps, check-no-game-ids, check-manifests, token freshness, check-no-raw-colors, cargo-deny), plus `cargo check --workspace --no-default-features`, `--all-features`, and `cargo check -p tabula-web --target wasm32-unknown-unknown` | — |
| The supply-chain policy still describes what we accept | `deny.toml`; a blanket allowance would make the gate meaningless | `cargo deny check` over the Leptos tree | FAIL on first run, four findings, all from Leptos: `BSL-1.0` (`xxhash-rust`), banned `getrandom` (via `uuid` in the `leptos_macro` proc macro), and the unmaintained `paste` / `proc-macro-error2` build-time macros. Each is now a **scoped, documented exception** with a removal condition; re-run PASS | This is a real widening of the third-party policy, accepted with the slice. `deps.toml` still forbids `leptos` outside `apps/web` and `apps/admin`, so none of it reaches rules, runtime or server crates |
| `check-no-raw-colors` reads source, not build output | xtask; a developer who ran `trunk build` failed on trunk's content-hashed copy of the generated `tokens.css` | The scanner's own walk | Reproduced (FAIL), fixed by skipping `target`, `dist`, `node_modules`, `.git` — the set `check-no-game-ids` already skips — then PASS | Verified by running the command before and after; no unit test covers the walk, which has none for any directory rule |

### What Stage B does not establish

- **No match is ever created.** There is no match runtime, no authority, and no
  server. `TABULA_PLAY_BASE_URL` is unset in every build here, so the Start
  action's only reachable outcome is the unavailable reason above.
- **No resume, room, queue, auth, rating, ranked, async, voice or asset
  delivery** exists. Each is rendered as unavailable with a reason.
- **Accessibility is not verified.** Native labels, `aria-describedby`,
  `aria-invalid`, live regions, focus order and the 3 dp focus ring are
  implemented and were read in the accessibility tree; no screen reader, no
  keyboard-only completion run, and no contrast measurement was performed.
  *Stage C measured focus order, the ring and contrast; the screen reader and
  keyboard-only completion are still unexecuted.*
- **Responsive behavior was checked at two widths** (1024 and 375 logical px)
  in one browser. 320, 390, 768, 1440 and 200% zoom were not each measured, and
  no touch-target measurement was taken. *Stage C measured all of these, and
  fixed the target defect it found.*
- **Only the light and dark schemes were seen.** `hc-light`/`hc-dark` are
  reachable through `prefers-contrast` but were not rendered. *Stage C rendered
  all four, and fixed the shell reading the preference only once.*
- **Golden images, performance budgets and bundle size** are unmeasured.

## Stage B conditions recorded before implementation

Re-pin `develop` and prove registry Phase-4 and shell Phase-5 gates before
implementation. Select one actually playable module through erased interfaces;
the shell must not link a game crate or branch on its ID. Settle the named form,
mode, availability, resource and creation/resume dependencies first. Keep native
shell implementation at its owning phase. Use real typed en/vi data and adapters
without fake success. Runtime acceptance must include:

- Direct Library/detail/setup entry, retained detail mode, filter/search/scroll
  restoration on Back, and removed mode without automatic replacement.
- Exact seat sets with gaps, invalid/duplicate/missing roster seats, unsupported
  bot factories/modes, and field parse/unit/overflow boundaries. Use the selected
  module's config contract: e.g. Chess untimed/Fischer/Bronstein, zero initial
  and addition overflow; Tiles 0/4,999/5,000 ms and creation-time overflow.
- Draft edits and late validation/fetch responses; single submission; rejection
  retains input; lost creation response reconciles the same request; leaving the
  screen does not cause late unsolicited navigation.
- Disabled new creation with eligible existing-match resume, audience exclusion
  without detail disclosure, stale/offline metadata, unsupported game/rules/
  protocol versions, and pinned continuation identity.
- Required pack binding/integrity/load failure versus optional-art fallback;
  measured progress; no unsupported cached-offline or accessibility badge.
- Unauthenticated browse/local startup; required network auth returns to the
  same draft; room/queue seat-plan checks precede full-roster match validation.
- Successful creation uses the validated config/context; web gameplay opens a
  separate `/play/:match_id` document and native switches scenes (ADR-011).
- Ready/loading/empty/error at 320/390/768/1440, 200% zoom/text scaling, soft
  keyboard and safe areas, four schemes, normal/compact density, reduced motion,
  ≥44 dp targets, keyboard-only completion and real AT announcements.

Run focused non-empty tests and relevant accessibility/navigation/platform
checks, then `just check`. Record commands, selected cases, results and residual
scope. Review changed goldens deliberately. Compilation, static artwork or an
empty test selection cannot establish these runtime acceptance claims.

## Stage C evidence

Implemented at `feat/issue-50-discovery-acceptance`, on top of `develop` merge
`9819094`. Stage B measured reflow at two widths in one scheme and said so.
This stage measures the whole acceptance line — 320/390/768/1440, 200% zoom,
pointer targets, keyboard focus, four schemes — and fixes what it found. No new
screen, route, capability or service: the scope is the shell's own treatment of
the viewer's viewport and preferences.

### How each width was reached

The pane's viewport emulation sets the CSS viewport; 200% browser zoom is
measured as the CSS viewport it produces (1440 × 900 at 200% is 720 × 450),
because that is the only thing zoom changes for layout. This is not the browser
zoom control, and the ledger does not claim it is.

A target is counted as reachable when its own border box is at least 44 × 44,
or when a 44 × 44 box centred on it hit-tests entirely to it or to the label
that controls it. Box size alone is the wrong oracle in both directions: a 20 px
radio under a full-row label is reachable, and a 44 px control under an overlay
is not.

| Claim / invariant | Owner and failure mode | Oracle / scope | Check and status | Residual scope |
|---|---|---|---|---|
| Every pointer target is at least 44 dp | `app.scss`; a 20 px native radio was the only thing a finger could land on in a 68–152 px row | `document.elementFromPoint` at the four corners and centre of a 44 × 44 box on every enabled control, after the box-size shortcut | FAIL before the fix (3 mode radios per setup screen, every width). Fixed by stretching `.group__label` over its row. Re-run `interaction-tested` PASS: 0 failures over `/`, `/games`, `/games/:id`, `?setup=1` for both games at 320, 390, 720, 768 and 1440 | Probing is per rendered state; a control that only appears behind an unreachable state was not probed |
| A row click selects the row's own mode, and a disabled row still selects nothing | `mode_group`; an overlay that swallowed the click, or one that defeated `disabled`, would be the defect | A real pointer click at the bottom-right corner of each row | `interaction-tested` PASS: the corner of the bot row selects it — the tonal selection moves and the game's own *Bot level* field appears, so the draft updated, not just the DOM — and the corner of the unavailable network row leaves all three unchanged | — |
| Nothing overflows the viewport at any measured width | Layout; a lost CTA or summary is the named failure | `scrollWidth - clientWidth`, plus every element whose right edge passes the viewport | `interaction-tested` PASS: 0 at every width above, in both locales. At 720 (1440 at 200%) both setup CTAs remain present at 56 px high | Only `en` and `vi` were rendered; a longer third locale is unmeasured |
| The scheme follows the viewer, and keeps following them | `parts::system_scheme`; the preference was read once at mount, so a viewer who turned on dark mode stayed on light until a reload they have no reason to perform | The two media queries and the `data-theme` the generated stylesheet keys off | FAIL before the fix, reproduced: `prefers-color-scheme: dark` became true and `data-theme` stayed `light`. Fixed with `change` listeners on both queries. The fix's execution is BLOCKED — the pane's media emulation flips `matches` without dispatching `change`, proven by a control listener added in the page receiving nothing either — so the listener wiring is `source-read` and `compiled` only | A real browser, or a wasm test, would execute it; neither is run here |
| Each pair of preferences names its own generated scheme | `scheme_for`; collapsing two cases would leave a viewer on a scheme they never asked for, with no symptom the shell could report | All four combinations, against the schemes `tokens.css` actually defines | `example-tested` PASS (`each_pair_of_preferences_names_its_own_generated_scheme`), exhaustive over the two-boolean domain and asserting each name exists in the generated stylesheet | The mapping is proven; which query is read is not — that is the `source-read` boundary above |
| All four schemes render | `tokens.css` and the shell | `light` and `dark` through the viewer's own media query; `hc-light` and `hc-dark` by setting `data-theme` directly | `screenshot-inspected` PASS for all four | `hc-*` were reached by attribute, not by `prefers-contrast`, which the pane cannot emulate |
| Text stays legible in every scheme | Generated roles; a role pair that failed in one scheme only | Computed foreground against the nearest painted background, WCAG 2 contrast | `example-tested` PASS, lowest 7.98:1 (light, on-primary on primary), 8 element roles × 4 schemes, all above AAA for body text | Measured on this screen's roles only; not a whole-palette audit |
| Keyboard focus is ordered and always visible | Shell; a removed or invisible ring | Tab from the document start, reading the computed outline at each stop | `interaction-tested` PASS: skip link → nav → locale → back → modes → clock → check, each with `solid 3px` at 2 px offset, including the radio under its new full-row overlay | — |
| Keyboard activation completes the flow | Shell | Enter on the focused control | BLOCKED, not a defect: the pane's key events activate a focused link (navigation observed) but synthesize no click on a focused `<button>` — a bare probe button injected into the page behaves the same. The controls are native `<button>` elements with `on:click`, for which activation is the browser's own (`source-read`) | Needs a real browser session or a wasm interaction test |
| The workspace stays green | Repository | The authoritative ordered local gate | `just check` — PASS (all gates, cargo-deny included). `cargo nextest run --workspace` — 831 run, 831 passed, 2 skipped, both pre-existing `#[ignore]` (golden regeneration, deep perft). `cargo check --workspace` with `--no-default-features` and `--all-features` — PASS. `cargo check -p tabula-web --target wasm32-unknown-unknown` — PASS | — |
| The new dependency is already permitted | `deps.toml`; a crate reaching past its allow-list | The matrix over the resolved graph | `cargo xtask check-deps` PASS. `wasm-bindgen` was already allowed for `apps/web`; `deps.toml` is unchanged | — |

### What Stage C found and did not fix

Each of these has an owner outside this change. None is hidden behind a passing
check above.

- **The generated tokens are in `px` only**, so a viewer's text-size preference
  (text scaling without page zoom) changes nothing. Page zoom does work, and is
  measured above. The unit is a token-source decision in `tokens.toml` and
  `tabula-design`, shared with the native client's goldens — issue #49's
  foundation, not a shell edit.
- **`--sys-state-disabled-content` is `0.38` in all four schemes**, so a
  disabled option's own name measures 2.68:1 in `hc-light` and 2.30:1 in
  `light`. WCAG exempts disabled controls, and every such row states its reason
  at full contrast beside it, so nothing is unreadable — but a high-contrast
  scheme that does not raise this is not doing what it is for. Same owner as
  above: it is one token, in four schemes.
- **Keyboard-only completion and real assistive technology remain unexecuted**,
  for the harness reason recorded in the table. Lighthouse, bundle size and
  performance budgets are still unmeasured, as Stage B said.

To reproduce the runs above: `trunk serve --config apps/web/Trunk.toml` and
drive `http://localhost:8080` at each width.
