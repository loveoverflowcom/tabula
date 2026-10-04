# Chess → Tabula local integration evidence

Date: 2026-10-04. Fresh worktree and branch `feat/chess-tabula-integration`.
Fetched baseline: `develop @ 8891bb34382dac4605c3aa29cf406528a737ff40`.
Frozen implementation commit: `0346191ce1116e3941c77f60b1b04836e2902188`.
No push, merge, deployment, workflow-trigger change or branch-protection change.

## Delivered behavior and ownership

- Normal Tabula home/Library → game detail → local two-human setup → validate →
  explicit Start → separate `/play/local/` document → existing board/terminal/
  restart → Return to Tabula detail/setup. Common shell styling is unchanged
- The registry owns supported runtime evidence, immutable normalized config,
  precise clock bounds, trusted return and EN/VI host locale. Only
  `TABULA_PLAY_BASE=/play` (or its trailing slash) binds this deployed runtime;
  absent or invalid binding stays unavailable. Binding never launches unrelated
  catalog games through the Chess-only host. Chess AI/online are visibly gated
- The real existing reducer, presenter, art, fonts, local match and Macroquad
  renderer are reused. No game rules, presenter, art, canonical state, wire
  contract or rules/package version changed. [ADR-0030](../../adr/0030-local-discovery-gameplay-handoff.md)
  records only the narrow local gate extension, retaining ADR-011 and ADR-0029
- Setup revision scopes validation and handoff; edits retire readiness, Pending
  locks duplicate Start, disposed owners reject results, refused navigation
  exposes retry, and browser-restored Pending/Handoff retires to Editing
- Integrated entry requires complete canonical metadata and fails closed with
  catalog recovery. The host passes only existing bounded gameplay arguments
  into Rust. It never passes return/source/locale metadata as rules data
- The document has one admitted bootstrap/instance. Failure/Return/pagehide
  abort owned downloads, cancel startup and virtual-file callbacks, retire
  owned file buffers/export references and prevent late frame/readiness/input
  re-entry. BFCache restoration explicitly reloads a fresh unsaved match
- Dialogs and Shift+Tab clear gameplay focus/held input through the existing
  runtime callback. Local clocks keep elapsed time through blur/hidden/dialog
  states; leaving/reloading destroys the local match rather than saving it
- The existing terminal/restart UI stays in the runtime. Return does not create
  a persisted results route, replay, resume card or server match identity

## Executed checks

Toolchain: official Rust **1.96.1**, rustfmt/Clippy and wasm32 target from the
workspace toolchain. Cargo build cache was shared through `CARGO_TARGET_DIR`;
source and distributions remained in this separate worktree. Build tooling:
Trunk **0.21.14** installed locked from crates.io; its pinned Sass 1.69.5,
wasm-bindgen 0.2.129 and wasm-opt version_123 were fetched by Trunk. No product
dependency added. Final command outputs are under [logs](logs/). Retained logs normalize only
trailing whitespace and excess final blank lines; raw/retained hashes are in
[log provenance](log-provenance.json).
[Source/build fingerprints](source-build-manifest.json) and
[actual runtime probes](runtime-probes.json) bind artifacts and blocked targets.

