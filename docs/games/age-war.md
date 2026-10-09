# Age War — D01 design entry

**Status: DRAFT / OWNER REVIEW PENDING.** Internal slug `age-war` is a working
identifier, not an approved commercial name. Design version `0.1.0-d01`.

This entry answers [D01 #119](https://github.com/loveoverflowcom/tabula/issues/119)
within [roadmap #118](https://github.com/loveoverflowcom/tabula/issues/118).
The owner's added skeleton request is interpreted as **data contracts, validation,
arithmetic oracles and explicit unsupported seams**, not production gameplay.

| Read | Purpose |
|---|---|
| [GDD](age-war/GDD.md) | Product loop, six ages, content/UX handoff and owner decisions |
| [Research](age-war/RESEARCH.md) | Source/version matrix; confirmed reference mechanics vs original proposals |
| [Rules](age-war/RULES.md) | Deterministic combat, economy, progression and finite edge-case decisions |
| [Content](age-war/CONTENT.md) | 36 units, 16 skill families, 12 spells, 12 turrets, technology/data dictionary |
| [Math and counters](age-war/MATH.md) | Explicit formulas, worked examples and matchup hypotheses |
| [Compatibility / draft ADR](age-war/COMPATIBILITY.md) | Board-runtime conflict, clock/authority alternatives and decision gate |
| [QA plan](age-war/QA-PLAN.md) | Fair AI, benchmark policies, independent oracles and planned experiments |
| [D01 verification](../verification/age-war-d01/README.md) | Only checks actually executed on this scaffold |
| [Design-only crate](../../games/age-war/README.md) | Compilable schemas/catalog/validation/math; no runnable game |

## Rules and information model

Two bases and one continuously contested horizontal lane; one human versus one
bot. Recruit, manage population/queues, cast two currently available commander
spells and decide when to invest in the next age. Destroy the opposing base;
earlier-age wins are valid. No mathematical quiz gates recruitment.

Proposed battle information is symmetric and public: bases, gold, cumulative
technology XP, age/research, queue entries/progress, units/HP/statuses, turrets,
spell slot deadlines and telegraphs. An AI consumes the same seat projection.
No hidden cards, fog, invisible unit, critical-hit RNG or concealed opponent
purchase is included in `0.1.0-d01`. Seed/RNG context, internal event heap,
authority bookkeeping and internal AI candidate evaluations are not client data.
These implementation facts remain private even when they are not gameplay
secrets. Public statistics do not authorize publishing canonical `State` (I-5).
Spectators use the public projection; an Audit viewer is never a player session.

This is an information-model **proposal**, not a tested `project`/`view_event`
implementation. There is no `GameRules`, `GameModule`, `GameBot` or
`GamePresentation` implementation, game manifest, registry registration,
discovery availability, asset pack or gameplay entrypoint in this PR.
`conformance!` therefore has no reachable game fixture yet and is
NOT_APPLICABLE to the D01 schema-only slice, required before C01 acceptance.

## Gate and version ownership

Written design, compilable types and passing arithmetic/catalog checks do not
approve the design, balance the game or waive architecture doc 00. Next is
D02 #120 art direction, followed by D03–D06; C01 #125 remains blocked by
versioned owner approval and the authority compatibility decision. No production
phase or invariant exception is opened here.

Changing an accepted combat rule or catalog value later requires a new design
version and a recorded diff. Before C01, choose a real SDK `RulesVersion` and
rules/config/content hash; `0.1.0-d01` is not a runtime replay identity.
