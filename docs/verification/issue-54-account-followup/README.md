# Issue #54 account, profile and social completion

Original start: `develop` at `c6d55a6fc3b326e14a466b6e9d988f897bb579e5`.
PR base updated to `1147f8f861e0ad59d996a917bf79f0bc670f65b3` to retain the
mobile workspace move, native host direction, shell parity and current review
workflow. The upstream native-host ADR-0043 is preserved; this account decision
uses ADR-0044.
The owner requests every original criterion in this new implementation PR.
[ADR-0044](../../adr/0044-isolated-account-registration-social.md) opens the
bounded account/social contracts alongside the existing ADR-0036/0038 authority.
Production startup and broad phase exits remain closed.

Login, Account and self-profile have short route-specific introductions.
Authenticated Login offers `/me`, with no ID on that route. Unavailable tasks
give explicit supported recovery and retain local play. Shared tonal status,
contained reasons and action hierarchy use existing foundation and en/vi keys.
Known browser disconnect retires private output and pending generations;
reconnect requires current HTTP authority before restoring profile output.
Saved unresolved logout never becomes confirmed sign-out from a network hint.

| Claim | Owner / plausible failure | Oracle and check | Status |
|---|---|---|---|
| Offline retirement, fresh recovery | Account core/controller; old completion restores ID | 87 default / 88 combined-feature web tests, including account/generation and strict JSON laws, plus Chromium offline/online events with HTTP DTO doubles | PASS |
| Logout intent stays suppressed | Core/controller; reconnect forgets unresolved revocation | Exact target markers and unresolved/storage dispositions in focused tests | PASS |
| Task routes and fixed recovery | Leptos view; disposed signals panic during SPA navigation | 28 legacy and 24 v2 browser interactions, fresh context/profile and exact route assertions, zero page errors | PASS |
| Reflow and focus | CSS/view; translated controls overflow or retiring actions lose focus | 192 legacy and 160 v2 render cases: 320/390/768/1440px, four themes, en/vi; focus and touch-target assertions; small-width and high-contrast pixels inspected | PASS |
| Repository contract | Dependency/token/code boundaries | Full `cargo xtask check`, feature/target checks and strict native account/social clippy | PASS: 1,288 tests, 0 failures; all portable gates passed |
| Bounded frontend resource profiles | Build feature / emitted artifact; new scope silently replaces old cap | Default 716,893 and online 774,855 bytes against 900,000; combined 1,040,870 against the explicit ADR-0044 1,100,000 cap; 3 profile/cap laws and 17 dashboard helper regressions | PASS; the original full artifact failed the old cap, recorded in ADR-0044 |
| New DTO field/version boundary | Session HTTP transport; hostile fields or stale shape admitted | `cargo test -p tabula-session-http accounts::` (3 tests), `--test dto_contract` (8 tests) | PASS |
| Registration/profile durable authority | Auth/session/storage; duplicate create, borrowed epoch or disclosure race | Actual pinned Kanidm 1.11.2 and PostgreSQL 18.6 acceptance | PASS: 3 provider cases and 5 account PG laws |
| Friends/presence durable authority | Lobby/storage/HTTP; forbidden mutation, stale Online or late private output | Actual PostgreSQL plus independent clock/transport oracles | PASS: 4 social PG laws and 7 focused rules/transport tests |
| Existing HTTP compatibility | Native session/match adapters; version or capture-route regression | Axum upgrade regressions, loopback browser-controller HTTP and separate acceptance workspace | PASS: 89 adapter, 47 controller HTTP and 23 standalone fixture tests; standalone strict clippy passed |
| Complete compiled browser journeys | Leptos/WASM, native HTTP/WSS, Kanidm and PG | Two independent trusted Chromium processes with real cookie-bound provider callbacks | PASS: 21 distinct cases, including fresh route/reconnect scopes, restored peer DOM and durable logout; zero runtime errors |

## Original acceptance map

| #54 criterion | Implementation and independent evidence |
|---|---|
| M3 Expressive grouping/hierarchy | Foundation semantic surfaces, shared task/status/form/list groups and localized Edit/Save/Search hierarchy; rendered matrix and inspected screenshots |
| Purposeful borders/shadows | Shared tonal grouping; focus/field-error outlines retained; raw-color gate and small-screen visual review |
| Primary/secondary action density | Explicit provider continuation, registration submit, profile Edit/Save and friends Search; named secondary recovery, Cancel and Decline |
| Shared 15/16/20/21 components and long text | Common form/status/action/list classes; Unicode names, handle bounds, translated controls and no-history presentation; width/theme/locale matrix |
| Accurate server state/permissions/presence | Durable enrollment receipts, session epoch issuance, profile CAS/disclosure, participant decisions, scoped WSS snapshots and current publication guards; actual provider/PG oracles |
| Local play and storage separation | Fixed library escape and existing separate-document local handoff; HttpOnly credential cookie; transient synchronizers; non-authorizing logout fingerprints retain ADR-0031 semantics |
| Owned data and observed Online | Approved profile fields only; no invented statistics/history/achievements; initial validated Hello and exact peer Pong renewal, current binding, timestamp and retained freshness deadline at frame handoff |
| Error/focus/keyboard/AT/IME/duplicates/expiry | Guarded route/controller laws; browser focus, accessibility-tree and composition events; operation receipts; actual PG expiry-after-lock-wait oracle; manual AT/OS IME limits recorded |

## Executed authority commands

