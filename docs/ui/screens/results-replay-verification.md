# Results and replay verification ledger

Issue [#52](https://github.com/loveoverflowcom/tabula/issues/52), 2026-10-03.
Implementation base: `develop @ eb7803b8a304f2be4ae9890ea6484dab02688540`.
The delivered changes are the [screen contracts](results-replay.md), an existing
canonical replay tooling correction, and an honest local completion action.
This ledger does not establish a persisted history or projected replay viewer.

[`tabula-engineering`](../../../.agents/skills/tabula-engineering/SKILL.md)
and [`tabula-game-audit`](../../../.agents/skills/tabula-game-audit/SKILL.md)
provide the evidence vocabulary. Source receipts below identify the final Rust
sources used by the local aggregate, focused execution and browser QA. Only
Markdown changed after those executions; remote CI is recorded in the issue
comment for the pushed commit.

## Scope and acceptance

| Claim / acceptance | Owner and failure mode | Oracle / evidence | Status and remaining scope |
|---|---|---|---|
| Results, history and generic replay have maintained screen contracts | Screens 09–11/shared boundary; sampled design facts mistaken for runtime facts | `source-read`: issue body and comments, docs 00/04/05/07, ADR-0028, pinned pack at `030da25d0098e240ab2cf36dacf9892e8b320a89`; actual result/history/mobile PNGs inspected | PASS as specification/design review; no desktop replay PNG exists in this pack |
| Completed local game remains distinct from fatal local stop | `LocalFeedback::sync_match`; timer/renderer/conflicting end effects relabeled as a rules outcome | `integration-tested`: real Chess timeout and whole Tiles match; actual `MultipleEndMatch` regression; stable fatal outranks completion, projected status/scores retained | PASS for local completion/fresh game; no full result document, persisted receipt or ratings |
| Completion action never mutates the accepted original | `LocalMatch`, pointer/keyboard ownership; inspecting final board emits canonical commands or repeats held activation | Final Chess/Tiles projection/outcome and attempted/accepted counts unchanged through inspection; generic stale-intent fixture additionally checks state hash; restart uses a new driver; held Enter/Space, repeated sync, pointer cancellation/resize/hidden-layout regressions | PASS in nonempty client tests; physical held-key and OS interruption acceptance still NOT_RUN |
| Canonical replay retains original attempt indices | Existing `.tbr` reader/writer/runner; gaps renumbered or rejected, changing deterministic RNG | Independent live create/apply fixture rejects attempts, accepts original indices `[2, 5, u64::MAX]`; stored-index RNG/hash/seek checks and compacted-index negative control | PASS: preserved format/version; `StateVersion`, seek and trailer count accepted frames; no local trace export or client playback added |
| Replay diagnostics make only supported claims | Checkpoint evidence and reproducer; numeric input adjacency mistaken for accepted-transition adjacency | Adjacent/sparse accepted checkpoint cases with original-index gaps; final hash/outcome diagnoses and reproducer byte/verification checks | PASS: accepted adjacency is private evidence; diagnostic IDs/reproducer preserve original indices |
| Seek enforces linked rules identity | `ReplayRunner::seek`; zero linked rules hash accepted while verify rejects it | Zero-hash regression fails on old source and passes after shared verdict guard; nonzero differing hash remains `CompatibleVersion` | PASS for canonical support tooling; projected playback remains unsupported |
| Authority and privacy stay with their owners | I-5/I-6/I-9/I-10/I-12, runtime feedback and replay tools; canonical state/seed or invented network facts copied into UI | `source-read` of changed consumers, existing hostile/parser/secret/conformance tests and aggregate architectural gates | PASS within changed boundaries; no new online authorization/projection adapter or hidden-information replay proof |
| M3 Expressive local action and keyboard recovery | Generated Theme/shared ActionButton; hidden final HUD, invisible focus or duplicate activation | `example-tested`/headless geometry in four schemes and three layouts; actual browser terminal/focus/restart inspection below | PASS in the measured scope; broad native/mobile/a11y certification remains open |
| Stored history → result → projected replay | Phase 4 authority/persistence/protocol, Phase 5 document shell, Phase 9 viewer/scrub/speed | Current scaffolds and ADR-0028 discovery/setup scope | BLOCKED by phase prerequisites; NOT_IMPLEMENTED, no fake rows/backend/cursor or AI reconstruction |
| Complete #52 acceptance and close issue | Real permissions, storage/resource errors, reconstruction, responsive/AT and interrupted/repeated future flows | Maintained contracts and deferred [bounded follow-up](../../work-plan/backlog/issue-52-history-replay.md) | BLOCKED; keep #52 open. #53 is a separate next chat and can reuse the generic contract without introducing a second replay controller |

## Regression and execution evidence

Pinned toolchain `1.96` resolved to Rust/Cargo **1.96.1** on this Mac.
Canonical replay focused commands used `--offline`; client focused commands
used the populated local dependency cache. Additional feature/target runs
used `--offline` and `RUSTFLAGS="-D warnings"`.

| Check | Final result | Retained local evidence |
|---|---|---|
| Canonical replay gap regression before correction | FAIL, exit 101: old contiguous validator rejects the valid gapped fixture | `/tmp/tabula-52-replay-gap-before.log` |
| Canonical seek identity regression before correction | FAIL, exit 101: old seek accepts an unreplayable zero linked hash | `/tmp/tabula-52-replay-seek-before.log` |
| Local terminal action regression against recompiled base | FAIL, exit 101: Tab passes through instead of activating completion focus | `/tmp/tabula-52-client-terminal-before.log`; isolated old-source fixture, no cached-build evidence counted |
| `cargo test --offline -p tabula-testkit --lib replay::tests` | PASS, 42 selected tests | `/tmp/tabula-52-replay-focused.log` |
| `cargo test --offline -p tabula-testkit --lib` | PASS, 59 tests | `/tmp/tabula-52-replay-testkit-lib.log` |
| `cargo clippy --offline -p tabula-testkit --all-targets -- -D warnings` | PASS | `/tmp/tabula-52-replay-clippy.log` |
| `cargo test -p tabula-game-client --all-targets` | PASS, 34 lib + 2 main + 15 integration = 51 tests | `/tmp/tabula-52-client-tests.log` |
| `cargo clippy -p tabula-game-client --all-targets -- -D warnings` | PASS after small main-loop helpers replaced an overlong function | `/tmp/tabula-52-client-clippy.log`; initial lint failure preserved separately |
| `cargo xtask replay tests/replays/<fixture>.tbr --verify --diagnose` for `chess-golden`, `chess-clock-golden`, `tiles-golden` | PASS, `EXACT` / `VERIFIED`, final hash/outcome agree, no divergence; 4/2/102 inputs and checkpoints | `/tmp/tabula-52-replay-{chess,chess-clock,tiles}-golden.log`; original golden files unchanged |
| Skill checker, checker fixtures, AI doc contracts | PASS: 2 entrypoints, 32 fixture tests, 6 doc tests | `/tmp/tabula-52-skills-check.log`, `/tmp/tabula-52-skills-tests.log`, `/tmp/tabula-52-ai-doc-tests.log` |
| `cargo xtask check` | PASS all nine ordered gates: fmt, clippy, tests, deps, no-game-ids, manifests, tokens, raw colors, deny; workspace 886 passed, 0 failed, 21 ignored (including doc tests) | `/tmp/tabula-52-gate.log`; 25 dependency rows, 28 manifests |
| `cargo check --offline --workspace --no-default-features` and `--all-features` | PASS both | `/tmp/tabula-52-features-none.log`, `/tmp/tabula-52-features-all.log` |
| `cargo nextest run --offline --workspace` | PASS, 884 passed, 2 skipped across 50 binaries | `/tmp/tabula-52-nextest.log`; skipped tests are not passing evidence |
| WASM client web feature check, wasm-release build, web target check | PASS | `/tmp/tabula-52-wasm-client-check.log`, `/tmp/tabula-52-wasm-client-release.log`, `/tmp/tabula-52-wasm-web-check.log` |
| Final production wasm-release rebuild and `cargo xtask stage-wasm-game` | PASS, WASM 845,109 bytes; bootstrap host and bundle staged | `/tmp/tabula-52-wasm-client-release-final.log`, `/tmp/tabula-52-stage-wasm.log` |
| Native game-client build | PASS (`compiled`, not native interaction proof) | `/tmp/tabula-52-native-build.log` |
| Markdown local links, whitespace and source receipt comparison | PASS: 11 changed Markdown files with no missing local links; `git diff --check`; all 173 Rust source hashes still match final UI QA receipt | Local link walk, SHA-256 receipts; no snapshot or generated-token expectations changed |
| Independent source reviews | PASS, no remaining functional findings; work-plan resume overclaim corrected | Agent replay-gap, local-completion and final specification review reports |

The first sandbox aggregate reached `deny` then failed on the read-only
advisory-cache lock. The authorized rerun executed the **entire** same gate
outside that restriction and passed; no gate was skipped or weakened. The
first client clippy attempt failed `too_many_lines`; it was fixed before the
final focused tests and aggregate. These failures are preserved, not counted
as passes.

Local logs and QA artifacts under `/tmp` are temporary machine evidence, not
committed CI artifacts. Source SHA-256 receipts are
`/tmp/tabula-52-replay-tested-sources.sha256`,
`/tmp/tabula-52-client-final-sources.sha256` and
`/tmp/tabula-52-ui-qa/tested-sources.json`. The exact additional commands,
exit codes and elapsed times are in `/tmp/tabula-52-additional-results.json`.

## Actual UI execution and limits

The early supported native CUA bind targeted a copy of the previous #51
binary solely to diagnose tooling. Although requested with a 10-second tool
timeout, it returned only after about 16 minutes. This is a tooling stall,
not final native UI evidence. Native interaction certification is BLOCKED;
no alternative OS automation or denial bypass was used.

The supported in-app browser remained responsive and exercised the actual
WASM renderer, runtime, presenters and shared components on loopback only.
A temporary QA executable copied the final production main loop with these
controlled changes: compile-time theme/game selection, a real 5-second Chess
timer, real bots in every Tiles seat, and a separate first-session stable
render-error feedback injection. No production fault flag or fixture route
was added. The fatal case proves the visible feedback/recovery binding, not
that a GPU failure was induced. All six final QA variant builds passed.
The source diff, original main, manifest, build logs and source receipt remain
in `/tmp/tabula-52-ui-qa`; earlier pre-final variants are not final evidence.

| Actual observed flow | Result / scope |
|---|---|
| Chess Light, 900 × 720 | PASS: real timer reaches `Game over / timeout`, final clocks 0:00 / 0:05; separate completion dock, filled primary action and final board visible |
| Light Tab → Enter | PASS: visible focus ring, fresh playable board at White 0:04 / Black 0:05; previous dock disappears |
| Dark, 390 × 610 and 568 × 320 | PASS: readable compact bottom dock and short-landscape side dock; no dock overlap with final status/clocks; visible Tab focus |
| Dark Escape then pointer New local game | PASS: Escape removes action focus but retains completion; click produces a fresh board and active clock |
| High Contrast Light, 900 × 720 | PASS: actual terminal board/dock and visible focus ring inspected |
| High Contrast Dark, 320 × 568 | PASS: actual terminal board/dock, fully visible button/copy and focus ring inspected |
| Light fatal feedback → Tab → Enter | PASS for controlled stable-error injection: `Local game stopped` / `The board could not be rendered`, distinct from terminal; focused recovery starts a fresh playable board |
| Tiles Light, 900 × 720 | PASS: real bot-completed match, final scores (28/22/36) retained, disabled gameplay actions and available camera controls above completion dock |
| Unmodified production WASM host, 900 × 720 | Board rendered at normal 5-minute clocks; pointer e2–e4 updates board and turn correctly. Console FAIL: repeated WebGL `already deleted texture ID 17` errors, also observed during preliminary baseline; no renderer/bootstrap source changed in #52. This smoke is not a clean-console certification |
| Tiles keyboard restart | Action sent; final screenshot is already completed again, so rendered restart transition is NOT_PROVEN in this fast all-bot harness. Real restart/reset assertions pass in integration tests |
| 320 × 320 terminal surface after resize settles | Known limit confirmed: full final board/status/clocks retained, completion dock suppressed until a usable layout fits. This is not full tiny-window action accessibility |

Screenshots of these actual states were captured and inspected through the
browser tool in the task transcript. Viewport/canvas dimensions were also
read from the page; an immediate resize screenshot can show the prior frame
scaled, so it was not used as the settled 320 × 320 oracle. Browser bootstrap
logs showed unused-plugin notices; no new error was observed in the final QA
variants. Historical errors from the preliminary baseline URL do not certify
or invalidate a different final variant.

Physical long key holds, real OS blur/minimize/reentry, browser/native Back,
touch/pinch/safe-area, 200% text/zoom, screen readers and Board Reader dispatch
are **NOT_RUN** here. The existing DOM canvas has no Board Reader bridge and
the page still restricts zoom; those inherited #51 gates remain open. Browser
viewport testing is not mobile touch evidence. Headless commands are not
font/pixel/AT proof, and compilation is not rendered proof. Do not carry this
bounded browser result forward as completion of #51 or #52's full UI gates.
