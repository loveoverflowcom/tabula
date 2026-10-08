# Agent workflows

Four canonical skills organize work by workflow. Architecture
[doc 00](../../docs/architecture/00-architecture-principles.md) and ADRs own the contract;
skills apply it and never authorize work past a phase gate.

| Workflow group | Entrypoint | Load on demand |
|---|---|---|
| Engineering: contract, design, implementation, evidence, handoff | [tabula-engineering](tabula-engineering/SKILL.md) | Design/prevention: types, boundary hardening, functional core, extraction. Verification: examples, properties, replay/differential, mutation, Kani, fuzzing. Documentation: AI contracts/schema/helper. |
| CMP engineering: shared mobile UI, state/lifecycle, navigation, host seams and inspected preview | [tabula-cmp-engineering](tabula-cmp-engineering/SKILL.md), composing engineering | Ownership/conventions, native design/parity, existing official Hot Reload MCP interaction, targeted tests and evidence boundaries. |
| Existing-change review: PR, range, patch or local tree; read-only findings and assurance gaps | [tabula-code-review](tabula-code-review/SKILL.md), composing engineering and relevant game criteria | [Changed-boundary lenses](tabula-code-review/references/review-boundaries.md), [report template](tabula-code-review/assets/review-report-template.md). |
| Game review: rules, SDK, security, replay, bots, presentation, readiness | [tabula-game-audit](tabula-game-audit/SKILL.md), composing engineering | SDK, rules oracles, hidden information, replay/versioning, presentation and the selected game's rubric. |

Choose the workflow first, then the smallest relevant reference set. The engineering skill
owns evidence vocabulary and implementation order; CMP engineering applies it to shared
mobile presentation and the existing preview loop, while game audit applies it to separate
game claims. Code review owns pinned-diff scope, base-relative candidates, severity/confidence and its
report; it reads existing owners as criteria rather than starting their implementation loops.
SDK conformance, correct rules, secrecy, deterministic replay and playable UI need
their own evidence. A local game audit does not certify networking or future-phase features.

## Discovery and one source

Maintain content only in `.agents/skills`. `.claude/skills` is a relative directory symlink to
that tree, so both filesystem entrypoints resolve identical skill files, references and scripts.
Root [AGENTS.md](../../AGENTS.md) routes engineering, code-review and game work; [CLAUDE.md](../../CLAUDE.md)
provides explicit fallback routing for a runtime that does not traverse the symlink.

The Codex session used for this migration discovered the previous nine skills in `.agents`.
The checker verifies the final bridge, metadata and resource paths. A fresh Codex session and
Claude's live skill inventory still need runtime inspection: filesystem resolution does not
prove runtime registration, and this migration does not claim either runtime was restarted.

The former draft essays have been removed after their functional-core, type-design and
verification topics were consolidated in the references below. Git history retains the research;
do not recreate a parallel instruction tree. [The documentation index](../../docs/README.md)
separates maintained contracts from historical reports and scoped evidence.

## Migration map

The nine former skills are now technique references under the engineering workflow, with no
remaining standalone skill metadata or competing verification router. Old `$rust-*` invocations
should use `$tabula-engineering` with the named technique. Git history retains earlier content.

| Former `.agents/skills/` path | Canonical replacement |
|---|---|
| `rust-verification-testing/SKILL.md` | [verification-testing.md](tabula-engineering/references/verification-testing.md) |
| `rust-verification-testing/references/strategy-catalog.md` | [strategy-catalog.md](tabula-engineering/references/strategy-catalog.md) |
| `rust-types-as-proofs/SKILL.md` | [types-as-proofs.md](tabula-engineering/references/types-as-proofs.md) |
| `rust-types-as-proofs/references/boundary-hardening.md` | [boundary-hardening.md](tabula-engineering/references/boundary-hardening.md) |
| `rust-functional-core/SKILL.md` | [functional-core.md](tabula-engineering/references/functional-core.md) |
| `rust-functional-core/references/extraction-recipes.md` | [extraction-recipes.md](tabula-engineering/references/extraction-recipes.md) |
| `rust-property-testing/SKILL.md` | [property-testing.md](tabula-engineering/references/property-testing.md) |
| `rust-replay-differential-testing/SKILL.md` | [replay-differential-testing.md](tabula-engineering/references/replay-differential-testing.md) |
| `rust-mutation-testing/SKILL.md` | [mutation-testing.md](tabula-engineering/references/mutation-testing.md) |
| `rust-kani/SKILL.md` | [kani.md](tabula-engineering/references/kani.md) |
| `rust-fuzzing/SKILL.md` | [fuzzing.md](tabula-engineering/references/fuzzing.md) |
| `rust-ai-doc-contracts/SKILL.md` | [ai-doc-contracts.md](tabula-engineering/references/ai-doc-contracts.md) |
| `rust-ai-doc-contracts/references/schema.md` | [ai-doc-schema.md](tabula-engineering/references/ai-doc-schema.md) |
| `rust-ai-doc-contracts/scripts/` | [engineering scripts](tabula-engineering/scripts/) |
| Independent `.claude/skills/rust-functional-core/` and `rust-types-as-proofs/` | Removed after reviewing unique extraction/serde material; useful guidance is folded into canonical design references. `.claude/skills` now shares the canonical workflow skills. |

The AI contract helper retains the `rust-ai-doc-contracts` graph schema identifier for output
compatibility. That identifier is data, not a discoverable skill or an old invocation path.

Historical `rust-*` citations inside canonical `src/rules` comments remain as migration-map
keys: even changing a comment there changes `RULES_HASH`. The skill-only migration leaves
those subtrees and committed goldens byte-identical; callers outside hashed rules use the new
reference names. These citations do not retain old skill definitions or entrypoints.

## Validation

From the repository root, with Python 3 and PyYAML available (CI installs the latter):

```bash
python3 .agents/skills/tabula-engineering/scripts/check_skills.py
python3 .agents/skills/tabula-engineering/scripts/test_check_skills.py
python3 .agents/skills/tabula-engineering/scripts/test_ai_doc_contracts.py
```

The checker rejects missing/malformed metadata, duplicate definitions, independent Claude copies,
broken local links and stale helper paths. Negative fixtures show these failures are detected.
It checks filesystem discovery surfaces, not a running model's selection or rule correctness.
The CI `skills` job runs these checks; required merge-gate configuration is a separate fact.

[Routing scenarios](../../docs/research/issue-47-skill-routing.md) cover a shared-rule edit,
Chess clocks, Tiles secrets, Caro, partial Werewolf, and presentation.
[The pilot](../../docs/research/issue-47-skills-pilot.md) records attributable local evidence.

When changing guidance, keep one home per rule, retain useful helpers, use maintained repository
examples, and state evidence scope. Load substantial detail through references instead of adding
ceremonial entrypoints. Never regenerate goldens or weaken an invariant to make an audit pass.
