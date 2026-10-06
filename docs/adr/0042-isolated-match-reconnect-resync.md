# ADR-0042: isolated match reconnect and resync

- **Status:** bounded owner-requested PR3 implementation; acceptance pending; production closed
- **Date:** 2026-10-06
- **Amends:** ADR-0041 interruption scope only
- **Invariants touched:** none relaxed; I-1–I-16 preserved
- **Related:** [work item](../work-plan/100-reconnect-resync-fault-recovery.md), [evidence](../verification/reconnect-resync/README.md)

## Scope

The third sequential owner-requested PR extends the explicitly opted-in direct
match composition with bounded reconnect, full projection resync and exact
PostgreSQL actor restart. The verified starting source is develop
`e75624ae870a74f62f0f734fbcf2f12043047dd4`, after PR80.
This does not activate either production service or change the supported game,
clock, audience, provider, deployment or mobile contracts.

## Decision

### Authority and operation identity

Reconnect reacquires current context, membership and a memory-only attachment
grant. Full reattach retires the former attachment, preserves the server-owned
seat, and starts a fresh per-viewer visible stream. Only projections, redacted
events and operation receipts cross match wire 0.1.

HTTP carrier **version 2** adds a stable, non-authorizing operation-scope hash to
attachment responses. It correlates the match, auth-session record, subject,
account epoch, server seat and seat generation; it excludes transport SessionId.
The hint cannot authorize anything or replace durable membership checks. HTTP1
carriers are rejected explicitly; match wire 0.1 stays unchanged.

One bounded pending original command may survive refresh in sessionStorage,
with only its routing/package/scope hints, exact command text and a 10-minute
expiry. Credentials, CSRF tokens, grants and projections remain absent from
persistent storage. Rust validates the original envelope and exact current
scope before retrying; JavaScript never decides a replay. A new account/session
record, epoch or seat generation cannot inherit old intent.

A timeout or dropped response has an uncertain result. The client hides the
last projection while authority is uncertain, obtains a fresh full projection,
and retries only the original same-scope operation. A retained receipt restores
its original result. A missing/expired/evicted receipt, conflict or exhausted
recovery budget produces an explicit unknown-result/read-only state; it never
turns the old sequence into a new command or assumes rollback.

### Serialization and restart

Attach and command share bounded match-level serialization. Current authority
is resolved again after waiting. The owned request task retains its sole journal
permit even when the HTTP requester disappears. An unresolved actor deadline
retires the local owner and its output; durable recovery resolves committed truth.

Online journals retain a dedicated physical PostgreSQL advisory-lock backend
for process-lifetime ownership. A competing live online owner cannot claim the
same match. Recovery after owner loss claims a strictly newer durable generation,
requires an initialized journal, and validates the exact approved identity,
creation config and immutable admission roster plus the bounded complete
history, snapshots, ledger and recorded time before any attachment output.
It does not recreate a started match from a fresh seed.

A bounded durable owner-publication exclusion and earlier local monotonic
deadline fence asynchronous queue preparation and actual first body-frame
handoff. New ownership respects the exclusion even after the old backend dies;
late or stale-owner callbacks cannot release queued output. Session authority is
also current at handoff. Already released TCP bytes cannot be recalled. The
closed effect adapter permits only the existing EndMatch-only no-op path;
external audience-sensitive effects and outbox delivery remain outside scope.

### Bounds and lifecycle

Existing frame, response, mailbox, admission and durable lifetime budgets remain
in force. Recovery load and replay share a whole 5-second deadline and the
complete 64MiB encoded-prefix bound. The browser has bounded retries and ignores
retired transport generations; BFCache/navigation needs fresh authority.
No secret grant is restored by document lifecycle or pending-command storage.

## Acceptance

The [evidence ledger](../verification/reconnect-resync/README.md) separates
source, focused tests, real components and actual rendered browser acceptance.
Required faults include before-send, pure apply/pre-commit, staged SQL before
COMMIT, committed/lost-Ack, output polling, pending/committed refresh, authority
changes, retained/evicted/expired original receipts, real server SIGKILL and
fenced restart, stale attachments and held old-owner output. Two independent
Chromium processes must use distinct profiles/cookie jars, genuine HTTPS and
real PostgreSQL; transport doubles do not establish that outcome.

Independent source/security/UI review, portable aggregate, feature/native/WASM/
resource checks and terminal exact-source pre-/post-merge CI remain required.
All existing provider/session/match/mobile gates continue to apply.
