# Goal

Six reviewable Caro implementation PRs. Every PR keeps the core gate green; no planning-file change
here implements a game. Defaults are in [00-decisions.md](00-decisions.md).

## Current state

Caro is a skeleton. Baseline conformance should arrive with its first GameRules implementation,
not in a later PR. Build identity should arrive with actual src/rules source, not before it exists.

## Decisions

`C1 → C2 → C3 → C4 → C5 → C6`; app/browser selector B1 is a separate prerequisite of the C5 browser
demo if still absent. Across the portfolio, the recommended first PR is **Werewolf W1**, whose
rules/knowledge decisions remove more contract uncertainty. Caro C1 is independent after Phase 2 exit.

## C1 — Decisions and validated domain primitives

| Field | Plan |
|---|---|
| Objective | Settle variant, size and terminal/timer behavior; introduce usable bounded board/coordinate types |
| Prerequisites | Phase 2 exit evidence, doc 00/02/08 and C-D1..11 review |
| Scope | Decision record, BoardSize/Board/Stone/raw-coordinate construction, serde validation |
| Out of scope | GameRules, manifest/build.rs before rules exist, bots/presentation/platform edits |
| Expected files | docs/games/caro.md; games/caro/src/rules/{mod,board,coord}.rs, src/lib.rs, Cargo.toml, constructor tests |
| Invariants | I-C1 and pure dependency boundaries; no constructor/deserialization bypass |
| Verification/evidence | Boundary/round-trip/invalid DTO tests: example-tested; decisions documented |
| Acceptance criteria | Every C-D row accepted/replaced; odd 9..19 bounds and raw coordinate policy explicit; gate green |
| What it unlocks next | C2 without gameplay ambiguity |

## C2 — Complete rules, build identity and baseline conformance

| Field | Plan |
|---|---|
| Objective | Implement complete GameRules/GameModule without a phase of unsafe or placeholder behavior |
| Prerequisites | C1 |
| Scope | Reference window detector first, production scan, validate/commit reducer, all Input arms, legal commands, distinct View, describe, metadata/capabilities, manifest/hash, baseline GameTestFixture |
| Out of scope | Bots, presentation, global SDK/scaffolder refactors |
| Expected files | games/caro/src/rules/{mod,state,win}.rs, src/lib.rs, game.toml, build.rs, Cargo.toml; tests/{rules,conformance}.rs, test oracle; docs/games/caro.md |
| Invariants | I-C1..8, every terminal cause representable, all fallible checks precede mutation |
| Verification/evidence | Line geometry/reference comparison, legality/terminal/timer tables, 11 conformance fixtures: example/differentially-tested |
| Acceptance criteria | Numeric RULES_VERSION marker exists before build script runs; nonzero identity; early/stale timer safe; off-turn resign in legal set; baseline conformance and core gate green |
| What it unlocks next | C3 deeper sequence/size evidence |

## C3 — Stronger verification and measured capabilities

| Field | Plan |
|---|---|
| Objective | Cover large reachable spaces and make state-size declaration factual |
| Prerequisites | C2 |
| Scope | L1..9/L14 properties and boundary extensions, all-size draw traces, independent line validity, snapshot continuation, state-size/capability parity checks |
| Out of scope | Mandatory Kani/fuzzing, synthetic timing inside pure rules, mutation campaign before stable code |
| Expected files | games/caro/tests/{rules,determinism,state_size}.rs, tests/support/; relevant rule tests; game.toml/src/lib.rs class correction |
| Invariants | Rejection byte equality, legal-command equivalence at valid time, deterministic order, meaningful terminal fixtures |
| Verification/evidence | 128 cases/property with shrinking; every supported board geometry and constructed draw; canonical size measurement |
| Acceptance criteria | No vacuous terminal-only legal fixture; all-size draw scripts reach draw without prior win; manifest and compiled class agree; gate green |
| What it unlocks next | C4 stable rules for replay/bots |

## C4 — Projection bots, selfplay and golden replay tooling

