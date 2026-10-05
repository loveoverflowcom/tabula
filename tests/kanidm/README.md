# Real Kanidm OIDC acceptance

This opt-in harness runs the actual upstream Kanidm provider. It does not replace
discovery, keys, authentication, consent, authorization codes, or token issuance
with a synthetic fixture. Production binaries remain gated.

## Run and scope

With an available Docker daemon, OpenSSL, Python 3, the repository's Rust
toolchain and a **disposable PostgreSQL 16 database**:

```sh
DATABASE_URL=postgres://tabula_test@127.0.0.1:5432/tabula_oidc_acceptance \
SQLX_OFFLINE=true \
bash tests/kanidm/run.sh bash tests/kanidm/acceptance.sh
```

The command fails if provider setup, HTTPS, version, configuration, public keys,
the explicitly selected real-provider test, or its assertions fail. The Rust
acceptance target uses the dedicated fresh disposable PostgreSQL database and
applies the existing session migrations in its default schema. It does not regenerate committed SQLx metadata.
The workflow runs ordinary helper regressions and compiles the real-provider
target before creating test identities, then explicitly checks that its ignored
test selection is non-empty before running it.

The server has a new temporary database and private CA/leaf keys, serves only on
runner loopback, and is removed with its temporary directory on normal exit,
failure, SIGINT or SIGTERM. Hosted-runner destruction also bounds cancellation
or abrupt termination. There are no persistent volumes, external accounts,
hosted OAuth clients, OAuth grants to third-party services, global trust-store
changes, saved admin sessions or reused operator secrets. Do not point this
harness at a live provider. A persistent-provider client/account/grant or access
change needs a separate, specific approval.

Only the one-time bootstrap uses the disposable provider's `idm_admin`. The
actual OIDC principal is a non-admin person in one app-specific group. That
group grants only `openid` to one confidential/basic client with one exact
HTTPS callback. The app client has no directory-management privileges. Its
secret stays in a mode-0600 temporary file and server-side acceptance process.
PKCE, provider consent and normal credential policy remain enabled.
Fresh Kanidm 1.11.2 requires MFA for all persons. Bootstrap enrolls a real
provider-generated TOTP and password for this disposable person; it never
weakens that policy. The server configuration omits `role` and uses its default
`WriteReplica`, avoiding a non-supported snake_case enum value.

## Rust acceptance interface

- `TABULA_KANIDM_DISPOSABLE=1`: explicitly identifies this test-only environment
- `TABULA_KANIDM_TEST_CONFIG`: private JSON, mode 0600, removed with the job
- `TABULA_KANIDM_TEST_HELPER`: absolute path to `provider.py`
- `DATABASE_URL`: disposable runner-local PostgreSQL

The private config has `provider_origin`, `issuer`, `client_id`, `client_secret`,
`callback_url`, `ca_path`, `admitted_subject`, `username` and `password`.
It also includes the provider's `totp` parameters/seed. These credentials exist
only inside the disposable provider's lifespan.
Never print, attach or persist the config, provider cookies, callback URLs,
authorization codes, tokens or admin-recovery output.

The Rust test creates its actual pending login and authorization URL, then
spawns:

```sh
python3 "$TABULA_KANIDM_TEST_HELPER" authorize --config "$TABULA_KANIDM_TEST_CONFIG"
```

Send `{"authorization_url":"..."}` to stdin and capture the returned
`{"callback_url":"..."}` from stdout. Neither pipe is a log artifact. The
helper requires `prompt=login`, `max_age=0`, `openid`, code flow, nonce, state and
S256 PKCE. It performs the real upstream `/ui/oauth2`, TOTP and password forms, signed
`/ui/oauth2/resume` and consent form over verified HTTPS. It then stops before
following the app callback. Tabula's Rust route receives the actual callback and
performs the real token exchange and signature/claims verification.

The supported test callback is
`https://app.localhost:8444/api/v1/auth/oidc/callback`. Routing that returned URL
into the real Axum router in-process proves real OIDC plus HTTP-route contracts;
it does **not** prove an app reverse proxy, browser-enforced Secure/HttpOnly
cookies, SameSite behavior, rendered pixels, password managers, accessibility,
or BFCache. Kanidm itself is always reached over verified HTTPS. No app TLS
listener is implied by this harness.

`test_provider.py` covers synthetic parser/guard regressions only. Those tests
must never be reported as real provider login or browser evidence.

