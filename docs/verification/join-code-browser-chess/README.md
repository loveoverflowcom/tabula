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

## Recovery branch integration (2026-10-05)

The owner requested review and sequential merge of gateway, browser and client
recovery into develop `d5b37d3`. Initial checkpoints `83f05bf`, `62a3c8b` and
`f330c11` were reviewed first. Remote updates discovered before publication were
reviewed in the same order: gateway `9b79c79`, browser `e88511a`, client `604cb0d`.
This integrates the recovered sources only; PR2 gameplay is still incomplete.
The initial gateway checkpoint was documentation; its updated head adds HTTP DTOs
and authority hooks, but still no native gateway implementation.
The browser workflow and manifest are preserved as inactive drafts under
[`tests/online-match`](../../../tests/online-match/README.md). Missing sources
are **NOT_IMPLEMENTED**, not an environmental failure or passing acceptance.

Review fixes:

- Reconciled ADR-0035/0037 with already-merged develop and removed the nonexistent
  gateway crate from the implemented dependency matrix.
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

Aggregate/target verification is recorded after the final checks below. These
source and unit checks do not satisfy the online delivery gates above.

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

## Pending online delivery gates

- Focused admission/session/codec/client/UI tests and strict lint
- Authoritative cargo xtask check
- Feature/native/WASM-release/staging/resource-budget gates
- Actual independent Chromium/PG acceptance and screenshot inspection
- Independent exact-source security/UI review
- Exact published head/tree, terminal CI, normal merge ancestry and post-merge CI

All pending stages are NOT_RUN until a nonempty executed receipt is recorded.
Production/listener/live migration/provider setup, robust network-drop/refresh/
server-crash/resync, timers/outage policy, private effects, lobby/social/voice/
ranking, load/backup and broad phase exits remain outside PR2.
