# Issue #54 PR1 — isolated durable session foundation

Source: `develop @ 729417ffb6de4b376d2736bdfbf78c46f455630e`, rechecked
2026-10-04 and 2026-10-05. Authorized boundary:
[ADR-0036](../../adr/0036-isolated-durable-session-validation.md).

This is the first of three new implementation PRs, following the merged
specification/session-policy/service frames. It provides checked internal
session policy and an opt-in native PostgreSQL authority adapter. Both service
entrypoints remain closed. Provider fixtures are synthetic; identity-key
structure is not authentication. No production migration/listener/deployment,
account/profile/social UI, whole-issue completion or phase exit is claimed.

## Claims and executed evidence

| Claim | Owner / failure and independent oracle | Evidence | Residual |
|---|---|---|---|
| Canonical opaque credentials and redaction | session credential boundary; alternate encoding/public ID/entropy failure/digest disclosure | PASS: 21 focused core tests, literal SHA-256/encoding vectors, checked raw records and real digest-only row inspection | Real provider, browser cookie, native store and transport remain gated |
| Exact identity/session storage | issuer+subject linkage, UTF-8 byte bounds, reassignment/corruption | PASS: real PostgreSQL migrations repeated, high-variation maximum keys, explicit row corruption and fixture rollback | No handle/registration/privilege policy |
| Expiry and activity classification | deadline equality, stale sampled time, resource waits, regression or revival | PASS: literal boundaries, terminal expiry/rejection floors across independent adapters, both injected clock and actual post-lock database clock | Deployment-clock guarantees and silent socket expiry unproven |
| Rotation/revocation/epoch ordering | duplicate refresh winner, stale issuance/authority, effect ordered after revoke | PASS: 19 real PostgreSQL cases with independent backend PIDs and observed lock graphs; both commit orders, immediate old-verifier rejection, stable binding, device revoke and epoch fencing | Protected marker is storage-private; actual HTTP/WS/private-output S09 fence NOT_IMPLEMENTED |
| Honest commit failures | rollback or lost acknowledgement reported successful | PASS: precommit rollback and simulated postcommit lost-ack controls; no known-success snapshot on uncertainty | This is a test fault hook, not a network fault campaign or browser credential-release test |
| Dependency/gate honesty | shell entropy reaches kernel, activated production or fabricated receipt | PASS: 27-crate all-feature resolved dependency gate, all-feature deny, unchanged service sources and actual failure exits | No provider/output/phase proof |

Real PostgreSQL 16 receipt: [session-postgres run 37247778233](https://github.com/loveoverflowcom/tabula/actions/runs/37247778233),
job 111568997384. CLI 0.9.0 migration/prepare and **19 passed, 0 failed,
0 ignored, 0 filtered** cases succeeded. The overall bootstrap job failed only
because metadata was not yet committed; its final offline step was skipped.
It is not reported as a successful complete CI run.

That run is associated with head `ec4027aa95b1f438e8307e61121e6e944cad7546`.
Actions checked synthetic merge `8fe36757aaadb1b5ca562841a0abdd11e4f5c28c`;
its tree `daf3ac5a10e09c853ea59b5a276f11f5e2032dbe` equals the head tree against
unchanged develop. The 13 unchanged query descriptions were imported from its
authenticated artifact; ZIP digest, every filename/internal query hash and
current SQL source bytes were independently verified. See
[offline-cache provenance](../../../.sqlx/README.md).

## Final-content local receipts

Existing Rust 1.96.1 toolchain, two build jobs, incremental disabled and dev/test
debug information disabled. SQLX_OFFLINE=true; no DATABASE_URL or historical
metadata-directory override. Local PostgreSQL/client/container/nextest are
unavailable; real DB cases execute in the disposable CI service.

- `cargo test -p tabula-session`: PASS, 21 passed, 0 failed/ignored
- Core strict all-target Clippy and focused WASM compilation: PASS
- `cargo check -p tabula-storage --features session-postgres --all-targets`: PASS using the authentic 0.9 cache
- `cargo xtask check`: PASS, ordered fmt, all-target/all-feature Clippy, workspace tests and internal/deny gates; 1,025 passed, 0 failed, 21 ignored across 75 Rust summaries, 44 non-empty
- `cargo deny --all-features check`: PASS advisories/bans/licenses/sources; existing non-failing duplicate-version warnings
- `cargo check --workspace --no-default-features` and `--all-features`: PASS
- Game-client web-feature and web-shell `wasm32-unknown-unknown` checks: PASS
- `cargo build -p tabula-auth -p tabula-server`, then both binaries: compiled, each exits 1 with its original closed gate and no listener
- Workspace formatting/whitespace, 62 relative link targets, skill drift and its 32 + 6 validator tests: PASS
- Independent source/security/metadata review: no remaining blocking finding

The aggregate's default storage target selects no DB cases. Ignored/empty
targets never establish behavior; the separately executed 19-case CI receipt
supplies database evidence. The dedicated workflow enforces non-empty ignored
inventory, actual setup, regeneration/comparison of committed metadata and
subsequent offline compilation. Final publication/CI are recorded at their
exact head in the PR description, including tested-merge/tree equivalence.

## Bootstrap repairs and compatibility

Initial SQLx 0.8 violated I-1 through PostgreSQL RNG feature unification with
kernel rand_core 0.6. I-1 was not weakened: official SQLx 0.9 uses a separate
runtime RNG package, and the final resolved graph leaves the kernel without OS
entropy. The opt-in native feature explicitly requires Rust 1.94; the pinned
toolchain is 1.96 and workspace/default storage/game SDK retain declared 1.85.
No executed Rust 1.85 build is claimed.

The coarse deny wrappers add the runtime session owner and SQLx 0.9's direct
rand 0.10 SCRAM wrapper; deterministic reachability remains forbidden by the
unchanged per-crate gate. Default-graph deny success is not substituted for
all-feature verification. Preliminary container health quoting, hidden-artifact
upload, test connection lifetime and documentation lints were repaired; early
failed/incomplete runs are historical rather than final passes. Authentic 0.8
metadata was used temporarily for source diagnostics only and is not the
committed 0.9 cache.

## Remaining security and product gates

S01/S02/S07/S08/S09 are addressed only in policy/storage domains. A context
binding ID is not a CSRF token; an observation/binding is trusted internal
historical data, never raw HTTP credential proof or a later effect permit.
Real provider authentication/auth-time synchronization, HTTP/CSRF/no-store,
upgrade/grants, private delivery/connection fences, anti-enumeration, native
secure stores, browser BFCache/AT/IME/password-manager and UI layout remain
NOT_IMPLEMENTED/NOT_RUN. Production services, registration/friends and phase
exits remain gated. Account-free existing local play is unchanged.

## ADR identifier reconciliation

The preliminary draft used 0035, already occupied by independent unmerged
Werewolf PR #69. This exception is ADR-0036 after an explicit correction; all
owned filename/index/code/test references were updated without changing PR
#69/#70, session behavior or any of the 13 query descriptions. The canonical
aggregate, feature modes, target checks and all-feature deny were rerun after
this correction. Publication records new-head CI separately from the historical
13/13 passing 44a86f4 runs.
