# Durable match PostgreSQL delivery ledger

Baseline: remote develop `fd0f1e4` after PR78. Authorization and laws:
[ADR0040](../../adr/0040-isolated-durable-match-postgres.md).
This is the PR1 ledger for the [three-PR sequence](../../work-plan/README.md#authorized-durable-to-online-match-series).

Implemented owners are the [pure journal contract](../../../crates/tabula-match-journal/src/lib.rs),
[actor](../../../crates/tabula-match/src/runtime.rs), [replay verifier](../../../crates/tabula-match/src/runtime_recovery.rs),
[registry bridge](../../../crates/tabula-registry/src/runtime.rs) and
[PostgreSQL adapter](../../../crates/tabula-storage/src/match_postgres/mod.rs).
The [actual DB actor harness](../../../tests/match-postgres/tests/recovery.rs) has
16 selected tests and an explicitly invoked ignored subprocess fixture; the
[adapter suite](../../../crates/tabula-storage/src/match_postgres/tests.rs) has
12 explicit real-PostgreSQL cases. Their local setup is unavailable, so real DB
execution remains NOT_RUN until the exact-head CI receipt is recorded. There is
no in-memory durability substitute. All source/compilation evidence below is
separate from actual database integration.

| Claim / invariant | Owner and plausible failure | Oracle and required domain | Recorded evidence / status | Residual scope |
|---|---|---|---|---|
| Atomic commit, I-7/I-8 | Journal writes snapshot/receipt independently of canonical input/head | Fresh real PostgreSQL reader after precommit fault: all prior or all next head/input/events/hash/snapshot/ledger; creation and accepted/receipt-only writes | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | No live DB migration or production durability configuration |
| Known-success-before-output/effects | Actor emits Ack/update/effect before commit or continues after failure | Write barrier and known rollback/committed-but-receipt-lost fault; capture zero speculative output/effects; owner terminates | 21 faithful/real-game offline actor examples PASS; actual DB 16 actor cases NOT_RUN locally | Actual socket/effect delivery fence remains gated |
| Durable original receipt | Committed input reapplied after dropped Ack, or rejection forgotten | Fresh actor and fresh process reopen; same operation/result with changed correlation; changed payload conflict; malformed/game-rejected operation-only rows | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | Client reconnect/transport is PR3 |
| Durable monotonic high-watermark | Receipt TTL/count eviction or restart makes old sequence new | Expired/evicted receipt returns StaleSeq before/after reopen; scope capacity preserves all old watermarks; +64/+65 and extreme bounds | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | Match-retention/compaction policy remains separate |
| Scope-admission privacy, I-5/I-6 | Private command consumes previously unreserved shared capacity | Authorized attach reserves before success; private/public observability controls; capacity probe cannot reveal private-only command existence | inherited offline actor private/public capacity counterexample PASS; durable reservation source-read and ordinary DB attach coverage implemented; no dedicated DB hidden-information counterexample | Constant-time/traffic shaping and full portfolio secrecy not proved |
| Durable single-writer, I-14 | Paused former owner persists over newer owner at same version | Two real DB adapter handles; new generation claims ownership; all old-owner accepted and receipt-only writes rejected; expected-version conflict | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | No distributed placement/lease/output coordination |
| Exact deterministic reopen, I-2/I-8/I-16 | Latest rules, reconstructed times or trusted corrupt snapshot produce wrong state | Actual approved game transcript; compare creation and each input events/hash/snapshot; exact game/package/rules/config/roster/seed binding | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | No replay CompatibleVersion/migration fallback or timer scheduling |
| Corruption fail-closed | Extra/gapped/partial input, skewed head/identity, wrong hash/event/snapshot or malformed ledger admitted | Deliberately altered real DB fixtures plus trusted-load validation; no live actor/output on each mismatch | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | Arbitrary malicious DB administrator authenticity is not claimed |
| Process-boundary recovery | In-memory actor recreation mislabeled as crash safety | Fixture executable killed/terminated, separate process opens same dedicated DB and verifies committed state/receipt; both rollback and landed-commit partitions | source-read; actual PostgreSQL cases implemented, NOT_RUN locally | Real online server kill/resync acceptance is PR3 |
| SQL-free ports and optional infrastructure, I-1/I-9/I-15 | Storage imports registry/game or default/WASM pulls SQL/Tokio | deps/entropy gates; selected default/no-default/all-feature native/WASM builds; unchanged service exit behavior | check-deps29 crates and pure contract/default actor WASM compilation PASS; full matrix pending | Compilation does not prove runtime behavior or MSRV |

## Recorded local gates and pending merge evidence

- `cargo test -p tabula-match --features isolated`: PASS, 17 fault/privacy tests
  and 4 approved real-game examples. This is offline port evidence, not DB evidence
- `cargo test -p tabula-registry --features test-support --offline`: PASS, 53 unit/
  integration tests and 5 compile-fail docs; 3 pre-existing scaffold docs remain ignored
- `cargo clippy -p tabula-match --features isolated --all-targets -- -D warnings`
  and registry strict test-support lint: PASS
- `cargo test -p tabula-storage --features match-postgres --lib`: PASS, 11 pure
  validators; 12 real PostgreSQL tests are ignored in this local command
- Storage strict all-target `match-postgres-test-support` lint and format: PASS
- Standalone acceptance `--no-run`, exact list of 16 actor tests plus the ignored
  child fixture, and strict Clippy: PASS compilation/selection only
- A selected actual DB test without `TABULA_MATCH_DATABASE_URL` fails setup,
  as intended; it never silently skips or returns a test PASS
- `cargo xtask check-deps`: PASS for 29 workspace crates. The all-feature storage
  normal graph contains journal/core/game-api/protocol/session and infrastructure,
  with no actor/registry/game edges. The dedicated contract is required to prevent
  Cargo feature unification from violating that boundary
- `cargo check -p tabula-match-journal -p tabula-match --target wasm32-unknown-unknown --no-default-features`:
  PASS compilation
- Authoritative `cargo xtask check`: first source errors were corrected; a later
  attempt was BLOCKED by disk exhaustion at workspace test linking. After
  authorized regenerable-cache cleanup, the jobs2/debug0/incremental0 canonical
  rerun PASS: 1,122 tests passed, zero failed, 20 pre-existing ignored; all gates
  completed in their authoritative order
- Real PostgreSQL 16 actor/adapter tests: NOT_RUN locally; required exact-head
  [workflow](../../../.github/workflows/match-postgres.yml) execution pending
- Existing genuine session SQLx query metadata remains unchanged. New match SQL
  uses explicitly documented runtime typed queries, not invented macro metadata;
  schema/type/query semantics require real PostgreSQL acceptance
- Independent source/security review found and fixed ticking-clock receipt time,
  foreign-target rejected receipts, unrelated expired scopes, zero rules hash,
  feature-unification leakage and paused-test observer deadlocks. Final immutable
  tree review and terminal CI remain pending
- Published head/tree, normal merge ancestry and post-merge main/session/provider/
  match CI receipts: pending. No workflow existence or focused gate is represented
  as exact-head terminal integration acceptance

Setup failure and zero selected/ignored cases cannot yield PASS. Record unavailable
executables as BLOCKED with the exact prerequisite; do not claim missing targets
exist. Keep focused tests, real DB integration, process execution, compilation and
CI/enforcement as separate evidence kinds. A successful local gate does not close
the real PostgreSQL/CI or target-browser obligations.

## Documentation validation

Source/link validation PASS: all 162 local file links in the 18 changed/new Markdown
files resolve to existing targets; `git diff --check` PASS at this documentation
edit. This establishes neither runtime execution nor a final published-tree gate.

## PR2 / PR3 handoff contract

Carry the final merged commit/tree, exact opt-in feature/constructor/port and
disposable migration/harness commands, real DB/process receipts, reviewed laws
and unresolved defects. Preserve durable operation identity across transport
retries and distinguish connection SessionId from the durable session/seat scope.
Fresh owner claims exclude stale DB mutations; they do not establish current
session authority or revoke previously queued socket bytes/effect requests.

No canonical versions/indices/hash/seed/logical time/state/events/ledger may be
added to wire 0.1 to implement resume. A new attachment begins its own projected
stream; any new resume cursor/wire contract needs privacy review and I-13 evidence.
Recovery verifies exact recorded approved identity and history before returning
live state. A snapshot is an accelerator after consistency proof. Uncertain write
means stop/reopen, and expired receipt means StaleSeq, not fresh apply.

## Explicit exclusions

No production listener or activation, live migration/provider provisioning,
application-level seed encryption acceptance, live auth-session/commit/socket
fence, join code/two-browser play, reconnect/resync, server supervisor/timer/outage
clock policy, external-effect outbox/reconciliation, delayed spectator, lobby/
queue/voice/friends, constant-time/traffic shaping, backup/PITR/load/SLO or phase
exit is claimed. ADR0031/0039's authority/output/effect limitations stay explicit.
