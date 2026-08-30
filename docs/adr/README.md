# Architecture Decision Records

Most decisions live as **short-form rows** in
[`docs/architecture/00-architecture-principles.md` §10](../architecture/00-architecture-principles.md#10-adr-register)
— ADR-001 through ADR-025. Each row states the decision, its status, why, and the
trigger that would make us revisit it.

This directory is for the cases where a row is not enough: a long argument, a
measurement write-up, or a decision that **supersedes** an existing ADR.

## When to write a long-form ADR here

- You are **breaking an invariant** (doc 00 §7.1). The process is: write the ADR
  that supersedes the relevant one, state what enforcement changes, update the
  I-table, and change the enforcement code **in the same PR**. Silent exceptions
  are how platforms rot.
- You are **resolving an EXPERIMENT** from doc 09 §3.2 — record what was measured,
  not just what was chosen. "We picked LiveKit" ages badly; "at 20 participants,
  self-hosted cost $X and managed cost $Y, and the failover story was Z" does not.
- You are **crossing a DEFER trigger** from doc 09 §3.3 (adding Redis, splitting a
  service, adding a region). Name the symptom that forced it and the number you
  measured.
- The decision took more than a paragraph to argue.

## When NOT to write one

If the decision is already covered by a doc 00 §10 row, cite the row. Two places
recording the same decision is how documentation starts lying.

## Format

```markdown
# ADR-0NNN: <short title>

- **Status:** proposed | accepted | superseded by ADR-0MMM
- **Date:** YYYY-MM-DD
- **Supersedes:** ADR-0XXX (if any)
- **Invariants touched:** I-N (if any)

## Context
What is true now, and what forced this decision. Include the measurement if there
was one.

## Decision
What we are doing. Present tense, specific.

## Consequences
What becomes easy. What becomes hard. What enforcement changes, and where.

## Revisit when
The concrete, ideally numeric, trigger. "When it hurts" is not a trigger.
```

Number sequentially from 0001. Never renumber; supersede.

## The five things most likely to go wrong (doc 09 §6)

Worth knowing, because they are what most future ADRs will be about:

1. A projection leak.
2. Silent determinism rot.
3. Phase 4 ordering/idempotency bugs under load.
4. Macroquad's ceiling arriving during Phase 6 mobile work.
5. Scope drift into building a UI framework or a game engine.
