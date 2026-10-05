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
The workflow asserts at least 13 selected real_postgres_online_ tests and
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
5. Both independently decode/render the same terminal “Game over / Black wins”
   verdict; the separate durable oracle checks the exact Checkmate reason. Four
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

## Commands and honest evidence

```sh
python3 -m unittest discover -s tests/online-match -p 'test_*.py' -v
cargo fmt --manifest-path tests/online-match/Cargo.toml --all -- --check
cargo clippy --manifest-path tests/online-match/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path tests/online-match/Cargo.toml --locked --bin online-match-fixture
cargo build --manifest-path tests/online-match/Cargo.toml --locked --bin online-match-fixture
cargo test -p tabula-storage --features online-match-postgres --locked real_postgres_online_ -- --ignored --test-threads=2
(cd apps/web && TABULA_PLAY_BASE=/play trunk build --release --features online)
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
Post-logout rejection also does not by itself establish a held HTTP first-frame
or a nonempty private queue racing with revocation, or recall of already released
TCP bytes. Any such claim requires its separately exercised ordering scenario.

Primary infrastructure references:

- [Playwright persistent-context process/profile isolation](https://playwright.dev/python/docs/api/class-browsertype#browser-type-launch-persistent-context)
- [Playwright browser-level Chromium CDP session](https://playwright.dev/python/docs/api/class-browser#browser-new-browser-cdp-session)
- [Chromium Linux certificate management and existing NSS database selection](https://chromium.googlesource.com/chromium/src.git/+/refs/heads/main/docs/linux/cert_management.md)
