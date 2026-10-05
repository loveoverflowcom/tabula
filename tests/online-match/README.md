# Actual two-browser durable Chess acceptance

This standalone, CI-only fixture supplies PR2's bounded rendered-target evidence
for ADR-0041. It does not start either production service or verify a provider
login. The existing genuine Kanidm job remains unchanged. Synthetic identities
are explicitly provisioned only by this disposable executable; their actual
sessions, admission/seat binding, command commits and protected output all use
the same real PostgreSQL session and match authority as the opt-in adapter.

## Required composition

The workflow supplies a disposable PostgreSQL 16 service. The fixture calls only
storage-owned APIs, including strict PgOnlineMatchStore::migrate(&pool). It
asserts the independently reviewed additive migration versions are exactly
202610040001, 202610050001, 202610050040 and 202610050041. It neither enables
ignore_missing nor manipulates session, admission or match rows directly.
Missing database, compiled/staged assets, TLS inputs, trust tooling or browser
setup is a failure, never an ignored test, HTTP mock or successful skip.

The opt-in fixture requires both CI=true and TABULA_ONLINE_MATCH_DISPOSABLE=1.
Its plain HTTP upstream binds only 127.0.0.1:3000. The fixture's HTTPS frontend
binds only 127.0.0.1:9443, serves the real Trunk shell and separately staged
Macroquad/WASM game document, and forwards authentic Cookie/Origin/CSRF headers.
The frontend keeps server output opaque and preserves duplicate Set-Cookie.
The workflow asserts at least 14 selected real_postgres_online_ tests and
explicitly runs them with --ignored against the disposable real PostgreSQL service.

## Actual browser and TLS boundaries

Official Playwright 1.62.0 pins the genuine Chromium download. Three calls to
launch_persistent_context create separate actual Chromium processes, each with
its own temporary HOME, user-data/profile directory, NSS database, cookie jar
and local/session storage. No storageState, cookie or profile is copied between
the two opponents. The third process begins without either opponent's session.
Chromium's sandbox is enabled. Browser-level CDP process information must report
three distinct browser process IDs; independent storage markers also verify that
the two opponents and third browser cannot see each other's local/session storage.

The enclosing script generates a job-lifetime CA and localhost SAN certificate,
verifies the chain and hostname, and imports the CA only into each temporary
HOME/.pki/nssdb. Current Chromium uses this existing legacy path even where its
default is HOME/.local/share/pki/nssdb. Actual browser HTTPS navigation must pass
normal certificate verification. No system/global CA, persistent access, ignored
HTTPSErrors, security-warning bypass flag, local tunnel or denied-route workaround
is used. Private keys and temporary profiles are removed on success or failure.

The fixture enrollment HTML and served static HTML documents use the narrowly
scoped `Referrer-Policy: same-origin`. The Fetch standard otherwise changes a
non-CORS form POST's Origin to `null` under `no-referrer`; that would correctly
fail the unchanged exact HTTPS Origin check. Protected/API replies still use
`no-referrer`, and missing/null/foreign Origin remains denied. The document
policy suppresses cross-origin referrers; no credential or invitation is ever
placed in a document URL.

Startup evidence is closed and bounded. `browser-result.json` records real
HTTPS enrollment-page status/readiness, an allowlisted Chromium network-error
enum, browser version, each actual form POST's HTTP status and Origin class
(`exact`, `missing`, `null`, `foreign`), expected form media type and successful
redirect flag. A non-303 form response fails before a generic redirect timeout.
Native/TLS child probes retain only process-phase enums and exact source-owned
failure classes from bounded private log reads. No raw error, URL, header, body,
request failure text, private log or process ID enters those artifacts. These
diagnostics do not bypass normal certificate validation or replace actual
browser enrollment, rendering and gameplay.

Each opponent actually navigates to the labeled fixture page and presses its
enrollment button. A same-origin HTTPS form POST issues an independently random
credential through PgSessionStore and returns a Secure/HttpOnly/__Host cookie.
The fixture checks real authenticated contexts, distinct actual accounts and
cookie values, HttpOnly exclusion from document.cookie, and the third browser's
initial absence of an opponent cookie. Enrollment is not a product login route.

## Nonvacuous target scenarios

1. White presses Create in the real shell and sees the returned join code
2. Black enters that code and joins the same match as the other server-assigned
   seat; an exact duplicate join preserves the binding
3. Both navigate into the separate actual gameplay document, attach through the
   same real authority, and render nonblank Chess canvas pixels
4. Real canvas taps play f2-f3, e7-e5, g2-g4, d8-h4. Visible revisions start at
   zero and advance to one, two, three and four as those projections change
