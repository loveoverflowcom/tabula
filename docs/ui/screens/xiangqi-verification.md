# Xiangqi specification and verification ledger

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53), 2026-10-03.
Inventory and execution base:
`develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`.
Delivered scope: specification slice A, [shared boundary](xiangqi.md), screens
[08](08-xiangqi.md), [11 extension](11-xiangqi-analysis.md),
[12](12-learn.md), [14](14-resources.md), and bounded deferred work.
No Rust, token, dependency, phase banner, game registration or production
runtime was changed. The existing supported consumers were inspected; no
concrete #53 defect justified an unrelated code patch.

[Tabula engineering](../../../.agents/skills/tabula-engineering/SKILL.md) and
[game audit](../../../.agents/skills/tabula-game-audit/SKILL.md) own status and
evidence vocabulary. A passing baseline cannot establish Xiangqi acceptance.

## Claims and acceptance

| Claim / invariant | Owner and failure mode | Oracle / evidence | Status and residual scope |
|---|---|---|---|
| Four screens follow actual design and current source | Shared specs; mock facts promoted to functionality | `source-read`: #53, #48 + ownership comment, docs 00/02/04/05/07/09, ADR-0028, pinned 05-xiangqi source/notes and foundation mapping | PASS as documented contract; no executable Xiangqi surface |
| M3 hierarchy, tonal grouping and deliberate primary/secondary shapes | Foundation/presenter/shell; decorative borders, uniform emphasis or copied raw palette | All three supplied PNGs inspected; Learn/Resources SVG source inspected; specs reuse generated tokens/components, 44 dp wrappers and four schemes | PASS as specification/reference inspection; runtime rendered acceptance BLOCKED |
| One rules/projection/replay owner and immutable original | Game/local driver; candidate/tutor overwrites accepted match or leaks canonical state | Shared proposed boundary explicitly preserves I-1/I-5/I-6/I-9/I-10/I-12/I-15, generic replay and branch gate; public-safe identity, no Audit/seed export | Documented; Xiangqi rules/branch tests NOT_IMPLEMENTED |
| Current-position evidence, cancel and stale isolation | Host/evidence consumer; same-board old response reapplied to new request | Contract includes session/artifact/rules/viewer/cursor/branch/revision/digest/resource/budget/request generation; retirement before transport cancel, including return to same digest | Documented; actual worker/integration tests NOT_IMPLEMENTED |
| No-engine/no-LLM and engine/book evidence stay distinct | Tutor/game/host; plausible prose substitutes for finite engine search or citation proves motif | Explicit unavailable/platform matrix, tiered template fallback after evidence gate, abstention/quality oracle and source-specific references | Documented; tutor quality/fallback execution BLOCKED by missing consumers |
| Fair-play is more than hidden UI | Authority; direct request, imported competitive position or cached output bypasses policy | Mode/policy matrix requires authorization before job and disclosure; default denial and permission-loss invalidation | Documented; real online enforcement NOT_IMPLEMENTED at Phase 4 |
| Exact resource readiness and rights | Resource host; hash or code license treated as executable/weights permission | Separate binary/weights identities, rights/integrity/compatibility/budget/probe/offline facts and atomic previous-pack retention; upstream primary source review 2026-10-03 | Documented; no selected/licensed/provisioned pack and no real installer/probe |
| Runtime code remains behind its explicit gate | Architecture/#48; directory/skeleton mistaken for permission | No `games/xiangqi`; No ML and Phase-9 deferral remain; ADR-0028 opens only discovery/setup; existing registry negative assertion executed | PASS for current boundary; #48 vision/ADR and rules gate BLOCKED |
| Complete #53 acceptance / close issue | All actual adapters, authority and UI consumers | Specifications, current source and tests below | BLOCKED; keep #53 open. Spec slice A is complete, B/C are deferred separately |

## Reference inspection and retained evidence

Design provenance is
[`030da25d0098e240ab2cf36dacf9892e8b320a89`](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi),
extracted with `git archive` to `/tmp/tabula53-design`. All three supplied
PNG files are 1440 × 1000 pixels; all five SVGs parse as XML. The desktop
Play/Analyze PNGs and mobile/state PNG were visually inspected, as were
Learn/Resources SVG sources. No desktop PNG is supplied for Learn/Resources;
none was fabricated or labeled as a runtime screenshot.

Inspection found gaps resolved by the contract: compact Learn selection and
full Analyze layout are missing from the mock; mobile board intersections
are roughly 32 dp apart and cannot prove 44 dp interaction targets; a visible
Cancel needs a real active job; checksum/transfer does not suffice for Ready;
sample clocks, PV arrows and CPU percentages cannot become authority/metrics.
SHA-256/dimensions/XML receipts and final Markdown local-link checks are in
`/tmp/tabula-53-checks/design-doc-receipt.json` and `doc-design-check.log`.
These are temporary local evidence; pinned source links preserve provenance.

## Executed existing baseline checks

Working directory: `/Users/manhblue/Documents/Codex/2026-10-03/task-5/tabula`.
Rust/Cargo **1.96.1**, native target `aarch64-apple-darwin`, WASM target
`wasm32-unknown-unknown`. Commands used `CARGO_TARGET_DIR=/tmp/tabula53-target`
and `CARGO_TERM_COLOR=never`; `RUSTFLAGS`/`CARGO_NET_OFFLINE` were unset.
The owned output cache was copied with APFS cloning from the clean #52
checkout; that checkout was not written. Receipts confirm all tracked build
inputs remained byte-identical throughout execution; only Markdown changed.

