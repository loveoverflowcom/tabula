# Tabula documentation

Start with [architecture doc 00](architecture/00-architecture-principles.md).
It owns invariants and the ADR register; other documents and agent skills apply that
contract. A specification or successful local check does not open a phase gate.

| Need | Maintained source |
|---|---|
| Architecture, ownership, stack and phase gates | [Architecture index, docs 00–09](architecture/README.md) |
| Recorded decisions and supersession | [ADR directory](adr/README.md), with the register in doc 00 §10 |
| Engineering workflow and technique references | [Canonical skills](../.agents/skills/README.md): `tabula-engineering`, composed with `tabula-game-audit` for game work |
| Game rules, variants and information models | [Per-game notes](games/README.md), plus source and conformance fixtures |
| Shared screen contracts and generated token adapters | [UI index](ui/README.md), [screen index](ui/screens/README.md); authored tokens live in [`tokens.toml`](../tokens.toml) |
| Upcoming slices and prerequisites | [Work queue](work-plan/README.md); linked GitHub issues own acceptance |
| Executed checks, provenance and remaining target evidence | [Verification index](verification/README.md); [verification tools](../verification/README.md) describe optional campaigns |
| Performance workloads and measurements | [Performance index](perf/README.md) |
| Renderer/embedding investigation | [Issue 60 RFC](rfcs/issue-60-renderer-embedding.md), [ADR-0029](adr/0029-renderer-embedding-spike.md) and its [execution ledger](verification/issue-60/README.md) |
| Earlier audits and workflow evaluation | [Research index](research/README.md); results apply to their recorded source refs |
| Retired Phase-0 prototype | [Tic-Tac-Toe retirement note](legacy/tictactoe.md); full historical material remains in Git |

## Current platform direction

Web uses a Leptos shell and a separate Rust/Macroquad WASM gameplay document
(ADR-011). The bounded opt-in local handoff is ADR-0030. Android/iOS use a
Compose Multiplatform shell in [`mobile/`](../mobile/README.md); ADR-0032 opens
only its foundation. WebView gameplay, voice and native services need their own
implementation and device evidence. Desktop gameplay remains native, with an
optional Tauri shell.

Use [AGENTS.md](../AGENTS.md) for task-specific reading and repository checks.
Package/app READMEs own their local commands. Keep decisions in architecture/ADRs,
workflow guidance in skills, and execution results in scoped ledgers. Retire
superseded drafts instead of maintaining another copy of the same guidance.
