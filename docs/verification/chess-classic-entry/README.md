# Classic Chess pieces and direct play entry

Fresh implementation base: `develop @ face8a3e058e42fc846efa32082a9b76efaef3c2`,
fetched on 2026-10-07. This is a new draft-review change. It does not stack
[draft PR112](https://github.com/loveoverflowcom/tabula/pull/112), merge, deploy,
close an issue, or activate production services.

## Review boundary

- Replace the game-owned twelve-piece raster atlas with the pinned Apache-2.0
  Chessnut artwork by Alexis Luengas, retaining its original SVGs, copyright,
  license, notice and exact provenance
- Reserve `chess@0.3.0`; PR112 separately owns `chess@0.2.0`. Existing piece
  resource IDs, bounded density selection, integrity checks and image-cache
  ownership remain the contracts
- Recompose the existing opted-in direct create/join shell around one primary
  create action, a separate labeled native join-code form and passive unavailable
  quick-match information. Existing registry, session, CSRF, admission and
  separate-document gameplay authority remain the adapters
- Preserve Tabula's shared M3 Expressive theme. Game artwork does not recolor
  the dashboard; the renderer remains generic and Chess rules do not change

The approved design was inspected locally before implementation. Those design
PNGs are reference artwork, not screenshots of this code, and are not included
as runtime evidence. Standalone design/issue publication was cancelled; this
draft contains only implementation inputs and scoped implementation evidence.

## Claims and evidence

| Claim | Owner and failure mode | Check | Status |
|---|---|---|---|
| All twelve original SVGs have exact pinned provenance | game asset source; accidental replacement or external dependency | source hashes/XML and retained rights | PASS |
| Both density atlases are reproducible, bounded and integral | game generator and managed pack builder; stale or mixed pixels | generator check, pack build and asset tests | PASS |
| Legal files are distributed without image decoding/preload | managed staging and named game files; attribution omission or text-as-image | host/staging/asset tests and optimized native bytes | PASS |
| Piece scale remains upright and projection-only | Chess presenter; stretch, orientation or hit-target mismatch | presenter examples/snapshots | PASS |
| Admission is capability-gated and duplicate-safe | online entry; false success, repeated create or stale completion | focused entry lifecycle/error tests | PASS |
| Input, session and public-safe denial semantics survive | online entry and existing HTTP boundary | focused shell/core tests, source review | PASS |
| Authoritative portable local core gate | `cargo xtask check` in its maintained order | frozen-source final aggregate invocation | PASS |
| Actual canvas/Leptos visual quality | actual built documents; font/renderer/reflow differences | dedicated exact-head graphics capture | NOT_RUN |
| Real two-browser authenticated create/join | existing isolated HTTPS/PostgreSQL composition | maintained real-authority fixture | NOT_RUN |

Commands, nonzero execution counts, toolchain/source identity and changed
statuses are recorded after execution. Compilation, RenderList snapshots and
source inspection do not establish actual browser/native pixels or physical
touch/screen-reader acceptance.

## Executed local checks

Toolchain: Rust/Cargo 1.96.1, cargo-deny 0.20.2. Builds use two jobs, no
incremental compilation, dev/test debug=0 and `SQLX_OFFLINE=true` where relevant.
The reviewed frozen-source inventory covered 55 changed implementation files,
including explicit deletions. Its aggregate SHA-256 is
`142a6249944b6059e37f6d5de6f4950df97c76463959bd6ae097ac49c8dfdf40`.
The independent 11-file web inventory hash is
`e41756395bd77d3023dfefcea0f6819602ec28ce4bdee90fb2a650adf801f099`.

- `python3 games/chess/assets/generate.py --check`: exact 12 pinned sources,
  reproducible atlases/source manifest and transparent gutters PASS
- `cargo xtask pack-assets chess`: exact binding/hash/bytes PASS. Pack 0.3.0 has
  8 physical files, 317,401 encoded bytes and 13 unchanged logical resources
- `cargo test --locked -p tabula-game-chess --features presentation --lib presentation::`:
  88 passed, 8 filtered. Seven reviewed snapshots contain only 213 centered
  sprite-rectangle changes from 89% to 97%, plus assertion-line metadata
- `cargo test --locked -p tabula-game-client --lib fixture_assets::`: 11 passed,
  40 filtered. Both density atlases decode as monochrome; every gutter is
  transparent; legal files do not enter selected texture decoding
- `cargo test --locked -p xtask wasm_stage_cmd::tests`: 17 unit tests passed;
  filtered process-level targets were not exercised by this command
- `cargo test --locked -p tabula-game-client --lib --test local_match`:
  51 unit and 15 integration tests passed, no ignored/filtered cases
- `cargo test --locked -p tabula-web --features online`: 100 passed, no
  ignored/filtered cases. Independently selected `online::`: 13 passed,
  87 filtered. Native unavailable-markup coverage does not exercise the ready
  form or real browser event/focus behavior
- `cargo clippy --locked -p tabula-web --features online --all-targets -- -D warnings`,
  the corresponding `wasm32-unknown-unknown` clippy and WASM check: PASS
- Native release build with `--no-default-features --features chess`: PASS.
  All four complete rights documents occur byte-for-byte in the optimized
  binary; optimized binary SHA-256 is
  `8392e9ce55399d5e6babd700221ba9930fc28e147fab9e01deca4a54011975d3`
- Gameplay WASM release build with `--no-default-features --features web,online`:
  PASS. Its 1,127,192-byte artifact SHA-256 is
  `073d81f70c0bba1e00ed236aa2813cfa3f41254e7fb680af6fa85958c2e2913b`.
  The compiled xtask staged the actual artifact and 12 immutable runtime
  resources; all 12 staged SHA-256/byte identities matched. No browser opened

The shared renderer's metadata budget still counts physical files. Two test
fixtures now admit the pack's eight files and retain exact assertions for only
four PNG decodes/uploads; production limits and cache policy are unchanged.
[Independent review](review.md) found no unresolved implementation defect in
this frozen scope and retains the actual-browser/authority evidence gaps.

The final frozen-source portable aggregate passed all maintained stages, with
1,297 passed test executions across 60 nonempty targets and 18 ignored tests.
Ignored and zero-test targets establish no missing real-boundary acceptance.
The local logs retain command order and non-fatal existing cargo-deny warnings.
Generated receipts, raw logs and PNGs are excluded from Git. Actual browser
evidence will use source-pinned Actions artifact links when available. No
unrelated GitHub CI monitoring was run.

## Entry recovery limits

Only the non-authorizing unresolved-admission bit survives a document reload;
no join code, match identity, seat, account identity, CSRF or attachment grant
is stored. Code and known admission can survive internal shell routes in memory,
with current-account fencing. The existing provider continuation replaces the
document and returns to Account. Preserving a typed code or chosen game through
that full provider round-trip is not implemented by the current adapter; return
to the game and enter the shared code again. This draft does not expand provider
continuation or persist the code to imply otherwise.

An uncertain dispatched admission is not a cancellation receipt. Fresh create
and join remain suppressed in the tab, including after reload, because the
current adapter has no admission-reconciliation endpoint. Definitive denials
and validated success remain distinct from local suppression-storage failures.
The shell does not create another match automatically.

## PR112 interoperability

PR112's original source pieces stay independent. It adds board material, grain,
motion/HUD work and generic projected-event/rejection hooks. This fresh-base
change does not copy those changes. If both drafts are later selected, reconcile
the game-owned generator, manifest/version and `presentation/assets.rs` once:
retain this draft's exact licensed Chessnut art and attribution, retain PR112's
separate grain resource/material/motion laws, and regenerate a new coherent pack.
Never assign two different pixel sets the same pack version or old digest.
No merge ordering or approval is implied.

## Remaining acceptance

Actual 320×640/390×844/desktop/landscape layouts, four schemes, DPR1/2, 200%
text, both orientations, all small silhouettes and promotion/check/focus need
named pixel inspection. Physical touch, screen-reader completion, native/mobile
embedding and performance are separate target evidence. Quick matchmaking stays
unavailable until #55 has an approved real queue contract and server authority.
Untimed/unranked direct play creates no clock, rating, bot or online-count claim.
The [backend roadmap #110](https://github.com/loveoverflowcom/tabula/issues/110)
separately sequences service composition, Room ready/start and durable results;
queue/matchmaking follows later. This draft implements none of those backend
steps and never treats a full roster as user readiness or matchmaker authority.
