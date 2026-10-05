# ADR-0041: isolated direct match browser play

- **Status:** accepted bounded owner-requested PR2; verification pending; production remains closed
- **Date:** 2026-10-05
- **Amends:** ADR-0039/0040's offline transport boundary and Phase 4 ordering only for direct play
- **Invariants touched:** none relaxed; I-1–I-16 preserved
- **Related:** [work item](../work-plan/090-join-code-browser-chess.md), [ledger](../verification/join-code-browser-chess/README.md), [session contract](0031-browser-native-session-contract.md)

## Context and authorization

PR79 normally merged at develop `60d0f1ad802c47848e4847def1705a449b51a7b7`,
tree `932887aeba56bc41e0090cca1a5b841fd78241cd`. Its journal/recovery proof
does not authenticate network users, assign seats or authorize private delivery.
The owner requests three sequential PRs in separate chats. This is PR2:
create/join by code and two independent browsers completing Chess. PR3 owns
reconnect/resync under network drop, refresh, revocation and actual server crash.
Normal self-merge follows independent review, actual target acceptance and
exact-tree terminal CI. No production activation, live migration or provider
provisioning is authorized.

Doc 07 calls for lobby-less direct join before rooms/friends/matchmaking. This
opt-in vertical slice reuses the separate Leptos shell and Macroquad game
document. It does not establish broad Phase 3, 4 or 5 exits.

## Recovery integration history and current source

Develop `9642e4a60a8041bde3652d96dae0d1544bfaec3a` integrated partial recovery
checkpoints after independently merged PR69/70/75. That historical integration
kept native gateway/fixture wiring and real direct launch closed. Its unit and
compile receipts do not establish this PR's online acceptance.

PR80 restores the complete opt-in gateway, SQL-owned admission, shell handoff,
loader transport and native acceptance fixture against that baseline, preserving
the external asset ownership, voice/local simulator changes and authority tests.
Ordinary registry bindings still advertise no deployed direct document. Only an
explicit `RuntimeBinding::direct_online` can expose a game-owned eligible package;
the native gateway independently rejects an ineligible package. The game module
owns configuration validation and actual construction. Implemented source is
not executed browser/database proof; the required gates below remain open until
exact-source CI and artifact inspection complete.

## Decision and ownership

Both default services remain closed. A non-default native HTTP gateway
composes registry, actor, durable session authority and PostgreSQL journal;
constructors do not start listeners or migrate databases. SQL stays in storage.
The disposable fixture explicitly selects a strict reviewed combined session,
match and admission migration set. The SQL-free journal contract still prevents
storage/auth feature unification from importing registry/game runtime.

Bounded same-origin HTTPS polling carries the existing opaque protocol 0.1
envelopes intact. New HTTP DTOs have their own explicit version and compatibility
vectors under I-13. This is not a production WSS implementation: enabling
Axum's current WS feature would unify Tungstenite's OS RNG into the kernel
graph, which I-1 forbids. WSS/native negotiation, heartbeat and load proof
remain separate. No dependency or invariant is silently relaxed.

Registry declares the actual direct-online package/configuration consumer and
owns typed validation and exact game construction. Platform code does not
branch on a game ID or decide rules. Untimed, unranked human play is the bounded
acceptance path. Unsupported clocks, bots, spectators and private/social
effects fail closed instead of silently discarding requested game effects.

### Admission law

The server creates match identity and an expiring bounded-attempt random join
code. Code is a discovery/admission hint, never account authentication or a
command credential. Code/public match ID alone cannot read a projection or
choose a seat. Current authenticated membership is assigned one stable
server-owned seat; duplicate admission cannot allocate another. Invalid,
expired and full codes have public-safe errors. Admission is limited to four
active rooms per member and 128 rooms over the lifetime of a disposable dataset,
including terminal and expired rooms. The latter matches the non-evicting live
actor budget; it is not a reusable 128-concurrent-room service capacity. Busy
capacity is explicit, and PR3 owns safe retirement/recovery before wider reuse.

Only current session authority plus durable membership can obtain a short-lived
session/match/viewer-scoped attachment grant. Grants stay in document memory,
never URLs/fragments, localStorage/sessionStorage, cache, DOM or logs. Grant
possession alone is insufficient: matching cookie/context, subject, match,
seat and generation are checked again. An authenticated same-member reattach
may replace connection SessionId without client seat claims. Credential
rotation does not reset seat generation or durable command watermarks.

### Commit and publication law

ADR-0031's Secure host-only HttpOnly SameSite cookie, JSON, exact configured
trusted HTTPS Origin and current synchronizer CSRF token remain mandatory.
No wildcard authenticated CORS, mixed bearer/cookie authority or public
session ID credential is accepted. Payload/frame/queue/attempt limits bound
work. Busy/unavailable authority is explicit, never local authority.

