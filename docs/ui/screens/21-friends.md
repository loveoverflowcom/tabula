# 21 — Friends

Issue #54 PR A; [shared owners/gates](accounts-social.md), [foundation](foundation.md),
[copy](accounts-social-copy.md) and [verification](accounts-social-verification.md).
Source base: `develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`.
Reference: pinned `06-accounts/screens/21-friends.svg`, inspected static raster
and mobile/state PNG. `/friends` and its typed social service are absent here.
PR C waits for real lobby/social APIs and the Phase-5 shell gate.

## Task, list and search

Find a permitted friend, understand known relationship/presence state, and
perform only an available authorized request. Use compact tonal contained
rows with safe identity, supporting handle/ID and freshness text; keep row
navigation distinct from trailing buttons. At desktop requests follow/beside
the list; at compact width stack sections with explicit headings. A supported
Add friend/Search action gets primary emphasis; routine profile/row actions
are tonal/quiet. No hero, ornamental border/shadow or invented online counts.

List and search are separate domains: filtering successfully fetched friend
rows does not query all accounts or create a relationship. Global lookup exists
only if a real service exposes it with approved query/normalization/bounds,
pagination, rate-limit and discovery/disclosure rules. Identify the search
scope in a persistent label. Prefer an exact service-supported handle/account
ID; never guess an ID from a display name, query private email, or treat a DOM
row index as identity. Every returned target must be stable-ID and viewer-scoped.
If duplicate display names are returned, show permitted distinct handles/IDs
before an action, without exposing extra private identifying fields.

Native search input supports Unicode/IME. Do not issue requests during composition;
use the approved explicit/debounced search behavior only after commit. Empty
query shows current permitted list, not a directory dump. Only a successful
authorized list read can show No friends. Zero filtered results and unavailable/
denied/failed search have different copy. Leave Library/local escape available.

## Relationship and request state machine (proposed)

Social service owns the resource kind, IDs, actor/recipient, revision, expiry,
allowed actions and resulting state. Friendship requests and room/game invites
are distinct: accepting a game invite does not accept a friend request; a friend
request never implicitly joins/creates a room. Doc 05's proposed `MatchInvite`
is not a friend-graph API.

| Authority status | UI and permitted action, only when supplied | Completion/conflict rule |
|---|---|---|
| No relationship | Supported Request friendship for a resolved permitted target | No local row creation or implicit friendship |
| Outgoing pending | Pending with allowed expiry; Cancel only for authorized sender | Await confirmed cancelled state; Cancel does not retract an already accepted request |
| Incoming pending | Safe sender identity; Accept/Decline only for current recipient | One operation; permission and expiry rechecked at service commit |
| Accepted | Confirmed relationship; safe Profile and independently supported invite action | No automatic room creation, match launch, seat reservation or voice access |
| Declined | Safe terminal status if viewer may see it | No immediate automatic resend or disclosure of private reason |
| Expired | No Accept; clear reason and permitted next action | Local elapsed clock disables apparent validity but cannot grant new actions or invent authoritative terminal state |
| Cancelled | Terminal; no stale Accept/Cancel controls | Late response cannot resurrect pending; refresh current permitted disposition |
| Restricted/denied | Generic action-unavailable boundary | Do not disclose who blocked whom, hidden user/request existence or private policy |
| Mutation pending | Named action/status, no duplicate/competing mutation | Do not optimistically mark accepted/declined/cancelled |
| Timeout/result unknown | Persistent indeterminate status, adapter-owned reconciliation | Same request/operation identity; no blind new create/retry |
| Conflict/unauthorized/expiry during mutation | Refresh authorized current state; retain safe context/error | Do not overwrite a newer terminal revision with an older pending/success callback |

No Remove/Block/unblock controller or privacy/security setting is enabled just
because the artwork mentions it. Restriction/blocked outcomes still affect read
and action permissions and must be safely represented. A separately approved
mutation requires its own real contract. Invitations to play are available
only through a real room/invite owner; matchmaking and voice are out of scope.

