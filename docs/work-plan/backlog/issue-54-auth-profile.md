# Issue #54 B — sign in, register and read self-profile

**Status:** deferred by phase and authority prerequisites; PR A specification delivered.

**Outcome:** a real identity/session service supports the short Login/Register
flows and a permitted self-profile read, while Library and supported local
play remain reachable without an account.

**Why:** actual auth/session/profile types and endpoints are absent. A mock
session or locally saved account label would hide missing authority/security
and falsely turn the discovery exception into a Phase-5 product shell.

**Dependencies:** documented Phase-4 identity/session and exit evidence,
opened Phase-5 shell gate; approved registration fields/handle/normalization/
password/agreement/public-disclosure policy and response/session disposition;
reconciliation of doc 03's HttpOnly cookie and doc 04/net-client localStorage
proposals, including HTTP/WS/CSRF/rotation/revocation. See
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
