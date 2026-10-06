# Code review: <change>

## Scope and assessment

- Comparison: <BASE/HEAD SHAs, merge-base/range; or patch/local content identities>
- Reviewed: <files, claims, consumers, target/features>
- Excluded/unavailable: <surfaces and why; truncated/binary/untracked coverage>
- Assessment: <finding-driven conclusion within this scope>
- Residual risk: <material uncertainty; no blanket approval or phase-exit claim>

## Findings

Order by impact and combine shared root causes. If none, say "No actionable
findings in the reviewed scope" and retain coverage/limits. Use the
[review labels](../SKILL.md#4-validate-candidates-against-base).

### <P0–P3> <specific consequence>

- Class / confidence: <DEFECT, ASSURANCE_GAP, CONTRACT_DRIFT> / <HIGH, MEDIUM, LOW>
- Changed location: <path:line range and symbol in pinned source>
- Claim / owner: <authoritative contract/invariant/gate and owning component>
- Trigger / impact: <input/sequence, target/features, affected consumer>
- Expected / actual: <contractual result and observed/inferred result>
- Evidence: <complete source trace or command, fixture/seed/config and artifact>
- BASE comparison: <introduced/worsened/newly exposed; causal changed behavior>
- Next action / remaining premise: <smallest fix/check and unresolved assumption>

Missing evidence is not itself a production defect. For a blocking gap cite its
existing requirement/requested acceptance claim. Separate optional improvements
and unchanged pre-existing problems.

## Changed-claim ledger

Use meaningful claims only; a non-behavioral change can need one sentence. Reuse
[engineering evidence/status vocabulary](../../tabula-engineering/SKILL.md#6-report-evidence-and-limits).

| Claim / source | Owner / consumer / build selection | Barrier and construction paths | Evidence / input freshness | Disposition / residual |
|---|---|---|---|---|
| <requirement distinct from observation/assumption> | <symbol, target/features> | <checked paths and open bypass> | <oracle, kind/status, source/config identity> | <finding ID, supported scope or next check> |

## Checks and coverage

| Check / claim | Exact invocation and selection | Evidence kind / status | Result / scope | Artifact / limitation |
|---|---|---|---|---|
| <check or source trace> | <command, cwd, source, target/features/profile, relevant config> | <engineering labels> | <exit result, selected/executed/skipped counts or bounds> | <log/fixture; missing prerequisite or residual> |

Record toolchain/tool versions and seeds/config when they affect reproduction.
Reused results need source, harness/oracle and relevant input identities plus why
they still apply. Separate compiled, semantic tests, real integration, inspected
screenshots and shipping-target execution. Zero cases, ignored checks or setup
failure cannot support a test PASS.

## Open questions and next checks

<Named unresolved premises, relevant pre-existing issues, unknown enforcement,
required unrun/blocked gates, unexercised consumers and smallest proportional
next checks. Do not manufacture work to fill this section.>
