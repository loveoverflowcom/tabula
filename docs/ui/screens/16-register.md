# 16 — Register

Issue #54 PR A; [shared owners/gates/navigation](accounts-social.md),
[foundation](foundation.md), [copy](accounts-social-copy.md) and
[verification](accounts-social-verification.md). Source base:
`develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`; reference is pinned
`06-accounts/screens/16-register.svg`, its inspected static raster and mobile PNG.
`/register` is proposed, not mounted at this base.

## Task, contract and anatomy

Create an account only through the approved identity service, with clear
field/consent requirements and the current Library escape. Keep the compact
form anatomy shared with [Login](15-login.md); no hero, game-specific art,
social-proof statistics or promotional panel.

The design proposes display name/email/password. Doc 03's conceptual schema
requires a unique handle and defines no display-name field. The identity owner
must reconcile the mismatch before PR B: approved required/optional fields,
handle allocation/selection, Unicode/length/normalization rules, password
policy, legal agreement requirements, public error/disclosure semantics and
whether registration produces a session. Do not derive a handle, add arbitrary
fields, hardcode a password-strength meter, or introduce verification/reset/OAuth.

| Proposed field/control | Requirement when its contract is real |
|---|---|
| Display name | Persistent label; native text input and IME; Unicode and long-text behavior from identity schema. Display name is not a unique account identifier |
| Email identifier | Native email semantics; stable `name`/ID and identifier autocomplete approved for the form; no live uniqueness/existence check |
| New password | `type=password`, `autocomplete=new-password`; paste/generator/autofill permitted; do not trim/normalize. Show/Hide shares Login toggle semantics |
| Legal agreement | Real title/version and URL; separate explicit unchecked consent only if the actual agreement requires it. Privacy notice is not automatically a contractual checkbox |
| Create account | One filled primary; pending label retains action; never enabled for a mock adapter |
| Browse games / Login | Tonal escape to current Library; supported Login link preserves only validated public return intent, never password |

No agreement placeholder, prechecked consent, invented marketing opt-in, avatar
upload, age field, provider grant or credential-persistence setting. Opening a
real agreement and returning preserves only non-secret draft state and actual
consent for the same agreement version; if terms change, require the applicable
new consent. An unavailable mandatory agreement blocks registration honestly.

## State and authority matrix

| State / trigger | Form/recovery | Required authority fact |
|---|---|---|
| Contract/session loading | Named status, Library escape; no premature fields/submit | Real registration and session disposition |
| Unavailable / offline | Safe reason and escape; no credential collection in an unavailable adapter | Failure is not a failed account creation or an empty user database |
| Editable ready | Only approved fields and actual legal requirement | Client syntax mirrors approved constraints; server remains final validator |
| Syntactic invalid | Associated field errors; first invalid field focus on explicit submit; summary links | No network request or existence query; validation waits for IME commit |
| Pending | One create identity tied to draft revision; busy region; prevent duplicate submit | Idempotency/unknown-result policy from identity service; no local account creation |
| Rejected | Generic `accounts.register.rejected`; retain appropriate non-secret fields and same-version consent; clear attempted password | No email-exists/provider-linked/internal-status disclosure; public field errors limited to safe entered-value syntax |
| Timeout/result unknown | Persistent indeterminate message, same-operation reconciliation where supported | Do not call failed, successful or resubmit with a fresh idempotency identity |
| Accepted | Show only the service's supported public disposition | Session must be independently confirmed if issued; no auto-login just because creation was acknowledged |
| Accepted without session | Supported Login action, if allowed by real public-disclosure policy | Do not invent verification, a mailbox notification, or a signed-in badge |
| Expired/revoked/account changed | Retire old operation and forbidden private state | No late response can overwrite current identity or redirect after departure |
| Back/leave/reload/restoration | Clear password; retire local generation; re-resolve/reconcile on entry | No automatic POST retry, replayed consent or duplicate account creation |

For duplicate email or any sensitive account-state failure, both public result
shape and observable status/timing must follow the server's anti-enumeration
policy. A generic string over distinguishable responses is insufficient. The
backend must also define accepted public behavior for each result class before
the UI can truthfully describe completion; this spec does not invent it.

## Interaction and acceptance

Use the shared native form/error summary, 44 dp Show/Hide and legal-control
wrappers, correct labels/help/error association, single submit outside IME and
persistent generic alert. Error focus follows Login's explicit-submit policy;
ordinary typing/blur never steals it. No custom canvas editor, blocked paste,
password echo in live regions or hover-only explanation.

One-column 320/390 dp, reflow at 200%/short landscape/keyboard viewport, and
bounded desktop form follow Login/foundation. Agreement links and non-secret
input can wrap without truncating legally relevant titles. Required runtime
cases include valid/rejected/duplicate/indeterminate registration, syntax and
Unicode boundaries, real/missing/changed agreement, Enter/IME/autofill, repeated
activation, departure/late response, actual session/no-session result and
forbidden public disclosure. No signup endpoint is implemented or tested by PR A.