| Check | Result and nonempty scope | Evidence kind |
|---|---|---|
| `cargo xtask check` | PASS: every ordered core gate; 980 passed, 0 failed, 21 existing ignored | example/property/integration tests plus static/dependency checks |
| Registry/setup focused tests (also in aggregate) | PASS: 32 registry, 23 web, 1 private-config compile-fail doc test; 50 setup restoration cycles | example-tested/type-enforced |
| `node --test apps/game-client/web/tests/standalone.test.cjs` | PASS: 30; canonical query, rapid/repeated starts, all async cancellation boundaries, stale readiness, focus/dialog/navigation errors, pagehide/BFCache and missing integrated metadata | mocked host unit tests |
| Actual registry export → `node tools/tests/check-local-handoff.cjs` | PASS: 18 actual normalize/resolve URLs, both locales, untimed and Fischer/Bronstein bounds; host roundtrip and exact Rust launch args | integration-tested configuration boundary |
| `python3 tools/test_serve_local_shell.py` | PASS: 2 tests; real loopback HTTP shell fallback, separate static game, missing JS/WASM and gated/nested routes return 404 | integration-tested HTTP routing |
| Workspace `--no-default-features` / `--all-features` | PASS: both entire workspace compilations | compiled |
| Gameplay `--no-default-features --features web --target wasm32-unknown-unknown` | PASS | compiled |
| `cargo build -p tabula-game-client` | PASS | native compiled |
| Gameplay `--target wasm32-unknown-unknown --profile wasm-release` | PASS: 1,902,364-byte WASM | WASM compiled |
| `NO_COLOR=true TABULA_PLAY_BASE=/play trunk build --release` from `apps/web` | PASS: shell WASM 863,787 bytes plus hashed CSS/JS | compiled/bundled |
| `cargo xtask stage-local-play` | PASS: complete static separate document in `apps/web/dist/play/local`; gameplay index promoted; source resources and canonical tokens byte-preserved | integration-tested staging |
| `cargo xtask stage-wasm-game` | PASS: independent standalone distribution still stages all 15 required resources plus tokens | integration-tested staging |
| Staging unit/process tests (also in aggregate) | PASS: 13 + 2; shell preservation, missing shell/resource failure, stale-output retirement, arbitrary output refusal | example/integration-tested |
| Maintained skill/helper checks | PASS: tree check plus 32 and 6 fixture tests | static/example-tested tooling |
| Independent frozen diff review and `git diff --check` | PASS: no actionable source blocker found | source-read |
| Native UI startup | BLOCKED: executable exits with `XOpenDisplay() failed!` before UI startup | no runtime visual evidence |
| dot cloud browser supported local HTTP probe | BLOCKED: `http://127.0.0.1:8000` returned `net::ERR_BLOCKED_BY_CLIENT` | no browser interaction or pixels |
| Real desktop/mobile/Safari/assistive-technology/zoom/BFCache/GPU checks | NOT_RUN: required display/browser path unavailable; no substitute design renders | residual acceptance scope |

The initial aggregate attempt correctly failed the game-ID policy for the new
test-only checker placed outside a tests directory. It was moved into
`tools/tests/` without a policy suppression; the complete ordered gate was then
rerun and passed. Preliminary Trunk attempts exposed a non-Send listener cleanup
capture (fixed with owner-managed local storage) and a read-only default tool
cache (moved to workspace `XDG_CACHE_HOME`). Final release bundling passed.
These recovered attempts are not the final gate result.

No remote CI was started for these new local commits. The previous standalone
CI result does not establish integration CI success. Before publication, review
the branch and residual target evidence; publication still requires instruction.

## Reproduce and run

Use the pinned repository Rust toolchain and installed `trunk`/`cargo-deny`.
On this workspace, source `../toolchain/env.sh` first and set Cargo's target
cache if reusing it. A writable `XDG_CACHE_HOME` is needed in read-only-home
environments. `NO_COLOR=true` avoids Trunk parsing this workspace's `NO_COLOR=1`
as an invalid boolean. On an ordinary developer machine those overrides are
unnecessary.

```bash
cargo xtask check
cargo check --workspace --no-default-features
cargo check --workspace --all-features
node --test apps/game-client/web/tests/standalone.test.cjs
python3 tools/test_serve_local_shell.py
set -o pipefail
cargo test -p tabula-registry export_local_handoff_boundary_urls -- --nocapture \
  | sed -n 's/^TABULA_LAUNCH_URL=//p' \
  | node tools/tests/check-local-handoff.cjs

cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
(cd apps/web && NO_COLOR=true TABULA_PLAY_BASE=/play trunk build --release)
cargo xtask stage-local-play
python3 tools/serve-local-shell.py --port 8000
```

Or use `just web-local-build` / `just web-local-serve 8000`. Open the advertised
loopback address in a permitted browser, browse Chess, configure local play,
validate and start. Ordinary unbound `trunk build` still reports the missing
runtime honestly. The devserver serves only implemented discovery paths with
SPA fallback; missing game resources return errors. It is not production hosting.

Standalone still uses `cargo xtask stage-wasm-game`, with `index.html` setup and
`play.html` gameplay. Its colocated diagnostic copy is
`/play/local/standalone.html` → `/play/local/play.html`; the integrated index
does not silently start a default match without complete handoff metadata.

## Remaining real-target acceptance

Exercise actual discovery/field keyboard navigation and focus, 320/390/760 dp,
short landscape, all four themes, 200% zoom, reduced motion; loading cancel,
missing/slow artifacts, rapid repeated Start, Back/Forward/close/reopen, modal
cancel, real BFCache restoration, stale async completion and live input; play
moves/promotion/draw/resign/end/restart and explicit Return; confirm hidden-clock
catch-up and repeat document cycles while measuring real heap/GPU resources.
Capture and inspect actual runtime screenshots. No screenshot is supplied here
because both attempted runtime paths were blocked, not because design images
were substituted. Full Board Reader, physical mobile, saved/online/rated/AI,
native catalog and production renderer migration remain separately gated.