All log names below are under `/tmp/tabula-53-checks/`.

| Exact check | Result / scope | Log |
|---|---|---|
| `cargo test -p tabula-registry` | PASS, 24 tests; 3 doctests ignored; catalog exactly Chess/Tiles and unknown Xiangqi returns None | `01-registry.log` |
| `cargo test -p tabula-testkit replay::tests` | PASS, 42 selected, 0 ignored; other tests filtered; existing canonical replay/index/identity only | `02-replay.log` |
| `cargo xtask check` | PASS in full original nine-step order; 886 test/doc-test passes, 0 failures, 21 ignored; deps 25 crates, manifests 28; deny advisories/bans/licenses/sources OK | `14-xtask-check-escalated.log` |
| `cargo check --workspace --no-default-features` | PASS, compiled | `04-features-none.log` |
| `cargo check --workspace --all-features` | PASS, compiled | `05-features-all.log` |
| `cargo check -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web` | PASS, compiled existing client | `06-client-wasm-check.log` |
| `cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release` | PASS, compiled existing client | `07-client-wasm-release.log` |
| `cargo xtask stage-wasm-game` | PASS after exact artifact placement; staged 845,109-byte WASM to `target/tabula-web-game` | `15-stage-wasm-retry.log`, `15-stage-copy-receipt.json` |
| `cargo build -p tabula-game-client --features native` | PASS, compiled existing client | `09-client-native.log` |
| `cargo check -p tabula-web --target wasm32-unknown-unknown` | PASS, compiled existing discovery shell | `10-web-wasm-check.log` |
| `/tmp/tabula-51-python/bin/python .agents/skills/tabula-engineering/scripts/check_skills.py` | PASS, 2 maintained entrypoints/resources/bridge | `16-skills-retry.log` |
| Same Python + `test_check_skills.py` / `test_ai_doc_contracts.py` at that scripts path | PASS, 32 / 6 tests | `17-skills-tests-retry.log`, `18-ai-doc-tests-retry.log` |
| Final local Markdown link walk and `git diff --check` | PASS; no missing local links or whitespace errors | `doc-design-check.log` |
| Independent gate, design and final source/spec reviews | Gate/design findings incorporated; no unresolved finding at publication | Task review reports and the delivered contracts |

The first aggregate failed at `deny` because the sandbox could not take the
exclusive advisory-cache lock at `/Users/manhblue/.cargo/advisory-dbs/db.lock`.
The authorized rerun executed the entire unchanged gate and passed; no check
was skipped or weakened. Original `03-xtask-check.log` is retained.
The first staging call could not find workspace `target` because that tool
does not consume the custom target path. The exact newly built artifact was
copied to its expected path, hash equality recorded, and staging passed;
original `08-stage-wasm.log` is retained. Default `python3` lacked PyYAML;
the existing Python 3.13.5/PyYAML 6.0.3 environment ran the same scripts
successfully, with original setup failures retained in `11`/`12` logs.

Machine-readable commands/exits/durations are in `results.json`,
`14-xtask-check-escalated-result.json` and `setup-retry-results.json`.
`before-source-receipt.json`, `final-source-receipt.json` and
`source-comparison.json` bind checks to unchanged build inputs. Ignored and
filtered tests are not passing evidence. Local nextest was NOT_RUN; the
authoritative aggregate uses Cargo tests. Remote SHA/CI for the published
commit is recorded in the issue evidence comment.

## Runtime limits and next handoff

Xiangqi engine/rules/conformance/branch/quality/resource integration checks
are NOT_IMPLEMENTED, not green tests. Native/browser Xiangqi interaction,
screenshots, four themes, repeat/interruption, recovery, touch/safe areas,
200% zoom, IME and AT are BLOCKED by missing consumers. No native CUA call
was attempted for an absent game. Native/WASM compilation and staging are
not runtime or cross-target byte-equality proof.

Inherited #51/#52 UI limits remain open: board targets below 44 dp in some
layouts, no real network/Board Reader bridge, restricted browser zoom,
uncertified AT/OS interruption, prior native CUA stall, and WebGL deleted
texture errors observed in #52's browser run. These were not re-executed or
fixed by this documentation change; see
[the prior ledger](results-replay-verification.md). No fresh clean-console
claim is made.

Deferred #53 slices are [board/modes](../../work-plan/backlog/issue-53-board-modes.md),
[one engine/resource adapter](../../work-plan/backlog/issue-53-engine-evidence.md)
and [Learn](../../work-plan/backlog/issue-53-learn.md). #48 first resolves
vision/ADR/ruleset/No ML/plugin ordering; the specs do not reopen those gates.
The queue continues with **#59 in a separate chat**, then #60. #59 should
re-pin remote `develop`, reuse verified asset bytes and address bounded decode,
texture/cache ownership, both Sprite validation/execution and real Tiles
native/browser evidence. Preserve #52 input-index/seek-identity corrections
and original golden fixtures; investigate the prior deleted-texture error as
a renderer/cache concern. No #59/#60 implementation, deployment, force-push
or protection bypass was performed here.
