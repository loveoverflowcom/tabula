# Scoped verification records

Each record owns its source ref, commands, selected workload, status and residual
scope. Read those fields before citing a result. Historical PASS results do not
establish a passing check at the current checkout, real device execution, or phase completion.
[Tabula engineering](../../.agents/skills/tabula-engineering/SKILL.md) defines the
shared evidence/status vocabulary.

| Record | Scope |
|---|---|
| [Core domain boundaries](core-domain-boundaries.md) | Constructor/decoding boundaries and historical Phase-0 refactor claims |
| [Standalone Chess](standalone-chess/README.md) | Local vertical slice, rules/presentation/build checks and platform residuals |
| [Chess integration](chess-integration/README.md) | ADR-0030 opt-in discovery/launch/return and local configuration/HTTP checks |
| [Game loading](game-loading/README.md) | Bundle loading, cache/resource budgets and wiring evidence |
| [Issue 59](issue-59/README.md) | Tiles renderer asset/workload baseline and outstanding target evidence |
| [Issue 60](issue-60/README.md) | Isolated renderer/embedding spike; artifacts, measurements and target limitations |
| [Session contract](session-contract/README.md) | ADR-0031 adversarial acceptance matrix; specified checks are distinct from executed runtime evidence |

UI-specific ledgers remain beside their contracts in
[`docs/ui/screens/`](../ui/screens/README.md). Optional tool installation and
bounded proof/mutation guidance live in [`verification/`](../../verification/README.md).
Retain raw logs, manifests and images referenced by a ledger as one evidence set;
documentation cleanup must not silently turn unavailable evidence into PASS.
