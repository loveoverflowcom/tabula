# Issue #110 PR01: local/dev service composition

Implementation base: `develop@d707756c9e43412fa3c4a387b9fd8114442c1f51`.
The issue's historical source pin was `face8a3`; this work repinned current HEAD.
[ADR-0047](../../adr/0047-local-dev-backend-lifecycle.md) owns scope;
the [runbook](../../local-dev-backend.md) owns application commands.
Executed artifacts stay outside source; the PR and Actions receipts identify
the delivered source. Local dirty-tree receipts explicitly distinguish base
HEAD/tree from actual binary/bundle and working-diff hashes.

| Claim / owner | Oracle / domain | Evidence / status | Residual |
|---|---|---|---|
| Explicit runnable service leaves | Actual `tabula-server` process and real PostgreSQL | integration-tested PASS: one executed ignored process test, zero ignored after selection | Not production deployment |
| Context issued by auth adapter permits only the current exact browser session at gameplay | Two HTTP adapter instances sharing a configured CSRF key; real durable sessions | integration-tested PASS: same-key current session accepted; wrong key, other session and Origin denied | Provider identity verification is separate |
| Check-only schema startup never silently migrates | Empty schema, actual migration history, known existing data and hostile history/object changes | integration-tested PASS: three PostgreSQL readiness tests | Required object/history probes, not arbitrary DBA tamper resistance |
| Existing durable authority remains intact | Atomic concurrent create at configured lifetime limit, admission/revocation/publication/owner partitions | integration-tested PASS: 18 real PostgreSQL online tests | Lifetime cap retained; active resource reclaim is PR02 |
| Accepted command/seat/receipt survives clean service restart | Actual binary SIGTERM, SQL-free journal readback, fresh attachment and exact original command retry | integration-tested PASS: unchanged index/hash, same seat/scope and duplicate Ack | Timed/private effects remain unavailable |
| Shutdown owns upgraded social tasks | Actual WebSocket upgrade with pending Hello, SIGTERM, Close 4411 and process exit | integration-tested PASS within the configured deadline | Delivery/receipt after transport handoff is not claimed |
| Shutdown preserves uncertain owned journal work | Existing actor behind append barrier; cancellation/quiescence/deadline partitions | example-tested PASS: gateway 24 unit tests and 4 DTO tests | No blind abort or timeout-as-failed-commit inference |
| Social cancellation drops the same hub attachment and reports failures honestly | Shutdown before/during work, callback cancellation, ticker failure, deadline | example-tested PASS: seven focused tests | Presence metadata is not a device-activity claim |
| Real provider/browser service journey | New `tests/local-dev/run.py` and `local-dev-backend` workflow | Local provider HTTPS/bootstrap PASS; local browser BLOCKED by existing user NSS. CI at b53b1e2 passed the process test/builds/provider setup, then browser acceptance FAILed before a useful stage was retained | Rerun has closed stage diagnostics, partial cases, verified TLS readiness and normal full Chromium; required service browser PASS remains pending |
| Recorded canvas/board timeout on current baseline | Independent normal Chromium, HTTPS, real PG and unchanged assertions at base d707756 | Main complete-game browser PASS; continuity FAIL reproduced. The corrected uncommitted crash partition PASSes; committed restart exposed an actual supported session-problem/transport marker mismatch | Full final-head suite remains pending; no backend rule defect inferred from timeout |

The real service process command is:

```sh
TABULA_LOCAL_DEV_TEST_DATABASE_URL=postgres://tabula_test@127.0.0.1:55432/tabula_service_acceptance \
cargo test -p tabula-server --features local-dev --test local_dev -- --ignored --nocapture
```

The new real-provider acceptance runs only with a dedicated disposable database,
two real provider subjects and actual service binaries:

```sh
cargo build -p tabula-server -p tabula-auth --features tabula-server/local-dev,tabula-auth/local-dev
TABULA_LOCAL_DEV_DISPOSABLE=1 TABULA_KANIDM_SOCIAL=1 \
TABULA_LOCAL_DEV_BROWSER_DATABASE_URL=postgres://tabula_test@127.0.0.1:5432/tabula_local_dev_browser_acceptance \
bash tests/kanidm/run.sh python3 tests/local-dev/run.py --receipt verification/local-dev-backend/browser-receipt.json
```

The harness strips inherited service overrides before fixture migration/startup,
preserves normal TLS verification and browser sandboxing, and never provisions
synthetic identities through application routes. Its job-local TLS edge is test
infrastructure; Caddy in the runbook is the application edge. Exact duplicate
readback is established by the process test, independently of rendered browser
interaction. A browser status-only response does not establish consumed output;
the journey requires both real rendered boards and the terminal verdict.

The authoritative `just check` passed: formatting, workspace clippy/tests,
dependency/game-id/manifest checks, generated tokens/raw-color checks and
`cargo deny`. Workspace compilation passed with both `--no-default-features`
and `--all-features`; both services also compiled with `--all-features` for
`wasm32-unknown-unknown`, preserving their native-only runtime boundaries.
The shared online Python helper suite passed 169 tests. The current gameplay
web JavaScript suite passed 123 tests, including 27 direct-transport tests,
with `node --test --test-isolation=none apps/game-client/web/tests/*.test.cjs`.
An ignored PostgreSQL test in the default suite is not a PASS.

The continuity investigation found separate harness sequencing defects:
an old White-turn board could satisfy the post-kill Black predicate before a
fresh attach, and screenshot work could consume the other browser's bounded
offline recovery budget. The harness now requires a completed post-kill attach
with the same operation scope and new attachment, and restores transport only
after the independent durable oracle confirms the required partition, before
sampling unrelated pixels. All original board/pixel and durable assertions remain.
The full run at clean `b53b1e2` passed seven cases including both actual crash
partitions, then failed same-record rotation with a durable prefix of two and
an unavailable White board. A focused trace confirmed six network retries
exhausted during a 24.5-second test-created outage. The rotation control page
is now prepared before loss; actual rotation, the second durable command and
stale-attachment denial still precede restoration, which now precedes unrelated
pixel sampling. The helper regression checks those barriers; actual rerun is pending.

An actual committed-crash probe then confirmed a separate transport defect:
the session adapter's supported `PublicProblem` rejection has `version`,
`status`, `title` and `code`; the client only recognized the gateway's one-field
`request_rejected` problem. Exact media/no-store headers and the complete body
were observed. The client compatibility fix recognizes the two closed forms
without accepting malformed/duplicate/extra fields, or making rejected input
successful; fresh context/grant/attachment remains necessary before render/retry.

Room/ready/start, results/history/rematch, production/provider provisioning,
native/mobile runtime and broad phase exits remain outside PR01.