These use the actual disposable PostgreSQL 18.6 instance. CI uses PostgreSQL 16
and must pass on the PR's exact committed tree; a local dirty-tree run is labelled
as working-tree evidence. `SQLX_OFFLINE=true` selects checked-in compile metadata,
while the adapters and test laws still execute against the real database.
Final local portable/feature/actual-browser runs use implementation commit
`bd09d735912ccf4f57b5a02b487c9b14d8cd04ee` (tree
`f5b57b59c80a00ee1bc9857b456436a4d6fac9dc`). The real-browser runner records a
clean checkout at that commit. This ledger's subsequent documentation update
records those results; the PR's hosted checks bind to its published head.
The rebase preserves the account/social behavior previously exercised by the
focused PG/provider laws. Its rebuilt HTML/JS/WASM/CSS resource hashes exactly
match the complete 352/52 UI-double artifact, and all 21 real-browser cases were
rerun successfully at the final clean source. The final optimized shell SHA-256 is
`8f0417cbb59b6d575de91208ac469eb24af27b296ee5c0754945d3a36c7db7e4`;
its 1,040,870 raw / 407,996 gzip9 bytes are static size evidence only. The real
browser runner checks that profile/budget before running its 21 cases. The full
UI-double selection uses that same final `online,account-social` artifact.

```bash
SQLX_OFFLINE=true cargo test -p tabula-storage --features social-postgres accounts::tests -- --ignored --test-threads=1
SQLX_OFFLINE=true cargo test -p tabula-storage --features social-postgres social::tests -- --ignored --test-threads=1
bash tests/kanidm/run.sh bash tests/accounts/provider_acceptance.sh
cargo test -p tabula-session -p tabula-auth --features tabula-auth/accounts --lib
cargo test -p tabula-web --features account-http-acceptance --test account_http
CARGO_TARGET_DIR=/path/to/shared-target SQLX_OFFLINE=true cargo xtask check
cargo check --workspace --no-default-features
cargo check --workspace --all-features
cargo check -p tabula-session-http -p tabula-lobby --target wasm32-unknown-unknown --all-features
trunk build --release --cargo-profile wasm-release --features online,account-social # from apps/web
python3 tools/tests/check-loading-budgets.py --shell-dist apps/web/dist --shell-profile account-social
python3 -m unittest discover -s tools/tests -p 'test_loading_budgets.py' -v
TABULA_ACCOUNTS_DISPOSABLE=1 TABULA_KANIDM_SOCIAL=1 bash tests/kanidm/run.sh bash tests/accounts/run_real.sh
```

Account laws: 5 passed; social laws: 4 passed; provider script: exact 3 selected
and 3 passed; domain/verifier libraries: 29 session and 13 auth tests passed;
loopback controller HTTP: 47 passed. Disposable opt-ins, provider configuration,
database URL, job-scoped certificate trust and nonempty selections are required
by the runners. The provider script covers original invited login, captured-epoch
legacy profile completion/preservation, and unmapped enrollment/login/profile
policy. Healthy publication lease cleanup, lost physical backend expiry and
permission-narrowing writes have independent database exclusion oracles.
The complete portable gate passed all 1,288 selected tests across 102 test/doctest
results; 18 opt-in acceptance tests remain ignored there and their relevant
nonempty selections run separately above. Workspace no-default/all-features,
pure HTTP/lobby WASM all-features, strict native account/social all-target clippy,
web native/WASM clippy, skill validation, 34 skill-helper tests and six
AI-document tests also passed. The new resource-profile laws pass 3 tests, and
historical dashboard helper compatibility passes all 17 tests. Hosted Ubuntu24.04/PostgreSQL16 execution is a
separate required PR check, not inferred from these local PostgreSQL18 results.
The prior base passed 1,294 portable tests; the final upstream base retired six
obsolete mobile WebView packaging tests. No account/social acceptance was removed.

The UI-matrix browser harness uses the actual compiled Leptos shell and deliberately
synthetic context/profile/HTTP responses. Its disposable self-signed HTTPS
server and browser trust bypass do not prove trusted deployment TLS, provider
login, server permissions or cookie enforcement. Browser layout/focus is
interaction evidence; it is not manual screen-reader, native, password-manager,
IME, real social or first-BFCache-restored-frame evidence. The separate
`tests/accounts/real_browser_acceptance.py` instead uses two independent browser
processes, the actual native HTTP/WSS authority, real PostgreSQL and pinned
Kanidm. Job-only CAs are trusted in private browser NSS databases with strict
certificate validation, and an existing home NSS database is untouched. No HTTP
or WebSocket response interception, fixture-issued session or TLS bypass is used
in that real-authority journey. Its 21 cases exercise signed-out enrollment,
explicit subsequent login, Unicode profile editing and privacy, unauthorized
request decisions, cancel/decline/accept, observed Online, route scope replacement,
offline retirement/reconnect and durable logout. Both harnesses require nonempty
exact case selections and zero uncaught page errors.

## Completion policy and evidence boundary

#54 stays open until every original criterion has implemented authority and
adequate executed evidence. [#99](https://github.com/loveoverflowcom/tabula/issues/99)
and [#100](https://github.com/loveoverflowcom/tabula/issues/100) track the same
registration/profile and social scope; they do not transfer it out of #54.
[ADR-0044](../../adr/0044-isolated-account-registration-social.md) records its
owner-approved exception. Creating this PR does not imply merge or deployment.

Registration is Tabula enrollment for an existing verified Kanidm identity;
provider person/credential onboarding stays external. No terms, statistics,
achievements, password or token storage is invented. Current permitted profile
and friend data come from real authority. UI doubles, accessibility-tree and
composition checks are recorded separately from provider/PG/browser integration.
Manual assistive-technology, OS IME, password-manager, native/mobile and broad
production acceptance are not inferred from browser automation.
