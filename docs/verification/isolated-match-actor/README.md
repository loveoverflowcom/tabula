# Offline match actor delivery ledger

Baseline: remote develop `0245dc72c6a356e8e7cbbb1f7601c1d09c634915`,
tree `55a47eec8b1872c1f20e6c48085eaa30d2cf3402`, after PR77 real-provider proof.
Authorization/scope: [ADR0039](../../adr/0039-isolated-match-actor-runtime.md).

| Claim / invariant | Oracle and domain | Evidence | Residual scope |
|---|---|---|---|
| One owner and deterministic order, I-7/I-14 | Actual approved four-move checkmate and nonzero-process-clock transcripts, both codecs; atomic contiguous journal | 4 real-game integration tests PASS | Offline authority/journal/output; no SQL/WS or full game correctness claim |
| Original operation/receipt identity | Exact retained result/bytes, changed correlation, payload conflict, malformed/rejected duplicates, scope/high-watermark/TTL/eviction partitions | Actor tests PASS | In-memory lifetime only; no exactly-once after process restart |
| Hidden event existence and gaps, I-5/I-6 | Genuine private state change and secret scrambling; same unauthorized full stream; public and authorized observability controls | Hidden fake integration PASS | Semantic frames/count/revisions, not constant-time/network traffic shaping or full portfolio secrecy |
| Authority at apply and output | Resolved seat/epoch/generation forgeries; revoke during append barrier; duplicate recheck | Offline guarded-port integration PASS | Already-applied input may commit after revoke; durable command/commit/private delivery not proved |
| Bounds, cancellation and failures | Per-sender/actually full mailbox; +64/+65/extreme sequence bounds; cancellation before/after apply; callbacks try-enqueue; uncertain append/effect/codec/panic; FIFO close | 17 fault/privacy integration tests PASS, including all 81 finite retry/conflict/rejection traces against an independent receipt model | No production supervision/recovery/watchdog/drain snapshot |
| Strict codec/domain boundary, I-13 | 22 committed vectors and direct-serde hostile version/seq/identity/collection/aggregate limits; both codecs | 12 protocol tests PASS | First isolated0.1 contract only; future xtask protocol commands remain stubs |
| Generic typed bridge, R2/R8 | Real approved game transitions/clock and broken mutation/RNG/serialization candidate counterexamples; private seed/init ownership barriers | 12 registry integration tests and 4 compile-fail doctests PASS | Rules-owned semantics; no untrusted plugin sandbox |

## Full execution gates

Toolchain: pinned Rust 1.96.1 on native x86-64 Linux; no MSRV acceptance inferred.

- `cargo xtask check` PASS: 1113 passed, 0 failed, 20 ignored across the workspace
  test/doctest invocation; fmt, strict workspace all-target/all-feature Clippy,
  dependency/game-ID/manifest/token/color gates and cargo-deny PASS in authoritative
  order. Default workspace tests do not activate actor tests; those run separately
- `cargo test -p tabula-match --features isolated` PASS: 21 actual actor cases
- `cargo clippy -p tabula-match --features isolated --all-targets -- -D warnings` PASS
- Workspace no-default/all-features checks PASS; all-feature protocol/registry/match
  WASM and web WASM checks PASS; native game and release WASM builds PASS
- `cargo deny --all-features --offline check` PASS: advisories, bans, licenses,
  sources; deterministic rand_core0.6 has no OS-entropy feature
- Staged current game host: 79 Node cases and 3 Python HTTP-tool cases PASS;
  emitted asset/WASM/graph budgets PASS (not runtime performance)
- Default server and default/opt-in auth entrypoints retain expected closed exit1;
  no listener was opened
- Skill/source/whitespace validators PASS; local nextest is unavailable. CI's
  existing nextest remains the separately executed required job
- Independent security source review fixed the two retained
  [counterexamples](review-counterexamples.md) and reports no unresolved blocker

Exact published head/tree, terminal CI and merge receipts belong in the PR.
All source/feature checks are rerun at the final published tree before merge.
Real PostgreSQL and provider cases are not run locally (no local adapters/tooling);
their existing non-empty disposable CI jobs must pass without weakening.

## Explicit exclusions

No SQL match adapter, lobby/queue, reconnect/resume, ownership lease, production
auth/commit/private-output fence, timer scheduler/recovery, real socket transport,
constant-time shaping, browser/native rendered acceptance, deployment or phase
exit is claimed. Journal test receipts establish atomic ordering in a process;
they are not PostgreSQL durability. Output authority establishes bounded enqueue;
it is not delivery or revocation of bytes already buffered. All effect requests
reach a port with committed-input keys; production adapters remain absent.
