# Architecture Decision Records

Most decisions live as **short-form rows** in
[`docs/architecture/00-architecture-principles.md` §10](../architecture/00-architecture-principles.md#10-adr-register)
— ADR-001 through ADR-041. Each row states the decision, its status, why, and the
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
4. Macroquad's ceiling or native CMP gameplay embedding and performance (ADR-0043), arriving during Phase 6 mobile work.
5. Scope drift into building a UI framework or a game engine.

[ADR-0035](0035-werewolf-local-simulator.md) records the opt-in local Werewolf
referee and isolated-seat simulator, merged in
[PR #69](https://github.com/loveoverflowcom/tabula/pull/69). Online/social play,
authenticated seats, voice and rollout remain gated.
[ADR-0036](0036-isolated-durable-session-validation.md) records the separate
isolated session exception.

[ADR-0037](0037-native-mobile-voice-client.md), merged in
[PR #75](https://github.com/loveoverflowcom/tabula/pull/75), opens only the bounded
native client/isolated development voice slice. Production/backend voice and
phase exits remain closed; actual device/SFU audio acceptance is still deferred.

[ADR-0038](0038-isolated-invited-kanidm-web-auth.md) is the explicit invited web
authentication exception, with real-provider merge proof and production closure.

[ADR-0039](0039-isolated-match-actor-runtime.md) records only the separately
approved offline actor/wire slice. [ADR-0040](0040-isolated-durable-match-postgres.md)
narrowly extends persistence/recovery with a consistent PostgreSQL journal,
durable receipts/owner fence and exact bounded reopen. Networking, production
activation, live migration and broad phase exits remain closed in PR1.

[ADR-0041](0041-isolated-direct-match-browser-play.md) opens only PR2 direct-match
browser play. Current commit/output authority and independent browser/PG acceptance
are required; reconnect/resync is PR3 and production remains closed.

[ADR-0043](0043-native-mobile-gamehost.md) supersedes mobile WebView gameplay with
the native Macroquad direction under #81. Current mobile selection/packaging is
retired without fallback; native adapters and device acceptance remain blocked.

[ADR-0044](0044-isolated-account-registration-social.md) opens the owner's
bounded #54 registration/profile/social completion, including its explicit
frontend feature and emitted resource cap. Real provider/PostgreSQL/browser
acceptance is required; production activation and broad phase exits stay closed.

[ADR-0045](0045-mobile-discovery-parity.md) adds bounded CMP discovery parity:
public registry metadata, Home/Library/detail and read-only setup review. Catalog
visibility does not establish native runtime availability or open Phase 6.
