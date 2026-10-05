# Issue #54 PR3 — isolated account-state and self-profile shell

This third slice starts in a fresh worktree from the verified [PR72](https://github.com/loveoverflowcom/tabula/pull/72)
head `3e539bfe0104510bcac823889c44d480a4250270`, tree
`1ddc04e64b6c4d008bee1c9ea69e5e44ca8fb18b`. Its focused stacked base is
`feat/accounts-http-54-pr2` while PR72 is draft and unmerged. The eventual
target is develop after its predecessors are merged/reconciled; develop was
rechecked at `729417ffb6de4b376d2736bdfbf78c46f455630e` before implementation.
[ADR-0036](../../adr/0036-isolated-durable-session-validation.md) authorizes only
this bounded series. The original #54 acceptance and broad phase exits stay open.

## User-facing and ownership boundary

The Leptos shell adds `/account` and read-only `/me`. The isolated adapter's
default versioned DTO surface supplies current context and exactly one profile
field: the immutable 32-hex account ID. No name, handle, avatar, biography,
statistics, history, edit/privacy policy or presence is inferred. `/login`,
`/register`, `/friends` and other-profile documents explain their unavailable
capabilities and retain a public Games escape. No credential form is mounted.

The app binary owns this isolated consumer; client-tier `tabula-net-client`
does not import the runtime-tier HTTP owner. The default shipping dependency
exposes DTOs only. The non-default native acceptance feature exercises the real
adapter over TCP with explicitly synthetic fixture identities, not Kanidm.
Neither production service gains a listener or new account dependency.

Requests use fixed same-origin paths, no-store and ambient browser cookie
attachment. JavaScript never reads/exports a session credential. The current
CSRF synchronizer stays only in document memory and is attached to exact empty
JSON mutation bodies. Context version/shape/capabilities and profile version/ID
are validated; profile publication also matches current context subject and
operation generation. A public ID/context/snapshot is never authority.
The browser adapter requires HTTPS, rejects redirects/missing no-store or
incompatible MIME, bounds decoded streamed bodies to 4,096 bytes and aborts a
request after 8 seconds. Neither a Fetch result nor HTTP 200 headers alone are a
successful private read.

## Lifecycle and honest results

Each context/profile/mutation step is tied to its operation generation.
Repeated activation is single-flight. Cancel, newer navigation, page hiding,
account change and logout retire pending work and clear private display/token
state. Browser abort is an optimization; generation checks exclude already
completed or uncancellable late responses. Refresh refetches current context
and fresh self-profile after rotation rather than treating 2xx as a reusable
permission. A failed private response body after 200 headers is a transport
failure, and recovery starts at current context.

Context exposes no server deadline or live revocation stream. This client clears
expiry/revocation when fresh HTTP context establishes loss or a lifecycle/hint
interrupts presentation. It does not prove immediate autonomous clearing of an
already visible profile at the server deadline or after a missed cross-tab
signal. Finite expiry-response tests are not live private-output fencing.

Logout starts by suppressing local private output. An uncertain/offline result
never claims durable revocation, cookie deletion or logout everywhere. The
app-level memory-only suppression survives route owners and rechecks in this
document. An active unresolved logout may retain its original revocation-only
CSRF for an explicit same-route terminal retry; it cannot read/refresh a profile.
Cancel/route exit/pagehide erase that reusable token and retain only its SHA-256
fingerprint as a non-authorizing target hint. A fresh mismatching context never
becomes the target of the old request, including a replacement same-account
session or restarted adapter. Terminal signed-out context after token erasure
stays honestly unconfirmed. Reconciliation must establish revocation before
private data is restored. Closing/reloading loses the memory-only intent:
if the surviving HttpOnly cookie is still valid, a new document may revalidate
and display that old account. Cross-reload offline-logout suppression remains a
production gate, not a hidden local-storage credential fallback.

The UI uses fixed permitted `/account`/`/games` navigation rather than trusting
arbitrary return parameters or replaying mutations. It never launches local or
online gameplay automatically. Existing discovery, game tokens/theme and
separate-document boundary stay intact.

## Design and evidence ledger

The [isolated design delta](../../ui/screens/account-state-isolated.md) adapts
screens 15/16/20/21 and the shared compact M3 Expressive foundation. Static
reference inspection is design evidence only. Native DOM semantics, CSS reflow,
localized copy and source assertions do not prove actual pixels, focus/IME,
password-manager, assistive technology or real first BFCache-restored frames.

| Claim / owner | Independent oracle / scope | Status | Residual |
|---|---|---|---|
| No stale private publication / lifecycle | Named generation, subject, abort, repeated, cancel, rotation and logout-uncertainty tests | PASS: 19 core units and 6 modeled Leptos owner/controller cases | Finite examples; real browser lifecycle separate |
| Strict data boundary / decoder | Independently authored malformed/version/unknown/oversized/error partitions | PASS: full positional root/nested arrays, duplicate fields, unsupported capabilities, subject mismatch and bounded data partitions | Validation is not server authority |
| Real adapter consumption / native acceptance | Actual TCP requests and literal headers/body, synthetic authority fixture | PASS: 11 consumer claims + 1 parser + 19 reused core cases, 31 total | Not TLS, browser cookie enforcement, PostgreSQL or provider evidence |
| Route/public escape / dev static handler | 3 actual loopback HTTP tests include account documents, missing API/runtime/extra paths and cache boundaries | PASS | Static routing only; local HTTP is not authenticated deployment |
| Compact semantics/copy / view | Existing components/tokens, en/vi parity and component/source checks | PASS: 7 view/HTML/source cases and 2 localization cases within 57 web binary units | Runtime layout/keyboard/AT NOT_RUN |
| Architecture/targets/closed services | Authoritative aggregate, feature matrix, native/WASM builds and actual exit gates | PASS: aggregate 1,068/0/21, both feature modes, native client, strict default/all-feature web-WASM and actual unchanged service exits 1 | Compilation alone is not UI interaction; bundle/CI receipts below |
| Durable authority / predecessor | Exact-head CI migrations, authentic metadata, 31 storage and 4 real HTTP+DB cases | Historical PASS in PR72 | Fresh PR3 CI recorded separately at final remote head |

Final remote SHA/tree/head-associated CI receipts are recorded in the linked
PR3 body before this slice is reported complete. A zero/ignored selection is
never database evidence.

Focused native checks so far: `cargo test -p tabula-web --features
account-http-acceptance --all-targets` executes 57 binary units and 31 consumer
target cases, all passing, with zero failed/ignored. The example target selects
zero tests and provides no behavior evidence. `cargo clippy -p tabula-web
--features account-http-acceptance --all-targets -- -D warnings` passes.
CI lists the acceptance target and requires two specific actual-transport case
names before running it, so reused pure-core units cannot hide an empty TCP
selection. These counts include intentionally reused core cases and are not
claimed as 88 unique independent scenarios.

Before acceptance, independent source/probe review found positional-array DTO
decoding, repeated-context ticket ABA, stale completion taking a newer abort
handle, listener-registration failure reopening bootstrap, author CSS overriding
the hidden mask, and unresolved logout targeting a replacement context. Those
were repaired with strict object envelopes/original-byte duplicate rejection,
step serials/exact owners, fail-closed hooks, priority masking and a redacted
revocation target/fingerprint. Named regressions retain these defects. An initial
focused build failed view-macro parsing, and strict native/WASM lint exposed
conditional/test-helper issues. Both strict native and default/all-feature WASM
all-target Clippy reruns pass; failed/interrupted attempts are not passed stages.

A final independent source/probe review also exercised Leptos route-owner
overlap: the replacement view is constructed before the old owner is cleaned
up. Its minimal pre-fix probe failed because old cleanup invalidated the new
context ticket. The shared session now selects a unique route lease; retired
route callbacks and cleanup cannot clear/mask the replacement task. Two retained
regressions exercise actual owner cleanup ordering, complete the replacement
context/profile and reject old hooks. These are controller/owner evidence, not
a real browser navigation or first-frame privacy receipt.

## Portable local checks

Official Rust/Cargo 1.96.1, shared target, default dev/test profiles, two build
jobs and `SQLX_OFFLINE=true` were used for the authoritative final runtime
source. `cargo xtask check` executes its required ordered formatting,
workspace all-target/all-feature Clippy, ordinary workspace tests, dependency,
no-game-ID, manifest, token freshness/raw-colors and cargo-deny gates. All pass:
**1,068 passed / 0 failed / 21 ignored**, 80 Rust summaries, 53 nonempty.
The ordinary workspace test command does not select the optional HTTP consumer,
wire or genuine DB cases; their explicit commands supply distinct evidence.

Both `cargo check --workspace --no-default-features` and `--all-features` pass.
`cargo build -p tabula-game-client` passes, as does building both services.
Both actually execute their unchanged closed failure exits (1), without a
listener. Their source, all mobile/game/shared-theme implementations, session
and HTTP runtime/SQL/migrations and all 16 authentic `.sqlx` bytes are unchanged
from the exact predecessor; only the shared synthetic test fixture is extended.
No local PostgreSQL/client/container was available and no ignored or empty local
DB selection is promoted to integration evidence.

Strict `cargo clippy -p tabula-web --target wasm32-unknown-unknown --all-targets
-- -D warnings`, also with `--all-features`, passes. These compile the actual
browser Fetch/lifecycle code; they never execute a browser. Skill drift, its 32
validator cases and 6 AI-doc checker cases pass. Three actual static-server HTTP
cases pass. All changed Markdown relative file links resolve; this checks file
targets and is not an external-link/anchor or product-interaction claim.

## Browser bundle compilation

`trunk build --config apps/web/Trunk.toml` completes with the official installed
Trunk 0.21.14, Sass 1.69.5 and matching wasm-bindgen 0.2.129. It compiles and
bundles the actual shell WASM, generated token CSS and account styles. The
interrupted first build and environment-only `NO_COLOR=1`/read-only default
cache attempts are not passing stages; the successful build uses a writable
workspace cache and removes the incompatible environment flag. This is
compilation/bundle evidence, not product pixels or browser execution. Final
source and output identities are preserved with the local verification receipts.

## Remaining gates

The earlier native runtime failed `XOpenDisplay`, and cloud CUA loopback access
was denied with `ERR_BLOCKED_BY_CLIENT`. These are respected blockers, not
bypassed browser proof. Actual browser interaction, four-scheme 320/390/768/1440
layout and 200% zoom, IME/password-manager/AT, cookie/TLS/storage inspection,
cross-tab/BFCache first-frame behavior, shipping native/mobile account adaptation
and credential stores remain NOT_RUN or gated as applicable.

PR2's S09 proof remains bounded logical server-frame ordering under its trusted
clock, not released buffered/TCP-byte receipt or WS queue/connection fencing.
Real Kanidm login/registration, provider auth-time synchronization, trusted
reverse proxy/TLS, profile editing/other users/history/metrics, friends/presence,
invitations (#55), grants/network gameplay and voice remain closed. Neither
service startup, production migration or deployment is authorized by this
slice. The owner subsequently authorized normal sequential Ready/merge of
PR71, PR72 and PR3 into develop after PR3 is verified. That separate history
action does not widen runtime scope or close #54; its exact merge and postmerge
CI receipts are recorded in the PR discussion.
