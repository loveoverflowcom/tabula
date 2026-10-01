# Issue 47: bounded local game-audit pilot

This report applies [issue #47](https://github.com/loveoverflowcom/tabula/issues/47) to a selected
Chess/Tiles scope. It separates SDK, rules, secrecy, replay and presentation evidence. No game
behavior was changed and no golden was regenerated. Raw executed output, including the failed
capability probe, is retained in [the pilot log](issue-47-skills-pilot.log).

## Provenance and scope

- Source ref: `44f6b74e07648abc7191363d7582efc1fceab262` on `develop`.
- Dirty scope: skills, routing/docs, validation scripts/CI and comment-only references to migrated
  skills. Rules, fixture logic, canonical encoding and `.tbr` bytes match that source ref.
- Working directory: `/home/manhpd/Projects/tabula`; native `x86_64-unknown-linux-gnu`,
  rustc/cargo `1.96.1`, nextest `0.9.143`. Commands below use the existing local Cargo cache offline.
- Tests: default `rules` plus explicit `bots,presentation`, normal nextest profile;
  `PROPTEST_RNG_SEED=47` pins property-test generation. The selected game fixtures use their
  checked-in seed/config; their source and the exact ref complete the reproducer.
- Chess benchmark scope is defined by [doc 08 §2](../architecture/08-first-games-validation-plan.md)
  and existing rules/clock tests; a maintained `docs/games/chess.md` is absent. Tiles uses the custom
  Carcassonne-family rules in [its spec](../games/tiles.md), not an official tile/scoring set.
- Contract owners: doc 00, doc 02 and game specs; kernel/game-api define the SDK, each game owns
  meaning, `project`/`view_event` own disclosure, presentation owns local UI state.

## Executed checks

All commands ran from the working directory above. Exact flags, output and exit codes are in
the log. A test-run status applies only to selected assertions; it does not certify a whole game.

| Check → oracle → evidence | Status and executed scope |
|---|---|
| Chess/Tiles suites → fixture assertions, independent perft/feature model, laws → example/property/differential/headless evidence | `PASS`: 284 selected/executed tests across 15 binaries, all passed. One ignored deep-perft test was skipped, as documented below. |
| Chess bounded selfplay → testkit transition/replay/projection checks → example-tested deterministic workload | `PASS`: 10/10 terminated matches, 3,927 inputs, zero failures; Fischer mode. |
| Tiles bounded selfplay → testkit transition/replay/projection checks → example-tested deterministic workload | `PASS`: 10/10 terminated matches, 1,347 inputs, zero failures; five seats, turn deadlines disabled by this CLI. |
| Three committed `.tbr` files → stored checkpoints/final hashes/outcomes → replay comparison | `PASS`: Chess 4, clocked Chess 2, Tiles 102 checkpoints, plus each final hash and terminal outcome; CLI verdict `EXACT`. |
| Unsupported `replay --all` probe → actual dispatch → capability discovery | `FAIL`: exit 1, `--all` treated as filename. This attempt is not replay evidence; corrected per-file checks above succeeded. |

```bash
PROPTEST_RNG_SEED=47 CARGO_NET_OFFLINE=true cargo nextest run -p tabula-game-chess -p tabula-game-tiles --features bots,presentation
CARGO_NET_OFFLINE=true cargo xtask selfplay chess --matches 10 --seed 47 --match-index 0 --max-inputs 2000 --clock fischer
CARGO_NET_OFFLINE=true cargo xtask selfplay tiles --matches 10 --seed 47 --match-index 0 --max-inputs 2000 --seats 5
CARGO_NET_OFFLINE=true cargo xtask replay tests/replays/chess-golden.tbr --verify --diagnose
CARGO_NET_OFFLINE=true cargo xtask replay tests/replays/chess-clock-golden.tbr --verify --diagnose
CARGO_NET_OFFLINE=true cargo xtask replay tests/replays/tiles-golden.tbr --verify --diagnose
```

Selfplay seed `47` means little-endian u64 in the first eight bytes of the 32-byte base seed,
with remaining bytes zero. Match-index starts at 0 and the input limit is 2,000 per match.
Projection checks stayed enabled. CLI setup fixes Chess initial time at 60,000 ms with 1,000 ms
Fischer increment; Tiles uses five Trivial bots and `turn_deadline_ms = 0`. See
[the actual CLI setup](../../xtask/src/selfplay_cmd.rs). Preserve all flags when reproducing:
the existing Tiles failure hint omits seat count, so the hint alone is incomplete.

## What the assertions cover

| Layer | Concrete evidence and limits |
|---|---|
| SDK | 11 conformance tests per game, with concrete invalid/terminal/randomness fixtures. `check-manifests` is schema/feature validation, not compiled metadata equivalence. |
| Chess rules/clocks | 28 clock integration tests, internal clock boundary/model tests, published-position perft through practical depths, legal/terminal/error tests. The ignored depth-five perft test remains `NOT_RUN`; this bounded pilot does not establish all chess rule correctness. |
| Tiles rules | Six feature tests include incremental graph vs whole-board recomputation, exactly-once scoring, follower conservation, and monotonic event/accounting checks. Six determinism tests include canonical-byte rejection and generated reachable sequences. Rules tests exercise the implemented custom variant. |
| Tiles secrecy | Unit suite in `src/rules/secret.rs` executed containment/security checks and bag permutation noninterference for both projections and view events, with viewer/public-change negative controls. This is pure-function evidence; it does not cover socket payload routing, scheduling or timing channels. |
| Replay | Golden expectations stayed committed and unchanged; checkpoints/final hashes/outcomes matched. Agreement with stored same-target vectors does not independently establish legality or native/WASM equivalence. |
| Presentation | Existing Chess presentation unit tests ran with the feature enabled (including clock/local-input behavior); Tiles presentation tests ran in its feature suite. These assert local behavior/RenderList data, not real renderer pixels. No screenshot or driven native/browser interaction was captured or inspected. |
| Budget | Tiles state-size tests ran. No new performance workload, wall-clock budget study or p99 claim was made. |

## Portfolio readiness and findings

These implementation classifications come from current source inspection, not successful execution
of absent targets. The game rubrics must rediscover status at future HEADs.

| Game | Implementation status / remaining scope |
|---|---|
| Chess | Implemented rules/module/bots/presentation. Selected checks above passed; real rendering, cross-target comparison, online and phase-exit readiness were not assessed. |
| Tiles | Implemented rules/module/bots/presentation and bag-order `SecretModel`. Selected checks above passed; CLI selfplay does not test nonzero deadline behavior. |
| Caro | `NOT_IMPLEMENTED`: placeholder library; missing adapter, rules, fixtures and manifest. Variant/board size remain TBD. No game-test or selfplay pass is claimed; no variant was selected by this audit. |
| Werewolf | Partial: validated domain/config, deterministic initial role assignment and compiled metadata/capabilities. `GameRules`, `GameModule`, projections, `SecretModel` and conformance wiring are `NOT_IMPLEMENTED`. Source review of this slice is not full game readiness. |

| Severity → location | Observed/inferred → next action |
|---|---|
| Documentation → `AGENTS.md` §7, `games/README.md`, `xtask/README.md` | Source-observed: `new-game` is intentionally unimplemented. This commit labels it as planned and directs manual, phase-aware authoring. |
| Specification gap → `docs/architecture/08-first-games-validation-plan.md` §2 | Source-observed: Chess has benchmark scope and source/test expectations but no maintained per-game rules/information-model document. Published perft is an independent move-generation oracle, not an authority for every draw/clock semantic. A full rules audit must identify the supported variant and an appropriate external authority. |
| Workflow capability → `xtask/src/replay_cmd.rs`, `justfile` `replay-all` | Execution-observed: `--all` is unsupported. Audit guidance uses explicit paths. A general corpus-discovery command is separate implementation work; do not rely on the existing alias as a passing check. |
| Reproduction → `xtask/src/selfplay_cmd.rs::print_report` | Source-observed: Tiles failure hint omits `--seats`, and hints omit explicit `--max-inputs`. Audit records full setup/flags rather than treating printed hints as complete. No CLI behavior was changed. |
| Evidence gap → `games/tiles/src/rules/secret.rs::a_bag_too_short_to_tokenise_declares_no_containment_secret` | Source-observed: the short-bag example truncates a driven state's bag without accounting for removed tiles or asserting the state's conservation invariant. It exercises the helper on a synthetic state; valid reachable short-bag secrecy remains a separate evidence requirement. Prefer a real near-terminal trace and assert validity before treating it as semantic coverage. No production test or rule was changed. |
| Oracle independence → `games/chess/src/rules/clock.rs::tests::bronstein_charge_matches_bounded_reference_model` | Source-observed: the bounded expectation repeats the production `saturating_sub` operation. The passing test exercises those cases but adds limited independent arithmetic evidence; use separately derived boundary expectations or a structurally different model before making a stronger claim. No clock behavior was changed. |
| Missing readiness → `games/caro/src/lib.rs`, `games/werewolf/src/lib.rs` | Source-observed: absent implementations cannot satisfy gameplay/conformance/security readiness. Respect their phase and implementation gates. |

## Workflow validation

The [routing scenarios](issue-47-skill-routing.md) define expected reference/check/report choices
for six realistic requests. A separate agent used the assembled skills and raw sources, without
reading that expectation table or this pilot. It made no production edits and ran no game checks;
these are `source-read` selection trials, not live model-discovery or executed-test evidence.

| Evaluated request | Actual route and useful outcome |
|---|---|
| Shared API rule | Foundation design/verification/differential references; traced R2/R8 and ordered inputs to focused API/testkit checks and downstream Chess/Tiles conformance. Preserved consumer scope without requiring full-game audits. |
| Chess Bronstein boundary | Foundation plus Chess/rules-oracles; chose finite deadline boundary regressions and existing clock tests, and noticed the model-independence limit recorded above. |
| Tiles bag disclosure | Tiles/hidden-information plus property reference; selected secret suite, public/negative controls and viewer equivalence. Identified the synthetic short-bag evidence gap rather than reporting full secrecy readiness. |
| Caro readiness | Caro/SDK references; identified missing reducer/module/fixtures/manifest, left gameplay `NOT_IMPLEMENTED` and variant unresolved. |
| Partial Werewolf | Werewolf/SDK/hidden-information with type/verification references; distinguished creation/config/metadata tests from absent projections/reducer/runtime and future social/voice phases. |
| Tiles camera selection | Foundation/presentation guidance; selected local camera/input/noninterference checks and separated headless assertions from actual pixels/interaction. |

The review caught an incorrect vocabulary-owner link in the audit entrypoint; it now points to
the foundation's §6. Report provenance/status fields were also completed during review.
Structural checker `PASS`: two canonical entrypoints, valid bridge and local resource paths;
32 negative/positive fixture tests passed. Both skill-creator validators passed, six preserved
AI helper tests passed, CI YAML parsed, Rust formatting and `git diff --check` passed. No CI run
or branch-protection requirement is inferred from this commit's new local/CI checker wiring.

## Not run and residual scope

`NOT_RUN`: ignored depth-five perft, fresh native/browser interaction, screenshot capture/inspection,
WASM execution/vector comparison, integration with sockets/storage/chat/voice/mobile, remote CI,
branch-protection discovery, long fuzz/mutation/Kani campaigns and the full workspace core gate.
`NOT_APPLICABLE`: running those expensive or future-phase campaigns merely for a skills/docs
migration with unchanged game behavior. This bounded pilot supports the selected assertions and
workflow example; it does not certify release readiness or any future-phase feature. No Lean/Kani
theorem or whole-game verification claim is made.