5. Both independently decode/render the same terminal Black-wins Checkmate
   verdict. The exact Rust game-owned accessibility status is
   `Game over / Black wins / checkmate`: it includes the reason after the
   shorter canvas HUD title. The separate durable oracle independently checks
   the exact Checkmate reason. Four
   secret-free actual canvas screenshots and action names are retained. Only
   then is White's first exact captured command retried: its original Ack is
   returned, and the opponent's visible revision remains four. The extra Ack is
   deliberately consumed after gameplay so the probe cannot create an artificial
   gap in Rust's live transport during the game
6. The third process cannot obtain another player's grant, join a full live
   match, issue a copied command or receive opponent projection output. The
   full-roster admission probe runs before completion can mask that boundary
7. Same-account cross-match attachment/command reuse is denied against a second
   actual admitted/attached actor, with the envelope match ID set to that actor
8. Real logout revokes Black's session; replaying its exact old runtime cookie
   cannot issue a cached command or obtain output through a new protected request
9. Actual CA-validated HTTPS probes reject missing/foreign/null Origin,
   missing/wrong CSRF, wrong content type, duplicate cookie and mixed bearer+
   cookie, malformed/foreign/tampered signed grant, client-selected seat,
   zero-sequence envelope and URL/envelope match-ID mismatch
10. Same-session reattachment retains next_seq=3; the old attachment can neither
    command nor receive output. This bounded replacement probe is not PR3
    reconnect/resync or interrupted-command acceptance
11. In the second real actor, Black is a passive recipient with its initial
    snapshot consumed and no pending Ack/Reject. White's real legal f2-f3 command
    supplies a queued projection. A native-only PollCaptureWitness is matched to
    the exact match, attachment and trusted session record, and must count an
    actual MatchUpdate before the fixture holds the real guarded poll body
12. After the existing authority lease expires independently, a separate normal
    current-credential logout must commit while that body stays held. Releasing
    the gate forwards the real inner publication error. A CI-only upstream TCP
    observer requires actual 200/JSON/no-store headers and zero body bytes,
    including partial bytes on a transport error, before TLS-edge buffering

The last scenario requires the standalone body-publication-test feature, which
enables only the gateway's acceptance-test-support native response extension.
Neither default service gains controls. The witness has checked private fields
and no serde/wire representation. Fixture controls are one-shot, bounded and
phase-only; their ephemeral capability stays in memory and cannot authorize a
game command or session action. The body gate never polls early, manufactures
suppression or discards unexpected private data: any real data is forwarded and
the byte-count assertion fails. Actual inner-error phase is also required, so
cancellation cannot masquerade as publication fencing.

Original captured command bodies stay as opaque strings only in runtime memory,
so JavaScript never rounds the u128 match identity during a duplicate probe.
Grants, CSRF tokens, cookies, invitation codes and private canonical facts are
excluded from URLs, logs, screenshot contents and uploaded artifacts. There is
no HAR, trace, page dump, saved login or browser-profile artifact.

After all browsers close, a separate private audit CLI claims a fresh journal
fence and calls runtime::recover against the exact real committed prefix. It
then checks the independently specified four Chess inputs, five rows including
genesis, immutable roster matching those actual page-issued accounts, distinct
session operation scopes, one terminal transition, atomic terminal snapshot,
and the decisive Checkmate verdict with Black as winner. No client output can
be emitted by the audit. Only closed boolean/count/verdict results are published;
the private account input and canonical data never become browser frames.
The private audit input is created only after both exact actual terminal views
are confirmed, outside the public artifact directory. After browser teardown the
real audit still runs if a later auxiliary check fails, preserving independent
main-stream evidence. `audit-result.json` separates audit pass/fail/not-run from
the mandatory browser exit. A later browser/auxiliary failure always keeps the
overall job failed and removes any PASS receipt; `result.txt` exists only when
every mandatory browser check and the real durable audit both pass. This ordering
does not skip or replace the live authority-loss/neutral-surface requirement.

## Actual UI image review

The same mandatory browser run captures these actual served UI states, after
their corresponding visible conditions succeed:

- `00-dashboard-discovery.png`: dashboard and featured-game discovery
- `01-game-library.png`: rendered discovery library
- `02-create-join-controls.png`: the actual create/join controls
- `03-created-code-waiting.png`: successful creation waiting for the opponent
- `04-opponent-joined.png`: successful join with the other seat
- `05-white-initial-board.png` and `06-black-initial-board.png`: both independent
  actual Chromium processes' rendered initial Chess canvases
- `07-white-terminal-result.png` and `08-black-terminal-result.png`: both
  independently rendered terminal canvases after the complete legal game
- `09-terminal-result-page.png`: the full authorized terminal result page
- `10-authority-unavailable.png`: a real live board's neutral unavailable UI
  after normal current-session logout and a denied live poll

