# Invited Kanidm web auth evidence

Source base: `develop172f222a8c087883019ba37ef4f146f627e13c35` after PR76.
Scope: [ADR-0038](../../adr/0038-isolated-invited-kanidm-web-auth.md).
Compiler used: rustc1.96.1. This is not executed MSRV1.85/1.88/1.94 proof.
Published head/tree and exact CI checkout/real-provider receipts belong in the
PR description; this ledger records the source's authoring gates without a
self-referential commit hash.

## Executed local gates

| Gate | Status and selection |
|---|---|
| `cargo xtask check` | PASS twice after convergence; final 1,086 passed / 0 failed / 21 ignored. Authoritative order unchanged: full fmt, workspace all-target/all-feature strict Clippy, workspace tests, dependency/game-ID/manifests/tokens/colors and cargo-deny |
| `cargo deny --all-features --offline check` | PASS: advisories, bans, licenses and sources. ring's native TLS/ES256 wrapper remains separate from deterministic kernel OS entropy |
| `cargo tree -e features -i rand_core@0.6.4 --workspace --all-features --offline` | Inspected: kernel ChaCha/default only, no OS entropy feature unification |
| `cargo test -p tabula-auth --features web-oidc --lib` | 12 PASS; real ES256 signatures with a synthetic key generated only in test-process memory, tamper/different valid P-256 key/HS256/none/JOSE URLs/duplicate nonce, exact claims/metadata, access-token hash and independently sourced RFC7636 S256 vector |
| Session/HTTP focused feature tests | 78 PASS: 26 session units +1 actual-TCP expiry unit +13 browser-login TCP +7 DTO +31 legacy isolated HTTP. Provider/authority doubles are labeled; zero selected PostgreSQL cases in isolated mode are not a DB pass |
| Web focused acceptance | 72 binary unit/static tests PASS; account_http44 PASS =30 repeated core +13 TCP +1 wire parser. These groups are not 116 distinct behavior claims |
| Native web/session/HTTP/auth strict Clippy | PASS for affected all-target feature graphs; auth includes `postgres-acceptance` real-provider target |
| WASM web Clippy | PASS with warnings denied, including top-level-only one-use login navigation and early synchronous private masking |
| Workspace no-default/all-features | Both `cargo check` selections PASS |
| DTO/web WASM all-features | `cargo check -p tabula-web -p tabula-session-http --target wasm32-unknown-unknown --all-features` PASS; no provider/SQL runtime in this client DTO graph |
| Native and game WASM builds | `cargo build -p tabula-game-client` and release WASM web build PASS |
| Production closures | Default auth, opt-in-feature auth binary and gameplay server all retain expected failure exit1; no listener activated |
| Provider helper | 26 synthetic parser/form/MFA/readiness guards and published RFC6238 TOTP vectors PASS; shell/Python/workflow syntax and diff whitespace PASS |
| Real-provider Rust target | Compiled. Local execution BLOCKED: no Docker/Podman/daemon or Kanidm binary. Disposable CI must run the non-empty ignored selection and fail setup failures |

No game/rules/kernel/protocol executable or storage SQL/migration/metadata source
was changed. Optional provider dependencies use native rustls system roots,
with an explicit private CA only inside the disposable test. No license waiver,
insecure TLS, root-store change or deterministic RNG exception was introduced.

## Independent security source review

Review found and fixed stale-cookie relogin/preauth cancellation, arbitrary
wrong-state/issuer flow cancellation, single-slot cross-tab marker lost updates,
and explicit recovery for lost/pending start. The converged source has no
unresolved confirmed security blocker in the isolated boundary. Final commit/
tree reconciliation and actual provider CI receipt are required before merge;
source review alone is not merge readiness or production acceptance.

HTTP regressions exercise exact Origin/CSRF/channel/body/query, concurrent
starts/callbacks, old preauth replay, cancellation during begin/complete/issuance,
active-account switch rejection, captured epoch invalidation, provider/store
outage and indeterminate commit with no session-cookie publication. Web tests
exercise stale generations, one-use redirects, abort/restart, non-secret reload
suppression, exact-target receipts, replacement contexts and storage failure.

## Required real components and limits

The [provider workflow](../../../.github/workflows/kanidm-oidc.yml) uses actual
official pinned Kanidm1.11.2 over verified private-CA HTTPS, real password+TOTP
reauthentication, signed resume/consent and code/token/JWK flows, plus real
PostgreSQL16 and current authority. It also must prove private-CA rejection and
provider discovery outage. The Rust acceptance uses in-process app Router calls;
it does not prove app HTTPS/cookie enforcement or rendered browser/native
pixels, password managers, actual Storage/BFCache/AT or secure store.

The receipt must show real login→durable cookie→current `/me`→logout, preserved
genuine flow after a bogus callback, replay rejection, stale HttpOnly-cookie
relogin, in-flight old-epoch rejection and fresh-epoch reauthentication. Operator
mapping fixtures use the actual provider subject but prove no identity by
themselves; only the verified real signed callback provides provider evidence.

Default production services remain closed. No phase exit, deployment, live
provider provisioning, public signup, friends, online gameplay or voice is
claimed. Provider password-change/security-event synchronization, multi-process
private transport/output fencing, capacity and app/browser/native target evidence
remain open. Persistent-provider access changes need separate explicit approval.

Logout markers retain only bounded non-authorizing fingerprints. Cross-document
signals/enumeration are observed hints, not atomic output fencing. Adapter
restart/replacement mismatches and late same-target writes remain conservatively
masked without a verified matching receipt; safe operator recovery remains an
explicit limitation. Existing session publication/trusted-clock limits remain.

## First provider CI and readiness repair

At head17641b65, the main12 and PostgreSQL1 jobs passed, while provider readiness
failed before bootstrap/MFA or real-test selection. That is a failed/unproved
provider gate. The source-confirmed repair removes unsupported log_level=warn,
validates pinned config first, and probes verified HTTPS health separately from
the version-header route. Diagnostics expose only fixed categories, container
state and numeric exit code; raw output stays private. The repaired exact tree
still requires an actual successful provider run before merge.
