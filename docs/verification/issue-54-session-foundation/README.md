# Issue #54 PR1 — isolated durable session foundation

Source: `develop @ 729417ffb6de4b376d2736bdfbf78c46f455630e`, 2026-10-04.
Authorized boundary: [ADR-0035](../../adr/0035-isolated-durable-session-validation.md).

The first of three new implementation PRs provides internal session policy and
an opt-in real PostgreSQL authority adapter. Both service entrypoints remain
closed. Provider identity fixtures are synthetic; structural key validation is
not authentication. No browser/native login, production listener, deployment,
profile/social UI or phase exit is claimed.

## Claims and independent oracles

| Claim | Owner / failure | Evidence | Residual |
|---|---|---|---|
| Canonical opaque credentials, digest-only persistence and redaction | session credential boundary; public record IDs/alternate encodings/secrets in diagnostics | Focused constructor/digest/entropy and real-row checks; exact result recorded in PR | Real provider, cookie/native store and transport remain gated |
| Checked identity/session storage | exact issuer+subject and validated raw rows; email merge, corruption or invalid arithmetic | Real migrations/uniqueness/row conversion tests; exact result recorded in PR | No handle/registration/privilege policy |
| Expiry and activity classification | pure policy + locked storage; deadline equality, lock waits, regression, revival | Literal boundary oracles, durable terminal expiry, independent adapters and fixed-clock controls | Trustworthy deployment clock and silent socket timer unproven |
| Rotation / revocation / epoch ordering | account→session locking and CAS; duplicate winner or stale authority | Barrier-controlled PostgreSQL interleavings in both orders; durable marker is storage-private | Database commit fence is only partial S09; actual private output/WS fence NOT_IMPLEMENTED |
| Honest commit failures | transaction owner; rolled-back or uncertain result reported successful | Precommit and lost-ack controls fail closed; exact result recorded in PR | Fault hook models lost acknowledgement, not a real network fault campaign |
| Gates and dependency direction | new runtime owner + optional SQL feature | Aggregate, matrix, targets and unchanged service failure exits | No Phase 2/3/4/5 exit or whole #54 completion |

## Commands and execution boundary

Existing toolchain: Rust 1.96.1, two build jobs, incremental disabled and dev/test
debug information disabled. Local PostgreSQL, psql/container tools and nextest
are unavailable. The canonical local `cargo xtask check` uses `cargo test`.

- `cargo test -p tabula-session`
- `cargo xtask check`
- `cargo check --workspace --no-default-features`
- `cargo check --workspace --all-features`
- `cargo check -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web`
- `cargo check -p tabula-web --target wasm32-unknown-unknown`
- CI: `cargo sqlx migrate run --source crates/tabula-storage/session_migrations`
- CI: `cargo sqlx prepare --workspace -- --package tabula-storage --features session-postgres --all-targets`
- CI: `cargo test -p tabula-storage --features session-postgres session::tests:: -- --ignored --test-threads=2`

DB tests are deliberately ignored by ordinary offline workspace runs. CI selects
them explicitly against an ephemeral PostgreSQL 16 service, requires setup to
succeed, regenerates actual query descriptions and compares committed metadata.
Ignored/zero-selected tests are never reported as PASS. Publication records each
command, exact final head/tree and final CI links in the PR description.

S01/S02/S07/S08/S09 are addressed only within policy/storage domains. Real HTTP,
CSRF, upgrade/grants, private delivery, provider anti-enumeration, browser
BFCache/AT/IME/password manager, native secure stores and UI layout are
NOT_IMPLEMENTED/NOT_RUN. No fallback auth, plaintext credentials or fabricated
provider login is introduced.

## Initial local receipts before metadata bootstrap

- Focused core policy/credential tests: PASS, 21 passed, 0 failed, 0 ignored
- Core strict all-target Clippy and focused WASM compilation: PASS
- Workspace formatting, whitespace, maintained-skill check and its 32 + 6 validator tests: PASS
- Cargo-deny advisories/bans/licenses/sources: PASS after adding only the approved runtime session entropy wrapper; deterministic bans remain
- Resolved dependency metadata: PASS; newly added packages do not exceed the unchanged declared Rust 1.85 bound. This is metadata inspection, not an executed 1.85 build
- Default storage target: compiled, 0 unit/0 doc tests selected; no behavioral test claim
- Real PostgreSQL: 19 ignored cases implemented, NOT_RUN locally. Authentic offline query metadata and final aggregate/feature/head CI remain pending

Initial publication exists to generate actual PostgreSQL query descriptions in
CI; missing metadata may fail preliminary optional-feature gates. Such a run is
not reported as PASS. The final PR description records subsequent authentic
metadata import, repaired checks and terminal exact-head results.
