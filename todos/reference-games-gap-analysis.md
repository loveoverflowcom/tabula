# Goal

Compare Caro, Werewolf and Tiles against the **actual source**, then separate missing Phase-3 work
from correctly deferred work. This is planning only; no game/platform implementation is added.
Doc 00 is normative even when code or downstream docs differ.

## Current state and baseline

Inspected `develop` commit `9e036cd8c10b6fc760b4a0505e7931ed9ee918c4` (retire-tictactoe merge).
`git ls-remote origin refs/heads/develop` confirmed that remote develop matched this checkout on
2026-09-04. Initial working-tree change was only the unfinished untracked todos directory.

Classification: **implemented** = working artifact exists; **partial** = some artifact/coverage
exists; **placeholder** = documented skeleton only; **missing** = absent current-phase work;
**phase-gated** = deliberately later; **not applicable** = claim does not apply to this game.
Compiled empty crates do not establish gameplay or security evidence. Phase-2 exit is a prerequisite
for implementation; this repository inspection does not independently certify all its demo gates.

## Gap matrix

| Dimension | Caro | Werewolf | Tiles |
|---|---|---|---|
| Manifest/build integration | **partial**: Cargo/features/lints; no game.toml/build.rs | **partial**: same | **implemented**: game.toml, build.rs, RULES_VERSION/HASH and package metadata |
| Rules/domain model | **placeholder** | **placeholder** | **implemented**: placement/features/scoring and timed turns |
| State/Command/Event/View/ViewEvent/Config | **placeholder**: illustrative sketches only | **placeholder**: illustrative sketches only | **implemented**: distinct View, typed rules model |
| GameRules/GameModule | **missing** | **missing** | **implemented**, including config validation |
| Projections/SecretModel | project **missing**; SecretModel **not applicable** (public game) | **missing**, highest-value security work | **implemented**: hidden bag-order projection + SecretModel and noninterference; short-token limits documented |
| legal_commands | **missing** | **missing**: phase/role-aware enumeration needed | **implemented**: Hints for placement, Enumerated for meeples |
| Bots/self-play | **missing** | **missing**: verification simulation; runtime substitution **not applicable/forbidden** | **implemented**: Greedy policy, in-crate campaign, xtask dispatch |
| Replay | **missing** | **missing** canonical rules replay; projected spectator replay **phase-gated** | **partial**: one committed complete golden + Exact tests; below doc 08's ≥3 normal/edge/timeout artifacts |
| Conformance | **missing** | **missing**, including separate projection_security suite | **implemented**: GameTestFixture/conformance; security fixture in rules/secret.rs |
| Property/differential verification | **missing** | **missing** | **implemented**: feature graph vs whole-board model, replay and bag-permutation laws; not exhaustive state-space proof |
| Presentation | **missing** (Phase 3) | **phase-gated** (Phase 7) | **implemented**: RenderList presenter, pan/zoom, local input |
| Snapshots | **missing** canonical round-trip/replay and presentation snapshots | **missing** canonical restore/replay; presentation snapshots **phase-gated** | **implemented** canonical replay/round-trip + three RenderList snapshots; full raster coverage not implied |
| Local client/demo | **missing**: native/bot/browser launch path | **missing** headless side-by-side projection demo; Macroquad/social client **phase-gated** | **partial**: native leaf wiring/LocalMatch test; browser game selection not exposed by current startup |
| Docs | **placeholder** docs/games/caro.md | **missing** docs/games/werewolf.md information model | **implemented** docs/games/tiles.md with scope and residual gaps |
| CI/nightly | **missing** game tests/dispatch/matrix | **missing** rules/security tests and simulation matrix | **partial**: PR tests and nightly selfplay exist; replay --all/fuzz/load workflow commands have pre-existing gaps |

## Source anchors

