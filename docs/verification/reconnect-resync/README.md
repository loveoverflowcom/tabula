# Reconnect/resync evidence

This is the archived implementation-checkpoint ledger. Final review, acceptance
and merge receipts are recorded in [PR83](https://github.com/loveoverflowcom/tabula/pull/83)
and its linked source-matched Actions artifacts. Pending statuses below describe
the original checkpoints, not the final review outcome.

Scope: [ADR0042](../../adr/0042-isolated-match-reconnect-resync.md), the third
owner-requested sequential match PR. Starting source develop
`e75624ae870a74f62f0f734fbcf2f12043047dd4`; PR83 is initially a draft.

| Claim | Oracle | Current status |
|---|---|---|
| HTTP2 is strict and match wire 0.1 unchanged | exact DTO JSON/hostile partitions | focused 4 tests PASS at first checkpoint 1c425daf; final rerun pending |
| Original pending identity survives same-scope reattach | Rust client and JS lifecycle tests | focused tests PASS; final source rerun pending |
| Exact admission-bound durable restart | complete journal replay and recovered ledger/time tests | focused 2 tests PASS; realPG pending |
| Live/stale online owner exclusion | actual PostgreSQL backend/PID and guarded callback oracle | implemented; realPG execution pending |
| Cancellation cannot overwrite the journal permit | serialized request task plus real interrupted commands | compiled; actual fault execution pending |
| Old queued private body cannot outlive authority/owner | native first-frame guard and held-body oracle | implemented; final focused/real execution pending |
| Drops, refresh and actual server crash do not double moves or seats | independent browser processes +HTTPS+PG and durable transcript | implemented harness in progress; NOT_RUN |
| Required architecture/core/feature/native/WASM/resource gates | authoritative commands on final source | NOT_RUN |
| Independent source/security/UI reviews | final-source review | NOT_RUN |
| Exact-source pre-/post-merge CI and normal merge | remote SHA/tree/job and ancestry receipts | NOT_RUN |

Default service entrypoints remain closed. This is neither production observation
nor deployment, live provider provisioning, native mobile runtime, external-effect
outbox, backup/load/SLO or broad phase-exit evidence. Unavailable target execution
must be named explicitly; setup failures and empty selections never count as PASS.
