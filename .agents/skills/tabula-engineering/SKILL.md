---
name: tabula-engineering
description: Implement, review, or maintain Tabula engineering work by connecting architecture contracts and changed claims to ownership, suitable design, adequate evidence, and honest check results. Use for Rust behavior, shared contracts, repository tooling, and engineering documentation; specialized workflows reuse this foundation.
---

# Tabula engineering

This is the shared engineering workflow for Tabula. Technique references deepen
individual steps; they do not establish separate workflows or substitute for repository policy.
Use only the references the changed claim needs. Routine glue needs only its relevant checks.

## 1. Read the contract

Read the nearest `AGENTS.md` and
[doc 00](../../../docs/architecture/00-architecture-principles.md) before editing. Doc 00 wins
over other instructions in the repository. Follow its task-specific reading map, then inspect
the changed module's public types, invariant docs, tests, and implementations as needed.

State the intended observable behavior and named invariant before choosing a tool. For a bug,
retain the smallest failing example; for a new rule, name the input and expected consequence.
Do not infer a rule from the implementation that is supposed to satisfy it.

## 2. Locate ownership, trust, and phase

Identify the owner of the decision, its caller, and the boundary carrying its facts or effects.
For mixed code, sketch `raw input → trusted facts → pure decision → effects → shell`.

- Game meaning belongs in game rules; mechanism belongs in the platform (doc 00 §6).
- Canonical state and events stay server-side. `project` and `view_event` own disclosure (I-5/I-6).
- Dependencies follow `deps.toml`; platform dispatch goes through the registry (I-1/I-9/I-15).
- Inspect doc 07, current implementation, and `PHASE N` banners. A skeleton is a future seam,
  not authorization to implement a later phase. An invariant exception needs the ADR process.

Record which construction paths admit untrusted values: wire, storage, serde, migrations,
fixtures, defaults, macros, and public fields. Scope authorization evidence to its resource
and the state/version for which it remains valid.

## 3. Encode the rule in types and a pure core

Choose the smallest representation that prevents the relevant invalid states: private newtype,
explicit enum, raw/domain conversion, or scoped witness. Use typestate only when one stable
lifecycle axis justifies it. Read [types as proofs](references/types-as-proofs.md) for design,
and [boundary hardening](references/boundary-hardening.md) for conversion and bypass paths.

Pass resolved facts into rules rather than clocks, repositories, or framework handles. Return
domain results and effects; the shell performs I/O. Rules stay synchronous, total, and
deterministic. For `apply(&mut State, ...)`, validate before committing; rejection preserves
canonical bytes and the deterministic context obligations of the game contract. Read
[functional core](references/functional-core.md), or
[extraction recipes](references/extraction-recipes.md) when moving mixed code.

Avoid an abstraction, dependency, or new crate unless the task and phase need it. Keep game
state free of presentation state, wall-clock reads, OS randomness, floats, and observable
unordered iteration.

## 4. Choose adequate evidence

Maintain one small working ledger for the changed claims:

| Claim / invariant | Owner and failure mode | Oracle and domain | Check / status | Residual scope |
|---|---|---|---|---|
| rejected input is a no-op | reducer; partial mutation | canonical bytes and context before/after | named test / NOT_RUN | reachable states exercised |

Use the cheapest oracle that can detect the named defect. Examples suit small partitions;
enumerate a manageable finite domain; use external reference data when it exists; use a law
or independent model when examples cannot cover the space. Round trips and determinism can
both pass while a rule is wrong. Read
[verification selection](references/verification-testing.md) only when the choice is nontrivial.

Partition valid, invalid, boundary, hostile, and terminal cases from the representation. A
semantic generator must reach valid states; a robustness generator must deliberately admit
hostile input. A test must fail on a plausible violation of its claim. Never let failed setup,
rejected advertised-legal actions, or ignored invariant results silently turn a check green.

Scale evidence to the change. Tooling, documentation, metadata, and comments may need only
existing validation or a focused source review. Do not invent domain types, regression tests,
or runtime behavior for a reversible edit that changes none of those claims.

Before relying on a tool, distinguish:

| Capability | Evidence to inspect |
|---|---|
| Configuration | manifest, config, command wiring; presence alone proves no run |
| Implemented target | actual test, harness, corpus, or fuzz target for this claim |
| Local executable | installed command and required toolchain/target support |
| Executed check | exact invocation, selected cases, output, artifacts, and exit result |
| CI execution | actual workflow trigger, command, scope, and relevant run result |
| Merge enforcement | required status / branch rules, when access permits inspection |

