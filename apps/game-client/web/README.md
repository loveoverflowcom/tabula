# Standalone Chess browser host

A real local hot-seat Chess setup page and a separate Macroquad gameplay document.
The board remains the production Rust presenter/Renderer path (ADR-011 and
ADR-0029); this host adds no DOM board, iframe, network service or shared WASM memory.

The normal Tabula discovery/setup handoff is an opt-in build of this same runtime
([ADR-0030](../../../docs/adr/0030-local-discovery-gameplay-handoff.md)). See the
[integration instructions and evidence](../../../docs/verification/chess-integration/README.md).
The standalone distribution below remains separate and unchanged in purpose.

## Build and serve

```bash
cargo build -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown --profile wasm-release
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

Staging also generates an exact public-resource manifest and SHA-256 URLs. The
served HTML pins its script/styles with content names and SRI. The fixed-name
copies are diagnostic compatibility output, never runtime fetch targets. The
host downloads only the selected WASM, three fonts and one critical piece atlas;
the loading/recovery UI uses system fonts and no hero image. Cover textures are
not needed when the host passes `--skip-setup`. Another piece density is fetched
only after a real DPI-tier change. Native builds keep their default game set.

The optional CacheStorage cache rechecks SHA-256/size on every hit and serializes
resource and budget work using Web Locks. Public payloads plus a bounded 32 KiB
index fit 150 MiB/32 entries. Private arguments, roles, identities and match state
are never stored. Storage/lock denial or absence falls back to verified network
loading. WebCrypto SHA-256 in a secure context and readable streaming response
bodies are required; non-streaming payloads fail before whole-body buffering.
No service worker, persistent-storage
permission or offline guarantee is added. See the
[loading ledger](../../../docs/verification/game-loading/README.md) for actual
emitted sizes, test scope and the outstanding real-browser cache/timing checks.

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

A host plugin implements the existing pinned `fs_load_file` API for those two
virtual files and bounded cancelable same-origin asset downloads. Generation
admission rejects late callbacks after failure, departure or page restoration;
the pinned upstream bootstrap itself is unchanged. File-loaded callbacks are
asynchronous; failures expose the recovery UI. The loading screen
is dismissed only after the ready acknowledgement and a successful runtime frame,
not after download, compile, `main()` or an async startup frame. Download reports
actual received bytes against the manifest's decoded size, including compressed
responses. A cache hit is labelled as reverified cached bytes, never a download.
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

The gameplay document boots at most once. Return, pagehide and runtime failure
abort owned downloads/timers, retire file buffers and export references and
cancel the known animation callback. A cached document restores by explicit
reload into a fresh local match; it never resumes discarded state. Browser
document teardown owns final WASM/canvas/GPU disposal. The mocked lifecycle tests
do not prove real browser BFCache behavior or total heap/GPU reclamation.

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
node --test apps/game-client/web/tests/*.test.cjs
cargo test -p xtask wasm_stage_cmd
cargo test -p xtask --test wasm_stage_cli
cargo xtask check-no-raw-colors
cargo xtask check-no-game-ids
python3 tools/test_serve_local_shell.py
python3 tools/tests/check-loading-budgets.py --game-wasm target/wasm32-unknown-unknown/wasm-release/tabula-game-client.wasm
```

The Node tests execute bounded configuration parsing, mocked setup/navigation and
bootstrap/file-loader lifecycle failures. They do not establish rendered pixels,
real browser input/visibility, assistive-technology play, physical mobile or native
visual parity. Actual browser testing in the current cloud session was blocked by
`ERR_BLOCKED_BY_CLIENT` on the supported local HTTP preview route; no alternate
route was used to bypass that denial. Desktop/mobile/high-contrast/200%-zoom pixel
and real keyboard/assistive-technology QA remain to be run on a permitted browser.

## Hosted by the mobile app (ADR-0033)

The integrated `/play/local/` document also runs inside the Android/iOS app's WebView. `host-bridge.js` is
inert unless the native host injects `window.TabulaHostNative` (an origin-restricted port); a browser
document never has it and behaves exactly as described above. When it is present, `bootstrap.js`:

- says `hello`, waits for `init` (generation, granted capabilities, host preferences) and **starts nothing
  before it**; silence for 5 s is a visible failure with no game fetch;
- replaces only `theme`, `motion` and `locale` from the registry-validated launch query with the host's
  preferences (read once; the board does not re-theme mid-game);
- reports `ready{bootMs}`, `failed{code}` and `exit` instead of navigating, and asks for the one granted
  service (`keep-awake`) after the board is on screen;
- handles `suspend`/`resume` (stop and restart the frame loop; the local clock keeps wall-clock time),
  `back-requested` (opens or dismisses its own leave confirmation) and `dispose` (retires the runtime);
- drops any host message for another generation, after `dispose`, or outside the schema.

The wire grammar is `tests/bridge-vectors.json`, run by `tests/host-bridge.test.cjs` here and by the Kotlin
`BridgeVectorsTest` in `mobile/`. The loader (`resources.js`) is unchanged: same-origin, SHA-256 and size
limits all still apply.
