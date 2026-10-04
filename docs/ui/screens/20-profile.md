# 20 — Profile

Issue #54 PR A; [shared boundary](accounts-social.md), [foundation](foundation.md),
[copy](accounts-social-copy.md) and [verification](accounts-social-verification.md).
Source base: `develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`.
Reference: pinned `06-accounts/screens/20-profile.svg` and its desktop PNG.
`/u/:handle` is a future permission-checked shell document, not mounted here.

## Task and permitted data

Read the permitted identity and source-backed facts about yourself or another
account. PR B begins with **self-profile read-only** after the identity/session
and shell gates. Other-profile lookup and edit are separate approved adapter
capabilities; `GET /api/v1/me` in doc 03 is only a proposal and establishes none.

Resolve handle through the service to stable account ID, authenticated viewer,
permitted field shape and read capabilities. Self/other is authority-derived;
matching text/handle, URL flags, avatar initials or local sample identity cannot
grant ownership. Missing/private/forbidden responses use the owner's public
disclosure policy, with no private object downloaded just to hide its fields.

| Content/action | Owner and display rule |
|---|---|
| Display name, handle, avatar | Only approved returned fields; safe text rendering, no HTML. Name can collide; stable ID is the target. Long Unicode names and unbroken handles wrap |
| Self/private metadata | Only explicitly granted self DTO fields; never included in other-profile DTO/cache. No password/token/device/grant dumps |
| Biography/about | Omit until actually defined by profile schema; sample artwork text is not account metadata |
| Statistics | Real metric/unit/source/coverage/as-of evidence. Distinguish no eligible history from unavailable/denied/error; missing is not zero |
| Ratings/achievements/streak/accuracy | Omit unsupported metrics; no inferred achievements or computations from local games/mock counts |
| Match history | Separately gated [#52 history](10-history.md) with its own permissions/persistence; no generated local history rows |
| Presence/visibility | Separate real lobby/visibility contract and viewer scope; use [Friends' freshness rules](21-friends.md#presence-and-stream-freshness), not a green signed-in dot |
| Edit/Add friend/Invite | Only real capability with current permission. No edit control in read-only B; friend relationship does not grant room/seat/spectate rights |

The artwork's editable fields, Save and Friends/Hidden/Public selector are not
implemented facts. No local-save fallback may claim to update the account or
privacy policy. Device presentation preferences remain [screen 13](13-settings.md),
not editable account identity. Default sample “Bạn” is design data, not a guest
account. Local play does not create a profile.

## State, route and permission matrix

| State | Presentation / recovery | Security and navigation rule |
|---|---|---|
| Session/profile loading | Named loading, bounded known-shape skeleton; Library escape | No self/edit/private fields before current permission response |
| Signed out | Sign in for supported self task; public other profile only with a real permitted public-read contract | No cached authenticated account/graph or guessed public visibility |
| Self read ready | Allowed read-only identity and supported source-backed facts | Subject ID matches verified session; capability does not outlive current scope |
| Other read ready | Only allowed public/viewer-scoped identity/facts | No email, flags, blocked reason, private history/presence unless explicitly disclosed |
| No eligible history | Plain explanation from a successful authorized read, supported next action | Missing statistics/provider is unavailable, not zero or no games played |
| Profile unavailable/private/missing | Shared safe unavailable copy according to concealment policy; Back/Library | No existence/private-field confirmation beyond allowed response |
| Offline/stale response | Named freshness/failure; only still-permitted non-sensitive data if approved policy supports it | No stale permission grants; deny private reads without current authority |
| Expired/revoked/account changed | Clear forbidden data, stop subscriptions/requests; supported Login and Library | Late self/other response cannot populate new viewer's UI/cache |
| Retry/new handle/Back/Forward/deep link | Resolve current target/session; preserve only allowed list/history navigation context | Retire older target generation; no stale page/focus/navigation overwrite |

Default route focus is the Profile heading, with self/other context in text.
The header is a compact identity row followed by tonal permitted-data sections;
history/statistics panels follow only when real. Back returns to its permitted
invoker, otherwise current Library. Identity links and trailing actions are
separate activation targets. Do not link a blank/no-permission placeholder.

## Future edit boundary, separate from B

Edit requires an approved field list/update API, server validation/permission,
version/conflict semantics and verified persistence. Only then expose an Edit
action and shared form. Pending preserves the dirty non-secret draft and locks
duplicate save; failed save retains it. A conflict fetches permitted current
values and offers explicit reconciliation, never silently overwrites newer
server state. Cancel discards only the local draft and restores invoker focus;
it cannot undo a dispatched server update. Success requires the confirmed
current server revision, not a changed label or localStore write.

Visibility mutation is its own authority/privacy operation, not ordinary
appearance. On permission narrowing clear now-forbidden fields/presence and
retire stale requests. Field values cannot be serialized into URLs/telemetry.
This specification adds no update endpoint, avatar upload, account deletion,
security settings or privacy controller.

## Layout, AT and required evidence

At 320/390 dp stack identity and facts, then supported actions; long name/handle,
200% text, keyboard viewport and error paragraphs grow without clipping. At
768/1440 dp use supporting columns only if readable; no large decorative hero.
Use foundation tonal lists, sans headings, full-contrast reasons, functional
focus/selection/error outlines and ≥44 dp targets in all four schemes.

Native document headings/lists and labeled links/buttons provide source-order
keyboard navigation. Passive statistics/presence are not Tab stops; announce
load/error or permission loss once, not each number on every refresh. Name and
handle have separate labels, so truncated initials do not become the accessible
identity. Edit sheet, if later real, follows trap/Cancel/invoker rules.
Required runtime evidence: self vs other, forged subject/handle, denied/private,
no-history/unavailable metric, long name/handle, account/permission change during
read, stale callback, safe Back/deep link, keyboard/AT/zoom/theme. Read-only B
cannot claim edit/privacy mutation or history acceptance.
