# Game audit: <game and ruleset>

Scope: <requested change/full audit/phase acceptance; layers and exclusions>.
Verdict: <finding-driven conclusion within that scope; never broadly "verified">.

## Provenance

- Game/ruleset: <package, selected variant/preset, rules version and hash>.
- Source: <branch/commit or other exact ref, dirty paths/content reviewed, diff scope>.
- Build selection: <features, native/WASM target and profile for each check>.
- Execution context: <cwd, toolchain/tool versions, relevant environment and full
  configuration including its source; per-check differences recorded below>.

## Implementation and phase status

| Surface | Implemented / partial / placeholder / deferred | Source and implication |
|---|---|---|
| <surface> | <status> | <maintained spec or actual code> |

## Findings

Order by impact. Separate reproduced defects, missing oracles, documented
deferrals, tooling limits and spec/code drift. If no defect was found, limit
that statement to the exercised scope.

| Severity / kind | Claim or invariant | Source location | Observed vs inferred behavior and impact | Evidence / complete reproduction | Next action |
|---|---|---|---|---|---|
| <critical/high/medium/low; defect/gap/deferral/drift/tooling> | <I-/R-/decision> | <file:line or symbol> | <observed/inferred; concrete consequence> | <check ID, command/config/seed or review artifact> | <smallest useful fix/check> |

## Evidence ledger

Use the shared engineering evidence/status vocabulary. Implementation status,
evidence level and execution result are separate fields.

| Check / layer / claim | Oracle and scope | Evidence level | Status and reason | Selected / executed counts | Log / artifact |
|---|---|---|---|---|---|
| <ID; claim> | <independent/shared/current-build/historical; bounds> | <documented/type-enforced/example-tested/etc.> | <PASS/FAIL/BLOCKED/NOT_RUN/NOT_IMPLEMENTED/NOT_APPLICABLE; reason> | <selected, executed, skipped/ignored/filtered, cases or matches where relevant> | <retained path/link> |

For each executed or attempted check, retain enough context for a full reproducer:

```text
Check ID:
Exact command:
Cwd:
Source ref and dirty scope:
Toolchain/tool versions:
Features / target / profile:
Relevant environment and complete configuration (including its source):
Seed(s), match/input coordinates, case/match/resource bounds:
Selected count / executed count / pass-fail-skip or filtered counts:
Exit status, canonical check status and reason:
Retained raw log and generated artifact paths:
Complete reproduction command and required input artifact/config:
```

For source-read or unexecuted claims, state the exact review path or missing
prerequisite and why execution counts are inapplicable. Zero executed cases,
ignored tests and unavailable targets cannot produce a test PASS.

## Replay and projection disposition

<When applicable: corpus identity/verdict, checkpoints/outcomes, viewer/secret/
phase coverage, containment/noninterference limits, migration disposition.
Otherwise mark NOT_APPLICABLE or NOT_IMPLEMENTED and explain.>

## Remaining uncertainty and next actions

<Ignored/filtered tests, unavailable tools/support, unexecuted targets,
browser/manual gaps, campaign bounds, deferred consumers and the smallest
next check. Do not turn omissions into passes or expand implementation scope.>