Protected apply and its journal transaction share durable account/session/
membership ordering. Revocation, epoch change and expiry cannot pass in a gap
between stale observation and commit. Accepted canonical input/events/hash/head,
due snapshot and complete original operation ledger still commit atomically.
Failed/indeterminate writes stop that actor without speculative success,
output or effects. Exact duplicate commands retain their original result;
changed-payload, foreign-match and unauthorized retries fail closed.

Fresh asynchronous authority preparation immediately precedes synchronous
actor output submission. Projected output may then be staged in a bounded
private server queue. Its HTTP consumer reacquires current durable authority
and retains the publication guard through the actual bounded first body-frame
handoff. Revocation ordered before publication suppresses queued output.
Already released Hyper/TCP bytes cannot be recalled; no full socket/S09 or
remote audience-sensitive effect guarantee is inferred from this boundary.

Only per-viewer View/ViewEvent/receipt frames cross the network. Canonical
state/events, seed, logical time, hash, global version/index and ledger remain
server-side. Per-attachment visible revisions preserve no-private-action-gap
privacy. Online clients render typed projections through the existing
presenter/renderer and send typed intents; they do not create/apply canonical
online state.

### Browser and interruption scope

The shell uses shared semantic-token UI and real separate-document navigation.
Only public match routing identity enters the address; the game document
reacquires context/grant. Existing selected-package assets, loader and cache
remain the gameplay loading path. Disconnection visibly blocks commands and conceals the last authorized canvas
and accessibility projection. Document lifecycle retirement cannot silently
restore an old grant/view; renewed navigation needs fresh authority.
Reliable pending replay/resync, refresh continuity and automatic server/actor
restart acceptance belong to PR3. Stable membership/operation identity must
permit that next slice without trusting client seats.

## Required evidence

The ledger separates configured/compiled targets from executed acceptance.
Actual served shell and Macroquad canvas must run in independent Chromium
processes with distinct HOME/profile/cookie jars, genuine HTTPS validation,
real session authority/actor transport and real PostgreSQL. Both clients play
all legal moves of a complete game and agree on the rules-owned verdict; a
server-side durable oracle checks the exact transcript/result. HTTP mocks,
RenderList snapshots, inferred pixels or shared credentials cannot substitute.

The fixture may issue explicitly labelled disposable identities through the
existing real isolated session authority. That is opaque-cookie/durable
authority proof, not another Kanidm login. Genuine provider CI stays required.
Ephemeral test CA keys, credentials and browser profiles are excluded from
artifacts and cleaned on failure. A temporary per-process NSS trust database
validates the CA-signed localhost leaf. No global trust change, ignored HTTPS
error or certificate-warning bypass is used. Secret-free screenshots are
inspected and bounded action/results/browser/CI provenance retained.

Adversarial checks cover cookie/CSRF/Origin ambiguity, unknown/version/length
and codec limits, third-client/cross-match attacks, duplicate admission/commands,
expired/full codes, bounded backpressure and revoke-before-publish. Independent
source/security and UI review, portable aggregate, feature/native/WASM/resource
gates and all-terminal exact-tree pre-/post-merge CI are merge prerequisites.
Absent PostgreSQL/browser setup and zero test selections are never PASS.

## Consequences and remaining gates

Polling has bounded request overhead/latency; it is not WSS performance proof.
This fixture is not public signup or live deployment. Disposable plaintext
seed storage does not close production encryption/access/backup obligations.
PR3's midcommand network drop, refresh, revoke/reconnect and actual server-crash
partitions remain open. Timers/outage-clock policy, external-effect outbox,
distributed ownership (including cross-process owner-generation fencing of
already queued output), spectators, native secure stores, online mobile,
rooms/friends/queues, ranking/history, voice, traffic shaping, load/SLO,
backup/PITR and broad phase exits remain separate. PR69/70/75 are already merged into develop; their local presentation, assets and
voice evidence does not establish this online slice.

Revisit before production/live migration, WSS/native, clock/hidden/private-effect
games, widened origins/audiences, relaxed bounds, changed scope/wire or robust
reconnect/server-crash recovery.

## Primary infrastructure contracts

- [PostgreSQL 16 explicit locking](https://www.postgresql.org/docs/16/explicit-locking.html)
- [Playwright isolation](https://playwright.dev/docs/browser-contexts) and [separate profiles](https://playwright.dev/docs/api/class-browsertype#browser-type-launch-persistent-context)
- [Chromium Linux certificate management](https://chromium.googlesource.com/chromium/src.git/+/refs/heads/main/docs/linux/cert_management.md)
