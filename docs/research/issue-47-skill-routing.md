# Issue 47: workflow routing scenarios

These are maintained routing expectations for
[issue #47](https://github.com/loveoverflowcom/tabula/issues/47), downstream of
[doc 00](../architecture/00-architecture-principles.md). They test the workflow's decisions,
not a particular heading or generated wording. They are check plans, not executed test evidence;
the [pilot report](issue-47-skills-pilot.md) records actual execution.

Every case starts at [tabula-engineering](../../.agents/skills/tabula-engineering/SKILL.md).
Game work composes [tabula-game-audit](../../.agents/skills/tabula-game-audit/SKILL.md).
References below are relative to those two skill folders.

| Realistic request | Expected references | Check plan and evidence to report |
|---|---|---|
| Change a shared `tabula-game-api` rule contract | Engineering `types-as-proofs`, `functional-core`, `verification-testing`; audit `sdk-conformance` plus rubrics for consuming games | Trace consumers, name changed invariant/owner, add deterministic regression evidence; run affected API/testkit tests and Chess/Tiles conformance. Assess Werewolf's implemented domain slice and Caro's absence separately. A file outside `games/` still affects games. Report exact features/targets and unresolved consumer coverage. |
| Fix Chess Bronstein delay at the timer boundary | Engineering `verification-testing`, `replay-differential-testing`; audit `chess`, `rules-oracles`, `sdk-conformance`, `replay-and-versioning` | Use the clock's stated rules and exact just-before/at/after assertions; run Chess clock/transition and clock replay checks. Rejected input preserves canonical bytes; timer cancellation and end effects matter. Small finite cases do not automatically require Kani or mutation. Report which clock modes/time boundaries ran. |
| Audit whether Tiles reveals future bag order | Engineering `property-testing`; audit `tiles`, `hidden-information`, `sdk-conformance` | Read `SecretModel` and existing security fixtures. Test containment and noninterference of `View` AND `ViewEvent` for seats/spectator; scramble only unauthorized bag order while preserving public counts/active tile. Assert event existence as well as payload. Report pure-function secrecy evidence with socket/timing channels untested. |
| Check whether Caro is ready to play | Audit `caro`, `rules-oracles`, `sdk-conformance` | Discover placeholder lib, missing rules/adapter/tests and unresolved variant/board size. Mark gameplay checks `NOT_IMPLEMENTED`, variant decision gap, readiness incomplete; do not choose freestyle or Renju, invoke unsupported selfplay, or call zero selected tests a pass. |
| Review Werewolf before its adapter is finished | Engineering `types-as-proofs`, `boundary-hardening`; audit `werewolf`, `sdk-conformance`, `hidden-information` | Check implemented config/role/initial-state rules and tests at current HEAD. Distinguish compiled metadata from missing `GameRules`/`GameModule`, projection and `SecretModel`. Full conformance/security/replay remain `NOT_IMPLEMENTED`; chat/voice/network are future-phase work. Do not infer enforcement from effect declarations. |
| Adjust Tiles camera selection or Chess board input | Audit `presentation-review` and selected game's rubric; engineering `verification-testing` if behavior changes | Drive input → command → observable result; run relevant presentation/headless tests and inspect real renderer pixels/interaction when available. Local camera/selection cannot change canonical state. Record actual viewport/target; separate RenderList assertions from screenshot inspection, interaction and WASM execution. A headless pass cannot certify real pixels. |

For each result preserve status (`PASS`, `FAIL`, `BLOCKED`, `NOT_RUN`, `NOT_IMPLEMENTED`,
`NOT_APPLICABLE`), oracle/evidence level, exact command/cwd/features/target, selected/executed
tests or workloads, logs, reproducible seed/config, and residual scope. Explain skips and
inapplicability. Source inspection and previous reports never become execution evidence at HEAD.

An independent agent should receive the request, skills and raw source, without this expectation
table, and produce its reference/check/report plan without side effects. Compare the actual plan
to these behavioral expectations; revise guidance only for a demonstrated routing gap. Record
the evaluated requests and conclusions in the pilot report.
