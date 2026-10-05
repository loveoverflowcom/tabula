# Issue #54 PR2 — isolated current-session HTTP

Stacked source: PR71 `36e13be9fd7895d1a291af44abe14e255e6ebbfd`, tree
`ed956a39ab1b9294170d6e6fa4a35ff43fd9c4ba`. PR71 remains unmerged at the
start of this work; the focused PR base is `feat/accounts-storage-54-pr1`.
Eventual target is develop after the predecessor is merged/reconciled.
Authorization: [ADR-0036](../../adr/0036-isolated-durable-session-validation.md).

## Delivered contract and ownership

`tabula-session-http` exposes version-1 JSON DTOs by default, including explicit
browser response-shape/version validation. Opt-in native `isolated` composes
Axum/Tokio handlers with the session authority ports; `postgres` is the disposable
acceptance composition. No service imports another service, no SQL is written
outside storage, and neither production entrypoint consumes this crate.

- GET `/api/v1/auth/context`: current authenticated, signed-out or unavailable
  disposition; immutable account ID and browser-only synchronizer token where
  permitted. No bearer, provider identity, record ID or binding is exposed
- GET `/api/v1/me`: current permitted immutable self account ID only. No query
  subject override, other-profile route, name, handle, statistics or edit policy
- POST `/api/v1/auth/refresh`: durable verifier/generation CAS, fresh OS random
  replacement, unchanged context/deadlines, browser cookie or native-only body
- POST `/api/v1/auth/logout`: current-verifier/context revocation, durable success
  then cookie deletion; terminal-current-verifier retry remains idempotent
- Login/register/friends return generic unavailable. They never provision a
  fixture, collect provider credentials or imply an implemented Kanidm flow

Browser credentials use exactly `__Host-tabula_session`; native uses an explicit
canonical Bearer credential and rejects any Cookie header. Mixed/duplicate/
malformed credentials reject. Origin is an explicit named HTTPS configuration,
never Host/proxy-derived. Any supplied Origin must match exactly; browser unsafe
requests require it. Foreign Fetch Metadata and preflight reject; no CORS is
installed. Unsafe input is only empty JSON object, at most 1,024 bytes with a
5-second body deadline and exact supported JSON MIME; arrays, unknown fields,
compressed bodies and subject overrides reject before mutation.

Synchronizer tokens use independently generated per-adapter HMAC-SHA256 keys,
bound to immutable session ID, context ID, subject, epoch and channel. Context
IDs alone are never CSRF or authentication. Tokens survive ordinary verifier
rotation, while adapter restart requires refetching context. Signed-out bootstrap
has a separate random non-authorizing host cookie, 10-minute server-monotonic
expiry, bounded 256-entry state and 120-per-minute process-wide issuance guard.
No token or bearer is persisted to browser storage. All handled statuses,
fallbacks and method rejections carry no-store; browser refresh never serializes
its credential. Stale/error refresh has no Set-Cookie, including no deletion.

## Private response ordering and honest S09 scope

Storage acquires an account advisory key derived from the resolved relation OID,
then account row, then session row. Different search paths resolving the same
account table share the key. A publication guard commits observed floors/expiry
before returning. It retains a dedicated close-on-drop backend plus an additive
committed account exclusion lease of at most 2 seconds, shortened by the session
deadline. All subsequent authority acquisitions honor that durable exclusion
before sampling operation time, including after the publishing backend dies.

The Body retains the one-shot guard through its first frame. The synchronous
publication callback is the server handoff ordering point; expired/dropped/
repeated handoffs do not invoke it. Queued unpolled reads cannot repopulate a
client after their lease. Commit failure never returns a guard. Connection loss
cannot silently return a pooled backend with a lingering session advisory lock.

This bounded fence assumes the trustworthy deployment clock already required
by PR1; silent forward database-clock correction relative to local monotonic time
is not proved. Abrupt backend termination and pool-search-path differences are
explicit acceptance partitions. S09 remains **partial**: already released Hyper/
TCP bytes may arrive later, no WS connection/queue fences exist, and this is not
whole-issue or production security acceptance. A failed private body may surface
as transport failure after headers; clients must refetch context rather than
interpret every network error as logout.

## Evidence ledger

| Claim / failure | Owner and oracle | Status / evidence | Residual |
|---|---|---|---|
| Exact channels, Origin, JSON, CSRF and no-store | real loopback HTTP requests and literal header/body assertions | Pending final isolated wire receipt | Named synthetic HTTPS Origin over loopback HTTP is not TLS/browser evidence |
| Current credential/context/terminal logout policy | checked session policy; raw/stale/rotated/channel/context partitions | PASS: 25 core cases | Fixture issuer+subject structure never proves Kanidm authentication |
| Concurrent HTTP rotation/logout/restart/unavailable | disposable PostgreSQL authority plus actual TCP HTTP | NOT_RUN locally; dedicated CI selection pending | No production/provider/secure-store observations |
| Private first-frame ordering and backend loss | independent PostgreSQL backends, observed lock graphs, pg_terminate_backend, actual unpolled Body | NOT_RUN locally; dedicated CI selection pending | Partial S09; trusted-clock assumption, buffered TCP and WS residuals above |
| Metadata and dependency isolation | authentic PostgreSQL SQLx 0.9 prepare, exact committed descriptions; resolved I-1 graph | Bootstrap CI pending; initial check-deps PASS across 28 crates | No fabricated metadata or executed Rust 1.85 build claim |
| Workspace/targets/closed startup | aggregate, features, native build, WASM compilation and actual service failure exits | Final-content verification pending | Compilation does not prove phase exits or browser interaction |

Local PostgreSQL/client/container tools are unavailable. Ignored or zero-test
selections are never database evidence. The workflow checks non-empty inventory,
real migrations, metadata regeneration/comparison, all real DB tests and offline
compilation. Publication and exact final-head CI receipts are recorded in the PR
body; bootstrap failures remain historical and are not reported as final success.

## Review repairs and remaining gates

Independent first-pass review found real defects before acceptance: volatile
advisory-only fencing on backend loss, current_schema key divergence, logout's
preliminary read defeating idempotency, empty Serde struct accepting arrays, and
CSRF token collision across distinct same-account records reusing a context ID.
The implementation was hardened with durable bounded exclusion, resolved-table
keys, terminal-only logout context, explicit object shape and immutable session
ID token binding; named regressions retain these partitions.

Production services, provider auth-time/reauth synchronization, trusted reverse
proxy/TLS, login/registration policies, native keychains, WS/grants/gameplay,
profile edits/other users/history/statistics, social/ratings/voice, real browser
cookie/BFCache/AT/IME/password-manager and all phase exits remain gated.
PR3 begins only after this PR's verified completion, in a fresh work session.
