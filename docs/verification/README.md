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
| [Standalone Werewolf](werewolf-standalone/README.md) | ADR-0035 local referee, projection/privacy, replay and presentation evidence; real target limitations |
| [Native mobile GameHost](mobile-native-host/README.md) | ADR-0043 source spike, mobile native-only policy and CMP viewport evidence; native adapters/device acceptance remain blocked |
| [Native mobile voice](native-mobile-voice/README.md) | ADR-0037 native builds and controller/CMP doubles; actual device/SFU audio remains deferred |
| [CMP shell parity](issue-101-mobile-shell/README.md) | Issue #101 adaptive chrome, navigation/restoration, vi/en and large-text previews; local toolchain and device residuals |
| [October 5 PR integration](pr-integration-20261005.md) | Sequential review/merge of #69, #70 and #75, tested source identities and #80's incomplete scope |
| [Chess integration](chess-integration/README.md) | ADR-0030 opt-in discovery/launch/return and local configuration/HTTP checks |
| [Game loading](game-loading/README.md) | Bundle loading, cache/resource budgets and wiring evidence |
| [Issue 59](issue-59/README.md) | Tiles renderer asset/workload baseline and outstanding target evidence |
| [Issue 60](issue-60/README.md) | Isolated renderer/embedding spike; artifacts, measurements and target limitations |
| [Session contract](session-contract/README.md) | ADR-0031 adversarial acceptance matrix; specified checks are distinct from executed runtime evidence |
| [Isolated session foundation](issue-54-session-foundation/README.md) | ADR-0036 bounded policy and genuine PostgreSQL authority evidence |
| [Isolated session HTTP](issue-54-isolated-http/README.md) | Current credential/CSRF/context/self-profile and bounded server-frame evidence |
| [Isolated account-state shell](issue-54-account-state-ui/README.md) | Third isolated slice; browser-memory lifecycle, typed adapter consumption and remaining platform gates |
| [Trusted HTTPS Origin](issue-74-trusted-origin/README.md) | #74 F1 fail-fast configuration, constructor/property/wire regressions and remaining session-review gates |
| [Offline match actor](isolated-match-actor/README.md) | ADR-0039 historical isolated ordering/authority/privacy/wire evidence; no SQL durability inferred |
| [Durable match PostgreSQL](durable-match-postgres/README.md) | ADR-0040 atomic canonical/snapshot/ledger commit, durable scopes/fencing and exact bounded recovery; actual statuses distinguish DB/process/local/CI evidence |

UI-specific ledgers remain beside their contracts in
[`docs/ui/screens/`](../ui/screens/README.md). Optional tool installation and
bounded proof/mutation guidance live in [`verification/`](../../verification/README.md).
Retain raw logs, manifests and images referenced by a ledger as one evidence set;
documentation cleanup must not silently turn unavailable evidence into PASS.

## Source-path history

Game-owned pack sources now live under `games/<game>/assets/`. Retained logs,
JSON manifests and receipts that record `assets/packs/<game>/` refer to their
original source revisions and are intentionally unchanged. Reproduction commands
and editable-source links use the current location; runtime pack URLs and hashes
are unchanged by the source relocation.
