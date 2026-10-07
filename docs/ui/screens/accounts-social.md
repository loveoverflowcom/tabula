# Accounts and social — shared boundary

PR A of [issue #54](https://github.com/loveoverflowcom/tabula/issues/54).
Original source review: fetched `develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`.
Session-policy compatibility refresh: merged
`develop @ 115159cd38cba11f7fa94c3c49698b7a45d59e5f` (PR #63), preserving
that review and design provenance as historical evidence.
Design provenance: `030da25d0098e240ab2cf36dacf9892e8b320a89`,
[`06-accounts`](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/06-accounts),
and `01-foundation/shared/material-component-map.md` at that revision.
The four desktop SVGs, mobile/state SVG and three supplied PNGs were inspected;
Register and Friends were also rasterized from their original SVGs for inspection.
Those are static reference images, not product screenshots or interaction evidence.

Doc 00 §4/§6.1 and I-5/I-6/I-9/I-10/I-13/I-15, doc 03 §2/§9.4/§14/§21,
doc 04 §1–§4/§7–§10, doc 05 §2/§5/§9 and doc 07 govern this contract.
[Foundation](foundation.md) owns shared visual/control anatomy.
Individual screens: [15 Login](15-login.md), [16 Register](16-register.md),
[20 Profile](20-profile.md), [21 Friends](21-friends.md).
[Copy](accounts-social-copy.md) and [verification](accounts-social-verification.md)
are shared across them. All account/social adapters below are **proposed** until
their typed owner is implemented and the relevant gate is evidenced.

## Current source and gates

The table below is the historical compatibility base. The later
[ADR-0036](../../adr/0036-isolated-durable-session-validation.md) exception adds
only isolated durable context/self-profile authority and the
[/account and /me shell slice](account-state-isolated.md), with its
[scoped evidence](../../verification/issue-54-account-state-ui/README.md).
The original login/register/social/other-profile contracts and production gates
remain deferred; historical NOT_IMPLEMENTED claims are not current-slice receipts.

| Surface rechecked at the compatibility base | Actual status | Prerequisite for runtime |
|---|---|---|
| `apps/web/src/views/mod.rs` | Mounted `/`, `/games`, `/games/:id`; setup is a detail substate | Auth/profile/friends are absent, not hidden implemented routes |
| `apps/web/src/main.rs`, `Cargo.toml` | PHASE 5; `AppState`/typed API client remain future layout; network dependencies commented | Phase 4 exit and opened Phase 5 gate, or a separately accepted bounded ADR |
| `services/tabula-server/src/main.rs` | PHASE 4 scaffold; binary exits with gate message | Real identity/session/authentication/authorization and integration evidence |
| `tabula-protocol`, `tabula-net-client`, `tabula-storage` | PHASE 4 scaffolds; identity/schema/transport are prose | Implemented versioned types, ports, ADR-0031 session enforcement and persistence |
| `tabula-lobby` | PHASE 5 scaffold; presence/room invitations are described | Real viewer-scoped social APIs, single shell lobby connection and tests |
| Discovery/local play | Bounded real registry/setup and opt-in separate gameplay document | Reuse current availability and launch validation; it does not open accounts |

[ADR-0028](../../adr/0028-discovery-shell-ahead-of-phase-gate.md) permits only
discovery/setup; [ADR-0030](../../adr/0030-local-discovery-gameplay-handoff.md)
adds an opt-in local gameplay handoff. Neither opens login, profile, friends,
online play, ratings or saved history.
[ADR-0031](../../adr/0031-browser-native-session-contract.md) resolves the
browser/native session policy only. Actual Phase-2 platform evidence and
Phase-3 portfolio/projection/freeze exit remain prerequisites for Phase 4;
real Phase-4 authority/server/protocol/persistence/session integration and exit
then open the Phase-5 shell gate. A passing existing aggregate check establishes
none of those exits. Keep the PHASE banners and the issue's B/C slices gated;
see the [session implementation backlog](../../work-plan/backlog/issue-54-session-contract.md).

## Accepted session policy and remaining authority contracts

The accepted policy and remaining implementation blockers are not choices
delegated to a mock form:

- [ADR-0031](../../adr/0031-browser-native-session-contract.md) supersedes the
  former cookie/localStorage and HTTP/WS disagreement: browser host-only
  `Secure`/`HttpOnly`/`SameSite=Lax` cookies, native secure-store bearer credentials,
  channel-bound HTTP/WS-upgrade authentication, credential-free `Hello`,
  memory-only scoped match grants, CSRF/context and server-owned
  rotation/expiry/revocation/lifecycle. Browser JS-readable credential storage
  and plaintext native fallback are forbidden. The policy is decided; actual
  enforcement and [S01–S14 evidence](../../verification/session-contract/README.md#required-acceptance-scenarios)
  remain missing. This spec implements no credential persistence or demo auth
- Doc 03 §2 proposes register/login/logout/refresh and `GET /api/v1/me`, but
  implements none. It does not define public-profile reads, edit-profile fields,
  friend search/graph/request operations or their error/disclosure policy
- The conceptual users schema requires a unique handle but the artwork asks
  for display name/email/password. Display-name support, Unicode/length rules,
  identifier normalization, handle creation and registration/session disposition
  need a real approved contract. Do not derive a handle from a name or invent a
  maximum/password rule in the UI
- Presence freshness, permitted last-seen detail, relationship visibility and
  invite lifetime/conflict/idempotency require service-owned typed facts. The
  documented 500 ms fan-out and 5 s durability debounce are implementation
  targets, not an approved freshness/expiry policy

No provider/OAuth/reset flow, security settings, new credential grant, account
verification flow, achievements or rating backend is designed by this slice.
Terms use the actual agreement identity/version and URL if signup requires it;
no invented checkbox, placeholder policy, or prechecked consent.

[#49 foundation](https://github.com/loveoverflowcom/tabula/issues/49) owns
shared components/theme; this slice reuses its maintained specification and
current CSS rather than declaring a complete DOM kit delivered.
[#55 lobby](https://github.com/loveoverflowcom/tabula/issues/55) owns room/
ready/queue and online match handoff. A permitted play-invite action in Friends
depends on that real owner and does not implement or duplicate those screens.
Both linked issues and their empty comment threads were checked for this review.

## Proposed typed-data ownership

Names here describe required adapter facts, **not new exported Rust/wire types**.
Phase 4/5 owners select final types/endpoints, bounds and compatibility evidence.
Wire changes retain I-13; frontend never parses arbitrary JSON into authority.

| Fact / operation | Authority owner and scope | Consumer rule |
|---|---|---|
| Session disposition | Identity/session service: resolving, signed out, authenticated, expired/revoked, unavailable; authenticated subject and current grant scope | UI loading is separate from signed out; unknown/offline is never authenticated |
| Auth form contract | Identity service: supported identifier/fields, syntax and credential constraints, accepted terms, public error mapping and response disposition | Native inputs use the same approved rules; no account-existence lookup on blur |
| Auth result | Identity service plus session adapter: accepted/rejected/indeterminate result and current session evidence | A 2xx, toast or locally toggled flag is insufficient to declare signed in |
| Self profile | Permission-checked profile service: immutable account ID, allowed display fields, read capabilities and revision | Self comes from authenticated subject, not handle/name/URL equality |
| Other profile | Profile service: public/viewer-permitted fields and disclosure decision | Separate response shape; do not fetch self/private fields then merely hide them |
| Statistics/history | Real ratings/outcome/history owner: named metric/unit, source, coverage and observed/as-of time where applicable | Missing is absent/unavailable, not zero; no locally computed wins/rating/streak |
| Friend list/search | Social service: viewer-permitted safe identity, stable ID, pagination and relationship disposition | Display names can collide; exact target comes from returned ID, not text/row index |
| Relationship/invite | Social service: kind, stable request ID, actor/recipient scope, revision, status, expiry and permitted next actions | Friendship request and room/game invitation are distinct resources |
| Presence | Lobby service: permitted state, observation/freshness information, stream ordering and visibility scope | No own socket, sample badge or cached database row proves another user online |
| Local presentation | Current client preference adapter: locale/theme/motion/audio/density; ephemeral draft/focus/filter/request generation | No account identity, permission or canonical state in device preferences |

Permissions are checked by the server on **every read and mutation**. UI allowed
actions improve clarity; they are never the permission boundary. A returned action
is not a durable grant after logout, account change, relationship/privacy change
or expiry. Refusal must not expose private profile, blocked-by identity, hidden
presence, request existence or credential/account diagnostics.

## Route, back and local escape contract

| Route (future unless noted) | Entry / exit |
|---|---|
| `/login` and `/register` | Public task documents; resolve service/session availability before collecting credentials. Switch form without carrying a password; successful auth consumes one validated return intent |
| `/u/:handle` | Resolve through permission-checked service to account ID and permitted view; self/other cannot be selected through a query parameter. History panel retains [#52's gates](results-replay.md) |
| `/friends` | Requires a current viewer session; signed-out state offers meaningful login and current Library escape, without requesting private rows |
| `/games`, `/games/:id` (current) | Local escape uses current registry availability and detail/setup; never auto-starts a match |

Return intent is client-local navigation context, never authorization or an
arbitrary `redirect` URL. Parse once through a typed route allow-list, validate
canonical path/parameters and recheck destination availability and permission
after auth. Default is `/games`. Allowed targets are current Library, a known
registry detail/setup, and, once implemented, the permitted profile/friends
document. Never automatically continue a mutation, invitation or match launch.
Reject external/protocol-relative URLs, schemes, credentials, backslashes,
control characters, encoded separator/dot traversal, double-encoded ambiguity,
unknown parameters, nested redirects and oversized input. Same-origin alone is
insufficient: reject admin, auth loops, logout/refresh endpoints and direct
gameplay targets. Exact parsing bounds and canonicalization belong to the future
route adapter; test allow/reject partitions before enabling it.

Do not carry email, password, session credential, join token or private account
data in URL/history/referrer, route state, analytics or a return context. Replace
the auth history entry on successful authorized navigation, so Back does not
replay a form POST or trap the user in an auth loop. Back/Forward/reload/deep-link
entry always re-resolves current session/permissions; never auto-submits.
On ordinary route arrival focus the heading; user-chosen form editing and modal
dismissal have their own focus behavior below.

Local escape stays reachable during pending, invalid, expired, unavailable or
offline account work. Return to the current Library or validated invoking detail
and keep only public setup/navigation context. Registry `normalize` and
`resolve_launch` still determine supported mode/runtime. At this base the opt-in
host supports local two-human play; a generic local label must not promise every
catalog game, bots, saved state or online play. An unbound host shows its current
reason. The user explicitly chooses a new local game through the existing flow.

## Request lifecycle and privacy

Use independent session, resource and operation states; unavailable service is
not empty data, permission denial is not offline, and stale presence is not
offline presence. Maintain a route/resource/viewer/session generation and an
operation identity tied to the submitted revision. Double click, repeated Enter,
retry, Enter during composition, or stale callback cannot produce another local
submission or navigation. Retire pending UI work on departure, changed target,
account/session change or cancellation. A late response cannot resurrect private
data, steal focus, reopen a sheet or navigate after a newer user choice.

Cancellation prevents local completion effects; it does **not** prove a dispatched
server mutation was rolled back. After timeout/cancel/reconnect, reconcile the
same authorized operation through its supported status/idempotency contract
before enabling another create/accept/cancel. Register and invitation retries
must not invent a new operation to turn an unknown result into duplicate state.
Login retry/session creation policy is likewise a session-owner decision.

Passwords remain only in the live native input/necessary transient request and
are never persisted, logged, debug-formatted, sent to analytics or copied to
navigation. Clear after an attempted server operation finishes or the route is
left, session changes, or the page is restored; permit password-manager refill.
Preserve only appropriate non-secret form input for correction. Do not suppress
paste/autofill, trim/normalize passwords, or save credentials on behalf of users.
No credential entry or usable submit appears when no real auth adapter exists.

Account-scoped responses/caches must be partitioned by authenticated viewer and
grant. On expiry/revocation/account change, retire requests/subscriptions and
clear now-forbidden profile, friends, invites and presence from UI and caches.
Never reuse another account's rows or label cached private data authenticated
while offline. Device-only preferences may survive logout; they neither become
account metadata nor grant permissions. No token/password in `prefs.v1`.

Error copy maps structured public dispositions to [shared keys](accounts-social-copy.md).
No raw response body, SQL error, stack trace, email echo, account existence,
provider linkage, suspension reason or secret is surfaced. Syntactic field
errors concern entered values only. Credential rejection uses one generic
message for missing account, bad password and disallowed authentication.
Registration must reconcile public success/failure behavior and timing at its
server boundary; generic frontend copy alone does not prevent enumeration.

## Foundation mapping and accessibility

Keep the current common shell navigation/theme. Accounts never acquire
game-specific Chess/Xiangqi artwork. The inspected auth artwork still has a
large illustration/hero: remove it for a compact task-first form. Its profile
edit, online-visibility selector, Remember/Reset and sample social controls are
conditional design suggestions, not delivered capabilities.

| Foundation component | Account/social use |
|---|---|
| Filled primary / tonal secondary / quiet labeled action | Login/Create account when real; Search/Add friend where supported; local escape and Back visibly quieter but always reachable |
| Filled labeled field | Native email/password and approved display metadata; search with supporting scope; `container-high`, label and functional error/focus boundary |
| Contained list / tonal section | Permitted profile facts and friend/request rows; clear supporting identity/time; separate row link from trailing buttons |
| Connected group / tabs | Only actual supported list/request panels or approved visibility choices; native radio or tab semantics, not decorative chips |
| Badge / banner / progress / empty | Explicit signed-out/expired/unavailable/unknown/stale/operation states; text/glyph, visible reason, no color-only green online dot |
| Dialog / sheet | Only bounded real supported actions; titled, focus trap and restoration, clear cancel, no board/game mutation |

Use generated CSS from `tokens.toml`, four atomic schemes, `surface`,
`container`, `container-high`, semantic on-colors, `primary` and `danger`,
foundation size/shape roles and 3 dp focus with surface separation. No new
library, purple palette fork, decorative shadow/border or blanket radius change.
Keep functional field/selection/error/focus outlines, especially when high
contrast flattens tonal surfaces. Main task title is sans; mono is for stable
IDs/counters, never the entire form. Primary is 56 dp when space permits,
routine 48 dp; every target stays at least 44 × 44 logical dp.

DOM order is toolbar/heading, context/status, task fields/list, primary action,
recovery/local escape, secondary links. Single-column at 320/390 dp; at 768 dp
supporting panels follow or sit beside content only when labels fit; at 1440 dp
cap useful content and avoid stretching a login form across the screen. Reflow
long Vietnamese names/handles, unbroken IDs, errors and controls at 200% zoom,
short landscape, keyboard viewport and safe-area insets. No fixed-height form
or horizontal page overflow. Preserve document zoom and local focus on theme/
locale changes; reduced motion keeps status and completion immediately readable.

Use persistent `label`/input association, stable IDs/names, correct autocomplete,
described help/error IDs and `aria-invalid` only after relevant validation.
Do not run submit/validation mid-IME composition. On explicit invalid submit,
focus the error summary/first invalid field using one consistent route policy;
the summary links to fields. On blur/change, never steal focus. Async generic
errors use one persistent alert; busy/success uses a separate polite status.
Do not announce the whole list or presence clock on every update. Password
visibility is a named `type=button` toggle with exposed state, fixed 44 dp hit
area and preserved caret/focus; it never submits. A dialog restores the invoker
or logical successor. Disappearing/disabled request controls cannot strand focus.
Real keyboard, password-manager, IME and AT behavior remains runtime acceptance.
