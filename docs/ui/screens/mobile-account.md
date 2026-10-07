# CMP account surfaces — issue #103 UI draft

[ADR-0046](../../adr/0046-mobile-account-surfaces.md) opens this bounded native
presentation slice. It follows [the CMP shell](mobile-shell.md) and uses the
[foundation](foundation.md), current web task semantics and generated tokens.
The implementation starts from `develop @ a386524` on 2026-10-07. It does not
alter the native GameHost prototype already merged in PR #108.

## Current web capability and native boundary

The older account artwork and issue text are design/history sources, not the
current capability inventory. [ADR-0038](../../adr/0038-isolated-invited-kanidm-web-auth.md)
and [ADR-0044](../../adr/0044-isolated-account-registration-social.md) define
implemented isolated web account semantics. The current
[isolated web contract](accounts-social-isolated.md) and
[web acceptance ledger](../../verification/issue-54-account-followup/README.md)
remain authoritative for that separate composition.

| Task | Current isolated web behavior | This native draft |
|---|---|---|
| Account/Login | Current session; explicit Kanidm continuation; no password in Tabula | Typed state and task introduction; native provider continuation unavailable |
| Register | Verify an existing Kanidm identity, then choose handle/display name; durable acceptance creates no session | Explain unavailable native enrollment; no fields, agreement, provider callback or simulated account creation |
| Self Profile | v1 immutable ID; feature-enabled v2 permits profile read/edit | Read only current adapter-supplied identity and permitted optional facts; no local edit/privacy controller |
| Other Profile | Current-viewer permission-checked `/u/:handle` in `account-social` | No native lookup route or target selection; an unknown route cannot select self |
| Friends | Scoped search, durable send/accept/decline/cancel and one authorized presence stream | Explain unavailable native lists/requests/presence; no mock friends, empty-success list or online badge |

The web `account-social` feature is non-default; a more capable backend cannot
activate absent frontend code. Its real provider/PostgreSQL/browser evidence is
not native evidence. Production service startup remains closed. Native copy
says "in this build" or names native support, rather than denying web capability.

Kanidm remains the sole credential issuer. Tabula enrollment is not provider
person/credential creation. No native password, email, reset, OAuth grant or
credential-persistence flow is invented here. No agreement exists in the isolated
web product; its absence does not authorize inventing native legal text.

## Routes and task hierarchy

| Destination | Shell identity | Main content |
|---|---|---|
| Account | `/account` | Compact neutral identity/status group, current task navigation and native capability explanation |
| Login | `/login` | Sign-in task, current account disposition and explicit native-provider limitation; confirmed identity may navigate to self Profile |
| Register | `/register` | Create-account task and unavailable native enrollment explanation |
| Self Profile | `/me` | Read-only identity only while Authenticated; otherwise named current state and supported public recovery |
| Friends | `/friends` | Friends task, unavailable native list/request/presence explanation |

These fixed, parameter-free paths carry no account ID, profile value, return URL,
callback code or authority. Shell parsing rejects extra queries, schemes and
unsupported account/other-profile paths. Account remains the selected top-level
destination on its nested tasks. Repeated activation does not duplicate the
same current route. Visible and system Back return to the caller; Browse games
returns to Library. Account screens never mount GameHost or load gameplay packs.

Account navigation is a task action even when that task's service is unavailable;
it does not imply a successful auth/social action. Strongest emphasis belongs to
the available task/recovery, with Back, Browse games and other destinations tonal
or quiet. A missing provider never gets a working-looking Continue action.

Saved state contains bounded public route identities only. Recreating at `/me`
re-resolves the current port; it cannot restore a prior identity or pending
operation. Existing public discovery state and game Back ownership stay separate.

## State and adapter ownership

`AccountSessionPort.state` is the presentation input. Its operations are refresh,
signOut, cancelPending, invalidate, onForegroundChanged and close. Only the port
may supply a current account outcome. The default
`UnavailableAccountSessionPort` is fixed at `NativeAdapterMissing`, with no
network integration, timed pretend work or fixture fallback.

