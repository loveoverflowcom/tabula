# Issue #74 F1 — trusted HTTPS Origin validation

Base: fresh `develop @ aee07128d111c0bd5c15eb2ccaacc40c880ba5c3`, tree
`07bf97c141f28120c6e1a2c36e66e48a101e86db`. This fix is independent of unmerged
PR #69/#70/#75. [Issue #74](https://github.com/loveoverflowcom/tabula/issues/74)
and ADR-0031/0034/0036 define scope; production remains closed.

## Decision and changed claim

The constructor accepts one canonical ASCII HTTPS tuple Origin, exactly equal
to the URL parser's Origin serialization. It rejects malformed input and any
configuration a browser would serialize differently, instead of silently
normalizing an allow-list. The native optional `url` dependency reuses locked
2.5.8; DTO-only default/WASM consumers do not acquire runtime URL parsing.
The locked native `isolated` graph reaches ICU 2.3 (declared Rust 1.88), so
`package.metadata.tabula-msrv.isolated` records 1.88. Default DTOs retain their
workspace 1.85 declaration; `postgres` still requires 1.94. Verification uses
pinned 1.96.1. Dependency/MSRV source inspection is not an executed 1.85/1.88
build claim; no lockfile versions or deterministic-kernel dependencies change.

- Reject invalid/empty/negative/overflow ports, `:443`, leading-zero ports,
  scheme/host case aliases, Unicode/percent-encoded hosts and IP aliases
- Reject missing/malformed authority, userinfo including empty `@`, slash/path
  including `/`, query/fragment including empty forms, controls and whitespace
- Accept canonical lowercase ASCII/IDNA, dotted IPv4, compressed lowercase IPv6,
  canonical nondefault u16 ports (including 0 and 65535), and domain trailing dot
- A domain with a trailing dot remains a distinct origin; do not DNS-equate or
  trim it. Canonical syntax is not DNS/TLS/connection reachability evidence
- Incoming Origin comparison, CSRF/channel checks, body limits, no-store,
  current-authority rechecks and private body/session guards are unchanged

