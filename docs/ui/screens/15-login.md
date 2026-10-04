# 15 — Login

Issue #54 PR A; [shared owners, gates and navigation](accounts-social.md),
[foundation](foundation.md), [copy](accounts-social-copy.md) and
[verification](accounts-social-verification.md). Source base:
`develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`; reference is pinned
`06-accounts/screens/15-login.svg` and its desktop/mobile PNGs.
`/login` is a future Leptos document, not a mounted route at this base.

## Task and form

Sign in through the implemented identity/session service, or return to the
current Library without authenticating. Keep the common shell toolbar/theme,
task heading and one compact tonal form. Replace the reference's large hero/
game illustration with a short local-play explanation. The actual credential
contract determines fields; the reference email/password is a proposal.

| Field/control | DOM/owner requirement |
|---|---|
| Email identifier, if supported | Persistent label; stable `name`/ID; `type=email`, `autocomplete=username`, email input mode, no autocapitalization/spellcheck. Account-owner normalization/bounds; no live account lookup |
| Password | Persistent label; `type=password`, `autocomplete=current-password`; preserve whitespace/case; allow paste and password-manager autofill. No independent frontend password policy |
| Show/Hide password | Labeled `type=button`, exposed pressed/visibility state, ≥44 dp target; preserve value/caret/focus and do not submit |
| Remember this device | Omit under ADR-0031's initial no-remember-me policy; longer-lived login needs a superseding ADR and target evidence, never a local token-storage checkbox |
| Login | One filled primary, supported real submit only; pending text retains purpose |
| Browse games / supported local escape | Tonal visible link through the [safe escape contract](accounts-social.md#route-back-and-local-escape-contract); remains reachable during every form state |
| Register / Reset / provider actions | Register only when its real route/contract exists. No reset link, provider or OAuth flow without a separately approved implemented adapter |

No email or password is collected when authentication is unavailable. The
artwork's dummy values/disabled button do not justify a credential-capturing
placeholder route. Do not present a local roster/name as a signed-in account.

## State and transition matrix

| State / trigger | Visible behavior and allowed action | Authority/navigation consequence |
|---|---|---|
| Session resolving | Named checking status; Library escape; no credential submit | Await real current-session result; do not interpret loading as signed out |
| Signed out, adapter ready | Task-first editable form and supported submit | No credentials sent until explicit valid submit |
| Local syntactic invalid submit | Field errors plus linked error summary; focus first correction; retain non-secret input | No request; errors describe entered syntax, never account existence |
| Pending | One request/revision; mark region busy, prevent duplicate submit, retain Library escape | No signed-in badge or destination before current session confirmation |
| Credential rejected | `accounts.auth.rejected`, persistent generic alert; clear attempted password, keep identifier for correction | Same public message for unknown account/bad password/disallowed auth; no secret reason |
| Rate limited | Same safe failure boundary plus authority-supplied retry hint | No invented countdown, automatic retries or changing identifiers to probe existence |
| Unavailable / offline | Explicit service/connectivity state and escape; useful Retry only through the real adapter | Neither state proves valid credentials or a signed-out/expired session |
| Result indeterminate | Explain that completion is unknown; adapter-owned reconciliation/retry | No locally fabricated session or blind extra session creation |
| Authenticated | Current subject/session confirmed; announce once | Consume one validated return intent; re-authorize destination; replace auth history entry |
| Expired/revoked | Explain sign-in is needed for account tasks; clear forbidden private state | Retire old session generation; local navigation remains available |
| Leave/Back/Forward/page restoration | Retire request, clear password; re-resolve session on return | Late response cannot force navigation/focus or resurrect account data; never auto-submit |

If already authenticated, skip requesting credentials and offer/perform only
the validated read-only return navigation. Do not auto-create/accept an invite,
start a match, or turn a return URL into authorization. Identity change while
pending retires the old form generation.
[ADR-0031](../../adr/0031-browser-native-session-contract.md) decides session
transport, CSRF/context, refresh/expiry/revocation and cross-document cleanup.
Real adapter/service enforcement and target-specific security/lifecycle evidence
remain prerequisites in the [shared contract](accounts-social.md#accepted-session-policy-and-remaining-authority-contracts);
a policy decision does not make this form operational.

## Keyboard, IME, layout and acceptance

Use a native form. Enter submits once only outside composition and only when
the current form is valid/idle. Enter/Space on Show/Hide is only that toggle;
repeat keydown cannot resubmit. Autofilled values participate in the same
validation as typed values. On async rejection announce the generic alert once
without repeatedly reading fields or exposing the password. Syntax errors have
`aria-describedby` and `aria-invalid`; blur/change validation never steals focus.

Heading focus on route arrival, then identifier/password/visibility, any real
remember choice, submit, Library and supported Register link in source order.
An explicit invalid submit focuses the first invalid field and exposes an
error summary with field links. Generic server failures stay on the current
task without forcing focus into a nonexistent field error.

At 320/390 dp use one column and full-width actions; software keyboard and
200% zoom must not hide recovery. At wider widths bound the form's readable
width, keep title/controls adjacent, and use no second hero column. Four
schemes, reduced motion, long identifier/error, autofill, password visibility,
IME, duplicate submit, expiry, interruption and hostile return intent require
real adapter/platform tests. PR A documents these oracles; it runs no auth flow.
