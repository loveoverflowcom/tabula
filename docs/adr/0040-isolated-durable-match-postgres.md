# ADR-0040: isolated durable match journal and recovery

- **Status:** accepted bounded owner-requested implementation; verification pending; production remains closed
- **Date:** 2026-10-05
- **Amends:** ADR-0039's offline-only persistence/recovery boundary and Phase 4 ordering only for this slice; doc 03 §8.2/§9/§13's unimplemented recovery sketches
- **Invariants touched:** none relaxed; I-1–I-9, I-13–I-16 preserved
- **Related:** [work item](../work-plan/080-durable-match-postgres.md), [evidence ledger](../verification/durable-match-postgres/README.md)

## Context and authorization

PR78 delivered the isolated actor/wire slice under ADR-0039. This change starts
from remote develop `fd0f1e4`, not an independent unmerged game/assets/voice branch.
The owner now requests three sequential PRs: (1) real PostgreSQL consistent match
commit and correct restart/write-failure recovery, (2) join-by-code between two
independent browsers completing Chess, and (3) reconnect/resync and network,
revocation and server-crash recovery. Only PR1 is being implemented in this chat;
PR2 and PR3 retain their own later-chat implementation and evidence boundaries.
Normal self-merge to develop is authorized after verification and independent
review. Production activation, live migrations and live-provider provisioning
are not authorized. No broad phase exit follows from any one of these PRs.

The existing actor's in-memory journal commits inputs/events/hash but loses its
operation scopes and receipts on process death. Replaying an input without its
receipt, or retaining a receipt without its canonical commit, can respectively
double-apply or falsely acknowledge a command. Independent snapshot writes can
also publish a state that the input log does not explain. Durable ownership must
exclude a paused former actor after a new owner has reopened the same match.

## Decision and ownership

The native, non-default `tabula-storage/match-postgres` adapter implements the
SQL-free journal contract in `tabula-match::durable`. Durable journal DTOs and ports
do not require registry/game dispatch or an async runtime merely to exist;
a dedicated contract crate prevents Cargo all-feature unification from pulling
actor registry/game dependencies into storage or account-authentication graphs;
the actor's optional registry/Tokio dependencies stay separate. Storage never
imports the registry or a game crate. Canonical state remains server-internal;
only the existing projection path produces client frames (I-5/I-6/I-9/I-15).

One actor still owns one erased match and its bounded mailbox. Storage owns
transactional persistence, consistent loading and durable ownership fencing.
Registry owns exact approved game construction, input decoding, canonical
serialization and deterministic replay. Recovery composes those boundaries;
SQL never determines game meaning. Both default production service entrypoints
stay closed. Constructors do not open a listener or automatically migrate.
The opt-in migration/harness targets only explicitly selected disposable or
non-production test databases; there is no general production schema rollout.

### Consistent commit law

Creation durably records exact game/package/rules identity, canonical config,
roster, seed, creation events, version/index zero, state hash and initial snapshot.
An accepted input records its original canonical bytes, recorded logical time,
input index, resulting canonical events, state version/hash and associated
snapshot when due together with the **entire bounded operation ledger** in one transaction.
The adapter updates the committed head in that transaction, with
`synchronous_commit = on`. A snapshot is never independently authoritative.

The operation ledger includes every reserved scope, its monotonic high-watermark
and bounded retained Ack/Reject receipts with exact game/match/package/command
identity and expiry facts. Correlation and process-local connection SessionId
are retry/routing metadata, not durable operation identity. The durable scope is
auth-session record, subject, epoch, seat and seat generation. The ownership
fencing generation is a separate internal value, not a new operation identity.

Authorized attachment reserves a scope durably before successful attachment
output; reservation is not delayed until a potentially private command. New
scopes fail closed at capacity instead of evicting old watermarks. Receipt TTL
or count eviction never removes the scope or reduces its high-watermark during
the match lifetime. Old sequences without retained receipts yield StaleSeq,
including after restart. Exact retained retries reproduce the original result;
changed command identity yields OperationConflict. Receipt replay always rechecks
current output authority and never re-applies an input or repeats its effects.

Malformed/game-rejected inputs consume a sequence and persist their receipt and
ledger change without advancing the canonical input stream/version. Authority,
admission and sequence-window rejection do not consume a new sequence. A
receipt-only or attachment-reservation transaction is subject to the same durable
owner fence as an applied-input transaction. Limits bound scopes, receipts and
serialized payloads; this PR does not claim unlimited-history memory or load SLOs.
The isolated policy snapshots creation, every 20 accepted inputs and terminal
state. A snapshot is at most 1 MiB and the complete ledger at most 4 MiB. Consistent
recovery admits at most 10,001 records (creation plus 10,000 accepted inputs) and
64 MiB encoded data. Larger matches require a separately reviewed streaming/
retention design; neither silent truncation nor log compaction is implemented.

### Failure and ownership law

Success, projection output and keyed effects follow only a known-success journal
commit. A known write failure or indeterminate commit stops the actor and closes
its mailbox. It does not acknowledge, broadcast speculative state, continue from
the candidate, or automatically retry apply. In-memory counters are not recovery
cursors. A new owner resolves uncertainty by loading the database's committed
head; an already committed receipt protects a retry after a lost acknowledgment.

Every mutation compares the expected committed state version and the durable
ownership generation in the same database ordering domain as the write. Reopen
claims a strictly newer persisted generation. A stale actor cannot append or
replace receipts/snapshots after that claim, even if its expected version still
matches. Fencing/expected-version conflict is terminal to that actor. This is
stale-owner exclusion for this isolated adapter, not a production placement
directory, expiring lease, supervisor or cross-process output/effect fence.

