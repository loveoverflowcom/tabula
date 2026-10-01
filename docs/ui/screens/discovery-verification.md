# Discovery/setup verification ledger

Stage A of [issue #50](https://github.com/loveoverflowcom/tabula/issues/50).
The [shared contract](discovery.md) pins source and reference versions; the
[availability inventory](discovery-availability.md) links inspected owners.
Evidence vocabulary follows [Tabula engineering](../../../.agents/skills/tabula-engineering/SKILL.md).
Documentation is the deliverable here. A specified layout or check is not an
executed runtime result; the issue's full runtime acceptance remains open.

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
| Decorative border/shadow removed; focus/selection/error retained | Foundation treatment and per-screen component/state sections | Documented; four-scheme runtime focus/contrast verification remains |
| Route → typed data owner → action → result for 01–03 | [Routes and typed mapping](discovery.md#routes-aliases-and-navigation), shared state machine and failure matrix | Documented/source-read; adapters and running routes NOT_IMPLEMENTED |
| Empty/loading/filter-no-result/unavailable/rejected UI | Library/detail tables, setup feedback, [shared matrix](discovery.md#shared-empty-loading-and-failure-matrix) | Documented; execution NOT_IMPLEMENTED |
| No unsupported rating/engine-ready/accessibility/offline badge | [Inventory](discovery-availability.md), typed mapping and claim restrictions | Source-read; future badges require scoped evidence |
| Normalized config/time/seat/mode summary and supported actions | [Setup form and summary](03-new-match.md#form-and-normalized-summary), immutable-validator dependency | Documented; form/normalization/creation adapters NOT_IMPLEMENTED |
| Local flow avoids inappropriate auth gating | Route/action contract and mode readiness; existing local driver | Source-read/documented; shell auth-return flow NOT_IMPLEMENTED |
| 320/390/768/1440, 200%, targets ≥44 dp, useful keyboard/focus | Per-screen breakpoint and keyboard sections; foundation | Documented; platform reflow/interaction/AT tests NOT_IMPLEMENTED |

## Separate Stage B after the gates

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