Repeated click/Enter, racing Accept/Decline/Cancel, retry and reconnect require
service idempotency and conflict semantics, not only disabled CSS. The local
operation identity is tied to request ID, revision, viewer/session and action.
Pending keeps the applicable safe row and disables conflicting actions; unrelated
navigation stays usable. Cancellation retires local completion effects but may
not undo a dispatched mutation. On return reconcile before another mutation.
Success/terminal state appears only from current authoritative disposition.

## Presence and stream freshness

Use the shell's single lobby/social connection, with authorized initial snapshot
and ordered deltas (doc 03 §14.3; doc 04 §2.2). No per-friend/match socket,
polling network added by this spec, or presence inferred from your own socket.
Presence is rebuilt from live sessions after server restart; database durability
is not evidence of a presently live friend. The 500 ms coalescing/5 s transition
debounce targets do not define client freshness. Service must supply sufficient
observation/expiry/ordering information or the UI falls back to Unknown.

| State/evidence | Badge/supporting text | Allowed inference |
|---|---|---|
| Unknown / no permitted observation | `accounts.presence.unknown` | No offline or online claim; omitted/private observation cannot imply blocking |
| Online, fresh permitted fact | Online plus actual checked/as-of time when useful | Online is not an invitation permission or seat/room availability |
| Offline, fresh permitted fact | Offline; last-seen only if specifically disclosed | Never expose a timestamp supplied outside current visibility scope |
| Busy/in-game, if defined and permitted | Exact authority-provided state; generic label otherwise | No private game/room/match ID or clickable join/spectate link without separate permission |
| Stale / disconnected / freshness expired | Stale/Unknown plus permitted last observation time | Do not leave a green Online assertion or relabel it confirmed Offline |
| Snapshot/delta gap or server generation change | Mark stale/unknown, retire old ordering, request real resync | No state assembled from missing, cross-account or old-generation events |
| Hidden/permission withdrawn/session expired | Unknown or concealment-safe absence; clear forbidden time/state | Hidden is not Offline; never use a privacy change as blocked-by disclosure |

Newest authoritative revision wins within its stream/resource generation.
Reject duplicate/late/out-of-order events; gaps/resync replace stale facts only
after current authorization. Old response for previous search/viewer/request
cannot alter a newer list. UI time may age a granted observation according to
owner policy; it cannot create Online, Accepted, or a permission grant. Show
localized actual timestamp/age when permitted and known, not invented last seen.

## Permission, interruption, focus and acceptance

On signed out/expired/revoked/account change, stop subscriptions/requests and
clear forbidden graph/invites/presence; re-authentication starts new scope.
Search/read/mutation enforce current permission server-side. A relationship,
badge or returned allow-action cannot grant room/seat/spectator/chat/voice rights.
Errors are safe structured dispositions, never raw server/account diagnostics.

Route arrival focuses Friends heading; Tab visits search and meaningful actions
in list/source order. Native lists and separate named buttons avoid nested row
activation. Announce search/list result once and request completion/error once;
do not read every presence delta or time tick. If a terminal row removes the
focused action, restore focus to that row's surviving action/heading or list
heading, not body. Any bounded sheet stores invoker, traps focus, cancels safely
and restores it; Back/Forward never reopens or auto-accepts a request.

Verify 320/390/768/1440 dp, short landscape, long Vietnamese name/handle/ID,
200% zoom, four schemes, full-contrast unknown/stale text and ≥44 dp controls.
Runtime cases: empty/filtered/no-match vs failed/denied reads; colliding names,
IME/search race, every request terminal state, duplicate/conflicting mutations,
expiry at commit, unknown result, stale/gapped/restarted stream, visibility
withdrawal, forged actor/recipient/resource, account switch, leave/late response,
keyboard/AT/focus/Back. Doubles may establish reducer behavior only; actual
social/permission integration and real rendered AT evidence remain separate.
