# Actual two-browser durable Chess acceptance

This standalone, CI-only fixture supplies PR2's bounded rendered-target evidence
for ADR-0041 and PR3's interruption acceptance target for ADR-0042. It does not start either production service or verify a provider
login. The existing genuine Kanidm job remains unchanged. Synthetic identities
are explicitly provisioned only by this disposable executable; their actual
sessions, admission/seat binding, command commits and protected output all use
the same real PostgreSQL session and match authority as the opt-in adapter.

## Required composition

The workflow supplies a disposable PostgreSQL 16 service. The fixture calls only
storage-owned APIs, including strict PgOnlineMatchStore::migrate(&pool). It
asserts the independently reviewed additive migration versions are exactly
202610040001, 202610050001, 202610050040, 202610050041 and 202610060001. It neither enables
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

## Authentic response observation

Every tested context installs a test-only init script before application scripts.
The wrapper calls the original browser `fetch` once, synchronously, with the same
receiver and argument objects, and returns its original Promise and Response.
It observes only a clone of the authentic response stream. A transparent native
Request constructor proxy keeps bounded plain string/empty body inputs in an
ephemeral WeakMap, including copies of already tracked Request bodies. It keeps
the native constructor arguments, Request objects and prototypes unchanged.
Body accessors are left to the original constructor/fetch and fail observation
closed; untracked nonempty bodies and streamed uploads also fail closed. No
upload is cloned, read or locked: cloning a Request makes Chromium omit its
original post data from Playwright even while the server receives the same bytes.
It does not fulfil routes, rewrite requests or replies, issue a replacement API
call, or fabricate an attachment, Ack or projection. TLS verification is unchanged.

The actual Playwright Request event binds a unique owner to the full URL, method
and SHA-256 of the original request bytes. Pending and failed competing records
participate in collision checks. An older identical request is excluded only
when its earlier unique request-event binding preceded the new fetch invocation.
Read correlation also requires the originating document UUID, the real request's
native start time, and exact response status, URL, bounded media/length metadata
and no-store predicate. An old-document first binding, ambiguity or mismatch
fails closed. Multiple and reentrant consumers use the same underlying specific
Playwright Response cache; they cannot select a retry or charge a body twice.

Observations are bounded per document to 64 records, 128 KiB request bytes,
2 MiB per response, 8 MiB combined retained response bytes and five-second reads.
The constructor source cache separately allows at most 64 protected captures and
8 MiB cumulative source bytes per epoch; unrelated native Requests are excluded.
Crossing its limits closes observation while preserving native construction.
Aborted, truncated, unreadable, oversized, timed-out or invalid JSON bodies stay
failures, with no CDP-body lookup fallback. Response text is retired from page
memory after its exact test-side transfer; its budget remains charged until
document disposal. Pagehide/BFCache disposal cancels readers and clears records;
navigation, page/context close and document checks clear or reject test caches.
Cookies, arbitrary headers, raw grants and observed bodies never enter diagnostics
or artifacts. Only fixed source-owned observation error enums may be retained.

Cloning a response tees its stream and can affect buffering and scheduling.
This is instrumented observation of authentic bytes, not proof that the application consumed its
original body. Existing Rust-decoded seat/private-field, real rendered board,
Ack, native publication-byte, actual SIGKILL/barrier and independent PostgreSQL
durable-prefix assertions remain mandatory. The Node and Python observer tests
are helper contracts only; they cannot establish actual browser acceptance.

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
cargo clippy --manifest-path tests/online-match/Cargo.toml --features continuity-test --locked --all-targets -- -D warnings
cargo test --manifest-path tests/online-match/Cargo.toml --features continuity-test --locked --bin online-match-fixture
cargo build --manifest-path tests/online-match/Cargo.toml --features continuity-test --locked --bin online-match-fixture
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
fixture source includes PR3 reconnect/resync/network-drop/server-crash targets,
but only their actual executed CI receipts can establish that acceptance. It does
not establish load/cross-target/game-portfolio quality or a broad phase exit.
Post-logout rejection alone does not establish buffered-publication fencing.
That claim needs successful execution of the nonempty held-body scenario above.
Bounded move diagnostics retain only the four public action names, observed
HTTP status and Ack-presence flag. Failed terminal waits retain only role, seat,
expected-range visible revision and fixed status/connection/availability enums,
never raw projected status, DOM, command body or protocol payload. A reasonless
Black-win title or another terminal reason cannot satisfy the exact predicate.
Failure diagnostics are captured inside a nested context that exits before the
active Playwright driver is stopped. They retain only a fixed exception-class
partition, bounded actual attachment status/public-seat/readiness subphases,
current public game-state enums and page-crash/closed/browser-connected flags.
Protected HTTP diagnostics count only fixed endpoint classes (context, grant,
attach, poll, command) and bounded status/count pairs per synthetic browser role;
queries, routing IDs and foreign endpoints are discarded.
No exception text, stack trace, URL, attachment body, routing ID or grant is
stored. The existing private in-memory attachment parsing/seat validation stays
unchanged; these facts locate a failure without replacing rendering or authority
assertions.
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

