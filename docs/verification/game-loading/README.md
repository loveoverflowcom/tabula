# Local game loading and cache evidence

Date: 2026-10-04. Baseline: `develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`.
Scope: a draft optimization PR for the existing local discovery-to-gameplay slice.
ADR-011/0029/0030 containment, game authority, common M3 discovery and remaining
phase gates are preserved. No renderer migration, service worker, offline service,
network match creation, merge or deployment is part of this work.

## What was already lazy

The emitted dashboard already referenced only its own shell bundle and CSS. It
never requested the separate game WASM, renderer, piece/role files or covers.
Cards use CSS placeholders. The normal shell graph has no Macroquad, presenter
or image decoder. This change does not claim to have removed an eager dashboard
asset waterfall that was absent at the baseline.

Registry discovery did construct bot factories just to inventory their levels.
The new game-owned descriptor keeps that policy data independent of optional bot
implementations, while retaining typed configuration validation. The measured
shell improvement is only 497 raw bytes. This is consistent with release LTO
eliminating unused policies; no compiler/disassembly evidence isolates that
cause. Source/dependency cleanup and emitted-byte savings are distinct claims.

## Reproducible emitted bytes

Toolchain: Rust/Cargo 1.96.1, Trunk 0.21.14, Node 24.19.0, Python 3.12.14.
Baseline and current builds used the same toolchain and release profiles. The
baseline game used its default multi-game features; the deployed current local
host uses `--no-default-features --features web`. Gzip figures are deterministic
Python gzip level 9 with `mtime=0`, not an observed server compression policy.

| Emitted artifact | Baseline raw / gzip-9 bytes | Current raw / gzip-9 bytes |
|---|---:|---:|
| Gameplay WASM | 1,902,364 / 904,631 | 1,131,678 / 419,276 |
| Shell WASM | 863,787 / 279,437 | 863,290 / 278,902 |
| Shell HTML + two CSS + JS + WASM, five unique static resources | 968,595 / 293,234 summed gzip | 968,098 / 292,700 summed gzip |

Gameplay WASM decreases 770,686 raw bytes (40.5%) and 485,355 gzip-9 bytes (53.7%).
This is an emitted-payload result, not a startup-time, CPU or heap claim.
[Baseline inventory](baseline-bundles.json), [current budgets](current-bundles.json)
and [source/build fingerprints](source-build-manifest.json) retain exact hashes.
The deployed normal dependency graph excludes Tiles and Leptos. Byte inspection
finds none of the three authored fonts, four Chess PNGs or two Tiles PNGs inside
that WASM; the baseline contains all nine complete byte sequences.

The CI-enforced gameplay budgets are 1,250,000 raw and 500,000 gzip-9 bytes.
The unchanged baseline fails the raw budget, establishing a nonvacuous regression
check. The optional local shell check caps its WASM at 900,000 raw bytes and
asserts zero eager gameplay/static-asset references. Its receipt is a static
emitted dependency inventory, not an executed browser waterfall.

## What explicit launch loads

The previous gameplay WASM embedded 280,240 bytes of Chess PNGs, all three fonts
(223,632 bytes), and Tiles atlases (17,530 bytes). Its startup decoded both piece
densities and both cover densities before checking `--skip-setup`; the DOM loader
also referenced a 168,293-byte cover and separately used the fonts.

The current minimal gameplay loader has no cover image and uses system fonts.
After explicit launch, Rust requests exactly these public runtime resources:

1. The selected Chess-only WASM
2. OpenSans-Regular, OpenSans-Semibold and NotoSerif-Bold
3. One manifest-selected critical piece atlas at the renderer's current density

At DPI1 the atlas is 21,488 encoded bytes and 248,832 decoded RGBA bytes; at DPI2
it is 46,139 / 995,328. The old four-texture startup was 2,012,160 decoded RGBA
bytes. These are decoded payload/accounting values, not measured GPU residency.
Cover resources remain staged for the native/standalone setup scene, and another
piece density is fetched only when a real renderer density-tier change needs it.
Ready textures skip refetch/decode/upload across restarts and returns to a prepared
density. Ordinary frames do not rebuild the resource-selection metadata.

