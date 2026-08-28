# Tabula — Architecture Plan Index

Tabula is a **Rust-first, cross-platform board-game platform**: a reusable runtime on which
many independent board games (chess-like, card, social-deduction, tile-placement, party) are
hosted, versioned, and deployed without modifying platform code.

These documents are the **architecture baseline**. A coding agent given a phase assignment
should be able to read 2–3 of these files and implement it without re-deriving the design.

---

## Documents

| # | File | Read it when |
|---|------|--------------|
| 00 | [`00-architecture-principles.md`](./00-architecture-principles.md) | **Always first.** Invariants, ownership boundaries, dependency rules, ADR register. |
| 01 | [`01-stack-and-repository-plan.md`](./01-stack-and-repository-plan.md) | Creating crates, adding dependencies, setting up CI. |
| 02 | [`02-game-module-and-sdk-design.md`](./02-game-module-and-sdk-design.md) | Implementing the game contract, or writing any game. |
| 03 | [`03-backend-and-multiplayer-plan.md`](./03-backend-and-multiplayer-plan.md) | Implementing gateway, sessions, match actor, persistence. |
| 04 | [`04-frontend-and-design-system.md`](./04-frontend-and-design-system.md) | Implementing Leptos shell, Macroquad client, presentation, tokens. |
| 05 | [`05-data-protocol-and-replay.md`](./05-data-protocol-and-replay.md) | Touching the wire protocol, serialization, replay, versioning. |
| 06 | [`06-scaling-deployment-and-observability.md`](./06-scaling-deployment-and-observability.md) | Deploying, instrumenting, or responding to load. |
| 07 | [`07-phases-and-implementation-roadmap.md`](./07-phases-and-implementation-roadmap.md) | Planning or executing any phase of work. |
| 08 | [`08-first-games-validation-plan.md`](./08-first-games-validation-plan.md) | Choosing/implementing the reference games. |
| 09 | [`09-synthesis-and-decision-register.md`](./09-synthesis-and-decision-register.md) | Needing the one-page answer, or the LOCK/EXPERIMENT/DEFER table. |

## Suggested reading paths

```text
New contributor        → 00 → 01 → 07
Implementing a game    → 00 → 02 → 08 → 04 (presentation section)
Working on the server  → 00 → 03 → 05 → 06
Working on the client  → 00 → 04 → 05 (client section)
Making a big decision  → 00 (ADRs) → 09 (register)
```

## Status legend used throughout

| Marker | Meaning |
|--------|---------|
| **LOCK NOW** | Decided. Do not relitigate without an ADR superseding it. Code may depend on it structurally. |
| **EXPERIMENT** | Direction chosen, details unproven. Build behind a seam; expect measurement to settle it. |
| **DEFER** | Deliberately not now. Preserve the seam, write no code. |

## Conventions

- Crate prefix is `tabula-`. Where the source research used `boardgame-*`, the mapping is
  1:1 (`boardgame-core` → `tabula-core`, etc.); see doc 01 §2.1.
- Game crates are named `tabula-game-<slug>` and live under `games/`.
- Invariants are cited as **I-*n*** and defined once, in doc 00 §7.
- Decisions are cited as **ADR-*nnn*** and defined once, in doc 00 §10.
