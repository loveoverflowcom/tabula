# Isolated durable match acceptance

ADR-0040 opens this standalone harness only. It exercises the real
`tabula-match` actor, registry-owned approved opaque game fixtures, and the
`tabula-storage` PostgreSQL adapter. It does not activate a gameplay listener,
provider authentication, browser join codes, reconnect/resync, or Phase 4.

## Run against a disposable PostgreSQL 16 database

Use Rust 1.96, as pinned by the repository. Set `TABULA_MATCH_DATABASE_URL` to
an explicitly disposable PostgreSQL 16 database. The database user must be able
to create schemas. Do not use production data or production credentials.

```sh
cargo test --manifest-path tests/match-postgres/Cargo.toml --locked \
  --test recovery -- --list
cargo test --manifest-path tests/match-postgres/Cargo.toml --locked \
  --test recovery real_postgres_ -- --test-threads=2
cargo clippy --manifest-path tests/match-postgres/Cargo.toml --locked \
  --all-targets -- -D warnings
```

Every selected test requires the database and verifies its actual major version.
Missing configuration, connection failure, failed migrations, or the wrong
PostgreSQL version fails setup. Nothing silently skips. Each test creates a
fresh disposable schema; reopened actors use independent pools in that schema.
The ephemeral CI database is destroyed after the job, including crash fixtures.
Local disposable schemas are intentionally retained for inspection.

The `recovery_process_child` test is ignored in ordinary selection. The process
kill test starts the current integration-test executable with `--exact`,
`--ignored`, and explicit parent-selected schema/mode/ID/signal parameters. Its
parameters are required. It is not an independent test to run manually.

## Assertions

- A finite real actor transcript commits exact inputs/events/hashes and the
  genesis/terminal snapshots, then reopens with matching projected state
- Snapshot cadence is exactly genesis and every twentieth accepted input, plus
  terminal input; the tail survives restart
- PostgreSQL commit precedes effects, Ack, and projected updates
- A staged rollback, actual deferred server rejection of COMMIT, and an
  indeterminate response suppress output/effects and stop the actor; recovery
  reads the actual committed database prefix
- Actual SIGKILL after staged writes and after committed append, before append's
  return, recovers the precise prefix and applies retry only when uncommitted
- Successful and rejected receipts survive restart; different payload conflicts,
  TTL expiry and eviction retain durable high-watermarks
- Wrong-match and wrong-game rejections retain the original opaque payload
- Replacement claims fence stale actor writers; concurrent identical mailbox
  commands add exactly one canonical input
- Persisted snapshot/input/event/hash/rules identity/format/effect/terminal/head/
  ledger corruption is rejected before mailbox, projection, or effect output
- Nonzero process uptime, restart downtime and a clock that ticks between reads
  preserve exact durable logical times and receipt timestamps
- Recovered attachments need fresh authority and fresh connection IDs; durable
  operation scopes survive connection changes
- Closing all actor handles without a drain still allows exact durable recovery

Canonical data is kept inside server-only journal adapters. Assertions on
canonical records compare encodings without printing private values. No SQL,
game names, command schemas, keys, or persistent credentials are defined here.
All SQL and fault/corruption hooks belong to storage's explicit acceptance-only
`match-postgres-test-support` feature.

The replay comparison is same-target self-differential evidence. It does not
independently prove game legality or cross-target determinism. Configured CI,
compiled tests, executed acceptance, and merge enforcement are distinct claims;
a workflow file alone is not executed evidence.
