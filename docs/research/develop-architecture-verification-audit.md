# Historical architecture audit — retired report

The full 2026-09-02 audit of `develop@f256e34` has been removed from the active
working tree. It describes an earlier implementation, retired Tic-Tac-Toe work,
and the former nine-skill routing hierarchy. Its implementation counts, test
results and proposed PR sequence must not be used as current status.

[Read the original report in Git history](https://github.com/loveoverflowcom/tabula/blob/cca6cbee2e7bca369a53403ad8927c59630c1589/docs/research/develop-architecture-verification-audit.md).
That permalink preserves the report, not a new execution against today's source.
This small note retains existing citations without maintaining a second roadmap.

| Question | Current source |
|---|---|
| What contract governs the repository? | [Doc 00](../architecture/00-architecture-principles.md) and [ADRs](../adr/README.md) |
| Which workflow should an agent load? | [Two canonical skills](../../.agents/skills/README.md) and their on-demand technique references |
| What can be implemented next? | [Phase roadmap](../architecture/07-phases-and-implementation-roadmap.md), [work queue](../work-plan/README.md) and linked issues |
| What checks actually ran? | [Scoped verification records](../verification/README.md) and their recorded refs/logs |

The earlier audit's useful review questions still apply: use independent rules
oracles, distinguish projection secrecy from deterministic replay, and separate
compiled targets from executed rendering. Maintained guidance for those questions
lives in the canonical skills; historical PASS/FAIL results remain historical.
