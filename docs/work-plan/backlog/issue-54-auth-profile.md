# Issue #54 B — sign in, register and read self-profile

**Status (2026-10-06):** ADR-0036/0038 delivered invited login and immutable
self-ID. The owner now explicitly authorizes every remaining #54 criterion in
[ADR-0043](../../adr/0043-isolated-account-registration-social.md), including
verified-provider Tabula enrollment and permitted profile read/edit. The
[completion ledger](../../verification/issue-54-account-followup/README.md)
records current implementation and acceptance; [#99](https://github.com/loveoverflowcom/tabula/issues/99)
tracks the same work rather than replacing #54.

## Original production proposal (historical)

The original dependencies and proposed separation below remain the production
boundary; ADR-0043 is the bounded isolated exception.

**Outcome:** a real identity/session service supports the short Login/Register
flows and a permitted self-profile read, while Library and supported local
play remain reachable without an account.

**Why:** actual auth/session/profile types and endpoints are absent. A mock
session or locally saved account label would hide missing authority/security
and falsely turn the discovery exception into a Phase-5 product shell.

**Dependencies:** actual Phase-2 platform evidence and Phase-3 portfolio/
projection/freeze exit, real Phase-4 identity/server/protocol/persistence/session
integration and exit, then the opened Phase-5 shell gate; approved registration
fields/handle/normalization/password/agreement/public-disclosure policy,
response/session disposition and self-profile API.
[ADR-0031](../../adr/0031-browser-native-session-contract.md) has resolved the
cookie/localStorage and HTTP/WS policy choice. Its channel-bound adapters,
CSRF/context, rotation/expiry/revocation/cleanup and native/browser behavior
still require real implementation and
[S01–S14 evidence](../../verification/session-contract/README.md#required-acceptance-scenarios);
see the [session backlog](issue-54-session-contract.md). See
[#54](https://github.com/loveoverflowcom/tabula/issues/54),
[shared contract](../../ui/screens/accounts-social.md) and
[acceptance oracles](../../ui/screens/accounts-social-verification.md).

**Review boundary:** one supported Login/Register/self read-only vertical
slice using real versioned session/protocol adapters and existing foundation.
Exercise current session, generic invalid credentials/duplicate identity,
pending/unknown/expiry, safe return/Back/local escape, native fields/autofill/
IME/error-focus, repeated submit, account switch/late results and forbidden
data. Run targeted security/UI checks then `just check` and applicable targets.

**Risks / unknowns:** frontend generic errors cannot prove anti-enumeration;
creation timeout may already have committed; cancelled UI cannot revoke a
server mutation. A handle/name/route never proves self ownership. Private
data and pending requests cannot survive a viewer/permission change.

**Non-goals:** choosing OAuth/reset/verification providers, new token storage,
friends/ratings/history or security settings. Other-profile and edit need
their separately approved API/fields/privacy contract; substantial editing
is a distinct PR with real revision/conflict/persistence evidence, after B.