## PR3 actual continuity acceptance target

`run.sh` retains the complete PR2 real-browser game and held-body proof, then
runs `continuity_acceptance.py` through two newly launched independent Chromium
processes with separate private HOME/profile/NSS/cookie state and normal HTTPS
validation. This is implemented acceptance source; execution and actual screenshot
inspection must be recorded separately in the ADR-0042 verification ledger.
The normal fixture carrier is now match HTTP version 2; fixture control DTOs stay
version 1. The game wire remains projection-only protocol 0.1.

Twenty-eight mandatory partitions currently form twenty-four independently audited matches:

- Actual pointer command network abort before send, pending document refresh,
  exact original sequence/payload replay, and retired attachment rejection
- Real staged PostgreSQL apply/COMMIT with browser network loss and exact retry
- Lost output poll, authorized full projection resync, and committed refresh
- Actual native server SIGKILL before COMMIT and after COMMIT before Ack;
  the independent opponent observes the durable partition before sender retry
- A retained original receipt returns its exact Ack without another move;
  evicted and expired original receipts produce explicit unknown-result,
  authorized full projection and a read-only board
- Current revoke, expiry, account epoch and membership changes ordered before
  command submission prevent a commit and restored private projection
- Actual database clock crosses the original session deadline while SQL is
  staged; deferred current-authority fencing rolls back the entire input
- Nonempty real projected output is held at native first-body handoff, then
  revoke, expiry, epoch, membership or physical owner-backend loss commits;
  the forwarded real inner publication error must produce exactly zero body
  bytes, including any partial bytes observed before a transport exception
- New fenced owner recovery precedes release of held old-owner output

The crash partitions wait for an authenticated Black attachment request that
started after the old process was SIGKILLed and reaped, and whose genuine body
completed after restart. Native request timing excludes queued pre-crash events;
canceled responses and headers alone cannot satisfy the witness. The fresh
attachment must preserve Black's seat and operation scope and replace its old
transport identity before the existing board, pixel and durable-prefix checks.
An unchanged pre-crash White-turn board is therefore insufficient resync evidence.

The one-shot gates exist only in the explicitly selected `continuity-test`
standalone fixture and `acceptance-test-support` native gateway. Hooks match
trusted match, auth record and attachment. They never manufacture command
results or projections. The SQL-staged gate comes from the existing storage
transaction barrier; committed-prefix observation uses non-locking MVCC and
returns only agreement with the independently specified public Chess transcript.
Storage-owned fixture controls enforce `CI=true` and the explicit disposable
opt-in plus the exact acceptance database/schema. No SQL enters the HTTP fixture.
Synthetic enrollment is bounded at 32 identities per process; production services
and genuine Kanidm/session/mobile/core workflows remain unchanged.

`process_supervisor.py` starts only the already-built exact fixture binary and
owns its private Unix-domain control socket, PID file and private native log.
Closed commands may confirm liveness, SIGKILL/reap, restart in one of three fixed
receipt-policy modes, or stop. No browser endpoint can supply an arbitrary PID,
command, executable or environment. Server restarts happen only after actual
child death. Missing Unix socket/listener/browser/PostgreSQL support fails actual
acceptance. The three socket helper tests report BLOCKED outside CI when the
executor denies AF_UNIX; CI=true never skips these helpers. Direct safe-child
SIGKILL helper checks do not establish native server recovery.

After all browser contexts close, the supervisor stops and reaps the current
native owner before any fresh-fence audit. Each private audit input names only
its actual accounts/match and independently expected public move prefix. The
native audit recovers the exact full committed history, verifies canonical
Chess commands and immutable actual roster, durable watermarks and terminal
snapshot/verdict where applicable, and forbids all client output. Audit inputs,
credentials, grants, CSRF, pending command bytes, ephemeral TLS keys and profiles
remain outside public artifacts and are destroyed by the enclosing trap.
The fresh audit claim respects the persisted two-second owner-publication
exclusion after server exit. Only a known `Busy` permits retry, with one total
five-second owner budget; other errors and timeout fail immediately with closed
public-safe classifications. No exclusion is removed or interpreted as success.

