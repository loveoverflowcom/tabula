# ADR-0038: isolated invited-account Kanidm web authentication

- **Status:** accepted narrow implementation intent; real-provider acceptance required before merge; production remains closed
- **Date:** 2026-10-05
- **Amends:** ADR-0034/0036's preparation-only provider boundary and Phase 4/5 order for this explicit isolated slice; ADR-0031's GET rule solely for the verified OIDC callback
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-13 and I-15 preserved
- **Related:** [#54](https://github.com/loveoverflowcom/tabula/issues/54), [#74](https://github.com/loveoverflowcom/tabula/issues/74), [delivery](../work-plan/060-invited-kanidm-web-auth.md)

## Context and authorization

The owner explicitly requested merging PR #76, then two new implementation PRs
from remote develop: real Kanidm web login first, then a separately reviewed
match actor/wire slice, with normal self-merges after checks. PR #76 merged as
`172f222a8c087883019ba37ef4f146f627e13c35`; this slice starts freshly there and
has no dependency on unmerged game/assets/native-voice PR #69/#70/#75.
ADR-0035 and ADR-0037 remain occupied by those independent branches; reserving
0038 imports none of their code or acceptance.

Existing phase exits, real browser/native evidence, deployment, live-provider
onboarding privileges and provider password-change synchronization are not
proved by an implementation request. This decision records only the narrower
approved code/test outcome. It does not authorize a production listener,
production migration, live provider OAuth client/accounts/credentials/grants,
new persistent access, public signup, friends, lobby, queue, voice or match store.
The second requested PR gets its own contract and review; it is not implemented here.

## Decision

`services/tabula-auth` implements a native-only, non-default `web-oidc` library
adapter. `http::isolated_router` composes actual configured Kanidm discovery,
provider verification and the existing session HTTP authority. Both default
service entrypoints continue refusing startup. The opt-in constructor opens no
listener and performs no migration or provider provisioning. Real-provider
acceptance explicitly owns disposable CI processes and their dedicated DB.

Kanidm remains the sole credential/OIDC issuer. The web shell navigates to its
own provider forms and never collects a password, receives an OIDC token/client
secret/session credential, or persists a bearer. There is one app-specific
confidential client, exact canonical trusted HTTPS origins, and a fixed callback
and `/account` destination. No return URL or request-selected issuer/client/key
is accepted. Register/friends/native-login capabilities remain unavailable.

Only previously invited exact issuer+subject pairs may log in. The bounded
operator admission list has at most 32 identities. Begin captures every active
admitted account's existing authorization epoch before redirect. Callback checks
the cryptographically verified exact pair and issues with that original epoch;
it cannot borrow a newer epoch after an intervening invalidation. Identity
mapping is pre-provisioned by the operator/test fixture, never inferred from an
email/name and never auto-created by login. Public signup is a separate product,
privilege, agreement and anti-abuse decision. Kanidm's documented credential
reset onboarding is not claimed to be a self-signup implementation.

The adapter uses independently random single-use state, nonce and S256 PKCE.
Pending state is cookie-bound, bounded in count/bytes and lives at most five
minutes. Start requires signed-out preauth synchronizer CSRF, exact Origin and
JSON. A live session cannot silently switch account. The callback is a narrow
GET exception for authorization-code redirects: its cookie+state claim replaces
the unsafe-method CSRF header only at this fixed route. Actual top-level
cross-site provider navigation is permitted there, while ordinary routes keep
the exact Origin/Fetch Metadata/channel rules. A wrong state does not consume
another browser's pending attempt; a matched code attempt is consumed before
network exchange and cannot retry after timeout or ambiguous outcome.

Pinned Kanidm 1.11.2 emits code/state without callback `iss`; a supplied issuer
must match exactly, while discovery and signed token issuer are always mandatory.
Discovery endpoints are exact client-specific Kanidm paths, not arbitrary
metadata-selected URLs. Upstream TLS/hostname verification stays enabled,
redirects/proxies are disabled, and connect/request/body bounds fail closed.
Keys are fetched freshly at each code completion: no stale-key or provider-outage
fallback. Public JWK kid uniqueness, EC/P-256, ES256 and signing use are checked;
JOSE key URLs/embedded keys/unsupported headers are rejected. The existing
jsonwebtoken 9.3.1/ring verifier handles signatures; required issuer/subject/aud/
exp and explicit nonce/iat/nbf/azp/auth_time checks are applied. Only this client
is an audience; a foreign/multiple audience is rejected. `prompt=login` and
`max_age=0` request real provider reauthentication, and signed `auth_time` must
be fresh for the attempt. Token minting `iat` alone never proves reauthentication.

Known successful durable issuance alone may set the host-only Secure/HttpOnly/
SameSite=Lax session cookie. Session credentials, ID tokens, code/verifier,
client secret and provider diagnostics are not exposed in JSON/logs/artifacts.
Provider/store failures emit no session cookie; no recoverable successor bearer
or provider refresh authority is retained. Existing `/me`, expiry, epoch,
rotation/logout and bounded first-frame authority fences remain in force.
Login/complete/cancel operations share per-preauth ordering; successful logout
invalidates server preauth/pending state, not only browser cookies. A deliberate
new Login cancels its own preauth attempt, refetches context, then starts again;
arbitrary GET does not cancel a live pending login.

The web consumer retains generation/step/cleanup/visibility fences. Unresolved
logout suppression persists only bounded non-authorizing SHA-256 target markers,
one per target, with targeted receipt removal and fail-closed storage/corruption
behavior. No CSRF, account ID or profile persists. This is local presentation
suppression, not durable revocation or an atomic cross-document authority fence.
Adapter restart/replacement context cannot be retargeted by a fingerprint;
without a matching known-success receipt it stays masked, with an explicit
recovery limitation. Actual Storage/BFCache/AT behavior remains target evidence.

## Dependencies and proof boundary

Provider/TLS/session implementation stays outside deterministic/game/client DTO
graphs. The native optional URL/ICU graph has an inspected compiler floor 1.88;
PostgreSQL acceptance retains 1.94, while unchanged default/SDK declarations stay
1.85. Executed checks use pinned 1.96.1, not an inferred MSRV acceptance.
The scoped cargo-deny ring wrapper allows its independent getrandom 0.2 TLS/ES256
backend; resolved deterministic rand_core 0.6 has no OS entropy feature. Existing
per-crate dependency/entropy gates and an explicit all-feature deny gate remain
required. No game rules/hash/protocol encoding changes are made.

The [provider harness](../../tests/kanidm/README.md) pins the official Kanidm
1.11.2 image/digest, fresh private CA and verified loopback TLS, one non-admin
person/group/openid-only confidential client, actual password+TOTP reauthentication,
signed resume/consent, code exchange/signature verification and PostgreSQL.
All provider/test access disappears with the disposable process/data/job;
there is no hosted provider change or reused secret. Setup failures and empty
selections fail. Synthetic ES256/HTTP/UI doubles remain separately labeled.

This proves only the actual components executed. In-process app Router calls
are not browser-enforced cookies, app TLS/reverse proxy, rendered pixels, AT,
password manager, BFCache or shipping native secure store evidence. No provider
password-change/account-suspension synchronization, online WS/private-output
fence, capacity/SLO or full #54/#74/phase-exit acceptance is silently passed.
The existing session lifetime/publication/trusted-clock limitations remain.

## Revisit before

Any production activation, live-client/account/grant provisioning, public signup,
additional issuer/signing algorithm/client, cross-origin deployment, native login,
provider security-event synchronization, online/private transport, or broad phase
exit claim. Real provider acceptance, independent security review and exact-tree
all-terminal-green CI are required for this isolated PR's authorized merge.

## Primary sources

- [Pinned Kanidm OAuth implementation](https://github.com/kanidm/kanidm/blob/v1.11.2/server/lib/src/idm/oauth2.rs)
- [OIDC/PKCE/provider client contract](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html)
- [Kanidm onboarding](https://kanidm.github.io/kanidm/stable/accounts/authentication_and_credentials.html)
- [OpenID Connect ID-token validation](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation)
- [PKCE S256 and independent Appendix B vector](https://www.rfc-editor.org/rfc/rfc7636.html#appendix-B)
- [JWT verifier 9.3.1 source](https://docs.rs/crate/jsonwebtoken/9.3.1/source/src/validation.rs)
