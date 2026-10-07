# Isolated account and social implementation

This is the current #54 implementation contract for the explicitly enabled
[ADR-0044](../../adr/0044-isolated-account-registration-social.md) composition.
It extends [the version 1 session slice](account-state-isolated.md). That slice's
unavailable registration/social behavior still applies to the legacy composition;
it does not describe the new version 2 routes. Production startup remains closed.

## Routes and authority

| Screen | Route | Data owner | Supported task |
|---|---|---|---|
| 15 Login | `/login` | Current version 1 session context and existing Kanidm adapter | Explicit provider continuation; authenticated users can open self profile |
| 16 Register | `/register` | Verified, cookie-bound enrollment grant and durable account policy | Verify an existing provider identity, choose handle/name, establish a Tabula account without a session |
| 20 Self profile | `/me` | Current session plus version 2 profile projection/revision | Read name/handle/visibility, explicitly edit approved fields |
| 20 Other profile | `/u/:handle` | Current viewer and target disclosure policy | Read only the currently permitted name and handle |
| 21 Friends | `/friends` | Current viewer, durable participant permissions and one shell-owned lobby socket | Search visible profiles, manage explicit requests, read current authorized friend observations |

All routes retain the foundation's local library escape. Local gameplay is a
separate document and does not acquire an account prerequisite from these screens.
Navigation destinations are fixed application paths; a provider URL is accepted
only from a validated current start response. URL text, DOM controls, a socket
Hello, persisted preferences and a public account ID confer no authority.

## Shared presentation

Use the existing semantic tokens and foundation form, status, action and list
classes. A short heading and task introduction precede a tonal status group;
current task actions receive the strongest emphasis. Recovery and navigation
remain labelled secondary controls. Readable spacing, shape and tonal grouping
carry hierarchy; decorative borders and shadows do not frame every row. Outlines
remain meaningful for focus, field errors and selected controls.

En/vi keys cover headings, form help, statuses, request decisions and observation
times. Names wrap; handles and opaque identifiers cannot force horizontal scroll.
Small-screen controls can wrap or stack without changing their permission or
action order. No decorative dashboard, invented rating, match history, statistic
or achievement is shown. Avatar treatment is neutral presentation, not a remote
profile field or provider image.

## Registration and form states

| State | Visible facts and actions | Transition rule |
|---|---|---|
| Signed out, enrollment enabled | Explicit verify-with-Kanidm action; local escape | Current preauth CSRF authorizes start; no password or terms form |
| Provider pending | Bounded pending status | Matched single-use provider callback only; fixed `/register` destination |
| Verified grant ready | Labelled handle and display-name fields, explained public visibility | Approved field policy and matching document generation are required |
| Invalid fields | Associated field error and retained draft | No submit during composition; committed Unicode is preserved |
| Registration pending | One disabled submit path | The same operation and exact payload are retained for outcome recovery |
| Durable accepted | Explicit account-created-without-session status and Login navigation | No automatic sign-in or inferred credential |
| Rejected/expired | Safe rejection and explicit restart/recovery | Do not identify duplicate handle, identity existence or policy reason |
| Outcome unresolved | Unresolved status and same-operation retry | No changed payload or second account creation is inferred |
| Unavailable/offline | Clear unavailable state, retry when current and local escape | A browser network hint cannot decide a server outcome |

Handle policy is 3–32 lowercase ASCII letters, digits or underscores. Display name
is 1–64 Unicode scalars and at most 256 UTF-8 bytes, with no controls or edge
whitespace. Labels and help are programmatically associated; handle uses
`autocomplete="username"`, display name uses `autocomplete="nickname"`.
Kanidm owns credential fields and their password-manager behavior. There is no
approved agreement in this composition and no invented agreement checkbox.

## Profile read, edit and disclosure

Self data includes immutable account ID/handle, display name, visibility,
resource revision and authority observation time. Only display name and visibility
are editable. A mutation uses current credential/CSRF, a canonical operation ID
and expected revision. A known 204 commit is followed by an explicit current GET;
it does not synthesize the new profile in the browser. Retry after ambiguity uses
the original operation. Conflict requires current refetch and user reconciliation.

Visibility is public to signed-in viewers, accepted friends, or self only. The
other-profile response omits self visibility and revision. Missing, private,
disabled and forbidden resources have the same safe unavailable presentation.
An authenticated profile without historical data does not acquire fake statistics.
Permission-narrowing profile and relationship writes share target publication
exclusion with reads and bounded socket handoff.

An older invited account may lack profile metadata. It keeps its immutable
identity, and Friends remains unavailable until a durable profile exists. The
supported recovery is explicit sign-out and verified registration to complete
only that missing profile when enrollment is enabled. Captured account epochs
fence this path; it cannot overwrite an existing profile or revive a disabled
identity. The accepted registration message covers account readiness rather
than disclosing whether the identity previously existed.

## Friends and observation states

Search is bounded, literal and current-viewer authorized; empty search does not
dump the directory. A request row is a participant projection with sender,
recipient, status, revision and authority timestamps. Send, accept, decline and
cancel are explicit HTTP mutations with scoped operation receipts. Only sender
can cancel; only recipient can accept or decline. Duplicate retry returns the
durable result. Conflicting operation reuse or revision requires reconciliation.
Terminal declined, expired or cancelled states may offer an explicit new Send
when current policy permits; they never automatically create a new request.

The single app-owned `tabula-social.v2.json` socket sends bounded full snapshots,
not mutation commands. A scope replacement, gap or resync masks previous private
facts until a new current snapshot. Navigation does not create a second socket;
rendering on a new route requires its fresh document authority. Hidden/frozen,
offline and account-switch transitions retire private output and pending work.

| Presence | Meaning |
|---|---|
| Unknown | Authority has no permitted fresh observation; no activity inference |
| Online | Current authorized live binding observed at `as_of_ms` |
| Offline | Trusted owner observed final attachment teardown; includes observation time and optional permitted last-seen time, without claiming the user's device or other activity is offline |
| Stale | Previous observation lost freshness; retained timestamps, when permitted, do not assert current activity |

Presence is visible only to accepted peers with current disclosure permission.
The owner revalidates peer session bindings, epoch, expiry and revocation. A badge
comes from that observation and timestamp, never from a local toggle. Request
expiry uses trusted authority time and is checked again at the durable decision
boundary, including after lock/receipt waits.

Online additionally requires a fresh positive transport observation: validated
Hello establishes the initial observation, and an exact Pong to the owner's
two-second challenge renews it, with a five-second eligibility bound.
Outbound snapshots do not count as a peer reply. Unconfirmed freshness loss is
Stale; an explicit final attachment teardown can produce a timestamped Offline
observation. Reconnect requires both current credential authority and fresh
transport evidence.

## Focus, keyboard and private output

Persistent route headings accept programmatic focus. Status/error transitions
provide appropriate live status or alert semantics; fields reference help/error
text and invalid state. Form submission respects composition events. Ordinary
Tab and Enter behavior remain usable; sign-out confirmation has a labelled
consequence, explicit Cancel and Escape recovery. A retiring private focused
control moves focus to the persistent heading before it becomes hidden/inert.

Every private profile/social region is marked `data-account-private`; document
retirement masks it synchronously. Async completions need matching viewer,
document generation and request generation. Browser storage holds only local
preferences; no password, provider token, session, CSRF, profile or social cache
is persisted. DOM accessibility-tree checks and synthetic composition events
are separately recorded from manual screen-reader and OS IME acceptance.

Executed checks and their limits are in the
[completion ledger](../../verification/issue-54-account-followup/README.md).