| Field | Plan |
|---|---|
| Objective | Exercise real matches and pin their historical behavior |
| Prerequisites | C3 |
| Scope | Trivial random placement; Easy take immediate win, else block immediate loss, else deterministic supplied-RNG tie-break; exclude resign while moves exist. Bounded crate campaign; three explicit-script replays and CLI registration |
| Out of scope | Strong engine, runtime/registry implementation, treating identical repeated runs as gameplay proof |
| Expected files | games/caro/src/bot.rs, src/lib.rs, tests/replay.rs; root tests/replays/caro-{normal,draw,timeout}-golden.tbr + README; xtask/Cargo.toml, selfplay_cmd.rs, replay_cmd.rs, replay_goldens_cmd.rs, xtask README |
| Invariants | Bots receive View only; rules never draw RNG; replay stores accepted inputs/times; Exact identity and literal hashes |
| Verification/evidence | Bot tactical/legal/determinism tables; 64 default-build synthetic matches; three reviewed goldens, per-input checkpoints and terminal outcomes |
| Acceptance criteria | Proposed `cargo xtask selfplay caro --board-size 9 --matches 1000` works (also 15/19); per-file replay works; timed fixture reaches real due timeout; gate green |
| What it unlocks next | C5 playable local integration; C6 nightly campaign |

## C5 — Presentation, native/browser integration and SDK accounting

| Field | Plan |
|---|---|
| Objective | Play Caro through current generic client contracts and measure addition cost |
| Prerequisites | C4; B1 generic browser selector if needed for browser launch |
| Scope | CaroPresentation/Local, pointer and keyboard focus, snapshots/reduced-motion, minimal staged pack/fallback, app leaf wiring and LocalMatch/replay test |
| Out of scope | Network, mobile wrappers, web application shell, generic renderer changes, deep Board Reader |
| Expected files | games/caro/src/presentation.rs, src/snapshots, lib.rs, Cargo.toml; assets/packs/caro/*; apps/game-client/{Cargo.toml,src/main.rs,tests/local_match.rs}; docs/games/caro.md |
| Invariants | I-10/I-15; semantic tokens; no game logic in LocalMatch/run_local; each game-id marker narrow and justified |
| Verification/evidence | RenderList snapshots/input tables; same semantic play across layout/animation states; native/browser manual demo; recorded changed-file/line accounting |
| Acceptance criteria | Hot-seat/solo terminal play, valid replay, keyboard path, pack build; browser actually launches Caro; zero platform/service edits; gate/feature/WASM builds green |
| What it unlocks next | C6 closure; later registry registration after Phase 4 begins |

## C6 — Hardening, nightly and documentation closure

| Field | Plan |
|---|---|
| Objective | Demonstrate assertion strength and record complete vs residual Phase-3 evidence |
| Prerequisites | C5 |
| Scope | Scoped mutants, real survivor regressions, 100k selfplay at 9/15/19, larger properties, synthetic apply/event budget reporting, documentation and manual play notes |
| Out of scope | Fixing unrelated nightly fuzz/load stubs; silent normative amendments; unsupported formal/security claims |
| Expected files | .github/workflows/nightly.yml; games/caro tests as gaps require; docs/games/caro.md; xtask reporting only if needed |
| Invariants | Existing laws remain unchanged; no weakened assertions to meet estimated budget/size |
| Verification/evidence | Mutation survivor classification; synthetic campaign and latency observations; cross-target checkpoint comparison if runner available (otherwise explicit dependency) |
| Acceptance criteria | All real survivors fixed with semantic regressions; nightly commands supported; all criteria marked executed or phase-gated; human play notes and SDK accounting present; core gate green |
| What it unlocks next | Caro Phase-3 acceptance when required evidence complete; Phase-4 online work stays separate |

## Verification ledger

Detailed claims/tiers are [L1–L16](02-verification.md) and the presentation ledger. Later CI edits
must retain existing games. Current nightly replay --all and fuzz/load jobs are pre-existing gaps;
use supported per-file commands or separate generic tooling repairs before claiming nightly coverage.
No blanket claim that conformance covers generated properties, replay goldens, timing or WASM execution.

## Residual risks

Phase-2 exit must be evidenced, not inferred solely from Tiles being implemented. B1 browser
selection and incomplete backend asset delivery are pre-existing app/infrastructure limitations;
report their cost separately. Game source hashes include tests/comments under src/rules, so
post-C4 edits may require explicit identity-header regeneration without semantic version changes.

## Next dependency

Start C1 after the phase gate; do not fold future networking into any Caro PR.
