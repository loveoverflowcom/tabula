# Standalone Chess implementation ledger

Source baseline: `develop @ c0a62088c159cc1f20409de41af245c98dcfcfee`.
Date: 2026-10-04. Presentation package version is 0.1.2; rules version remains 3. This is a local native/WASM vertical slice, not a phase exit.

## Delivered boundary

- The existing pure Chess reducer still decides moves, promotion, timers and
  terminal outcomes. Canonical State and repetition history do not reach the
  presenter or browser host. `View.actions` projects non-move eligibility from
  the reducer's shared validation; `View.in_check` projects the side-to-move fact
- Macroquad remains behind Renderer/RenderList, with separate gameplay document
  per ADR-010/011/029. No renderer migration, online/rated/AI gameplay or deployment
- Standalone browser setup uses accessible DOM, only local two-human hot seat,
  bounded untimed/Fischer/Bronstein setup, system/four themes and reduced motion.
  Configuration crosses the existing safe Macroquad virtual file-loading seam;
  it carries no match state, credential, player identity or canonical data
- Native setup constructs the match only after intentional Start. New-game
  recovery constructs fresh rules, logical time, local interaction and clocks
- Approved original piece/cover exports are pinned, named and hash-verified
  before bounded decode. Four physical files total 280,240 encoded bytes. Full
  source artwork remains external to the binary's tiny local-pack exports
- Game-art roles are authored in `tokens.toml`; existing shared system roles
  are unchanged. Focus, selection, threat, last-action and legal-target semantics
  remain canonical and have text/shape counterparts
- Renderer owns bounded built-in Open Sans/Noto fonts; prepare, measure, wrap
  and draw share font selection. Replacement after the first frame is rejected
- Presenter owns tap/drag/keyboard input, responsive player bars, clock estimates,
  figurative pieces, promotion/cancel, flip, explicit locally admitted seat
  selection and confirmed eligible non-move controls. Network hosts must not
  enable the local hot-seat viewer override
- The visible move strip is at most 256 observed coordinate ViewEvents from
  this session. It is explicitly not SAN, complete match history, saved replay,
  engine analysis, a server history projection or an implementation of #52

## Evidence

The final checks below ran on the frozen implementation. The commit containing
this ledger identifies the checked source; delivery reports its exact remote SHA.
RenderList assertions establish commands and input behavior, not real pixels.
The approved asset PNGs were inspected; they are artwork exports, not runtime
screenshots. The existing gameplay WASM and native target are built separately.

Native UI execution was BLOCKED before startup by `XOpenDisplay() failed!`
in the display-less build executor. Real browser UI capture/interaction was BLOCKED:
`ERR_BLOCKED_BY_CLIENT` on the local HTTP preview. No alternate private-origin
route bypassed that restriction. Desktop/mobile/high-contrast/200%-zoom pixel
verification, actual assistive-technology Board Reader play, Safari/physical
mobile evidence and automatic browser-visibility suspension remain unverified.
The host does not claim full Board Reader support or saved local games.

## Reproduce

Use pinned `rust-toolchain.toml`, including `wasm32-unknown-unknown`, rustfmt and
Clippy. Install `cargo-deny` from crates.io if not present.

```bash
cargo xtask check
cargo check --workspace --no-default-features
cargo check --workspace --all-features
cargo build -p tabula-game-client
cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-wasm-game
node --test apps/game-client/web/tests/standalone.test.cjs
python assets/packs/chess/generate.py --check
```

Native default entry opens local setup. Explicit direct launch:

```bash
cargo run -p tabula-game-client -- --game chess --skip-setup --clock fischer --initial-ms 300000 --increment-ms 2000
```

Serve the staged `target/tabula-web-game` host through its documented HTTP workflow
on a permitted browser. `index.html` owns setup; `play.html` owns gameplay. A
reload starts a new local game; leave confirmation does not promise persistence.

## Final executed checks

- PASS: `cargo xtask check`, including fmt, workspace all-target/all-feature
  Clippy, workspace tests, dependency/game-ID/manifest/generated-token/raw-color
  checks and cargo-deny. 962 tests passed; 21 existing ignored tests were not run
- PASS: workspace no-default-feature and all-feature compilation
- PASS: native executable build; actual display launch BLOCKED as above
- PASS: gameplay WASM check (`web`, no defaults), release build and atomic staging;
  final WASM 1,902,364 bytes, all 15 required host resources staged with tokens
- PASS: Leptos shell WASM compilation, preserving the separate runtime boundary
- PASS: browser-host Node boundary/lifecycle suite 14/14, including actual pinned
  Tab handler forwarding, canvas focus interruption, ready/error and repeated flows
- PASS: 86 presentation/art/HUD tests, including all seven deliberately inspected
  updated command snapshots, and independent interrupted-drag/control/latch examples
- PASS: 37 client unit/integration-boundary tests and 15 external local-match tests,
  including verified bounded art-cache acceptance and completed-feedback keyboard flip
- PASS: native setup/config tests 9/9; renderer tests 80/80, including font lifecycle
- PASS: committed Chess and clock replay verification, with checked checkpoints,
  final hashes and outcomes. Both verdicts are `COMPATIBLE_VERSION`, `VERIFIED`,
  not exact source-hash identity after the projection-only change
- PASS: original art export reproducibility, deterministic pack builder, token
  generation and shared-token preservation; asset tests 2/2, design tests 9/9
- PASS: maintained skill/helper checks and their 32 + 6 failure-fixture tests
- BLOCKED: actual native/browser runtime screenshots, real touch/keyboard/zoom/
  platform visibility and assistive-technology play. No artwork is labelled as
  a runtime screenshot, and no unexecuted ignored/future-phase check is a pass

The existing CI workflow triggers on pull requests and pushes to `main`, not
`develop`; direct develop publication does not itself establish remote CI success.
No workflow trigger, branch protection, force update or deployment was changed.
