# ADR-0044: isolated account registration, profiles and social authority

- **Status:** accepted bounded implementation; isolated local acceptance passed; production closed
- **Date:** 2026-10-06
- **Amends:** ADR-0036/0038 and Phase 4/5 ordering for the remaining #54 criteria
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-13 and I-15 preserved
- **Related:** [#54](https://github.com/loveoverflowcom/tabula/issues/54), [registration/profile tracking](https://github.com/loveoverflowcom/tabula/issues/99), [social tracking](https://github.com/loveoverflowcom/tabula/issues/100)

## Authorization and scope

The owner requested a new PR completing #54 and explicitly chose every original
criterion, including new registration and social contracts, with any needed phase
exceptions recorded here. Moving those criteria to follow-up issues does not
complete #54. This decision permits their implementation and disposable acceptance
in the isolated native composition. It does not open production startup, apply
live migrations, provision hosted provider accounts/clients/grants, or declare
Phase 4/5 exits. Default service entrypoints keep refusing startup.

## Registration and profile ownership

Kanidm remains the sole credential issuer, using the existing app-specific
openid-only client and verified OIDC/PKCE flow. Tabula registration means creating
a Tabula account for a verified identity at that configured issuer. Creating a
person or changing credentials at Kanidm remains provider/operator onboarding;
Tabula collects no password, provider token, email, reset credential or new OAuth
grant. Provider forms own password-manager behavior. No agreement has been
approved for this isolated product, so there is no invented terms checkbox.

An explicitly enabled enrollment policy permits previously unmapped verified
subjects to register. Enrollment captures its policy epoch before provider
navigation. A matched single-use callback persists a cookie-bound five-minute
grant and redirects only to `/register`; it does not issue a Tabula session.
Registration atomically commits account, exact issuer/subject mapping, profile
and operation receipt. Known durable acceptance is `accepted_without_session`;
the user subsequently chooses Login. Retry of the same operation/payload returns
its receipt without another account or profile mutation. Ambiguous outcomes stay
unresolved until current receipt lookup. Duplicate handles, existing identity,
denied policy and missing resources expose no existence-specific error.

An existing enabled invited identity may explicitly complete only its missing
profile through signed-out verified enrollment. Its optional account epoch is
captured before provider navigation alongside the enrollment policy epoch, then
compared under the account exclusion when registration commits. The account ID,
status, epoch and an existing profile remain unchanged. An uncaptured mapping,
stale epoch or disabled account is denied; no session is issued. Current context
advertises Friends only once a durable profile exists. The UI explains this
completion path when profile details are unavailable and enrollment is enabled.
An already completed operation can still return its historical accepted receipt
after an epoch change; that replay grants no current authority, session or write.

Enrollment is bounded to 64 accounts and bounded pending attempts, with TTL,
capacity and rate checks owned by the native authority. It never replaces or
revives an existing disabled account. Subsequent login captures existing admitted
identity epochs before redirect through a bounded durable admission snapshot,
then uses the existing issuance CAS; callback cannot borrow a fresh epoch.
Legacy invited-only mode retains its original policy.

The profile owns immutable account ID and unique exact lowercase ASCII handle
(3–32 letters/digits/underscore). Display name is editable Unicode, 1–64 scalars
and at most 256 UTF-8 bytes, without controls or edge whitespace; accents and
committed IME input are preserved. Visibility is `public` (other signed-in
accounts), `friends` (accepted peers), or `private` (self). Default public
discoverability is explained before registration. Self includes visibility and
revision; other-profile projection omits self policy/revision. No history, rating,
statistics, achievements, remote avatar URL or provider field is invented.

Profile edits require current browser credential/CSRF and expected revision,
with operation receipts. A conflict requires explicit current refetch and user
reconciliation. Missing/private/disabled/forbidden other profiles share the same
safe unavailable response. A handle is a purpose-scoped directory/participant
identifier, not authorization; directory search includes only currently visible
profiles. A private display name is omitted even from relationship summaries.

## Friends, requests and presence

`tabula-lobby` owns pure social DTOs, decisions and SQL-free ports. Its newly-real
default surface has no registry, match, game or networking graph. PostgreSQL
adapters and explicit migrations live only in `tabula-storage`. Axum/WSS is a
native opt-in `tabula-session-http` adapter; the browser consumes DTOs only.
New account/social APIs use version 2 and `/api/v2`; version 1 session/context
and immutable `/me` remain compatible. No gameplay wire changes are made.

The native adapters move together to Axum 0.8.8. Axum 0.7's WebSocket dependency
would enable `getrandom` on the same `rand_core` 0.6 used by the frozen kernel
through Cargo feature unification, violating I-1. The newer transport graph uses
a separate RNG generation. The kernel RNG and dependency ban remain unchanged;
`check-deps` must prove that separation. Captured route declarations use the new
syntax while their HTTP URLs and wire behavior stay compatible. Existing session
and match adapter regressions are required alongside the new social checks.

Friend requests are explicit `pending`, `accepted`, `declined`, `expired` or
`cancelled` records. Only the sender can cancel and the recipient can accept or
decline; current session and target/relationship policy are checked under the
durable lock order. Requests expire after 24 hours of trusted authority time.
Mutations carry a canonical operation ID and expected revision where applicable.
Repeating the same scoped operation is idempotent; mismatched reuse or stale
revision conflicts. The browser never creates or accepts a friendship locally.
Search is bounded to 20 results; friend/request collections to 200. No rooms,
matchmaking, game invites, push notifications, ranked system or voice is opened.

The shell owns one authenticated `/api/v2/lobby/ws` connection with exact Origin,
browser cookie and `tabula-social.v2.json` subprotocol. Mutations remain protected
JSON HTTP, avoiding a second mutation authority. Hello, inbound/outbound bytes,
queues and wait times are bounded. The stream sends full authorized snapshots,
with opaque connection scope and monotonic revision. A gap, scope replacement,
disconnect or resync retires old private facts until a fresh snapshot. Account
switch, hidden/frozen document, offline state and route disposal retire pending
work and private output synchronously. Credential refresh does not turn a public
session identifier into authority or needlessly invalidate a still-current
connection binding.

Presence is a server observation, not a browser's desired badge. Only accepted
peers with current disclosure permission receive it. `unknown` makes no activity
claim; `online`/`offline` carry observation times; loss of freshness becomes
`stale`, never silently offline. A validated Hello supplies the first transport
observation. A two-second transport challenge and exact Pong renew attachment
eligibility for at most five seconds; periodic outbound
snapshots alone cannot renew it. Expired eligibility cannot contribute Online.
An explicit owner teardown, with no remaining eligible attachment, may establish
an Offline observation at that teardown time. It makes no assertion about the
user's device or activity outside Tabula. Heartbeats/reconnects establish fresh
authority; late scope/revision observations cannot restore a previous viewer's
state.

## Authority and publication

Current viewer authority alone does not permit another person's mutable profile
or presence. Reads retain a bounded publication fence over the viewer and each
disclosed target/resource, using deterministically ordered account locks shared
by profile visibility and relationship mutations. A permission-narrowing commit
cannot overtake a protected server-frame handoff. Socket outputs recheck current
session binding, epoch, expiry/revocation and target disclosure under that fence;
queued output is dropped on authority loss. Candidate construction continues to
use the existing pure synchronous `SessionPublication::publish` callback; that
callback does not perform I/O. A separate `SocketFramePublication::handoff`
accepts one prevalidated, bounded owned frame and performs its single transport
ownership transfer under the shared fence. A pending sink does not transfer a
frame; readiness alone grants no authority. The guard checks current authority
again at this transfer point. Healthy cleanup clears only its exact owned lease
under the same physical PostgreSQL exclusion; a lost backend leaves the durable
lease until expiry. Storage commits and publication leases retain the existing
trusted deployment-clock assumption.

The claim ends at bounded synchronous server-frame handoff. It cannot recall bytes
already released into transport buffers or establish client receipt. No broad
private-output, distributed presence, deployment or shipping native/mobile claim
is inferred. Local preferences remain separate from account/session authority;
no credential, CSRF or private profile/social data persists in browser storage.

## Evidence and revisit

### Opted-in frontend and emitted resource cost

The new browser composition is explicitly selected with `account-social`,
independently of `online`. The feature opens the enrollment/editable-profile/
other-profile/friends routes and single social socket only within this isolated
slice. Feature-off builds retain the prior account/session views and unavailable
registration/friends presentation, even if connected to a more capable authority.
It is not permission to silently activate the composition or omit any #54
criterion from its acceptance.

The first combined optimized artifact measured 1,096,779 raw WASM bytes, exceeding
the existing 900,000-byte shell ceiling. Shared guarded reactive readers and
request/conditional implementations reduced that to 1,040,622 bytes before the
feature split. That full artifact did **not** pass the existing ceiling. The new
registration, profile and social state machines have a separately enforced
1,100,000-byte raw ceiling, with bounded headroom over the measured composition.
This amends doc 04 §3.2 only for the non-default `account-social` artifact;
default and `online` keep 900,000 bytes. The larger budget requires its compiled
profile marker and actual emitted bytes; the standard check rejects that marker
and preserves markerless historical dashboard comparisons at 900,000 bytes.
No arbitrary caller-selected size limit is accepted. Both old caps and the new cap are mandatory CI checks. All 352
render cases, 52 interactions and 21 real-authority cases run on the same final
optimized `online,account-social` artifact, rather than on a feature-off shell.
The game remains a separate document with its own unchanged resource budget.

The [working ledger](../verification/issue-54-account-followup/README.md) records
actual commands, nonempty selections, results and remaining scope. Acceptance
must exercise real PostgreSQL permissions/idempotency/expiry and actual Kanidm
enrollment/login, then independent HTTPS/WSS browsers for account and social
flows. HTTP doubles, composition events, accessibility-tree assertions and static
renders are labeled separately; none substitute for provider or durable authority.
The portable core gate, feature/target checks and exact-tree CI remain required.

Revisit before production activation/live provisioning, additional credential or
agreement policy, wider account/capacity/retention bounds, multiple presence
owners, native/mobile social delivery, provider security-event synchronization,
or any phase-exit claim.