Reference: [WHATWG URL port rules](https://url.spec.whatwg.org/#port-state),
[host equivalence](https://url.spec.whatwg.org/#host-equivalence),
[ASCII Origin serialization](https://html.spec.whatwg.org/multipage/browsers.html#ascii-serialisation-of-an-origin)
and [url API](https://docs.rs/url/2.5.8/url/struct.Url.html#method.origin).

## Regression sensitivity

Tests were added before the implementation changed. Running
`cargo test --offline -p tabula-session-http --features isolated --test isolated_http configured_origin_rejects_review_port_and_case_regressions -- --exact`
on the unchanged constructor failed with all five original review cases
accepted: `:abc`, `:999999`, empty port, `:443`, uppercase host. The expanded
literal constructor test separately failed on missing-host `https://:8443`.
Both are actual constructor failures, not a parser-only surrogate. Rejections
assert `SessionError::InvalidInput`, so an unrelated entropy failure cannot
satisfy the configuration oracle.

Two fixed-seed property tests run 128 cases each: a semantic generator of
canonical lowercase DNS labels/optional trailing dot/shortest decimal u16 ports
requires constructor acceptance and Origin round trip; a separate hostile
transform generator requires exact InvalidInput for overflow/text/empty ports,
default/zero-padded ports, uppercase host, userinfo, backslash and controls.
Literal IP/IDNA/opaque-scheme partitions complement those generated laws.
The generator's validity/hostile expectation is independent of the constructor;
round-trip alone is not a proof of correctness or universal URL conformance.

The new wire regression uses ten independently specified canonical HTTPS
Origins on actual loopback TCP. For each it obtains the current browser CSRF,
rejects missing/null/foreign, case/path aliases, distinct port/trailing-dot
peers and invalid CSRF without
mutation, then succeeds at refresh and current-verifier logout with no-store.
Existing wire/DB/consumer tests retain broader session, channel and output
ordering coverage. The synthetic HTTPS Origin does not make loopback HTTP TLS
or real browser-cookie evidence.

## Verification ledger

Environment: official Rust/Cargo 1.96.1, SQLX_OFFLINE=true, two build jobs,
unchanged default profiles. Target/feature checks additionally use
RUSTFLAGS=-D warnings. Ignored/filtered or zero selected tests are not passes.

| Claim / oracle | Command or selection | Result |
|---|---|---|
| Actual constructor regression sensitivity | exact F1 regression on unchanged constructor | expected FAIL: all five aliases/invalid ports accepted |
| Constructor and wire/CSRF/session boundary | `cargo test --offline -p tabula-session-http --features isolated --test isolated_http --test dto_contract` | PASS: 31 isolated cases + 5 DTO, zero failed/ignored/filtered; includes 2 × 128 fixed-seed property cases |
| Independent literal serialization oracle | Node v24.19.0 WHATWG `URL` on the same input partitions | PASS: 10 canonical + 58 invalid/noncanonical vectors; parsing only, no browser execution |
| Native isolated/postgres lint | `cargo clippy --offline -p tabula-session-http --features postgres --all-targets -- -D warnings` | PASS |
| Authoritative portable local core gate | `cargo xtask check` | PASS: fmt, workspace all-target/all-feature Clippy, 1,068 passed / 0 failed / 21 ignored, 28-crate dependency graph, 608-file game-ID scan, 31 manifests, generated tokens, raw colors and cargo-deny |
| Full workspace feature matrix | `cargo check --offline --workspace --no-default-features` and `--all-features`, RUSTFLAGS=-D warnings | PASS both |
| Actual isolated HTTP downstream consumer | `cargo test --offline -p tabula-web --features account-http-acceptance --all-targets` | PASS: 57 binary units + 31 acceptance-target cases, zero failed/ignored/filtered; 31 includes 19 reused core units, 11 TCP consumer claims and 1 parser, not 31 browser scenarios |
| Session policy | `cargo test --offline -p tabula-session` | PASS: 25 tests, zero failed/ignored/filtered |
| SQLx source/cache integrity | exact query content, SHA-256 filenames/internal hashes and PostgreSQL identity | PASS: all 16 current SQL queries; no metadata or SQL changes |
| Native game-client build | `cargo build --offline -p tabula-game-client`, RUSTFLAGS=-D warnings | PASS; compilation only, no native pixels |
| Gameplay WASM target and release artifact | `cargo check --offline -p tabula-game-client --target wasm32-unknown-unknown --no-default-features --features web`; matching `cargo build ... --profile wasm-release` | PASS with RUSTFLAGS=-D warnings; build only, no browser interaction |
| Web/DTO WASM lint | `cargo clippy --offline -p tabula-web -p tabula-session-http --target wasm32-unknown-unknown` with default and `--all-features`, both `-- -D warnings` | PASS both |
| Default native and all-feature WASM DTO dependency graphs | `cargo tree -p tabula-session-http` with normal edges in the two modes | PASS: no url/Axum/Tokio/SQLx/session runtime dependency |
| Repository skill validators and shell routing | maintained validator; 32 drift + 6 AI-doc tests; `python3 tools/test_serve_local_shell.py` | PASS: validator + 38 units + 3 actual loopback routing tests |
| Production entrypoints remain closed | `cargo run --offline -p tabula-server` and `-p tabula-auth` | both expected exit 1 with their gate; no listener/provider handling |
| Formatting/patch hygiene | `cargo fmt --all -- --check`; `git diff --check` | PASS |

Genuine PostgreSQL migration/races and authentic metadata
regeneration cannot run locally: psql/PostgreSQL/container support is absent.
The existing disposable PostgreSQL 16 CI job remains the authoritative check.
Cargo-deny passed with existing duplicate/wrapper/license warnings; no new
advisory, ban, license or source failure was reported.

## Remaining review scope

F1 is the resolved configuration finding. [The remaining queue](../../work-plan/backlog/issue-74-session-followups.md)
preserves G1–G5 and explains their prerequisites. No DB lease ordering/release,
preauth logout locking, provider, live expiry/WS output, persistent suppression,
native secure-store or UI pixels are changed or claimed here. Issue #74 stays
open; this PR references it rather than claiming its full acceptance.