A real staged HTTP smoke executed one HTML, two styles, five pinned scripts and
the five DPI1 runtime payload requests: 13 explicit requests, 1,523,072 body bytes.
Every script/style SRI, runtime SHA-256/size and immutable response header matched.
See the [HTTP receipt](http-wiring.json). This exercises actual staged paths and
HTTP responses; it neither executes CSS font selection nor simulates a browser
waterfall, WASM frame or cache hit. The declared fonts/pieces runtime payload sum
is 1,376,798 bytes including the current WASM, before the small host dependencies.

## Cache and lifecycle contract

- Staging produces full SHA-256 content URLs. HTML stays `no-store`; its scripts
  and styles use immutable names and SRI. Fixed-name diagnostic copies are not
  runtime fetch targets. Same bytes keep their URL; changed bytes get a new URL
- Runtime aliases are a strict bounded manifest. Unknown aliases, versions,
  fields, hashes, paths, sizes and conflicting URLs fail closed before fetch
- Payloads require streamed response bodies, with one manifest-sized preallocated
  buffer and checks on every chunk. Non-streaming delivery fails before calling
  `arrayBuffer`. Size/SHA-256 verification precedes WASM compile and font delivery;
  pack bytes additionally pass existing Rust BLAKE3 verification before decoding
- Identical in-document requests share their pending Promise. Four loads reserve
  at most 64 MiB of declared encoded payload. Settled promises/buffers are not kept
  in a second memory cache; Miniquad buffer tracking is retired after consumption
- Optional CacheStorage/Web Locks reuse only verified public resource bodies.
  Per-resource locks avoid duplicate concurrent document downloads; a shared
  budget lock serializes index/reconciliation/eviction/writes. Cache payloads plus
  a bounded 32 KiB index fit 150 MiB and 32 entries (31 payloads plus the index)
- This is file-LRU for the local host. Browser storage bookkeeping, WASM/JS heaps,
  compiled modules and GPU memory are not that payload budget. Already consumed
  live modules/textures remain usable when their disk copy is evicted. The
  renderer retains its existing 128 MiB texture-residency budget and live leases
- Every cache hit is size/SHA-256 reverified. A corrupt hit is evicted and gets one
  fresh verified fetch. Interrupted/corrupt network bytes are not admitted. A
  changed content version cannot reuse a stale alias payload
- Storage/lock absence, denial or quota falls back to verified uncached network.
  WebCrypto and body streaming are required. No persistent-storage permission or
  service worker is requested. Launch arguments, identities, private assignments,
  state, virtual launch/ready files and credentials are never cache entries
- Progress distinguishes downloading, verification and reverified cache use.
  Cancel, pagehide, timeout, error, blocked navigation and BFCache retirement reject
  late callbacks. Return/reload restores a fresh local match. Missing/SRI-rejected
  Miniquad globals still permit Error/Return/Retry; the plain integrated HTML also
  has a safe `/games` escape when bootstrap cannot execute

Mocked warm-cache tests execute five cold runtime fetches and zero additional
fetches for the same startup bytes across locale/theme changes, while constructing
fresh instances/matches and virtual configuration. These are cache-contract call
counts using mocked CacheStorage/Web Locks, not browser disk/transfer/timing results.
No offline availability or general CDN/native pack-cache implementation is claimed.

## Executed checks and limitations

Retained command output is under [logs](logs/); [log provenance](log-provenance.json)
records raw and retained hashes. Normalization only trims trailing whitespace and
excess final blank lines. Final aggregate checks passed after the earlier recovered
lint/empty-WASM staging failures. Independent review found and drove quote-style
staging, startup-alias/mock-demand and missing-Miniquad recovery regressions before
publication.

