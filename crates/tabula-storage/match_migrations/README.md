# Isolated atomic match journal (ADR-0040)

These additive migrations are an explicit opt-in. `PgMatchStore::new` and
`claim` never run migrations. Use `PgMatchStore::migrate` only against an
explicitly selected disposable acceptance schema with its own migration history.
Migration history validation remains strict. Combining the separate session and
match migration sets, or configuring separate schemas in the same database,
requires explicit later composition; this adapter proves no combined-session
transaction fence. No production entrypoint
imports this adapter or migrations.

The head owns the current full bounded operation ledger, immutable creation,
logical-clock watermark and owner fence. Each input row holds one canonical
atomic input/events/hash/effects/snapshot record, omitting historical ledger
copies. BLAKE3 checksums cover the exact creation, ledger and record bytes.

The isolated recovery limits are 10,001 records, 64 MiB encoded history plus
head creation and ledger, 4 MiB ledger and 1 MiB snapshot. A replacement claims
an incremented durable fence, then loads a repeatable-read consistent prefix.
Stale owners fail append, ledger updates and load; ambiguity never means success.

SQLx runtime typed queries are used for this bounded adapter, the documented
fallback in `tabula-storage`. They require real PostgreSQL integration acceptance;
they are not advertised as compile-time checked. Existing session macro metadata
remains authentic and unchanged. No synthetic `.sqlx` cache is provided.

Acceptance tests create independent writer/recovery pools in a fresh schema and
require `TABULA_MATCH_DATABASE_URL`. Selecting ignored tests without a database
is a failed setup, never a successful skip:

    cargo test -p tabula-storage --features match-postgres real_postgres -- --ignored

The caller configures connection limits and timeouts (doc 03 §19.3). Each write
transaction sets `synchronous_commit = on`; seeds, canonical snapshots and SQL
errors must never be sent to clients or diagnostic logs.

Test-only fault controls distinguish explicit pre-commit rollback, a real
server-rejected COMMIT (a deferred constraint trigger installed only in the
disposable schema), and response loss after a successful real COMMIT. The
trigger is never a production migration. Recovery retries only after a new
claim and validated committed prefix. Genesis cannot contain consumed command
receipts; the adapter allows reserved-zero scopes for fixtures, while the actor
commits its initial ledger empty before admitting any attachments.
