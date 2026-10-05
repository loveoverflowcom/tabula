# ADR-0036: isolated durable session validation ahead of runtime gates

- **Status:** accepted bounded implementation exception; production remains closed
- **Date:** 2026-10-04
- **Amends:** the preparation-only boundary of ADR-0034 and Phase 4/5 ordering,
  solely for the isolated #54 series below
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-13 and I-15 preserved
- **Related:** [#54](https://github.com/loveoverflowcom/tabula/issues/54),
  [ADR-0031](0031-browser-native-session-contract.md),
  [ADR-0034](0034-kanidm-auth-service-skeleton.md)

## Context and authorization

The initial draft used 0035, which collides with the separately published
Werewolf ADR in unmerged PR #69. Before handoff this exception is numbered
0036; PR #69/#70 are unchanged and remain independent dependencies only where
actually needed. No phase completion or acceptance of their unmerged code is
inferred from reserving the identifier.

Fresh source on 2026-10-04 is `develop @
729417ffb6de4b376d2736bdfbf78c46f455630e`. The account specification and session
policy have merged, but both service entrypoints still refuse startup. Their
frames are not authority APIs. Phase 2 platform evidence, Phase 3 portfolio/
freeze exit and Phase 4/5 acceptance remain separate obligations.

The owner explicitly authorized a narrower implementation exception: three
sequential, separately reviewed PRs for durable sessions with real PostgreSQL
validation, isolated session/self-profile HTTP, then account-state/profile UI
against that isolated boundary. Unavailable login, registration and friends
remain honestly unavailable until their real provider/authority gates pass.
This exception does not claim the whole issue complete or authorize production.

## Decision and delivery boundaries

1. **This PR:** `tabula-session` owns internal identity/session policy and ports;
   `tabula-storage` implements them only behind non-default native
   `session-postgres`. Its additive migrations are an explicit isolated schema,
   not automatic production startup. Real PostgreSQL 16 acceptance runs in a
   disposable CI service. Both service binaries retain their existing failure
   exits and gain no dependency on this implementation.
2. **Next PR, after this one completes:** an isolated same-origin HTTP session/
   context and permitted read-only self-profile boundary. Exact channel/CSRF/
   origin, public error/no-store and current-authority integration are required.
   Synthetic provider fixtures remain labeled; they cannot prove Kanidm login.
3. **Third PR, after the HTTP PR completes:** compact M3 Expressive shell
   account-state/self-profile UI using existing foundation and en/vi copy,
   operation-generation/cleanup/navigation tests and the isolated adapter.
   Real browser/IME/password-manager/AT evidence remains target-specific.

PRs may stack on their predecessor when it is unmerged; their dependency and
exact parent SHA must be explicit. Game/asset PRs are not dependencies of the
independent first slice. No merge, deployment, provider provisioning, OAuth
grant, security settings or persistent external access is authorized here.

## Ownership, policy and trust

Kanidm still owns credentials, OIDC issuer and identity authentication.
`ProviderIdentityKey` is only a structurally bounded exact `(issuer, subject)`
lookup key, never proof of authentication. Email/name matching, handles,
registration policy, agreements, ratings, social data and onboarding privileges
are not invented. The isolated harness provisions synthetic identities only.

The shared library keeps provider/session policy out of deterministic game
contracts and wire types. Services remain leaves. SQL, migrations and row
mapping remain exclusively in storage. Dependencies and doc 00's matrix are
updated with this owner. The coarse cargo-deny entropy wrapper allow-list adds
`tabula-session` and SQLx 0.9's `rand` 0.10 SCRAM wrapper; deterministic I-1/I-4 bans remain unchanged in the
per-crate resolved graph. SQLx 0.8 was rejected by that actual gate: its
PostgreSQL `rand` 0.8 feature unification enabled OS entropy on the kernel
`rand_core` 0.6 dependency. The optional native adapter and metadata CLI use
official SQLx 0.9, whose separate RNG dependency preserves the frozen kernel
graph. This native `session-postgres` feature requires Rust 1.94; the existing
pinned toolchain is 1.96. The workspace/default storage and deterministic SDK
retain their unchanged 1.85 declaration. The feature-specific requirement is
explicit in storage manifest metadata and here, never silently applied to game
authors. Source/MSRV metadata inspection is not an executed 1.85 build claim.
No game-state, rules algorithm or executable protocol
encoding changes; I-13 wire vectors are therefore not activated by this PR.

Session credentials use 32 bytes of OS entropy and canonical URL-safe unpadded
encoding; persistence receives only SHA-256 digests. Secrets/digests have
redacted diagnostics and no serde representation. This is not secure-store,
browser-cookie, HTTPS or successful-provider-login evidence. A stable,
non-authorizing context binding ID survives rotation; actual synchronizer-token
creation/retrieval/checking is the second PR's obligation.

Issuance compares the expected account epoch from the authentication attempt.
A stale attempt cannot acquire a new epoch after invalidation. Real Kanidm
reauthentication/auth-time and password-change synchronization must still be
proved before any production issuance path exists.

## Durable ordering and failure semantics

Use one lock order: account row, session row, then protected resource. Lookup
or a returned snapshot confers no later effect authority. Re-read status,
epoch, channel, current verifier/generation and deadlines while holding the
same transaction that commits a protected effect. Epoch invalidation, issuance,
rotation and current-device revocation follow this ordering. The bounded
implementation intentionally serializes an account's devices; optimize only
with evidence preserving these guarantees.

Authoritative time is sampled after lock waits. Equality at 30-minute idle or
24-hour absolute deadline expires; observation/refresh/rejection/duplicate
operations never count as activity. Terminal expiry and an observed-time floor
are durable, so an observed regression fails closed and cannot resurrect an
expired record. Strict elapsed time across unobserved clock corrections still
assumes a trustworthy deployment clock, which requires production evidence.
Checked arithmetic and raw-row validation reject overflow or corruption.

Rotation CAS has one winner, preserves session/context/deadlines and immediately
invalidates the old verifier. Established connection bindings omit credential
generation, preserving ordinary live sockets, but still require current
record/epoch checks at each actual effect boundary. Revocation targets one
record; account invalidation advances the account epoch. Commit failure or an
indeterminate acknowledgement never yields a known-success result. SQL and
identity/credential contents stay out of public errors.

**Database commit fencing is not private-output fencing.** The storage-private
acceptance marker demonstrates transaction ordering, not gameplay or a socket
send. A check followed by an unrelated await and private output is still unsafe.
No LISTEN/NOTIFY/polling/cache or eventual socket close is claimed to solve it.
S09 remains partial until actual HTTP/WS publication/connection fences are
implemented and measured. Existing bootstraps cannot report successful logout,
readiness or auth availability.

## Evidence and remaining gates

Compile-time checked SQL uses committed `.sqlx` descriptions generated against
real migrated PostgreSQL; ordinary feature/aggregate builds remain offline.
The CI job regenerates and compares those descriptions, executes non-empty
ignored real-DB cases explicitly, and fails setup errors rather than skipping.
Synthetic identities, injected server time, barriers and fault hooks are test
controls, not provider/transport doubles presented as production integration.

The [delivery ledger](../verification/issue-54-session-foundation/README.md)
records exact commands, status and residual scope. Existing full-workspace
checks, feature modes and target builds still apply. Real DB lifecycle receipts
refine S01/S02/S07/S08/S09 only within their stated storage/policy domains;
S03/S04/S05/S06/S10/S11/S12/S13/S14 and UI acceptance are not silently passed.

Revisit before a production listener, provider-backed issuance, shipping credential store,
production migration, private outbound publication or broad phase-exit claim.
That review requires actual target/provider evidence and coherent cross-service
revocation/expiry, trusted proxy/TLS, outage/restart and rollback behavior.

SQLx version/dependency evidence: [official 0.9 changelog](https://docs.rs/crate/sqlx/0.9.0/source/CHANGELOG.md),
[PostgreSQL driver source](https://docs.rs/crate/sqlx-postgres/0.9.0/source/Cargo.toml).

## PR2 refinement: isolated HTTP and bounded body publication

The second approved slice adds the internal `tabula-session-http` library.
Its default/WASM surface is versioned JSON DTOs only; non-default native
`isolated` composes the authority ports and Axum/Tokio, while `postgres` is
for disposable acceptance. Actual loopback listeners belong only to the
explicit test harness, not either production service. No verified provider
issuance path, OAuth grant or production rollout is opened.

The exact HTTP/data contract and evidence are in the
[PR2 ledger](../verification/issue-54-isolated-http/README.md). Self-profile
returns the existing immutable subject ID only. Browser synchronizer tokens
use an independent per-adapter HMAC key bound to immutable record/context,
subject/epoch/channel; process restart refetches context. IDs/snapshots cannot
replace current verifier/channel checks, CSRF or later effect authority.

The account ordering boundary gains a resolved-table advisory key plus an
additive committed exclusion lease bounded by 2 seconds and the session
remaining deadline. A first-frame guard commits observed facts before it is
returned, holds a dedicated close-on-drop connection, and all current authority
acquisitions honor its persisted exclusion even after backend loss. The private
Body owns the one-shot guard through frame handoff. This is bounded server-frame
ordering under the existing trusted-clock assumption, not client arrival of
already released buffered bytes, silent clock correction, WS delivery/connection
fencing or all S09. Production bootstraps and broad phase exits remain closed.
