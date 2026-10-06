# Rendered account-shell acceptance

`browser_acceptance.py` drives the actual Trunk-built Leptos/WASM shell in an
installed Google Chrome or Playwright Chromium through Python Playwright. It uses **explicit synthetic
version-1 and version-2 HTTP/WebSocket doubles** for the bounded account/social routes. The legacy matrix explicitly returns unavailable for the v2 self-profile probe. No provider,
PostgreSQL instance or real account is used. The disposable HTTPS/WSS static server
creates a temporary self-signed certificate and the browser bypasses certificate
trust **only for this test fixture**. This proves no secure-transport acceptance.

Build the shell, then run from the repository root with Python Playwright already
installed (the acceptance environment uses `/tmp/tabula-brand-venv/bin/python`):

```bash
cd apps/web
trunk build --release --cargo-profile wasm-release --features online,account-social
cd ../..
/tmp/tabula-brand-venv/bin/python tests/accounts/browser_acceptance.py \
  --web-dist apps/web/dist \
  --receipt /tmp/tabula-account-ui-receipt.json
```

`account-social` explicitly opens ADR-0044's isolated frontend composition;
`online` additionally retains the existing direct-match controls. The complete
account/social selection requires this combined optimized artifact. CI first
checks the separate default and `online` artifacts against their unchanged
900,000-byte raw WASM caps, then checks the combined artifact against its
documented 1,100,000-byte cap and uses that artifact for both browser harnesses.

Other environments can install the pinned `requirements.txt`, run
`python -m playwright install chromium`, and use their Playwright Python.
The harness selects local Google Chrome when present, otherwise the installed
Playwright Chromium; `--chrome` explicitly selects an executable.
`--screenshots /tmp/tabula-account-ui-screenshots` optionally records
the rendered matrix. Screenshot capture alone is not visual inspection.
`--only-interactions` is an explicitly labeled focused 52-case iteration run;
the default command above also runs the 192-case legacy rendering matrix plus 160 v2 rendering cases.

The nonempty receipt must contain exactly 192 legacy render cases and 160 v2 render cases. The legacy cases are: signed-out `/login`
and `/me`, unavailable `/register` and `/friends`, and authenticated `/login` and
`/me` at viewport widths 320, 390, 768 and 1440, in English and
Vietnamese, across light, dark, high contrast light and high contrast dark.
Themes are selected through the real color-scheme/contrast media preferences;
the resulting generated-token `data-theme` and document `lang` are asserted.
Every legacy route checks its translated task heading, bounded readable column, no
horizontal scrolling, generated minimum control height, computed control-text
contrast of at least 4.5 against the composed solid backgrounds, one usable principal
action, unavailable-feature escapes and absence of credential/profile inputs in the unavailable legacy selection.
Text buttons must resolve to the generated semantic primary color, compared with
an independent temporary color probe rather than changing the actual control.
Authenticated Profile renders the synthetic immutable ID; authenticated Login
renders only the permitted profile destination. This exercises actual ID wrapping
across the same viewport, locale and theme matrix.

Twenty-eight interaction cases run at width 390 in light theme, fourteen per language:

- Cancelling held sign-in preparation prevents the provider-start request; an
  explicit synthetic provider-start error reports failure without credentials,
  private identity or external provider navigation.
- Authenticated logout Cancel and Escape focus Cancel, send no logout, and restore
  focus to the invoker.
- Recheck hides private output; the pending native control prevents duplicates.
  Cancel restores heading focus and a deliberately late context response cannot
  restore private identity.
- Refresh retires private output until fresh context and matching self profile
  are received.
- The browser's offline event retires identity and a focused logout confirmation
  moves to the persistent heading; online recovery requires fresh context and
  profile requests for the replacement synthetic subject. Focus on the
  account-independent local escape stays in place across the same transition.
- A focused signed-in Login profile link also moves to the heading when its
  permitted destination retires on disconnect.
- Signed-in `/login` displays no private ID and links explicitly to `/me`.
- Actual shell links traverse Account, Register, Login, Profile and Friends,
  including Friends' Back-to-account escape. Each route must replace the title
  and permitted private task correctly. Client history Back/Forward between
  Login and Profile also requires fresh context and profile; this is no BFCache
  first-frame claim.