While an actual command boundary is held, the existing ephemeral CI-control
token can check only whether that already-bound match has the independently
expected public transcript prefix (zero through four moves). Its strict bounded
DTO accepts no match, record, seat or game action. Exact HTTPS Origin, current
token, held phase and deadline are checked before and after the one-second
storage-owned nonlocking committed-MVCC observation; only a versioned boolean
is returned. This avoids fixture authentication queueing behind its deliberately
held room/session transaction. It supplies no Ack, projection or commit result.
After release/restart, ordinary current-authority browser checks and every final
independent native recovery audit remain required.

`finalize_evidence.py` requires the unchanged complete-game browser/audit success,
all twenty-eight distinct successful fault partitions, and all twenty-four successful
independent native prefix audits. Empty, duplicate, missing or failed selections,
partial protected bytes, missing actual inner errors, absent SIGKILL receipts,
and wrong audit prefixes remove the PASS receipt. The native owner selection
also executes at least two real PostgreSQL ownership/publication tests in CI.
Helper tests, compilation and configured workflows cannot seal actual acceptance.
`continuity-ui/` contains additional masked actual screenshots and exact-build
provenance; inspect those pixels only after the executed job supplies them.

Additional real-target lifecycle partitions cover pure apply before durable append,
refresh after known COMMIT before the original Ack, a newly issued current auth
record for the same actual account, an actual switch to a different synthetic
account, late old-attachment delivery after a fresh authorized attachment,
two complete bounded-recovery/exhaustion/explicit double-Retry cycles, canceled
app leave and browser beforeunload, confirmed leave and real history Back.
Original pending operations cannot cross current auth-record/account scope.
The current Chromium's real `pageshow.persisted` is observed during history
return. BFCache receives PASS only if that actual flag is true and the restored
document obtains a new attachment; otherwise its optional receipt states BLOCKED
because the current target did not restore this document from BFCache. A normal
Back navigation and Node lifecycle doubles cannot substitute for BFCache execution.

The focus-only partition uses a separate actual headed Chromium opponent pair
under the CI job's disposable Xvfb display (no TCP listener). A genuine popup
window must produce trusted native window blur/focus while the opener remains
visible and receives zero visibilitychange events. Real current-session logout
commits in that popup, then focus restoration must reject authority before any
old projection or first input is shown/sent. No synthetic DOM event can satisfy
this oracle. Missing headed-window/display support fails actual acceptance.
The other twenty-seven fault partitions retain their original headless processes.

The same-auth-record rotation partition holds an actual committed White command
before Ack, disconnects only its page and rotates the real browser cookie with
the existing authenticated `/api/v1/auth/refresh` operation in another page of
that same browser. A legal Black move retires White's cached old-digest attachment.
A current-cookie/current-CSRF poll must return exact `409 reattach_required`
with no frames. White then reacquires a fresh attachment with the same durable
operation scope, retries its exact original sequence/payload and receives the
stored original Ack. Completing the game and its independent native audit proves
that credential rotation did not make the uncertain move new or duplicate it.
The two opponents never share credentials, and nothing is persisted in artifacts.
Genuine authorization loss remains `401`/`403`; a stale local transport alone
cannot be mistaken for account, epoch or membership revocation.

The grant/attach restart partition holds a genuine browser attach before its
network send, then SIGKILLs and restarts the native process. Its exact original
body and CSRF header continue unchanged: the earlier server CSRF fence must
produce bounded no-store `403 request_rejected` with no frames. The real adapter
reacquires context, grant and the same operation scope before restoring its
projection or retrying the original pending sequence/payload. Full game/audit
agreement proves no duplicate input. A separate, explicitly labelled CA-validated
probe uses newly acquired memory-only CSRF with that old signed-grant body and
must receive exact `409 fresh_grant_required`, with no attachment/projection/apply.
That probe isolates the grant boundary and never substitutes for the unchanged
browser request or its earlier CSRF rejection. Old grants, cookies and CSRF are
runtime-only and never become artifacts.

Admission failure diagnostics now identify only exact create/join endpoint
classes, request/response presence, fixed click/response/body-parse phases,
bounded status/content-type/no-store/body-length facts, and allowlisted browser
protocol error classes. Raw exceptions, call logs, URLs, headers, bodies, join
codes and credentials are never retained. These facts diagnose a failed actual
shell action; they do not replace that action or weaken its mandatory result.

The headed focus witness runs immediately after the application's synchronous
native window-focus concealment handler, before another rendered frame. It
requires hidden visibility/ARIA, cleared scope/status/accessibility/selection,
and observational readPixels agreement with the cleared default framebuffer.
The test first selects a real board piece, holds only genuine current-authority
requests unchanged, then attempts trusted pointer/keyboard gestures at the former
board while revalidation remains unresolved. No command may be emitted. Only
then are those requests released and real revoked-authority terminal concealment
required. Eventual zero dimensions alone cannot seal this first-restored proof.
