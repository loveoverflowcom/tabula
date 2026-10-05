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

## Workspace recovery and evidence discipline

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

## Pending gates

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
