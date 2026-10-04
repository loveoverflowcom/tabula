# ADR-0034: Kanidm authentication service and gated server skeleton

- **Status:** accepted skeleton and ownership direction; runtime remains gated
- **Date:** 2026-10-04
- **Amends:** ADR-013/015/020 only for the externally managed identity provider
  and separate account-authentication boundary; doc 01's local-password proposal
- **Invariants touched:** none relaxed; I-1, I-5/I-6, I-9, I-13 and I-15 preserved
- **Related:** [issue #54](https://github.com/loveoverflowcom/tabula/issues/54),
  [ADR-0031](0031-browser-native-session-contract.md)

## Context

The owner requests a minimal Rust server frame and a separate auth service using
Kanidm, learning from VOT Workspace. The existing architecture keeps auth inside
the gameplay binary and proposes local argon2 passwords. This decision records
the requested exception without opening the Phase 4/5 implementation gates or
claiming issue #54's login/profile/friends acceptance.

The reference is VOT Workspace at `59190cc5185a54ef40ec4f9625283511a4ed3755`:
`services/vot-auth/src/main.rs`, `kanidm.rs`, and `services/kanidm`.
Its code shows a backend provider adapter and pending OIDC flows; some prose
still describes an older relay. Tabula adopts the boundary, not legacy routes,
branding, token behavior or copied credential code.

## Decision

| Owner | Future responsibility |
|---|---|
| Kanidm (operator-managed infrastructure) | Credentials, password policy, identity directory and OIDC issuer/signing keys |
| `services/tabula-auth` | Provider adapter/verified OIDC identity, account login/registration and opaque Tabula session lifecycle |
| `services/tabula-server` | Current session enforcement, resource/game permissions, match grants, profile/social/presence and the authoritative gameplay runtime |
| `crates/tabula-storage` | All Tabula SQL, migrations and atomic durable identity/session operations behind ports |
| App shells | Forms, navigation and browser/native credential adaptation under ADR-0031 |

Kanidm is an external credential datastore, not another Tabula gameplay store.
PostgreSQL remains the only Tabula datastore (ADR-013). Kanidm credentials and
OIDC tokens are distinct from Tabula's opaque browser/native session credentials
and scoped match grants. Bind provider identity by issuer+subject, never email.
Tabula does not store password hashes or mint provider access tokens.

The future browser deployment exposes `/api/v1/auth/*` through the **same trusted
HTTPS origin** as app documents and gameplay APIs via reverse proxy routing.
Service binaries remain leaves: no service imports another service crate. Shared
contracts/ports stay in library owners when implementation needs them.
Before either service handles credentials or sessions, their shared durable
authority and cross-process revocation/expiry ordering must be specified and
tested against ADR-0031. A cached identity or valid Kanidm token alone is not
current Tabula authorization. The two processes add an enforcement boundary;
this skeleton does not solve it or relax revocation guarantees.

Both binaries currently print their gate and exit unsuccessfully. Module files
contain `TODO(phase N, #54)` notes for their future owners. No listener, routes,
schema, discovery document, credential store or provider integration is created.
The new auth package uses only std; `deps.toml` grants it no dependencies. Future
dependencies arrive with their implementation and policy update, not placeholders.
Delete TODOs only as the corresponding implementation and evidence land.

## Consequences and remaining gates

The gameplay process stays a modular monolith; only account authentication is
reserved separately. Lobby/chat/catalog/presence are not split. Credentials stay
with Kanidm, at the cost of an operator-managed provider and coherent revocation
across the auth/gameplay services. No deployment or Kanidm provisioning is added.

ADR-0031's channel binding, same-origin cookie/native secure store, CSRF, session
deadlines, rotation, revocation fences and credential-free Hello stay in force.
The prior local argon2 proposal is superseded for Kanidm-backed accounts only.
No protocol types/vectors change (I-13); local play remains account-free.
Phase 3 exit and Phase 4/5 integration evidence are still owed. Profile fields,
registration agreement, friends/invite permissions and presence remain issue #54
contracts, not invented auth-service behavior. CI compilation proves the frame,
not successful login or production security.

## Revisit when

Before the first auth/session runtime slice: resolve cross-service atomic
revocation/expiry, provider outage and restart behavior, trusted proxy routing,
onboarding privilege and real Kanidm integration evidence. A cross-origin browser
deployment, provider replacement or independent session store needs a further ADR.