Input and textarea values, the active invitation-code display, secret-marked
nodes and exact in-memory credential/code text matches are masked by Playwright
at screenshot capture. The fixture does not edit the page to create a state,
retain raw DOM, serialize secret mask values, generate images or infer rendered
pixels from HTTP results. Full-game screenshots are taken while authorization
is current, before those documents are closed. The last image has its own third
auxiliary actor, created/joined through real UI controls, so it cannot consume the
passive recipient's queued projection in the held-body proof. White first renders
that live initial board. A separate page in White's own context then performs
normal current-credential logout; the live game must receive an actual poll401.
Before capture the real UI must positively show its neutral error, hide and
zero-size the canvas, remove private status datasets and clear status spans.
The live game is explicitly foregrounded after opening its separate control
page, because Macroquad transport polling is animation-frame-driven. Closed
auxiliary subphase/status facts identify setup, visibility, logout, actual401,
neutral verification and capture failures. The host intentionally aborts a
non200 response body, so the fixture observes the real401 without forcing a
Playwright JSON read after that abort. The independent revoked-output JSON
probes and native held-body zero-byte oracle remain required and unchanged.
Stale game pixels or private status must never be captured after authority loss.
This bounded active-document concealment check does not claim PR3 reconnect,
interrupted-command or BFCache lifecycle acceptance.

`screenshots-provenance.json` labels the disposable synthetic authentication,
separate genuine Kanidm CI, actual screenshot conditions, exact checkout commit
and tree from a required clean checkout, deployed shell/game WASM and HTML/JS/CSS
hashes, native fixture hash, tool versions,
actual Chromium and Playwright versions, viewport, device-pixel ratio, capture
time, dimensions and PNG SHA-256. It records only allowlisted route paths, never
queries, fragments, grants or credentials. The artifact directory is
`verification/online-match-artifacts/`.

Helper tests and this source do not produce actual UI evidence. A review report
or GitHub issue must wait for the executed CI job's successful game, durable
audit and nonempty held-body proof, then inspect the actual PNG pixels and verify
their manifest hashes. Until that happens, image capture and visual review are
`NOT_RUN`. Only inspected secret-free images and their public provenance may be
published in the single user-requested review issue.

## Commands and honest evidence

```sh
python3 -m unittest discover -s tests/online-match -p 'test_*.py' -v
cargo fmt --manifest-path tests/online-match/Cargo.toml --all -- --check
cargo clippy --manifest-path tests/online-match/Cargo.toml --features body-publication-test --locked --all-targets -- -D warnings
cargo test --manifest-path tests/online-match/Cargo.toml --features body-publication-test --locked --bin online-match-fixture
cargo build --manifest-path tests/online-match/Cargo.toml --features body-publication-test --locked --bin online-match-fixture
cargo test -p tabula-storage --features online-match-postgres --locked real_postgres_online_ -- --ignored --test-threads=2
(cd apps/web && TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online)
cargo build -p tabula-game-client --no-default-features --features web,online --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-local-play
bash tests/online-match/run.sh
```

Python helper tests are in-memory/mocked infrastructure checks only. Their PASS
does not establish actual TLS, PostgreSQL, browser rendering or gameplay. Build
and staging evidence is also separate. An actual browser claim requires the
executed new CI job's terminal PASS and durable audit, and a screenshot-inspected
claim requires a human/assistant to review the captured pixel images. This
fixture does not establish PR3 reconnect/resync/network-drop/server-crash
acceptance, load/cross-target/game-portfolio quality or a broad phase exit.
Post-logout rejection alone does not establish buffered-publication fencing.
That claim needs successful execution of the nonempty held-body scenario above.
Bounded move diagnostics retain only the four public action names, observed
HTTP status and Ack-presence flag. Failed terminal waits retain only role, seat,
expected-range visible revision and fixed status/connection/availability enums,
never raw projected status, DOM, command body or protocol payload. A reasonless
Black-win title or another terminal reason cannot satisfy the exact predicate.
Its TCP observer establishes the native server first-frame/network boundary
before the buffering TLS edge; actual browser TLS/gameplay is proved separately.
It does not establish recall of already released TCP bytes or PR3 reconnect.

Primary infrastructure references:

- [Playwright persistent-context process/profile isolation](https://playwright.dev/python/docs/api/class-browsertype#browser-type-launch-persistent-context)
- [Playwright browser-level Chromium CDP session](https://playwright.dev/python/docs/api/class-browser#browser-new-browser-cdp-session)
- [Chromium Linux certificate management and existing NSS database selection](https://chromium.googlesource.com/chromium/src.git/+/refs/heads/main/docs/linux/cert_management.md)
- [Fetch Standard: append a request Origin header](https://fetch.spec.whatwg.org/#append-a-request-origin-header)
- [Referrer Policy: same-origin](https://w3c.github.io/webappsec-referrer-policy/#referrer-policy-same-origin)
- [Playwright request failures versus HTTP error responses](https://playwright.dev/python/docs/api/class-request#request-failure)
