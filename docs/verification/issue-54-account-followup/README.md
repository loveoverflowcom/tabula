# Issue #54 account, profile and social completion

Original start: `develop` at `c6d55a6fc3b326e14a466b6e9d988f897bb579e5`.
PR base updated to `8c5e3e8d9df133e14d30b0c61b34c3fae97332a8` to retain the
mobile workspace move and current read-only review workflow.
The owner requests every original criterion in this new implementation PR.
[ADR-0043](../../adr/0043-isolated-account-registration-social.md) opens the
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
| Offline retirement, fresh recovery | Account core/controller; old completion restores ID | 53 account tests plus focused Chromium offline/online events with HTTP DTO doubles | PASS before v2 additions |
| Logout intent stays suppressed | Core/controller; reconnect forgets unresolved revocation | Exact target markers and unresolved/storage dispositions in focused tests | PASS before v2 additions |
| Task routes and fixed recovery | Leptos view; disposed signals panic during SPA navigation | 28 legacy and 24 v2 browser interactions, fresh context/profile and exact route assertions, zero page errors | PASS |
| Reflow and focus | CSS/view; translated controls overflow or retiring actions lose focus | 192 legacy and 160 v2 render cases: 320/390/768/1440px, four themes, en/vi; focus and touch-target assertions; small-width and high-contrast pixels inspected | PASS |
| Repository contract | Dependency/token/code boundaries | `cargo xtask check`, web HTTP suite and WASM clippy | PASS before v2 additions; final changed-tree gate owed |
| New DTO field/version boundary | Session HTTP transport; hostile fields or stale shape admitted | `cargo test -p tabula-session-http accounts::` (3 tests), `--test dto_contract` (8 tests) | PASS |
| Registration/profile durable authority | Auth/session/storage; duplicate create, borrowed epoch or disclosure race | Actual pinned Kanidm 1.11.2 and PostgreSQL 18.6 acceptance | PASS: 3 provider cases and 5 account PG laws |
| Friends/presence durable authority | Lobby/storage/HTTP; forbidden mutation, stale Online or late private output | Actual PostgreSQL plus independent clock/transport oracles | PASS: 4 social PG laws and 7 focused rules/transport tests |
| Existing HTTP compatibility | Native session/match adapters; version or capture-route regression | Axum upgrade regressions and actual loopback browser-controller HTTP suite | PASS: 89 adapter tests and 47 controller HTTP tests |
| Complete compiled browser journeys | Leptos/WASM, native HTTP/WSS, Kanidm and PG | Two independent trusted Chromium processes with real cookie-bound provider callbacks | PASS: 21 distinct account/profile/social cases; strengthened route/reconnect scope and DOM assertions require a final rerun |

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

```bash
SQLX_OFFLINE=true cargo test -p tabula-storage --features social-postgres accounts::tests -- --ignored --test-threads=1
SQLX_OFFLINE=true cargo test -p tabula-storage --features social-postgres social::tests -- --ignored --test-threads=1
bash tests/kanidm/run.sh bash tests/accounts/provider_acceptance.sh
cargo test -p tabula-session -p tabula-auth --features tabula-auth/accounts --lib
cargo test -p tabula-web --features account-http-acceptance --test account_http
```

Account laws: 5 passed; social laws: 4 passed; provider script: exact 3 selected
and 3 passed; domain/verifier libraries: 29 session and 13 auth tests passed;
loopback controller HTTP: 47 passed. Disposable opt-ins, provider configuration,
database URL, job-scoped certificate trust and nonempty selections are required
by the runners. The provider script covers original invited login, captured-epoch
legacy profile completion/preservation, and unmapped enrollment/login/profile
policy. Healthy publication lease cleanup, lost physical backend expiry and
permission-narrowing writes have independent database exclusion oracles.

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
[ADR-0043](../../adr/0043-isolated-account-registration-social.md) records its
owner-approved exception. Creating this PR does not imply merge or deployment.

Registration is Tabula enrollment for an existing verified Kanidm identity;
provider person/credential onboarding stays external. No terms, statistics,
achievements, password or token storage is invented. Current permitted profile
and friend data come from real authority. UI doubles, accessibility-tree and
composition checks are recorded separately from provider/PG/browser integration.
Manual assistive-technology, OS IME, password-manager, native/mobile and broad
production acceptance are not inferred from browser automation.