| State | Display and allowed response | Private-data rule |
|---|---|---|
| Unknown | Session not yet checked; an actual supported refresh may be offered | No account identity inferred |
| Loading(operation) | Named checking/sign-out status and busy affordance; no duplicate dispatch; Cancel when supported | Hide old identity while current outcome is unresolved |
| SignedOut | Explicit signed-out disposition; task navigation and Library escape | Comes from the port, never an unavailable-service default |
| Authenticated(identity, canSignOut) | Read-only permitted facts; self Profile navigation; confirmed sign-out capability when supplied | Only the current adapter/lifetime's identity is rendered |
| Expired | Explain that the session is no longer confirmed; permitted recovery and Library escape | Clear old identity; no retained signed-in badge |
| Unavailable(reason) | Safe localized native limitation; available public navigation | Not SignedOut, no-profile or No friends |
| Error(reason, retry operation) | Safe error; retry only the operation actually supplied by the port | No raw diagnostics or old identity |

Refresh revalidates current presentation facts, not credential rotation. Calls
and completions run serialized on the platform owner's thread; StateFlow is not
itself a concurrency lock. Operation identity/lifetime fences apply to repeated
clicks, cancellation, backgrounding, invalidation, adapter replacement and
closure. Retiring work must mask identity before any old completion can publish.
The app forwards lifecycle ON_START/ON_STOP through `LocalLifecycleOwner`, closes
the old port on disposal/replacement and cancels pending work when leaving its
task route. Navigation alone starts no refresh. Foreground return requires
current explicit port resolution; cached identity cannot restore itself.
Cancellation retires local completion effects but does not claim undo of a
dispatched server operation. Native provider/deep-link integration is absent, not covered by the
read-only port or its doubles.

Sign-out appears only with a current `canSignOut` capability. A labelled
confirmation explains the session consequence and has explicit Cancel. Back or
Cancel dismisses it without dispatch; repeated confirmation submits once. The
pending state hides identity. Only a confirmed SignedOut port result may claim
success, scoped to this device rather than global logout; error/ambiguity remains
non-authorizing. Stop waiting during pending sign-out retires the local response
and preserves unresolved-logout suppression. It cannot restore identity, run a
refresh over that suppression, or claim that sign-out was undone. Retry targets
the original unresolved sign-out. This quarantine is in-memory only. A future
native adapter must reconcile unresolved revocation before a later bootstrap;
secure-store deletion and process-restart suppression are not implemented here.
The default port performs no sign-out and cannot revoke a real session.

## Permitted profile facts and avatar

Current Rust boundaries are read-only source references, not a new Kotlin wire:

- `SelfProfileResponse` v1: `version`, `account_id`
- `SelfAccountProfileResponse` v2: `version`, `account_id`, `handle`,
  `display_name`, `visibility`, `revision`, `as_of_ms`
- `OtherAccountProfileResponse` v2: `version`, `account_id`, `handle`,
  `display_name`, `as_of_ms`; self visibility/revision are absent

Sources: `crates/tabula-session-http/src/lib.rs` and `src/accounts.rs`.
The current subject and operation/lifetime must match in addition to DTO shape
validation. A typed `AccountId` is identity data, not a credential or permission.

The native read-only identity exposes account ID and optional permitted display
name, handle and self visibility as an optional profile group only. A missing
profile group is omitted; there is no sample "Bạn" profile. Visibility is a fact, not an editable selector;
the web meanings are Public to signed-in viewers, accepted Friends, or self-only
Private. Revision/observation data, when not supplied by the adapter, is not
invented. No email, biography, rating, statistic, history, achievement, presence
or provider metadata appears. Unavailable history does not mean zero played games.

The managed neutral human avatar is shared with shell chrome. It conveys no
session/presence status. An optional already-loaded
`AccountAvatarImage(identity: AccountIdentity, image: ImageBitmap)` may display
only while its identity is the exact active Authenticated instance (`===`).
Matching the same account ID or equal profile values is insufficient: a refresh
or authority replacement retires the old bitmap. A mismatched or retired
identity returns to neutral. The synthetic image loader is preview-only.
Decorative pixels cannot provide accessible identity in place of labelled text. No remote URL, upload, provider-photo lookup or
account image-delivery capability is added to backend DTOs.