- Skeletons: games/caro/src/lib.rs, games/werewolf/src/lib.rs and their Cargo.toml files.
- Working reference: games/tiles/src/rules/{mod,state,feature,secret}.rs, src/{lib,bot,presentation}.rs,
  tests/{rules,features,determinism,conformance,replay,state_size}.rs and root tests/replays/tiles-golden.tbr.
- SDK: crates/tabula-game-api/src/{rules,module,effect,capabilities}.rs.
- Test boundaries: crates/tabula-testkit/src/{selfplay,projection,determinism}.rs and conformance/{mod,security}.rs.
- Integration: apps/game-client/src/{main,lib,replay_capture}.rs, tests/local_match.rs, web/index.html.
- Tooling: xtask/src/{main,selfplay_cmd,replay_cmd,replay_goldens_cmd,pack_assets_cmd}.rs and Cargo.toml.
- Gates: .github/workflows/{ci,nightly}.yml, justfile, xtask/src/check_cmd.rs, deps.toml.

## Missing Phase 3 versus future scope

Caro lacks a settled ruleset, all rules/types, conformance and independent win evidence, bots,
three canonical goldens, local presenter/snapshots/client wiring, pack source, SDK-cost record and
nightly coverage. Choose a benchmark variant now while preserving a later product variant decision.

Werewolf lacks role presets/assignment, phases/timers, night/vote/death/victory rules, projections,
view_event None cases, SecretModel and transient event secrets, canonical replay/simulation,
information model, scope **values** and the terminal side-by-side projection viewer. These are
Phase-3 gaps. Missing role-card UI, chat socket enforcement, social rooms, real-human online demos
and voice/SFU work are **correct phase gates**, not evidence that Werewolf is behind.

Phase 7 owns Werewolf presentation/assets/UI snapshots, social and chat enforcement and 12-person
playtest. Phase 8 owns voice grants/providers/media enforcement. Generic metadata/routing/security
contracts must be settled in Phase 4 before protocol freeze. Spectator replay needs Phase-9 review.

## Architecture/document drift register

Reported here; normative architecture is not silently rewritten.