Do not invent a missing target to satisfy a tool checklist or treat an unavailable tool as a
passing check. Local executable absence need not imply CI absence, and a CI YAML file does not
establish merge enforcement. Record inaccessible enforcement as unknown. Discover with file
reads and cheap capability probes; do not install tools or launch remote/scheduled campaigns
unless the task authorizes that work. Bound necessary local campaigns and preserve their output.

## 5. Implement and check the impact

For a regression, make the focused test fail on the old behavior, then implement the change.
For generated failures, commit the minimized deterministic case. Preserve public contracts,
dependency direction, phase gates, and rejected-operation transactionality as you change code.

Run the focused semantic check, then the affected crate/module and downstream checks justified
by changed contracts. A shared kernel, encoding, or public API change may require wider checks;
an isolated local rule need not trigger every expensive technique. Confirm test selection is
non-empty and that the assertions or corpus entries actually ran.

Run formatting and linting at the required scope. Before a PR, the repository's authoritative
portable core gate is `just check` (`cargo xtask check`); it owns its internal command order.
Do not replace that order with a skill's preferred sequence. CI also covers the feature matrix
and target builds. Follow game conformance and `SecretModel` requirements for game changes.
Run only existing, applicable gates: future-phase prose is not an implemented command.

Update ordinary rustdoc and relevant architecture documentation with behavior changes. Use
[AI doc contracts](references/ai-doc-contracts.md) for sparse durable law-to-evidence links when
they reduce future discovery cost. An annotation is a documented claim, not proof.

## 6. Report evidence and limits

Report what changed, the invariant, exact checks and results, and material remaining scope.
Keep outcome separate from evidence kind; use the narrowest accurate label:

| Evidence kind | Establishes within stated scope |
|---|---|
| `documented`, `source-read` | contract recorded, or implementation inspected; no execution implied |
| `type-enforced`, `statically-checked`, `compiled` | construction barrier, static rule, or selected build succeeds |
| `example-tested`, `integration-tested` | fixed semantic assertions, or the exercised real component boundary |
| `property-tested`, `differentially-tested` | sampled law, or agreement with an independent oracle |
| `mutation-tested`, `bounded-model-checked` | assertion sensitivity, or the stated proposition/domain/assumptions/bounds |
| `cross-target-tested` | executed comparable artifacts match on the named targets; a target build is `compiled` |
| `interaction-tested`, `headless-asserted` | exercised input behavior, or RenderList/headless assertions; rendered pixels are not implied |
| `screenshot-captured`, `screenshot-inspected` | image recorded, or visually reviewed for named criteria; capture alone implies no inspection |
| `production-observed` | the named behavior was observed on stated real traffic |

| Status | Meaning |
|---|---|
| PASS | applicable check executed successfully over the stated, non-empty selection |
| FAIL | check executed and found a mismatch, violation, or failed gate |
| BLOCKED | intended check could not execute because of an identified external/prerequisite condition |
| NOT_RUN | implemented check was not executed; state why and what remains uncovered |
| NOT_IMPLEMENTED | the claimed target, harness, or enforcement does not exist |
| NOT_APPLICABLE | the check does not address this changed claim; state the reason |

Compilation success, zero selected tests, ignored tests, empty corpora, or no-op setup cannot
produce a test PASS. A conditional claim needs evidence that its guarded case was reachable.
Name missing targets and unknown CI/enforcement explicitly; never promote planned tools into
evidence. Do not call a crate “formally verified.” State a harness's proposition, symbolic
domain, assumptions, reached functions, stubs, and bounds instead.

## Technique groups

| Group | Read when needed |
|---|---|
| Design | [types as proofs](references/types-as-proofs.md), [boundary hardening](references/boundary-hardening.md), [functional core](references/functional-core.md), [extraction recipes](references/extraction-recipes.md) |
| Verification | [selection](references/verification-testing.md), [strategy patterns](references/strategy-catalog.md), [properties](references/property-testing.md), [replay / differential](references/replay-differential-testing.md), [mutation](references/mutation-testing.md), [Kani](references/kani.md), [fuzzing](references/fuzzing.md) |
| Documentation | [AI doc contracts](references/ai-doc-contracts.md), [tag schema](references/ai-doc-schema.md) |

Specialized workflows add domain review targets and artifacts; this foundation retains ownership
of implementation discipline, capability discovery, check status, and evidence reporting.
