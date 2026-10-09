# Age War D01 — scoped evidence

Date: 2026-10-08 UTC. Issue [#119](https://github.com/loveoverflowcom/tabula/issues/119),
roadmap [#118](https://github.com/loveoverflowcom/tabula/issues/118).
**Design-only / UNBALANCED / OWNER REVIEW PENDING.**

Initial source-read base: `023ecf296352b2f3285c9dbd9c703fe867c9d6cb`.
Fresh develop recheck before publication found
`87435f350f82d4d8bace3ad91358c3885c664e74` (CMP Library #129).
The isolated D01 contribution was rebased onto that base before final checks.
Reviewed code/design head: `e080a23646a903be7631ab563d090657bd3eb924`; a later
evidence-only commit updates this ledger/index without changing tested Rust data.
No Age War registry/runtime registration or production availability is added.

## Scope

- Source-grounded research, GDD/RULES/content/math, draft compatibility decision,
  fair AI/experiment plan and finite owner review questions
- Typed authored descriptors, local validation, raw/checked numeric boundaries,
  original unbalanced catalog, typed unavailable seams and pure arithmetic
- No reducer/projection/bot/presenter/renderer/assets/game manifest or runtime
- No production platform/protocol change; the existing games wildcard owns
  crate dependencies and workspace membership

## Reproduction environment

Pinned Rust1.96. Cloud Linux x86_64, two build jobs; no target substitution.
`CARGO_HOME=/workspace/scratch/429f9ca90843/rust-home/cargo` and corresponding
`RUSTUP_HOME` were set; commands run from the isolated repository root.
Existing external target cache reused, never tracked or uploaded.

## Checks

Final focused code checks and the aggregate ran after rebase and review fixes.
The aggregate fails on the inherited BASE I-9 violations below; no full-gate
PASS is claimed. Missing tools and skipped/empty targets do not establish PASS.

| Check | Status | Evidence scope / residual |
|---|---|---|
| Focused default/no-default/all-feature tests before rebase | PASS | 31 executed: 8 arithmetic,22 schema/catalog,1 document matrix; zero ignored; matrix asserts576 cells |
| Focused no-default/all-feature Clippy before rebase | PASS | all targets, `-D warnings`; types/data/test scaffold only |
| Final rebased focused checks | PASS | `cargo test -p tabula-game-age-war --no-default-features` and `--all-features`:34 executed each (8 math,25 descriptor/catalog,1 matrix),zero ignored/filtered; all-target all-feature Clippy `-D warnings` PASS |
| Mandatory `cargo xtask check` | **FAIL** | Prescribed fmt PASS,workspace all-target/all-feature Clippy PASS,workspace tests1410 passed/0failed/18ignored,check-deps31crates PASS; then I-9 four inherited mobile test literals. Later aggregate stages not executed |
| Architecture/hygiene gates | PASS / inherited FAIL | Dependency graph31crates PASS; manifest34checked PASS; raw-color and mobile-catalog freshness checks PASS; repository hygiene PASS +6tests;69local doc targets checked. I-9 inherited FAIL reproduced on clean BASE |
| Read-only `tabula-code-review` | Completed, no unresolved actionable finding | Independent review pinned BASE87435f3→HEADe080a236; findings fixed/rechecked; separately ran both34-test focused selections; no owner approval or full-game claim |
| `cargo deny` / `cargo nextest` availability | BLOCKED | executables absent in cloud; probe returned `no such command`; no installation claimed |
| Gameplay conformance | NOT_APPLICABLE to D01 | no GameRules/GameTestFixture; required actual C01 acceptance target, not a zero-test PASS |
| Gameplay/AI/replay,1000/10000 match campaigns | NOT_IMPLEMENTED / NOT_RUN | QA contains planned seed/side/CI methods, no matches/results |
| Selected WASM scaffold | compiled | `cargo check -p tabula-game-age-war --no-default-features --target wasm32-unknown-unknown` PASS; no execution/render/replay equality claim |
| Cross-target canonical replay equality | NOT_RUN | no canonical rules/replay exists; compilation alone cannot establish equality |
| Browser/native visual/performance/human playtest | NOT_RUN | no gameplay/assets in D01 |
| Native Android/iOS Age War | BLOCKED / NOT_IMPLEMENTED | separate #81/ADR0043 native host and packaging/device acceptance |

`tests/design_math.rs` reads literal MATH.md contact cells. The simple one-stage
armor-only derivation is separate from the multi-stage function under test;
shared trusted input is catalog HP/basic damage/type armor. This only establishes
that576 analytical cells and finite worked math agree, not measured counters,
formation behavior, balance or fun. The enum/phase errors and hostile descriptor
assertions establish their actual construction/rejection cases, not R2/R8 game
transactionality (there is no reducer).

## Gates and owner decisions

The [GDD review list](../../games/age-war/GDD.md#6-ownership-and-next-step) is
PENDING. Draft compatibility proposal is not a registered/accepted ADR. Next
is D02 #120, then D03–D06. No CI/test/review result approves the design, opens
C01, closes issue119, merges/deploys, creates assets or proves mobile gameplay.

## Inherited aggregate blocker and check selection

At fresh BASE87435f3 and final D01 code, `check-no-game-ids` reports exactly
four game-name literals in
`apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/DiscoveryCatalogTest.kt`:
three names at line95 and one at120. D01 changes neither that file nor the
scanner/policy. The same built xtask resolves workspace by current directory;
running it in a detached clean87435f3 checkout reproduced all four (884files
scanned at BASE,902at D01). This is a base-relative diagnostic, not a waiver.
No unrelated mobile/source fix or scanner allow-list weakening is included.

The final aggregate was run twice; the second includes all review fixes and
34D01 tests. It stops at I-9 by design, so token-freshness/deny later aggregate
stages are not passing. Manifests, no-raw-colors and check-mobile-catalog were
then executed separately and passed; `cargo deny`/nextest probes verify missing
executables. The CLI has no `check-tokens` command; that probe is not evidence.
`gen-mobile-catalog --check` regenerates rather than checks; final explicit
`check-mobile-catalog` PASS and Git diff showed no generated source change.
No skipped or zero-test target is counted as a tested behavior.

Review corrections reconcile root-immunity origin, volley repeated-target
semantics, specific unit loadouts, summon-model count and final-floor refunds.
Descriptor validation now rejects enemy/structure cleanse, unsupported DoT
stack/refresh/expiry and projectile range beyond flight lifetime, with reachable
hostile regressions. Review checked36unit stats/basic/bindings,28fullcards and
12turret rows against authored Rust; these facts do not establish gameplay.