The first helper flow must observe real provider consent. A private, non-secret
job-lifetime receipt allows subsequent flows to use the provider's previously
granted consent. Every flow still requires TOTP/password authentication and signed
resume; the helper never disables provider consent or edits the requested scopes.

## Pinned upstream and provenance

The workflow uses official `docker.io/kanidm/server`, linux/amd64, pinned to:

```text
sha256:d87475bf9c9cfd24872d8b25957c9fc13fc090ace09ddc37c3b25fe394b397ac
```

The digest was resolved on 2026-10-05 from the official publisher's
[1.11.2 amd64 tag link](https://hub.docker.com/layers/kanidm/server/1.11.2/images/sha256-d87475bf9c9cfd24872d8b25957c9fc13fc090ace09ddc37c3b25fe394b397ac)
on the [official tag listing](https://hub.docker.com/r/kanidm/server/tags), matching
the [upstream v1.11.2 release](https://github.com/kanidm/kanidm/releases/tag/v1.11.2).
The harness verifies `kanidmd version` and the HTTPS server version header are
both 1.11.2. Digest/publisher/version checks are provenance evidence, not a
claim that an image signature or build attestation was verified.

Upstream contracts/source inspected:

- [OAuth/OIDC endpoints, PKCE, scopes and client kinds](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html)
- [Pinned authorization, fresh-auth handling, ID-token issuance and discovery](https://github.com/kanidm/kanidm/blob/v1.11.2/server/lib/src/idm/oauth2.rs)
- [Actual provider OAuth integration tests](https://github.com/kanidm/kanidm/blob/v1.11.2/server/testkit/tests/testkit/oauth2_test.rs)
- [Server-rendered OAuth resume and consent](https://github.com/kanidm/kanidm/blob/v1.11.2/server/core/src/https/views/oauth2.rs)
- [Server-rendered login routes/forms](https://github.com/kanidm/kanidm/blob/v1.11.2/server/core/src/https/views/login.rs)
- [Bootstrap/auth and credential-update client APIs](https://github.com/kanidm/kanidm/blob/v1.11.2/libs/client/src/lib.rs)
- [Person credential helper](https://github.com/kanidm/kanidm/blob/v1.11.2/libs/client/src/person.rs)
- [OAuth client/scopemap/strict-redirect APIs](https://github.com/kanidm/kanidm/blob/v1.11.2/libs/client/src/oauth.rs)
- [Fresh-domain MFA policy](https://github.com/kanidm/kanidm/blob/v1.11.2/server/lib/src/migration_data/dl15/groups.rs#L375-L397)
- [Version 2 configuration and role defaults](https://github.com/kanidm/kanidm/blob/v1.11.2/server/core/src/config.rs)
- [Upstream TOTP algorithms and validation](https://github.com/kanidm/kanidm/blob/v1.11.2/server/lib/src/credential/totp.rs)
- [RFC 6238 published independent test vectors](https://www.rfc-editor.org/rfc/rfc6238.html#appendix-B)

Kanidm's issuer is client-specific and has no trailing slash:
`https://localhost:8443/oauth2/openid/tabula_oidc_acceptance`. Discovery appends
`/.well-known/openid-configuration`; its `jwks_uri` points to the same client
path's `/public_key.jwk`. The normal signing algorithm is ES256 and the code
challenge method is S256. Callback issuer is optional because this pinned
provider returns code/state without `iss` and does not advertise the issuer
response extension. A supplied issuer must still match exactly; configured
discovery and the signed ID-token issuer are always checked.

The provider supports `prompt=login`, `max_age` and signed `auth_time`, but fresh
authentication does not replace Tabula's account-epoch CAS. The invited identity
and epoch must be captured before redirect and the returned issuer/subject
must match that admission. A callback must never reacquire a newer account
epoch after invalidation. Token minting `iat` is not a reauthentication time.

## Evidence artifacts and honest limits

Only public provider image/version/digest, Tabula SHA, discovery, JWKS, selected
test names and a terminal success label are preserved under
`verification/kanidm-oidc-artifacts`. No provider log, request/response body,
HTML form, private database, credential file, CA private key or screenshot is
uploaded. A missing success label is not a pass. The CI run and exact Tabula SHA
establish execution; this README and workflow establish configuration only.

The initial authoring environment had no Docker/Podman daemon or Kanidm binary.
Helper unit/syntax checks are locally executable; real-provider execution is
BLOCKED there until the disposable CI job runs. Native secure stores, provider
password-change synchronization, cross-service output fencing, deployment,
production rollout and full phase exits are outside this harness.
