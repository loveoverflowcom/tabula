# Migrations

Plain SQL, run by `sqlx migrate`, versioned in this repository, applied in CI and
at boot behind a flag. One tool, no second migration framework. (doc 01 §1.2)

```bash
cargo sqlx migrate add <name>     # creates NNNNNNNNNNNNNN_<name>.sql
just db-migrate
just db-reset                     # local only
```

## The rule that makes rollback possible

**Additive only.** A deploy must be safe to roll back, which means migration *N*
must not break release *N-1*. CI gates on this (doc 06 §11.1).

| Change | How |
|---|---|
| Add a column | Nullable or defaulted; the app tolerates its absence for one release |
| Remove a column | Two releases: stop using it, then drop it |
| Rename | Never. Add new → migrate → drop old. |
| Change a type | New column + backfill + swap |
| Add an index | `CREATE INDEX CONCURRENTLY`, outside a transaction |
| Long backfill | A job — batched, resumable. Never a migration. |

## Planned migration order (Phase 4)

Split by concern so a failure names the aggregate it broke, and so Phase 5's
lobby tables land separately from Phase 4's log tables.

```text
0001_identity.sql     users, user_identities, sessions
0002_catalog.sql      games, game_versions
0003_matches.sql      rooms, matches, match_players
0004_event_log.sql    match_inputs, match_events, match_snapshots
                      ← the load-bearing one. See doc 03 §9.4 for columns and
                        doc 03 §19.2 for the monthly range partitioning by
                        created_at (NOT by match_id).
0005_effects.sql      pending_effects (the outbox)
0006_timers.sql       durable_timers + index on fire_at
0007_social.sql       ratings, replays, chat_messages, presence, queue_entries
```

Phase 10 adds `match_placement` (doc 06 §4.4) if and only if a second
match-owning process exists.

## Reminders that cost nothing now and a lot later

- `matches.seed` is **encrypted at rest**. It is the highest-value secret in the
  platform: it reproduces every shuffle and role assignment in that match.
- `matches.id` is UUIDv7 — time-ordered, so range partitioning and time-bounded
  queries stay cheap.
- `match_events.state_hash` is non-null every N inputs (default 20). It is the
  only production detector of determinism drift (I-8).
- Indexes that are load-bearing, from doc 03 §9.4:
  ```sql
  create index on matches (status, game_id) where status in ('live','hibernating');
  create index on matches (room_id);
  create index on match_players (user_id, match_id);
  create index on chat_messages (scope, scope_id, created_at desc);
  create index on durable_timers (fire_at);
  ```

## Backups are not backups until a restore is rehearsed

pgBackRest or wal-g to object storage, **restore tested monthly** (doc 06 §3.2).
Rehearsing the restore is a Stage 0 exit criterion.