| ID | Finding and source | Consequence / owner |
|---|---|---|
| G1 | AGENTS/doc 02 advertise xtask new-game; xtask/src/main.rs dispatches it to unimplemented_command | Scaffold game additions manually; generic scaffolder is separate tooling work |
| G2 | doc 02 §11.1's “mandatory suite” exceeds conformance! expansion: actual macro is 11 fixed fixture tests, without goldens/selfplay/budget/security scans | Plan explicit game properties, goldens, campaigns and projection_security; no “property-tested” claim from macro alone |
| G3 | doc 02 §12.0 labels Caro symmetric while proposed freestyle opening assigns a meaningful first-player role | C1 recommends asymmetric capability; record downstream-doc amendment request instead of silently changing it |
| G4 | doc 02/Werewolf sketch uses ChatScopes::new().allow and VoiceScopes::rooms; current structs expose fields without these builders | Struct literals suffice; no platform builder PR required |
| G5 | TeamSpec describes uniform team sizes; Werewolf factions are unequal | teams=None and ranked=No; alignment/outcomes game-owned; no invented team API |
| G6 | doc 05 §2 exposes StateVersion in game frames, ack and resync. I-7 requires canonical increments for accepted inputs, including hidden ones | Record ADR request for observer cursor/emission/ack semantics before Phase 4 wire freeze; empty-frame suppression alone cannot hide counter gaps |
| G7 | Secret::tokens scalar granularity is unresolved; Tiles uses multi-value bag sequences; raw role/seat bytes collide | Werewolf needs actual compound tokens with negative controls plus primary noninterference/typed policy tests; no fake tagged tokens |
| G8 | nightly fuzz commands name protocol_decode/command_decode but no fuzz targets exist; load dispatch is also unimplemented | Source-level workflow defects; actual CI run not inspected. Repair/gate separately; do not add irrelevant game fuzz targets |
| G9 | docs/games/README calls Werewolf model important/unwritten; doc 02 §7.1 requires it | Information model is W1/W6/W7 hard deliverable, not W10 cleanup only |
| G10 | Registry is Phase-4 skeleton; app currently uses documented restricted-zone leaf markers for direct game references | Follow narrow app/tooling precedent. Do not generalize this into permission for platform game-id branches |
| G11 | nightly and justfile call replay --all; replay_cmd::run treats first argument as a file path and only dispatches chess/tiles | Current --all attempts to read a file named --all. Add per-file game dispatch and use explicit file jobs, or separate reusable batch-tooling PR |
| G12 | nightly comments promise auto-committed regression replays and production samples; selfplay returns seed/index diagnostics, workflow has no artifact/commit/upload logic | Schedule comments are not executed evidence. Add owned synthetic reporting/artifact policy explicitly; no production claim |
| G13 | VoiceRoom has members only; doc 07 Phase 8 requires dead hear living channels but publish only to dead | Generic permission model/ADR needed before Phase-8 implementation; Phase-3 membership tests cannot prove that behavior |
| G14 | Event has no generic Audience wrapper despite RolesAssigned described as ServerOnly | Enforce with view_event=None and later routing; Audience exists on Notify, not arbitrary canonical Event |
| G15 | doc 07 still includes active tic-tac-toe Phase-3 acceptance/menu/smoke rows; latest develop retired it in PR #43 | Keep historical docs separate; record roadmap cleanup request rather than re-add retired game |
| G16 | Current native argument parser/web loader has no browser game selector; asset builder expects assets/packs/<slug> | Caro C5 names reusable app prerequisite B1 and correct source paths; no “four menu edits proves browser play” claim |
| G17 | doc 02 §7.2 ties client_preview to view folding while doc 00 §4.1 defines nonauthoritative projection-based preview; no generic folding oracle is implemented | Keep Caro/Werewolf preview false for this scope; record semantic clarification separately, do not infer a Chess defect from a missing generic test |
| G18 | Current conformance Scenario uses fixed 1-second input ticks; Werewolf needs due-timer traces | Use matching valid fixture durations/padding and direct Ctx/replay tests, not relaxed production timer validation |

## Verification choices

| Tool | Caro | Werewolf |
|---|---|---|
| Tables/enumeration | Geometry windows, legality/terminal/timer partitions | Preset counts, role authorization, small ballots and wolf/nonwolf count pairs |
| Property testing | Primary for reachable sequence laws, rejection, legal enumeration | Primary for phase sequences, rejection and all knowledge noninterference |
| Differential | Primary fast win scan vs independent whole-board detector; replay baseline | Primary night/vote resolution vs simple set/tally model; canonical replay |
| Mutation | Useful on stable win/legality validators | Useful on projection/view_event guards and win/tie/resolution code |
| Kani | Not required: cheap adequate oracles exist, despite enormous full board state space | Not required; narrowly scoped inactive candidate documented with proposition/bounds/trust/exclusions |
| Fuzzing | Not applicable to typed rules scope; future byte boundary owns it | Same; no game-specific decoder introduced |

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| Gap analysis matches current code | Plans depend on absent helpers | Inspected source anchors and remote develop hash | documented (source-inspected) | Manual/security review | Future commits can invalidate findings |
| Planning leaves implementation untouched | Scope silently expands | Changed-path audit limited to todos/ | statically checked | Every PR | New docs still need human judgment |
| Existing core gate stays green | Planning introduces build/gate issue | cargo xtask check, exact recipe behind just check | statically checked | Every PR | Does not establish new game correctness or validate all nightly jobs |

## Acceptance criteria and next dependency

Each game has decisions, invariants, per-claim evidence, explicit PR prerequisites/files/acceptance
and residual gaps. Start **Werewolf W1** first to reduce information-policy uncertainty, then
Caro C1 independently. See [index and critical paths](README.md).