| Check | Result and scope | Evidence kind |
|---|---|---|
| `cargo xtask check` | PASS: every ordered gate, 994 passed / 0 failed / 21 existing ignored | static/example/property/integration |
| Registry tests | PASS: 33 unit tests plus one compile-fail doc test; config/host behavior retained | example/type-enforced |
| Game bot descriptors, bots enabled and rules-only | PASS: 3 Chess + 2 Tiles tests in each configuration, all four bot levels | example-tested |
| Client tests | PASS: 43 library, 9 entry, 15 integration; six new resource-selection/lifecycle tests | example/integration-tested |
| `node --test apps/game-client/web/tests/*.test.cjs` | PASS: 58; loader/cache faults, bounds, races, alias wiring, no eager cover/2x, fresh warm matches and missing bootstrap globals | mocked host/cache example tests |
| `python3 tools/test_serve_local_shell.py` | PASS: 3; fallback, missing/corrupt resources, immutable/ETag and fresh entry | real HTTP integration |
| Staged HTTP/SRI/manifest smoke | PASS: 13 explicit requests, all digests/sizes and source aliases | real HTTP integration |
| Staging unit + process checks | PASS: 15 unit + 2 process tests; atomic output, source validation, quote/whitespace/SRI and safe static Return | example/integration-tested |
| Emitted budgets and baseline negative control | PASS current; baseline intentionally FAILS size cap | compiled-artifact inspection |
| Workspace no-default/all-features | PASS both workspace compilations | compiled |
| Chess-only WASM `web` check + `wasm-release` | PASS | compiled |
| Native `--release` | PASS, default multi-game features retained | compiled |
| Bound Leptos `trunk build --release` | PASS | compiled/bundled |
| Both local and standalone staging commands | PASS | integration-tested staging |
| Maintained skill/helper checks | PASS: structural gate, 32 + 6 fixtures | static/example-tested |
| Native startup | BLOCKED: current release exits with `XOpenDisplay() failed!` | no rendered runtime |
| Real cloud-browser local preview | BLOCKED: supported origin previously returned `ERR_BLOCKED_BY_CLIENT`; restriction was not bypassed | no browser cache/timing/pixels |
| Real cold/warm browser latency, cache quota/eviction, BFCache/disposal, Safari/WebKit, physical mobile, zoom/accessibility/visual QA | NOT_RUN because the required runtime path is unavailable | residual target acceptance |

Heap/GPU reclamation, real font traffic, physical disk overhead and browser startup
latency remain unmeasured. Mock counts, successful builds and real HTTP receipts do
not establish those claims. Exact-head remote CI is reported in the draft PR after
publication; this local ledger does not substitute for it.

## Reproduce

Use the pinned toolchain. A read-only-home container needs a writable Cargo/tool
cache and `XDG_CACHE_HOME`; `NO_COLOR=true` is required for this Trunk environment.

```bash
cargo xtask check
cargo check --workspace --no-default-features
cargo check --workspace --all-features
node --test apps/game-client/web/tests/*.test.cjs
python3 tools/test_serve_local_shell.py
cargo build -p tabula-game-client --release
cargo build -p tabula-game-client --no-default-features --features web \
  --target wasm32-unknown-unknown --profile wasm-release
(cd apps/web && NO_COLOR=true TABULA_PLAY_BASE=/play trunk build --release)
cargo xtask stage-local-play
cargo xtask stage-wasm-game
cargo tree -p tabula-game-client --no-default-features --features web \
  --target wasm32-unknown-unknown --edges normal > target/local-game-graph.txt
python3 tools/tests/check-loading-budgets.py \
  --game-wasm target/wasm32-unknown-unknown/wasm-release/tabula-game-client.wasm \
  --game-tree target/local-game-graph.txt --shell-dist apps/web/dist
python3 tools/tests/check-staged-loading.py --shell-dist apps/web/dist
python3 tools/serve-local-shell.py --port 8000
```

Use the actual Cargo target directory if `CARGO_TARGET_DIR` is overridden. A
permitted real browser must still run the residual cold/warm waterfall, slow/missing
resource, cancellation, rapid relaunch, Back/Forward/BFCache, all themes/locales,
DPI/zoom and disposal checks before making target-performance or visual claims.
