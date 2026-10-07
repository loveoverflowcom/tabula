---
name: tabula-game-audit
description: Audit a Tabula game or game change against the SDK contract, independent rules oracles, projection security, deterministic replay, and presentation boundaries. Use for game reviews, regression investigations, release or phase acceptance, or changes that materially affect these guarantees; scale the audit to the requested impact rather than requiring a full audit for ordinary small edits.
---

# Tabula game audit

Produce an evidence-backed review of the requested game or change. A passing
test suite supports its exercised claims, not a declaration that the game is
correct or a phase is complete.

Read the repository's `AGENTS.md` and
[architecture principles](../../../docs/architecture/00-architecture-principles.md)
first. Use [Tabula engineering](../tabula-engineering/SKILL.md) for shared
functional-core, type, and verification techniques. Its
[evidence and limits section](../tabula-engineering/SKILL.md#6-report-evidence-and-limits)
owns the evidence/status vocabulary; do not invent a competing scale here.

## Select scope before checks

1. Identify the game, revision/diff, requested audit depth, current phase, and
   claims at risk. Inspect `Cargo.toml`, exported modules/traits, actual test
   targets, and the maintained game specification. Classify each relevant
   surface as **implemented**, **partial**, **placeholder**, or **deferred**
   with a source path. A declared feature or capability is not an implemented
   consumer.
2. Load the game rubric below and only the layers relevant to the request.
   A narrow fix may need one layer and one regression check; a full-game or
   phase review normally covers all applicable layers and records exclusions.
   Do not implement future phases to make an audit pass.
3. Map claims to existing oracles and meaningful commands. Inspect fixture
   reachability, skips, ignored tests, filters, and feature gates. Distinguish
   unavailable support from a passing check and a failing implementation.
   Report zero selected tests as **not exercised**.
4. Run proportionate checks. Record exact command, cwd/toolchain,
   features/target, relevant environment, seed/full configuration,
   selected/executed counts, bounds, and retained logs/artifacts. Review code
   where automated evidence has gaps. Follow repository gates for tasks that
   include implementation or a commit. Keep audit-only work read-only unless
   fixes or an explicit output artifact were requested.
5. Report findings by impact, connect each to a rule or invariant, identify
   the source location and observed vs inferred behavior, and give a complete
   reproduction or evidence path. State remaining uncertainty and the
   smallest useful next check. Use the
   [report template](assets/audit-report-template.md) for requested report
   artifacts; a chat review can use its fields without creating an unrequested
   file.

## Audit layers

| Layer | Read when | Reference |
|---|---|---|
| SDK contract | Module wiring, configuration, inputs/effects, legality hints, fixture adequacy | [SDK conformance](references/sdk-conformance.md) |
| Rules correctness | Legality, scoring, outcomes, algorithm changes | [Rules oracles](references/rules-oracles.md) |
| Security | Hidden information, projection/event changes, spectator/seat knowledge | [Hidden information](references/hidden-information.md) |
| Determinism and history | RNG, canonical state, rules identity, replay, migration | [Replay and versioning](references/replay-and-versioning.md) |
| Presentation | Intent construction, rendering, input/camera/animation, accessibility | [Presentation review](references/presentation-review.md) |

## Game rubrics

| Game | Audit entry |
|---|---|
| Chess | [Complex legality, clocks, perft, public projections](references/chess.md) |
| Caro | [SDK-friction benchmark; currently a design placeholder](references/caro.md) |
| Tiles | [Spatial legality, graph/scoring oracle, bag secrecy, camera](references/tiles.md) |
| Werewolf | [Partial headless foundation, phase and knowledge boundaries](references/werewolf.md) |

For another game, derive the rubric from its maintained spec and implemented
capabilities. Reuse these layers instead of a new audit framework. Per-game
references locate authorities and sensitive edges; they are not second
rulebooks. Recheck their current-status statements against the checkout.

## Evidence that must not be overstated

- `conformance!` excludes the hidden-information suite. Its legality check
  currently examines `Enumerated` at fixture endpoints, not `Hints` or every
  reachable state.
- Self-play and same-build replay detect divergence and broken transitions;
  they are not independent rulebook oracles. Historical goldens add a separate
  claim, whose rules identity/verdict must be checked.
- Containment scans cannot rule out derived or event-existence leaks.
  Noninterference needs a real secret change and an observability control.
- Render-list snapshots, WASM compilation, and headless runs do not establish
  browser/native visual quality or cross-target byte equality.
- Missing tools, unsupported games, planned CLI commands, empty fixtures, and
  filtered-out/ignored tests are never passing evidence. Preserve failures and
  original goldens; do not regenerate expectations to make a review green.
