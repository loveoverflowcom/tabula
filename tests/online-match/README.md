# Recovered online-match acceptance scaffold

Status: **NOT_IMPLEMENTED**. This is partial source recovered from
`wip/pr80-browser-recovery-20261005`, not an executable or passing browser test.

`draft/online-match.yml` preserves the proposed workflow outside GitHub's active
workflow directory. `draft/Cargo.toml.template` preserves the intended standalone
workspace manifest; its relative paths are for `tests/online-match/Cargo.toml`.
Do not activate either until the missing implementation and checks exist.
`run.sh` fails before creating test credentials/processes when sources are absent.

Missing at integration:

- `tabula-match-http` gateway and DTO crate, workspace/dependency policy entries;
- `tabula-storage/online-match-postgres`, composed migration/authority scenarios;
- `apps/web` and `apps/game-client` online features and projection-only transport;
- fixture `src/main.rs`, lockfile, TLS frontend, browser acceptance and Python tests.

Completion must include nonempty helper/test selections, strict fixture lint,
real disposable PostgreSQL and two independent HTTPS-validated Chromium processes,
plus inspection of public-only screenshots and the durable terminal verdict.
Do not count workflow syntax or this source inventory as executed online evidence.
The acceptance requirements remain in [ADR-0041](../../docs/adr/0041-isolated-direct-match-browser-play.md)
and its [delivery ledger](../../docs/verification/join-code-browser-chess/README.md).