### Recovery law

Recovery accepts only the exact recorded approved game/package version,
RulesVersion and nonzero rules hash, canonical config, roster and seed. It never
chooses a latest game build or the replay tool's CompatibleVersion allowance.
Replay uses stored input indices and logical times, never wall-clock values
recomputed for historical inputs. The isolated actor log starts at creation zero
and advances accepted input index and state version contiguously; rejected
attempts live only in receipts. This does not change the separate Phase-1 replay
format's permission to retain original attempt-index gaps.

Reopening validates one consistent committed load before returning a live actor:
creation identity/state/events, contiguous accepted inputs, per-input derived
events and state hashes, snapshot metadata/bytes/hash at its recorded version,
committed-head agreement, and ledger scope/sequence/receipt bounds and consistency.
Bad encoding, missing/extra records, skewed identity, rejected stored inputs,
event/hash/snapshot mismatch, partial data or malformed ledger fail closed.
No corrupted snapshot can hide a bad earlier input or supply an unexplained
state. Snapshots may accelerate reconstruction only after the relevant log's
consistency proof; this PR does not claim a fast-path that skips that proof.

Historical recovery does not emit client frames. New attachments establish new
per-attachment streams and reacquire current authority. Canonical counters,
seed, times, hashes, events and ledger details do not enter wire 0.1. Its privacy
suppression and visible-revision rules remain unchanged; no wire bump is implied
by a server-internal storage format. Future client/wire changes still follow I-13.

## Laws and adequate evidence

The [ledger](../verification/durable-match-postgres/README.md) owns execution
statuses and exact commands. The implementation/review must establish:

1. Atomic head/input/events/snapshot/ledger: after failure, a fresh DB reader sees
   either the complete prior commit or the complete next commit
2. Durable duplicate safety: dropped Ack, actor recreation and actual process
   termination cannot reapply a committed operation; malformed/game-rejected
   receipts also survive, with conflict/expiry/eviction behavior preserved
3. Permanent high-watermarks and admission privacy: bounded receipts never make
   an old operation new, and private commands do not allocate shared scope capacity
4. Stale-owner exclusion: a newer generation rejects every former-owner mutation,
   including same-version receipt-only changes
5. Exact recovery: deterministic recomputation agrees with every stored event,
   hash and snapshot; deliberately corrupted/skewed/partial fixtures are refused
6. Failure containment: known rollback and committed-but-receipt-lost uncertainty
   produce no speculative success/output/effects and require reopen
7. Architecture isolation: default/WASM graphs stay SQL/runtime-free where promised,
   storage has no registry/game dependency, and service startup remains closed

Faithful in-memory faults establish actor policy only. PostgreSQL 16 integration
must execute non-empty transaction/fault/reopen/concurrency selections against
a real disposable server. A fresh process fixture proves process-boundary recovery;
dropping a Rust handle alone is not that evidence. Setup failures or unavailable
databases cannot silently skip these cases. Exact-tree CI, the authoritative
portable gate, applicable feature/target checks and independent source/security
review remain merge prerequisites. Planned checks are not PASS receipts.

## Consequences and remaining gates

This slice replaces process-lifetime duplicate protection with a coherent durable
commit/reopen boundary. Keeping the whole bounded ledger with each commit favors
inspectable correctness over a separately updated cache. Snapshot/replay and
ownership validation add startup cost; performance/cadence tuning needs measured
data and cannot discard integrity or monotonic sequence facts.

Persisted seed/config/state/events and session/subject identifiers are private
server data. Test artifacts/logs must not dump them. Production application-level
seed encryption, storage access control, key handling and backup/restore evidence
from doc 03 §19.4 and doc 06 remain activation gates; an isolated test database is
not production at-rest security evidence.

The synchronous ADR-0039 authority guard still does not atomically fence an awaited
DB commit against durable revocation. Already-applied input may commit after
revocation while later output is suppressed. This PR does not satisfy ADR-0031's
online command/revocation commit fence or actual private socket-delivery fence.
Production Notify/chat/voice/bot adapters need their own current audience/session
authority at their actual effects. A stable committed-input key is not delivery
permission, and durable external-effect/outbox reconciliation is not claimed.

Join codes, authenticated WS, two-browser gameplay, network reconnect/resync,
timer scheduling and outage clock policy, automatic server supervision, delayed
spectators, lobby/queue, online revocation, native credential stores, load/SLOs,
deployment/live migration and broad Phase 3/4/5 exits stay outside PR1. PR2/PR3
must revisit the relevant authority/wire/lifecycle contracts with real target
evidence before advertising those outcomes.

## Revisit before

Any network consumer, production activation or live migration; shortening scope
retention, log compaction or snapshot proof bypass; a different rules/package
recovery policy; remote leases/placement or external-effect recovery; online
session/commit/output fencing; or broad phase-exit claim.

## Primary infrastructure contracts

- [PostgreSQL 16 transaction isolation](https://www.postgresql.org/docs/16/transaction-iso.html): recovery uses a stable repeatable-read snapshot; a conflicting lock fails closed instead of accepting a mixed prefix.
- [PostgreSQL 16 explicit row locking](https://www.postgresql.org/docs/16/explicit-locking.html): claim, append and ledger mutation serialize on the same head row; load holds a shared lock while reading its prefix.
- [PostgreSQL 16 WAL durability configuration](https://www.postgresql.org/docs/16/runtime-config-wal.html): isolated writes request synchronous commit; production fsync/storage/backup acceptance remains outside this slice.
