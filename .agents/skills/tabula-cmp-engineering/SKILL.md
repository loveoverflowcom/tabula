---
name: tabula-cmp-engineering
description: >-
  Implement, refactor, test and inspect Tabula Compose Multiplatform mobile UI,
  navigation, presentation state and host seams in apps/mobile. Uses the existing
  Desktop Preview and official Hot Reload MCP for semantic interaction and inspected
  renders. Compose tabula-code-review for read-only diff reviews; Rust game rules
  and Web-only work stay with their owning workflows.
---

# Tabula CMP engineering

Own the shared mobile UI change through an inspected, reproducible result. Compose
[tabula-engineering](../tabula-engineering/SKILL.md) for contract, phase discipline
and evidence vocabulary. Read root [AGENTS.md](../../../AGENTS.md),
[doc 00](../../../docs/architecture/00-architecture-principles.md),
[mobile instructions](../../../apps/mobile/AGENTS.md) and the
[mobile README](../../../apps/mobile/README.md) before editing. Use doc 04 and the
relevant mobile ADR/screen contract for the changed boundary.

For an existing PR, range, patch or local-diff review, start with
[tabula-code-review](../tabula-code-review/SKILL.md). These references supply CMP
criteria; review does not enter the implementation or live-edit loop. Compose
[tabula-game-audit](../tabula-game-audit/SKILL.md) when the change also affects a
game, Rust presentation or a shared game contract.

## Choose the relevant references

| Changed claim | Read |
|---|---|
| Screen/component API, package decomposition, state, effects, navigation or native port | [Ownership and conventions](references/ownership-and-conventions.md) |
| Layout, tokens, branding, localization, accessibility or Web parity | [Design and visual review](references/design-and-visual-review.md) |
| Interactive UI, selectors, agent-driven preview or live Kotlin reload | [MCP preview inspection](references/preview-inspection.md), then the maintained [agentic coding guide](../../../apps/mobile/AGENTIC-CODING.md) |
| Test selection, target builds and completion evidence | [Testing and evidence](references/testing-and-evidence.md) |

## Development loop

1. **Pin the actual consumer.** Record source SHA and local patch, shared production
   composable/callers, source sets, state/resource owners and the observable claim.
   Name the failure mode and cheapest oracle. For a refactor, sketch before/after
   dependencies and lifetimes with an old→new symbol/path map; folder moves alone
   do not establish separation of responsibilities.
2. **Resolve ownership.** Reuse `shared` and `previewApp`. Kotlin/Swift own shell
   UI/navigation and permitted device services; Rust owns rules, projection,
   replay, presenter and rendering. Generated tokens/catalog remain at their
   existing source owners. Check the current ADR and real adapter before enabling
   any affordance: discovery data and preview doubles grant no runtime authority.
3. **Implement at that seam.** Bind state/effects at the route or existing lifetime
   owner; reusable content takes values/events. Preserve focus, scroll, stable
   identity, Back ordering and stale-result fencing. Introduce a port, layer or
   dependency only for a concrete gap, with every affected target checked.
4. **Test the changed behavior.** Select pure state/port tests for decisions and
   shared production-component tests for actions/semantics. A build, cached report
   or empty filtered suite is not a test pass. Use the existing mobile checks;
   choose native/device evidence when the platform boundary changes.
5. **Inspect, interact and recheck.** For visual or interactive changes, launch the
   existing preview and discover MCP schemas. Inspect the current semantic tree,
   interact through current tagged nodes, edit and reload, then assert the intended
   result in a fresh tree. Capture and **open** the relevant screenshots, record
   findings, fix the responsible owner and rerun the same scenario. A reload or
   click response alone does not prove the UI changed. Preserve the before image
   and reference for parity work; never fabricate a product fix for a skill pilot.
6. **Deliver scoped evidence.** Report changed behavior, checks and remaining
   scope using the foundation's statuses. Separate host assertions, live MCP
   interactions, inspected pixels, platform compilation/linking and device
   acceptance. Keep raw output in existing ignored build reports; a committed
   verification ledger is reviewed Markdown with bounded excerpts and hashes.

The [agentic coding guide](../../../apps/mobile/AGENTIC-CODING.md) owns commands,
registration, toolchain recovery and the smoke client. This skill adds engineering
decisions, not another screenshot runner, MCP server or application shell.

## Maintaining the specialization

Adapted from VOT's CMP ownership, component, visual-review and evidence practices;
Tabula's architecture and current tooling own the resulting workflow. Do not copy
VOT-specific build commands, token sources, WebView or provider architecture.
For skill-only edits outside `apps/mobile`, run the skill metadata/link checker,
its negative fixtures and the creator validator. Changes under `apps/mobile`,
including documentation, retain the repository's required mobile gate.
Validate routing and realistic decisions independently when guidance changes
substantially. Such evaluations do not establish new UI or device execution.
