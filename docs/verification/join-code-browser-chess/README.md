# Join-code browser Chess delivery ledger

Baseline: develop `60d0f1ad802c47848e4847def1705a449b51a7b7`, tree
`932887aeba56bc41e0090cca1a5b841fd78241cd`, after normal PR79 merge.
PR2 of the [three-PR series](../../work-plan/README.md#authorized-durable-to-online-match-series), under
[ADR0041](../../adr/0041-isolated-direct-match-browser-play.md).

| Claim / invariant | Owner / failure | Oracle / domain | Evidence / status | Residual scope |
|---|---|---|---|---|
| Authenticated server seats | Code/public ID or client seat impersonates a player | Distinct browser users, opposite seats, third/cross-match client denial and duplicate admission | In progress; NOT_RUN | No lobby/spectators/reassignment |
| Current commit authority | Observation stale during apply/awaited COMMIT | Actual PostgreSQL ordered revoke/expiry/epoch/membership cases; no unauthorized commit | In progress; NOT_RUN | Production/distributed authority |
| Actual queued-output guard, I-5/I-6 | Revoked viewer drains old private queue | Current durable guard at real HTTP body handoff, revoke-before-publish | In progress; NOT_RUN | Already released TCP bytes; full S09/socket |
| Original durable operation | Retry allocates another seat or applies twice | Exact duplicate/conflict cases and durable transcript count | In progress; NOT_RUN | PR3 interruption/refresh/crash |
| Projection-only renderer, I-10/I-12 | Fake/local canonical online board | Actual WASM presenter/canvas and network commands/projections; exact durable terminal oracle | In progress; NOT_RUN | Native/mobile online |
| Independent complete game | Shared credentials or HTTP-only proof | Separate Chromium processes/HOME/profile; verified HTTPS, real shell navigation, legal complete game and same terminal result | In progress; NOT_RUN | Existing chess rule-domain oracles remain distinct |
| Secret-free versioned boundary, I-13 | Grants leak through URL/cache/DOM/log or canonical counters | All DTO vectors/hostile serde partitions, field/privacy scan and artifact inspection | In progress; NOT_RUN | Timing/traffic shaping and hidden-game portfolio |
| Bounded failures | Probing/queues/payloads unbounded, failed transport creates local authority | Exact expiry/attempt/size limits, full queues, disconnected input gate | In progress; NOT_RUN | Load/SLO and robust recovery |
| Architecture, I-1/I-9/I-15 | Optional HTTP/auth/storage features pull forbidden game/renderer deps | Dependency/entropy gates, feature/native/WASM builds, service default startup closed | In progress; NOT_RUN | Deployment/configuration |

## Historical partial recovery integration (2026-10-05)

The owner requested review and sequential merge of gateway, browser and client
recovery into develop `d5b37d3`. Initial checkpoints `83f05bf`, `62a3c8b` and
`f330c11` were reviewed first. Remote updates discovered before publication were
reviewed in the same order: gateway `9b79c79`, browser `e88511a`, client `604cb0d`,
then browser `fa9bb60` and client `1973db1`. These are the frozen reviewed heads;
later remote changes are outside this integration.
This integrates the recovered sources only; PR2 gameplay is still incomplete.
The initial gateway checkpoint was documentation; its updated head adds HTTP DTOs
and authority hooks, but still no native gateway implementation.
The browser workflow and manifest are preserved as inactive drafts under
[`tests/online-match`](../../../tests/online-match/README.md). Missing sources
are **NOT_IMPLEMENTED**, not an environmental failure or passing acceptance.

Review fixes:

- Reconciled ADR-0035/0037 with already-merged develop and removed the nonexistent
  gateway implementation claim from the dependency matrix; the later recovered
  crate is recorded only as a pure HTTP DTO contract.
- Kept the incomplete workflow out of `.github/workflows`; its runner rejects
  missing sources before creating credentials, processes or DB connections.
- Updated the lockfile for the removed net-client → registry dependency.
- Closed the command gate after the last representable sequence is acknowledged.
- Normalized direct setup as network mode through the same typed validation as
  local setup, parsing once; candidate configuration does not enable deployment.
- Kept every actual adapter's direct document unavailable until the missing
  online host exists. A test-only deployed adapter exercises URL validation.

| Claim / invariant | Owner / failure | Oracle / domain | Check / status | Residual scope |
|---|---|---|---|---|
| Ordered opaque commands; I-10/I-12 | Sequencer falsely ready at exhaustion, receipt releases wrong command | Maximum sequence, oversized payload, projection while pending, contiguous frames/revisions and matched receipts | 7 unit tests PASS; exhaustion regression failed before fix | No transport, reconnect or authorization |
| Typed direct configuration; I-9 | Setup reports local mode or skips module/seat checks | Untimed typed bytes, unsupported clocks/game/keys/seat counts and network summary | Registry tests PASS; mode regression failed before fix | Server must validate actual authenticated roster |
| Honest handoff availability | Recovered helper advertises absent online document | Real adapter refuses direct URL; test-only deployed adapter validates public routing hints | Registry tests PASS | No browser navigation or pixels exercised |
| Incomplete acceptance fails before setup | Restored workflow invokes absent files/features | Source inventory, `bash -n`, explicit opt-in with missing manifest | PASS for shell syntax and preflight rejection only | Actual browser/PG acceptance NOT_IMPLEMENTED |

Executed focused command: `cargo test -p tabula-net-client -p tabula-registry --locked`:
66 passed (7 client, 37 registry unit, 17 registry integration, 5 compile-fail
rustdoc); 5 pre-existing illustrative rustdocs ignored, not counted as passes.
Toolchain: Rust/Cargo 1.96.1, Linux x86_64. Shared build cache only:
`CARGO_TARGET_DIR=/home/manhpd/Projects/tabula/target`.

Updated-head review also removes broken references to the absent gateway module
and `online-match-postgres` feature, retaining a pure DTO-only HTTP crate and
closed production service. New actor tests cover failure of fresh authority before
initial projection, receipts and updates, plus retirement after a post-apply guard
fails. JSON-extension tests cover exact Origin/CSRF/channel/body limits and prove
that an observation cannot authorize publication after revocation. Python TLS
helper tests use memory doubles, not real TLS/browser execution.

The final client recovery adds an opt-in generic projection presenter and WASM
loop. Its tests make every canonical rules operation panic if called, check
malformed-batch atomicity, and verify pending/disconnected input gating. The
existing local Chess/Tiles/Werewolf wiring is preserved. The online loop compiles,
but its special loader transport names are not implemented by the deployed
loader; direct registry launch stays unavailable. No browser gameplay is inferred
from the native presenter tests or WASM compilation.

### Executed local verification

Commands ran in the isolated recovery worktree on the integrated source, with
Rust/Cargo 1.96.1 and the shared target directory noted above. Counts overlap;
they must not be added into a unique-test total. Empty feature-gated targets
and ignored tests are not passes.

| Command | Result | Scope |
|---|---|---|
| `cargo xtask check` | PASS, 1,222 tests; 18 ignored | All portable gates, including workspace all-feature lint, dependencies, manifests, tokens, raw colors and cargo-deny |
| `cargo test -p tabula-match --features isolated --locked` | PASS, 25 | In-memory actor authority/fault cases, not PostgreSQL fencing |
| `cargo test -p tabula-session-http --features isolated --test isolated_http --locked` | PASS, 33 | HTTP/session boundary including new extension methods |
| `cargo test -p tabula-game-chess --features presentation --locked` | PASS, 165; 1 ignored | Projection codec roundtrips and existing rules/conformance/replay/presentation tests |
| `cargo test -p tabula-game-client --features online --no-fail-fast` | PASS, 72 | Native projection-only presenter and existing local client tests |
| `node --test apps/game-client/web/tests/standalone.test.cjs` | PASS, 57 | Loader unit tests, not browser execution |
| `python3 -m unittest discover -s tests/online-match -p 'test_*.py' -v` | PASS, 46 | Offline TLS/browser helper tests |
| `cargo check --workspace --no-default-features --locked` | PASS | Native compilation |
| `cargo check --workspace --all-features --locked` | PASS | Native compilation |
| `cargo check -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web,online` | PASS | Opt-in online WASM compilation only |
| `cargo clippy -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web,online -- -D warnings` | PASS | WASM-only loop lint |
| `cargo check -p tabula-web -p tabula-protocol -p tabula-registry -p tabula-match -p tabula-match-http -p tabula-net-client --target wasm32-unknown-unknown --all-features --locked` | PASS | Shared/browser target compilation |

Local logs are `/tmp/tabula-recovery-integrated-check.log`,
`/tmp/tabula-recovery-{actor,session,chess,online-client,loader,browser-helpers}.log`
and `/tmp/tabula-recovery-{final-features,online-wasm,online-wasm-clippy,final-wasm}.log`.
Cargo-deny reports existing allowed dependency warnings; no gate failed.
These source and unit checks do not satisfy the online delivery gates above.

## Historical cloud recovery context


The cloud filesystem was replaced between 12:39:22 and 12:39:57 UTC on
2026-10-05, before publication. Uncommitted code is reconstructed from current
task context, never restricted session history. Fresh remote clone reverified
the exact baseline/tree. All restored implementation needs fresh checks;
pre-replacement local results do not count as final-source passes.

Local PostgreSQL server/client/initdb/container tools are absent. Prior cloud
CUA loopback denial is not bypassed with another port/tunnel/shell browser.
Dedicated authorized CI owns actual browser/database execution. Configuration,
compilation, screenshot capture and pixel inspection are distinct evidence.
Setup failure/zero selected/ignored tests cannot be PASS. Genuine Kanidm CI is
separate from labelled disposable identity issuance. No credential, private
grant, CA key or canonical state is included in artifacts.

## Current PR80 composition and fresh verification

The conflict-resolution baseline is develop
`9642e4a60a8041bde3652d96dae0d1544bfaec3a`. Current PR80 restores the native
match gateway, SQL admission/current authority, opt-in shell/loader and active
standalone fixture/workflow. Historical inactive drafts remain archival. The
root lockfile gains internal composition edges only; external package versions
are unchanged. Current-source test counts overlap and are not additive.

| Fresh check | Status / scope |
|---|---|
| `cargo xtask check-deps` | PASS: 30 workspace crates, complete native composition |
| Actor isolated suite | PASS: 26 tests, including external authority tests and post-commit/lost-after-apply regressions |
| Match HTTP focused suite | PASS: 15 tests, native routes/ports and strict DTOs |
| Session HTTP focused suite | PASS: 54 tests, including preserved external extension partitions |
| Registry setup/eligibility suite | PASS: 38 unit cases; ordinary deployment stays closed, explicit online package binding only |
| Strict native gateway/session/storage and registry lint; online shell WASM lint | PASS for the stated selected targets |
| Actual composed PostgreSQL tests | NOT_RUN locally: PostgreSQL binaries/container runtime absent; active CI requires nonempty selection |
| Held first native body-frame oracle | Implemented; NOT_RUN locally. Actual queued MatchUpdate witness, expired lease plus committed logout before release, protected-body bytes must be zero |
| Actual independent Chromium full game and PNG inspection | NOT_RUN locally. Dedicated CI is the permitted target; cloud CUA loopback denial is not bypassed |

The first-body oracle does not separately prove revocation without lease expiry,
recall of previously handed-off bytes, browser/TLS disconnect behavior, or
cross-process journal-owner fencing of queued output. The actor startup path
refuses takeover of an already started room; distributed owner output/effect
fencing belongs to PR3. Privacy concealment and the global lifetime room-budget
review fixes require their own focused checks and final exact-source re-review.

## First executed composed-database CI receipt

PR80 checkpoint `ceaf059b4790222effc3b5852154bfae554d2738`, tree
`59132909b4ad2744fff4741148499fcbf380a94b`, triggered all five current workflows.
The [online acceptance job](https://github.com/loveoverflowcom/tabula/actions/runs/37351418509/job/111902910697)
compiled/linted the standalone fixture and actually selected/executed all 14
composed PostgreSQL cases. Result: **FAIL**, 10 passed and 4 failed. Failures
were strict-migration/advisory-lock availability, current-session ordering setup,
and the exact room-lock wait oracle. The new completed/expired lifetime-budget
case passed. These are real execution failures under investigation, not missing
PostgreSQL setup and not final-source acceptance.

The workflow correctly skipped shell/game build and actual browser play after
that failure. Its uploaded selection-list-only artifact contains no runtime
screenshots or complete-game evidence. No Chromium game/pixel PASS is inferred.
The next exact published source must rerun the entire job after the corrected
fixture/resource/lifecycle composition. All final merge gates remain open.

## Corrected composition/build receipts

PR80 checkpoint `c3677bbf38c6c837465a397d175c2422921d2404`, tree
`3d5f746934f39c38107941782aae09e27a03d00a`, corrects strict migration
connection cleanup and the real PostgreSQL backend/lease wait oracles. Its
[real composed-authority gate](https://github.com/loveoverflowcom/tabula/actions/runs/37355097909/job/111915394819)
completed successfully (step 9); final raw case-count and full-job/browser
receipts remain to be collected. The current actual provider, durable session
and durable match workflows also completed successfully on that head. None of
these receipts establishes the later browser step or final exact-tree CI.

Fresh local `cargo xtask check` passed all portable gates with 1,223 executed
cases and 18 ignored; ignored/empty selections are not passes. The complete
loader/transport Node suite passed 105 cases and Python fixture helpers passed
58. Actual release shell/game builds, unchanged selected-package size/dependency
caps and the staged HTTP/SRI/cache smoke passed: shell 896,361 raw bytes under
900,000; game 1,024,823 under 1,250,000. Shell static inventory has five resources
and zero eager gameplay references. Staged smoke has 15 explicit resource
requests; this is not a browser waterfall or cache-performance claim.

The small build fix uses the existing size/fat-LTO WASM profile, strips private
function-name/debug sections (not required exports), and passes both Trunk
`--release` and `--cargo-profile wasm-release`. Trunk's pinned wasm optimizer
retains validation with the standard Rust copy/fill bulk-memory feature enabled.
No cap, wire schema, rules identity, asset ownership or default deployment gate
is relaxed. Recorded capture build commands match that exact pipeline. The
final source must rerun all gates and actual Chrome acceptance after publication.

## Pending online delivery gates

- Final capacity/lifecycle review fixes and their focused regressions
- Portable aggregate and feature/native/WASM/resource gates on the final composition
- Actual composed PostgreSQL scenarios, native body-frame oracle and independent Chromium complete game
- Screenshot capture, secret scan and pixel inspection, then the owner-requested GitHub review issue
- Independent exact-source security/UI review
- Exact published head/tree, terminal CI, normal merge ancestry and post-merge CI

These pending stages remain NOT_RUN until their explicit receipts are recorded.
Production/listener/live migration/provider setup, robust network-drop/refresh/
server-crash/resync, timers/outage policy, private effects, lobby/social/voice/
ranking, load/backup and broad phase exits remain outside PR2. Mobile native
GameHost direction (issue #81) is separate from this web-WASM delivery.
