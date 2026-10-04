# ADR-0031: browser and native session contract

- **Status:** accepted contract; runtime implementation remains phase-gated
- **Date:** 2026-10-04
- **Supersedes:** the conflicting session-storage/attachment sketches in doc 01
  §1.3, doc 03 §2/§3.1, doc 04 §3.1/§3.4/§4.5, doc 05 §2 and their scaffold comments
- **Invariants touched:** none relaxed; I-5/I-6, I-9, I-13 and I-15 preserved
- **Related:** [issue #54](https://github.com/loveoverflowcom/tabula/issues/54),
  [PR #62 (specification A)](https://github.com/loveoverflowcom/tabula/pull/62),
  [acceptance and evidence](../verification/session-contract/README.md),
  [remaining work](../work-plan/backlog/issue-54-session-contract.md)

## Context and scope

Source was rechecked at `develop @ 7f3aad749cd1ed1140a32d5f629f14a66711868e`.
Doc 03 §21 says HttpOnly web cookies; doc 04 §4.5 and net-client rustdoc say
`localStorage`; doc 03 §2/§3.1 and doc 05 §2 assume bearer credentials in HTTP
and `Hello`. Doc 01 chooses opaque, revocable server sessions and scoped signed
match grants. Those downstream sketches do not resolve one another by filename
or recency. This ADR records the reconciliation in doc 00's decision register.

`tabula-protocol`, `tabula-net-client`, `tabula-storage` and the server's account
surface are PHASE-4 scaffolds; the server exits with its gate message. There is
no shipped account-session wire format, migration, credential store, CSRF
middleware or security harness to change. The illustrative `Hello` shape is
corrected here; this is not a deployed protocol change. Future executable wire
types/vectors and any compatibility change still follow I-13.

This decision does not implement auth, register fields, provider integration,
profile/social APIs, account-security screens or an OAuth/reset/verification
flow. Existing provider proposals remain deferred. ADR-0028/0030's discovery
and local play stay account-free within their declared capabilities; no local
match acquires an identity, network token or saved resume.

## Decision

### 1. Identity, credentials and ownership

Distinguish three objects instead of calling all of them a session:

- **Auth session:** a durable server record for one authenticated account and
  device and transport channel (browser-cookie or native-bearer), with an internal
  ID, credential generation, account authorization
  epoch, creation/last-activity/deadline/revocation facts
- **Session credential:** an opaque, unguessable 32-byte OS-CSPRNG value,
  encoded canonically for transport. Store only its SHA-256 digest, not the
  bearer value, in the server credential index. Public record IDs/UUIDv7 are
  not credentials. Passwords remain argon2id under doc 01; never use game RNG
- **Connection SessionId:** doc 03's process-local WS routing identity. It
  cannot log in, refresh, prove a seat or outlive the auth-session authority

Identity policy and HTTP/WS enforcement belong to the server shell; durable
records and atomic credential/revocation operations belong to storage behind
ports; client adaptation belongs once in `tabula-net-client`. Pure game rules,
presenters and `KvStore` never own login truth. No new crate/dependency or SQL
port is introduced by this documentation change.

### 2. Storage and deployment boundary

| Target | Credential storage | Attachment | Forbidden fallback |
|---|---|---|---|
| Browser shell and separate gameplay document | Browser-managed `__Host-tabula_session` cookie: `Secure; HttpOnly; SameSite=Lax; Path=/`, no `Domain`, no `Expires`/`Max-Age` | Same-origin HTTPS requests and WSS upgrade automatically carry the cookie | localStorage, sessionStorage, IndexedDB, CacheStorage, JS/WASM exports, URL, fragment, postMessage, DOM or service-worker account cache |
| Native desktop/mobile and non-browser test client | In-memory while used; persistent native credential only in the OS keychain/credential store, account+server scoped | Explicit `Authorization: Bearer <session>` over HTTPS and native WSS upgrade | KvStore/plain file/preferences/UserDefaults/SharedPreferences; silent plaintext fallback if the secure store fails |

The native adapter must prove the selected OS secure-store behavior on each
shipping target; a trait/mock or compilation is insufficient. Passwords are
never persisted by Tabula. This does not prevent user-controlled browser/OS
password managers from filling native form fields.

Initial authenticated browser deployment uses **one trusted HTTPS origin** for
app documents, `/api/v1` and `/ws`, via reverse proxy if needed. Public CDN
assets and explicit pack fetches use `credentials: omit` and contain no account
responses. Trusted same-origin document/bootstrap/static requests may carry the
ambient cookie: static handlers ignore it for authority, redact it from logs,
return account-independent public content and never set session cookies. Strip
cookies before forwarding a static request to an external public CDN. Public
hashed files stay cacheable; account responses never enter that cache. Host
and proxy configuration use an explicit trusted origin; never derive it from
untrusted `Host`/forwarded headers. Cross-origin authenticated browser API/WS
and third-party embedding require a separate ADR; no wildcard credentialed
CORS or widening to sibling subdomains. Host-only cookies do not isolate ports;
never colocate an untrusted application on another port of the same host.
Dev tests use a named trusted HTTPS
origin and preserve cookie protections; local-play's HTTP fixture is not an
authenticated deployment.

HttpOnly limits credential extraction; same-origin script compromise can still
act with cookies and read private responses. CSP/script hygiene, no unsafe HTML
and dependency review remain required. `SameSite=Lax` preserves ordinary inbound
navigation and is defense in depth, not the complete CSRF boundary. A session
cookie may survive browser restoration; server deadlines, not browser closure,
decide validity.

### 3. HTTP, CSRF and public results

- Browser auth uses the cookie, never a JS-readable session in a login body or
  `Authorization` header. Native auth uses only explicit bearer attachment;
  it never accepts ambient cookie authority. Reject mixed cookie/bearer input,
  duplicate session cookies, malformed or conflicting authorization values;
  do not pick a preferred identity from ambiguous input. Credentials are
  channel-bound: a browser credential cannot be reused as a native bearer, nor
  a native credential as a browser cookie
- Browser unsafe requests, including login/register/logout/refresh and future
  account/social mutations, require an exact trusted `Origin` (scheme, host,
  port), JSON content type and `X-Tabula-CSRF` synchronizer token bound to the
  current server context. Reject missing/`null`/foreign origins,
  simple form/text content types and missing/stale/other-context tokens before
  any credential or mutation effect. Fetch Metadata is an additional rejection
  signal, not a substitute. GET/HEAD have no account/permission mutation
- A **proposed**, read-only `GET /api/v1/auth/context` supplies typed session
  disposition and a CSRF token. Signed-out bootstrap uses a short-lived,
  non-authorizing pre-auth context cookie with the same host/security flags
  (distinct name `__Host-tabula_preauth`, 10-minute server expiry). It cannot
  attach, read private data or upgrade to a session by presenting its ID. Its
  synchronizer token stays in document memory. Login destroys the pre-auth
  context and mints a new auth-session record/token; logout destroys that context. Rate-limit bootstrap/login/register. No credential is
  returned to browser JS. Re-fetch context after document navigation/reload
- No authenticated browser CORS access is enabled; reject cross-origin fetch
  preflight. Native header-auth requests without cookies have no ambient
  browser authority and do not need a browser CSRF token. An Origin header, if
  present, must still pass the exact allow-list; a platform string/User-Agent
  never selects a weaker authentication policy
- Session/context/profile/auth results use `Cache-Control: no-store`, bypass
  service-worker account caching and carry no bearer or private diagnostics
  in logs, analytics, URLs or errors. Native credential responses also require
  redaction and no-store. Generic login/registration outcomes cover known and
  unknown accounts; server status/body/timing/rate-limit disclosure tests are
  required. A generic UI string alone does not prove anti-enumeration

Registration fields/normalization/handle/password constraints, actual agreement
and accepted-with/without-session disposition are still separate contracts.
Issuing a session must use this policy but registration is not assumed to log
in automatically. Login/register with a live authenticated browser context do
not silently replace the account: the explicit account-switch flow logs out
the old context first. Route return intent is navigation only, subject to
PR A's bounded allow-list and current destination permission; it never retries
a mutation or launches a match automatically.

### 4. WebSocket and match attachment

Authenticate at **HTTP upgrade**, before `101` or any private output:

- Browser `/ws`: valid session cookie plus mandatory exact trusted `Origin`
- Native `/ws`: explicit valid bearer and no cookie; absent Origin is allowed
  for native clients, but any supplied Origin must be trusted
- Reject ambiguous credentials, revoked/expired/suspended accounts, untrusted
  origin and unsupported codec before upgrade. Rate-limit handshake attempts

`Hello { protocol, client, codec }` negotiates application compatibility after
upgrade. It carries **no session credential**. Bind the connection to the
resolved auth-session record/account epoch; client identity/platform metadata
cannot change that subject. No token in query strings or WS subprotocols.
The server enforces the existing bounded first-message/frame/rate limits, plus
a 5-second first-Hello deadline; before HelloAck only negotiation is accepted.
HelloAck's SessionId is routing metadata, never a refresh or bearer token.

Authentication is not match authorization. A signed join grant lasts **at most
10 minutes** and is scoped to purpose/audience, account subject, auth-session
record and authorization epoch, match, allowed seat/viewer and expiry (no later
than the auth session's current idle/absolute authority deadline). Verification
pins the signing algorithm/key; never accept an algorithm from hostile input.
Every Attach, including resume and spectator attachment, requires a fresh or
still-valid same-binding scoped grant; missing grant fails closed.
`Attach` rechecks the current session, current membership/seat/spectate permission
and matching grant claims, including issuer, type and not-before. Bounded replay
of a still-valid grant is permitted only for the same live session/subject/match/
viewer binding; no single-use guarantee or stateful consumption store is invented.
A signature or friend relation alone grants no seat,
private projection or `Viewer::Audit`. A revoked session invalidates its grants
immediately even if signatures remain cryptographically valid.

For web handoff, `match.ctx` may persist **only non-secret hints** (match/game/
pack IDs and bounded return context). The same-origin game document obtains a
fresh grant from a permission-checked HTTP operation and holds it in memory for
Attach. Refresh/deep link/back/reconnect reacquire permission/grant rather than
persisting a bearer in sessionStorage. The grant operation is future API work,
not activated here. Native scene handoff also keeps grants in memory.
This adds a request and requires same-origin deployment, but preserves
ADR-011's independent documents without exporting the account credential.

Resume cursors, client sequences, pending commands and match IDs are hints,
not authority. Reconnect first revalidates the current auth session and seat;
it cannot replay old-account or old-permission commands. Reissued grants do
not reset match state, clocks, idempotency or rule-owned disconnect consequences.

### 5. Expiry, refresh, rotation and revocation

Initial Tabula policy, deliberately conservative and **not a standards-mandated
timeout**: 30-minute server idle limit, 24-hour absolute lifetime, no remember-me
or separate long-lived refresh credential. Equality with either deadline is
expired. Successful explicit protected mutations and accepted game commands
update last activity (server-classified operation, never a claimed UI gesture);
reads and rejected/duplicate/control operations do not.
Heartbeats, asset fetches, context reads, background polling and refresh do not.
Long idle spectators/turn waits may need sign-in again; socket liveness does not
extend account authority or pause a rule-owned clock.

`POST /auth/refresh` is same-session renewal/credential rotation, not a second
login. Require an unexpired, unrevoked current credential and CSRF for browser;
atomically compare-and-swap the generation/digest and issue fresh OS randomness.
It preserves the auth-session record/subject and absolute deadline, does not
extend idle, and immediately rejects the old credential. Keep the session-bound
CSRF token stable across ordinary verifier rotation so another document can
continue safely; invalidate it at auth/privilege boundaries. No grace verifier,
offline refresh or refresh-after-expiry. Existing
sockets remain bound to the same live record for ordinary credential rotation;
new connections use the new credential. Rotation never expands permissions.

Only the winning refresh may emit a replacement credential/cookie. A stale
refresh returns generic unauthenticated/conflict disposition with **no
Set-Cookie**, including no cookie deletion. Coordinate same-tab auth mutations
and use cross-tab single-flight where available, but do not make security
depend on browser locks. If a response is lost, do not indefinitely retry the
old credential or retain recoverable plaintext successor credentials in DB;
revalidate context and reauthenticate if recovery cannot be established.
A late successful response may overwrite a newer browser cookie with a revoked
credential: that can cause sign-out, never restoration of authority. Generation
guards prevent stale completions from publishing profile/session data.

Logout revokes the current **auth-session record**, durably and idempotently,
then clears the matching browser cookie/native secure-store entry. Password
change/reset, account suspension/deletion, or privilege changes invalidate all
affected sessions/account authorization epochs and require new authentication;
these future operations/screens are not implemented by this ADR. Another device
is unaffected by ordinary current-device logout unless explicitly revoked.

Revocation/expiry fences HTTP reads/writes, Attach, command execution and private
outbound delivery, including queued work. Server success is reported only after
durable revocation and the in-process connection fence is established; revoke
closes associated WS with `4401`, detaches subscriptions and discards queued
private output. Authorization and commit must have a defined ordering against
revocation; a check before an unrelated await is insufficient. Already committed
effects are not rolled back; work ordered after the revocation fence cannot
commit. Permission narrowing/account epoch change forces detach/resync through
the authorized projection path. Store/unverifiable-session failures fail closed,
with unavailable separate from valid/signed-out; no stale cache extends validity.
An expiry timer closes idle sockets even if no frame is received. Multi-process
revocation propagation remains gated until its own coherent fencing design.

### 6. Client lifecycle and logout honesty

Browser tabs share one host cookie/account; tab-local storage is not identity
isolation. Account switching clears old-authority connections/data in all active
documents. Treat signed-out, authenticated, resolving, expired/revoked and unavailable as
distinct typed dispositions. Bootstrap/context is the authority for recovery;
cached profile/name/flags or a surviving cookie are insufficient. On `pagehide`,
synchronously mask/clear private DOM/canvas/accessibility output and retire
pending operations/grants before potential freezing, without an awaited request
or animation frame. On `pageshow`, keep it masked until current session/permissions
and fresh authorized data are established; unavailable recovery stays masked.
After sleep, reload or account change, revalidate before restoring private UI;
a broadcast/storage event is only a prompt to do this. Real first-restored-frame
and accessibility inspection is required: pagehide is not delivered in every
interruption and no-store does not universally disable BFCache. An exclusion
fallback needs verified browser-supported behavior, not an assumed header effect. A missed signal
is handled on pageshow/focus/bootstrap. Browser JS cannot read a failed WS upgrade
status: a socket error remains transport failure until HTTP context establishes
auth disposition, rather than turning every network error into logout.

On logout/expiry/revocation/switch, increment client operation generation; abort
requests, close both owned lobby/match connections, clear account-scoped caches,
pending commands/grants/projections/presence/drafts and private accessibility
output. Late responses cannot repopulate them. Public preferences/verified public
assets may remain; private caches are keyed by server+account+authorization scope.
No cross-account replay, implicit resign, canonical-state mutation or OAuth grant
is implied by UI cleanup; the server owns seat lifecycle via ordered Inputs.

An offline logout can clear local private display/native credential, but cannot
claim durable server revocation or clear an HttpOnly cookie in script. Mark
revocation pending, persist only a non-secret local suppression intent, and do
not automatically restore the old account on the next bootstrap. Reconcile
revocation when connection returns before clearing that intent or switching
accounts. If the native credential is gone and server revocation cannot be
retried, report the unresolved revocation; the server deadline still applies.
Do not say "logged out everywhere" or "server revoked" from local cleanup.

## Consequences, enforcement and remaining gates

Browser credential extraction/persistent JS-token leakage is reduced at the
cost of CSRF/context state, same-origin authenticated deployment and an extra
grant fetch. Native clients retain explicit bearer transport without weakening
the browser path. A stolen bearer can still act until revoked/expired; this is
not device-bound authentication. This ADR chooses a contract, not measured
production security or an account-service implementation.

No PHASE banner, dependency matrix, runtime, schema, frozen algorithm or
executable wire vector changes. Comment/sketch reconciliation is behavior-neutral.
The [acceptance matrix](../verification/session-contract/README.md#required-acceptance-scenarios)
names future independent oracles, failure partitions and security evidence.
It maps to PR A's A01/A05/A13/A15 requirements without replacing registration,
form/accessibility, profile or social contracts. PR #62 itself is a separate
docs-only specification and is not blocked by absent runtime services.

Before B/C: establish actual Phase-2 platform evidence, Phase-3 portfolio/
projection/freeze exit, Phase-4 authority/server/protocol/persistence/session
integration and exit, then the Phase-5 shell gate. Any narrower exception needs
its own accepted ADR. B additionally needs real registration/public-error/
self-profile contracts; C needs real social permissions/ordering/freshness/
expiry/idempotency. This ADR removes only the conflicting session-policy choice.

## Revisit when

- The first authenticated browser deployment needs another origin/embedding
- A shipping native target lacks a demonstrably secure credential store
- Product evidence requires longer idle/absolute lifetime or persistent login
- Multi-process session enforcement, another auth mechanism, or an implemented
  wire/session compatibility migration is proposed

Use a superseding ADR with target evidence, threat model, migration/revocation
and rollback plan; do not silently fall back to localStorage or JWT-only sessions.

## Primary references

These establish mechanisms and threats, not Tabula's chosen timeout numbers:

- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html): opaque entropy, rotation, expiry, invalidation and cookie protections
- [OWASP CSRF Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html): synchronizer tokens, login CSRF, origin checks and SameSite limitations
- [OWASP HTML5 Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTML5_Security_Cheat_Sheet.html): JS-readable storage and session identifiers
- [OWASP WebSocket Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/WebSocket_Security_Cheat_Sheet.html): cookie-authenticated upgrade origin validation, lifecycle/session checks and sensitive logging
- [RFC 8725 JWT best practices](https://www.rfc-editor.org/rfc/rfc8725.html): pin algorithm/key and distinct issuer/audience/type validation for signed grants
- [RFC 6455 client authentication](https://www.rfc-editor.org/rfc/rfc6455.html#section-10.5): HTTP authentication/cookies at upgrade
- [MDN pagehide](https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event) and [Chrome no-store/BFCache](https://developer.chrome.com/docs/web-platform/bfcache-ccns): lifecycle limits and required real restored-frame evidence
- [MDN request credentials](https://developer.mozilla.org/en-US/docs/Web/API/Request/credentials) and [crossorigin](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/crossorigin): default same-origin resources can carry ambient cookies
- [MDN sessionStorage](https://developer.mozilla.org/en-US/docs/Web/API/Window/sessionStorage): reload/restoration/opener behavior; tab hints are not identity isolation
- [WHATWG WebSockets handshake](https://websockets.spec.whatwg.org/#opening-handshake): browser credentials mode and constructor/subprotocol constraints
- [MDN Set-Cookie](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie): HttpOnly, host-cookie requirements and browser session restoration