- A failed logout keeps private output hidden and offers retry; only an acknowledged
  synthetic retry completes the task.
- Leaving `/me` while its profile request is held retires the route owner; releasing
  that old response cannot restore private output on the destination.

The fixture asserts expected methods and document-memory mutation tokens, captures
uncaught page errors, and fails on unexpected APIs or incomplete test selection.
Cancellation/late-answer assertions exercise the real Fetch cancellation and route
cleanup. They do not bypass AbortSignal to inject callbacks inside the Rust core.
The focused pure-core tests remain the oracle for generation rejection independent
of transport abortion.

This is `interaction-tested` evidence within the stated doubled boundary. It does
not establish real provider/password/autofill behavior, real durable sessions,
BFCache restoration, assistive-technology output, production authentication or
broad phase exits (ADR-0036/ADR-0044).

The additional160 v2 cases cover provider-proven ready registration fields, self profile details and editing, a permitted other profile, and friends/search/request lists at every width/theme/language combination. Long Unicode names exercise wrapping. Twenty-four v2 interactions cover native accessible names in Chromium’s AX tree, invalid fields, synthetic composition events and keyboard Enter, single-flight registration and writes, accepted-without-session output, CAS conflict/refetch,204 save reconciliation including failed-read GET-only recovery, unknown-write identical retry, the app-owned socket across SPA navigation, serialized late scope rejection, five-second stale presence, offline masking and fresh-context recovery, and retired search responses. Browser AX-tree inspection and dispatched composition events do not establish manual assistive-technology or operating-system IME acceptance.

Social snapshots use native browser WSS framing to the same disposable HTTPS fixture, with an explicit fixture-only cookie selecting synthetic data. This cookie authorizes no account or provider operation. The fixture sends bounded snapshot frames and normal close handshakes; every case asserts one active stream per document. This avoids relying on Playwright’s routed-WebSocket teardown, whose 1.62 optional-close payload can omit fields. The server and certificate are disposable, and secure transport remains outside this receipt’s claim.

## Real account/social authority

`run_real.sh` is a separate job-only journey against the compiled native
`account_social_acceptance` example and shell, actual PostgreSQL, and the pinned
Kanidm provider. It uses two independent Chromium processes and strict HTTPS/WSS
certificate validation. Its exact 21 cases and scope/revision assertions exercise
enrollment, explicit login, self edit/other privacy, friend request permissions
and decisions, positive presence, SPA resync, offline recovery, and logout. No
browser API response is mocked in this journey.

Build the shell as above and compile the fixture with
`cargo build -p tabula-auth --features accounts-acceptance --example account_social_acceptance`.
Provide a fresh disposable database named `tabula_accounts_browser_acceptance`
at loopback as `tabula_test`, with `DATABASE_URL` and `SQLX_OFFLINE=true`.
Then run:

```bash
TABULA_ACCOUNTS_DISPOSABLE=1 TABULA_KANIDM_SOCIAL=1 \
  bash tests/kanidm/run.sh bash tests/accounts/run_real.sh
```

The provider runner supplies its disposable guard and private configuration.
`openssl` and `certutil` are required. A clean CI home uses a task-only
`XDG_DATA_HOME/pki/nssdb`; if the user's legacy home NSS database exists, the
runner requires an explicitly supplied `TABULA_ACCOUNTS_CHROME` wrapper with
`TABULA_ACCOUNTS_PRIVATE_NSS=1` that isolates the browser's trust database.
It never imports CAs into the user's database or disables certificate checks.
Ports 3001,8443 and8444 must be free; child readiness is bounded and the TLS edge
is checked against that run's unique CA. Each run retires the previous public
receipt before setup, and failures propagate a nonzero exit.

Only fixed case labels, bounded verdicts/counts and Git provenance are public in
`verification/account-social-artifacts`. Private provider/native/browser logs
are task-only and deleted during cleanup. `TABULA_ACCOUNTS_KEEP_PRIVATE_ON_FAILURE=1`
may retain them for local diagnosis; it is absent in CI. The real journey does
not establish manual AT, OS IME, password-manager, native/mobile or production
acceptance. See the [#54 evidence ledger](../../docs/verification/issue-54-account-followup/README.md).
