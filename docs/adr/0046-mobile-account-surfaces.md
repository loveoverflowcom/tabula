# ADR-0046: Bounded account surfaces in the CMP shell

- **Status:** accepted owner-requested UI draft; production native account/provider/social integration unavailable
- **Date:** 2026-10-07
- **Extends:** ADR-0032/0045's CMP shell and public navigation, using ADR-0031's account/session boundary
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-10, I-13 and I-15 unchanged
- **Related:** [issue #103](https://github.com/loveoverflowcom/tabula/issues/103), [screen contract](../ui/screens/mobile-account.md), ADR-0038/0044's separately isolated web implementation

## Context

Issue #103 follows the owner-requested mobile shell and discovery slices. Its
older claim that account login and social services are absent on web is no longer
the current source contract: ADR-0038 implements isolated Kanidm web login, and
ADR-0044 implements the explicitly selected `account-social` composition for
verified enrollment, profile read/edit and durable friends/presence. Feature-off
web builds retain narrower unavailable surfaces. These are isolated web/service
capabilities, not implemented native account adapters or permission to activate
production services.

The owner requests a substantive CMP account UI draft with typed state and an
adapter seam. Opening that presentation slice must not manufacture an account,
copy credentials into Kotlin, imply mobile provider/social delivery, or absorb
the separate native GameHost work. Source starts from `develop @ a386524`, which
already includes the native host adapter-contract prototype from PR #108.

## Decision

The existing CMP app may provide Account, Login, Register, read-only self Profile
and Friends task screens. Parameter-free shell identities are `/account`,
`/login`, `/register`, `/me` and `/friends`. Navigation is ordinary bounded public
presentation state; these are not OS deep links, provider redirects or identity
grants. All tasks retain Back and a Library escape. Browsing needs no account.

The app consumes an explicit `AccountSessionPort` and typed account states:
Unknown, Loading with operation, SignedOut, Authenticated with identity and
sign-out capability, Expired, Unavailable with a safe reason, and Error with a
safe reason and permitted recovery operation. The production default
`UnavailableAccountSessionPort` reports `NativeAdapterMissing`; it neither
fabricates a signed-out session nor runs an artificial loading sequence.
Test ports and preview data are named doubles, not production fallback services.

The port owns current identity, refresh/sign-out outcomes, cancellation,
invalidation and foreground lifetime. Refresh revalidates presentation facts;
it does not rotate a credential. Calls and completions are serialized by the
platform owner; StateFlow alone is not a concurrency lock. UI state is not
authority. Loading,
expiry, error, invalidation, backgrounding, adapter replacement and closure mask
private identity; a completion must belong to the current port/operation/lifetime
before publication. Cancellation retires local completion effects and never
claims that a server mutation was undone. An unresolved sign-out retains its
non-authorizing suppression and permits only targeted sign-out recovery, rather
than a refresh restoring old identity. This quarantine is in-memory only; a
future native adapter must reconcile unresolved revocation before later bootstrap.
No persistent suppression, secure-store deletion or process-restart recovery is
proved by this controller. Sign-out requires explicit UI
confirmation and a real port capability; only a confirmed port disposition may
show SignedOut. Confirmation means current-device durable revocation, not global
logout. An ambiguous result stays non-authorizing and safe.

Read-only identity may include only the current adapter's permitted account ID
and optional display name, handle and self visibility. Absent optional fields
are omitted. No display name/guest account, zero statistic, history, achievement,
presence or account-existence claim is synthesized. The existing v1/v2 Rust DTOs
remain unchanged; CMP presentation types do not define another wire contract.

No native provider login, enrollment, profile edit, directory lookup, friend
graph, request mutation, presence stream, deep-link callback or secure-store
implementation is added. Login/Register/Friends explain their native limitation;
they collect no password/email, start no network request and cannot create a
session. Web capability alone cannot enable a native action. A future native
provider adapter requires its own reviewed platform contract and real-provider,
secure-store, callback and lifecycle acceptance under ADR-0031. A system-browser
OIDC flow may be part of that contract; ADR-0043's WebView prohibition concerns
gameplay and does not forbid external authentication.

The generated semantic tokens, vi/en copy and shared shell components govern
layout. The neutral human avatar remains the default in every account state.
An optional already-loaded `AccountAvatarImage` is managed presentation data,
bound to the exact active `AccountIdentity` instance. Referential matching, not
account-ID/value equality, prevents a prior bitmap returning after same-account
refresh or authority replacement. Mismatched or retired identity uses neutral.
This is not an avatar URL, upload, provider-image lookup or image-delivery
service; the synthetic image fixture lives only in the desktop preview.

Credentials, tokens, CSRF, identity, profile facts, avatar images and pending
account operations do not enter saved navigation, preferences, URLs or logs.
Restored routes resolve current adapter state afresh. ADR-0031's future native
secure-store requirement is not replaced by local preferences.

## Evidence and consequences

The [screen contract](../ui/screens/mobile-account.md) specifies route/state,
layout, localization, accessibility and interrupted/repeated-flow obligations.
The implementation ledger records exact selections and check results. Port
doubles can establish reducers, lifecycle fencing and shared CMP interaction;
they cannot establish provider authentication, native secure storage or social
permission enforcement. Desktop pixels, when actually captured, establish only
shared shell presentation. Android compilation, iOS compilation, device touch,
TalkBack/VoiceOver and real provider execution are separate evidence kinds.

The unavailable production port and remaining native integration are explicit
scope limits, not omitted acceptance silently declared complete. This UI draft
does not complete issue #103's unexecuted UI acceptance or a phase exit. Real
native integration acceptance is additionally required when that adapter exists.
Portable repository gates and affected mobile tests/builds remain required;
blocked or unexecuted checks are not reported as passed.

No gameplay code, native GameHost lifecycle, asset packaging, drawing bridge,
voice authority, server startup, provider provisioning or wire type changes
follow from this decision. ADR-0043 and PR #108 retain their independent scope
and evidence. Phase 4/5/6 exits, networked mobile play and store release remain
closed.

## Revisit before

Adding any native provider/deep-link/secure-store adapter, real social transport
or profile mutation, account-dependent game launch, persistent private cache,
avatar delivery/upload, new native authority capability, production activation,
or issue-completion/phase-exit claim beyond the recorded evidence.
