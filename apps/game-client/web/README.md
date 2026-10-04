# Standalone Chess browser host

A real local hot-seat Chess setup page and a separate Macroquad gameplay document.
The board remains the production Rust presenter/Renderer path (ADR-011 and
ADR-0029); this host adds no DOM board, iframe, network service or shared WASM memory.

## Build and serve

```bash
cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-wasm-game
just wasm-serve
```

Open the server's `index.html` to configure a game. Start performs a real document
navigation to `play.html`. Only same-device hot-seat is enabled; bot and online
choices are visibly unavailable. Local matches are not saved. Refresh/retry starts
a new match, and leaving from the host asks before abandoning play.

`stage-wasm-game` atomically copies the two documents, pinned bootstrap, scripts,
styles, cover exports, fonts/licenses/provenance and compiled WASM into
`target/tabula-web-game`. It copies generated `apps/web/style/tokens.css`
byte-for-byte. Any missing or empty required resource fails staging and invalidates
the previous output. Run `cargo xtask gen-tokens` when authored tokens change.

## URL configuration

The standalone host is Chess-only. For example:

```text
play.html?game=chess&mode=hot-seat&clock=bronstein&initial-ms=300000&delay-ms=2000&locale=en&theme=dark&motion=reduced
```

- `clock`: `untimed`, `fischer` (default), or `bronstein`
- `initial-ms`: integer 1000–10800000, default 300000
- `increment-ms` / `delay-ms`: integer 0–60000, default 2000; only the selected control is passed to Rust
- `theme`: `system` (default), `light`, `dark`, `hc-light`, `hc-dark`
- `motion`: `system` (default) or `reduced`; the OS reduced-motion preference is always honored at launch
- `locale`: `vi` (default) or `en`; this covers the host UI, not native runtime localization

Registry-style `chess.clock`, `chess.initial-ms`, `chess.increment-ms` and
`chess.delay-ms` are accepted with the same bounds. Conflicting, repeated, unknown
and credential-shaped options fail closed. The host does not consume join tokens,
canonical state, identities or replay data. Preferences are applied at launch.

## Safe WASM bridge and readiness

The pinned Miniquad version has no browser `env::args` API. Native Rust reads its
ordinary process arguments; WASM uses safe `macroquad::file::load_file`:

- `tabula-launch.txt`: host-provided UTF-8, newline-separated allowlisted arguments,
  bounded to 4096 bytes / 64 tokens, including `--skip-setup`
- `tabula-ready.txt`: a one-time readiness acknowledgement requested by Rust only
  after a full nonfatal game frame has been submitted and flushed

A host plugin wraps the existing pinned `fs_load_file` import for exactly those
two virtual files. All other assets go through the original loader. File-loaded
callbacks are asynchronous; failures expose the recovery UI. The loading screen
is dismissed only after the ready acknowledgement and a successful runtime frame,
not after download, compile, `main()` or an async startup frame. Download reports
actual received bytes; unknown/compressed lengths use an indeterminate progress bar.
Missing HTTP artifacts, missing imports, bootstrap-version mismatch, runtime
exceptions, startup timeout and WebGL context loss have explicit recovery.

Macroquad continues to own canvas input, resize and visibility/focus callbacks.
Canvas blur/focus is also forwarded to its existing focus export after readiness,
so entering a host dialog clears held input and drag state without pausing clocks.
The host does not pause the rules clock when hidden. Ordinary Tab remains presenter input to reach the in-game HUD from the last
board square. Shift+Tab explicitly leaves the canvas and focuses the host Leave
button; browser Tab then reaches Help. Arrow/Enter/Escape remain presenter input. Leave/help use native dialogs with safe cancellation and
board focus restoration. Browser zoom is unrestricted. Setup is accessible DOM;
a complete assistive-technology Board Reader is not implemented or claimed.

## Provenance

- `mq_js_bundle.js`: unchanged pinned upstream Macroquad 0.4.16 / Miniquad 0.4.11,
  dual licensed MIT / Apache-2.0; original header retained
- Cover: supplied approved `chess-design` editorial artwork; original and responsive
  export hashes and exact generation provenance in `assets/chess-cover-provenance.md`
- Fonts: supplied Open Sans (Apache-2.0) and Noto Serif (SIL OFL 1.1), with complete licenses retained
- Shared header: canonical system CSS variables; game-art variables scoped to the
  Chess surface, generated from the authored token authority

## Focused checks

```bash
node --test apps/game-client/web/tests/standalone.test.cjs
cargo test -p xtask wasm_stage_cmd
cargo test -p xtask --test wasm_stage_cli
cargo xtask check-no-raw-colors
cargo xtask check-no-game-ids
```

The Node tests execute bounded configuration parsing, mocked setup/navigation and
bootstrap/file-loader lifecycle failures. They do not establish rendered pixels,
real browser input/visibility, assistive-technology play, physical mobile or native
visual parity. Actual browser testing in the current cloud session was blocked by
`ERR_BLOCKED_BY_CLIENT` on the supported local HTTP preview route; no alternate
route was used to bypass that denial. Desktop/mobile/high-contrast/200%-zoom pixel
and real keyboard/assistive-technology QA remain to be run on a permitted browser.
