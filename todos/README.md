# Caro and Werewolf implementation plans

Planning only, based on latest remote/local develop `9e036cd` checked 2026-09-04. No implementation
code or normative architecture has changed. Doc 00 remains the contract. Recommended rule defaults
are explicit decisions for the first implementation PRs, not silently approved architecture changes.

## Contents

| Document | Purpose |
|---|---|
| [Reference games gap analysis](reference-games-gap-analysis.md) | Caro/Werewolf/Tiles status across 16 dimensions, source anchors and drift register |
| [Caro decisions](caro/00-decisions.md) | Eleven decisions: variant, size, overlines, blocked ends, opening, outcomes and timers |
| [Caro rules](caro/01-rules.md) | Types, construction barriers, full Input policy and independent win detector |
| [Caro verification](caro/02-verification.md) | L1–L16 claims, evidence levels, replay and automation |
| [Caro presentation/integration](caro/03-presentation-integration.md) | LocalMatch, snapshots, asset paths and browser selection prerequisite |
| [Caro PR sequence](caro/04-pr-sequence.md) | C1–C6 with scope/files/invariants/evidence/acceptance |
| [Werewolf decisions and phases](werewolf/00-decisions-and-phases.md) | Eighteen decisions, exact presets, simulation versus forbidden runtime substitution |
| [Werewolf rules](werewolf/01-rules.md) | Phases, timers, role actions, vote/death/win and scope values |
| [Werewolf secrecy/projections](werewolf/02-secrecy-and-projections.md) | Knowledge matrix, Some/None policy, noninterference and metadata leaks |
| [Werewolf verification](werewolf/03-verification.md) | V1–V20, canonical replay, headless projection demo, tool selection |
| [Future social/voice gates](werewolf/04-future-social-voice.md) | Explicit Phase-7/8 obligations; no early implementation |
| [Werewolf PR sequence](werewolf/05-pr-sequence.md) | W1–W10, buildable private kernels before complete module activation |

## First PR and critical paths

**Start Werewolf W1: decisions, knowledge policy and validated primitive types.** It removes the
largest uncertainty: role semantics, dead vision/Hunter interaction and hidden-observation rules
before the generic protocol freezes. Confirm Phase-2 exit evidence before implementation.

- Caro Phase 3: C1 decisions/types → C2 complete rules + identity + baseline conformance → C3 deeper
  verification/size → C4 bots/replay → C5 presentation/local client → C6 hardening/nightly/docs.
  Actual browser play requires generic app selector B1 if still absent; record its separate cost.
- Werewolf Phase 3: W1 decisions → W2 assignment/create → W3 phase/timers → W4 night → W5 vote/death/win
  → W6 projections → W7 events/SecretModel/complete module → W8 stronger evidence → W9 simulation,
  canonical replays and headless demo → W10 security hardening/measurements/docs.

Werewolf presentation/chat enforcement waits for Phase 7; voice waits for Phase 8. Phase-3 scope
values and headless output tests do not prove socket/SFU enforcement. Version/ack metadata needs a
separate generic ADR before Phase-4 wire freeze, preserving internal I-7.

## Reading evidence

Each ledger uses Claim / Failure mode / Cheapest oracle / Evidence level / Tier / Residual gap.
Tiers are Every PR, Nightly, Phase exit and Manual/security review. Unless explicitly reported as
executed below, ledger entries are **future evidence targets**, not completed verification.

Properties are primary for large sequence spaces and secrecy. Independent differential models are
primary for Caro win detection and Werewolf resolution. Tables/enumeration cover small partitions;
mutation checks assertion strength after kernels stabilize. Kani is not required, and typed rules
inputs do not justify byte fuzzers. WASM compilation is not cross-target behavioral evidence.

## Planning validation

- Remote develop equals the inspected commit.
- Only todos/ files were created/edited; no game/platform/CI implementation changes.
- The machine has no just executable. `justfile` maps `just check` directly to
  `cargo xtask check`; that exact underlying gate passed, including cargo deny. The initial sandbox
  attempt reached cargo deny but could not lock its external advisory cache; the permitted rerun passed.
- Existing nightly replay --all, fuzz/load and failure-artifact claims have source-level gaps;
  passing the local gate does not validate those scheduled jobs.

All thirteen requested documents are present. Follow individual acceptance criteria when implementing;
this planning completion does not declare either game or the whole Phase 3 complete.