No identity, profile, avatar image, session credential, CSRF, provider token or
pending operation enters `rememberSaveable`, preferences, navigation paths,
telemetry or logs. Public route restoration carries no authorization.

## Copy and adaptive presentation

The maintained vi/en vocabulary aligns with web: Account / Tài khoản,
Sign in / Đăng nhập, Create account / Tạo tài khoản,
Your profile / Hồ sơ của bạn, Friends / Bạn bè,
Browse games / Xem thư viện trò chơi, Back / Quay lại, Cancel / Hủy.
Complete localized messages describe unknown/loading/signed-out/expired/error
and each native limitation. No email-exists, duplicate handle, hidden profile,
denial reason or raw server text appears in errors.

Use generated semantic token roles for purple emphasis, warm-paper canvas,
contained identity/status groups, focus and errors; author no account palette.
No oversized hero, decorative dashboard, repeated ornamental borders or shadow
frame is needed. Available primary actions are visually distinct from navigation
and recovery. Persistent explanatory text remains readable without hover.

At 320/390 dp, in landscape and with 200% font scale, names/IDs/reasons wrap and
actions stack. Content scrolls within existing safe-area/chrome slots; controls
meet the generated minimum target and remain reachable above bottom navigation.
Wider layouts preserve source order and use the existing shell rail. Light,
dark, high-contrast light/dark and vi/en all retain contrast and hierarchy.
Account paragraphs and profile values use `bodyMd`; identity names and section
headings use `titleMd`, while individual field labels retain `labelLg`.
These generated roles retain the full OS font scale.
The identity card stacks its avatar above the details when the remaining row
width would be less than twelve body-text ems at the current scale. Long names,
handles and IDs retain all their characters; they are not ellipsized to fit.

Headings, labelled state regions and separate named buttons retain semantic
roles. Unknown/loading/error/permission-loss is announced at a useful transition,
not every recomposition. Private controls cannot remain focusable after masking.
Confirmation focus and dismissal return to the surviving invoker or heading.
Keyboard/Enter/repeated activation and Back must remain usable. There are no
credential fields here, so successful credential autofill/IME is not claimed.
A future enabled form must use native labels, associated help/error/validation,
IME actions and password-manager/autofill hooks, then execute those acceptance
cases; the read-only state/port cannot establish that form integration.

## Required acceptance and evidence limits

The [working ledger](../../verification/issue-103-mobile-account/README.md) records
exact commands, nonempty selected tests, outcome and evidence kind. The following are acceptance obligations, not executed claims:

- Parse/restore each fixed route; reject unsupported paths/queries; exercise nested
  Account selection, repeated activation, Back and Library escape
- Exercise every typed state with named test ports, optional/missing profile
  facts, long Unicode text, owner-matched/mismatched avatar and no fabricated data
- Cover duplicate operations, Cancel/Back/departure, late completion, adapter
  replacement, expiry, foreground/background, close and sign-out confirmation,
  known success and unresolved failure
- Assert no private identity/operation/image restoration, no credential field,
  native provider redirect, social mutation, profile edit or gameplay side effect
- Compile and test the affected shared/mobile code; run the required portable
  repository gate and affected Android build with exact toolchain provenance
- Capture and inspect actual shared CMP pixels at 320/390 dp, vi/en, four schemes,
  200% text, long identifiers and unavailable/error/loading/authenticated-double
  states; inspect target sizes, scrolling and confirmation focus separately
- Separately execute Android/iOS touch, lifecycle, TalkBack/VoiceOver and native
  builds on suitable targets; desktop Compose is not those targets

Actual native provider/secure-store/callback integration and social authority are
NOT_IMPLEMENTED in this slice. A future integration needs real provider/session
and device interruption oracles; UI doubles cannot satisfy it. A Gradle bootstrap
or native-toolchain blocker is BLOCKED, not compilation/test success. Uncaptured
screenshots and unexecuted device/assistive-technology checks are NOT_RUN. The
implementation ledger supplies actual results; no executed test is claimed here.

This draft does not close issue #103 while requested UI acceptance remains unexecuted,
replace future real native integration obligations,
establish production authentication, alter PR #108's native GameHost prototype,
or pass Phase 4/5/6, networked mobile play or store-release gates.
