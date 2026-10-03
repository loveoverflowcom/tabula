# Web Host for `tabula-game-client`

Minimal checked-in browser host for running the Macroquad gameplay client as WebAssembly.

## Provenance

- **`index.html`**: Checked-in host page providing the `#glcanvas` canvas and bootstrap loader.
- **`mq_js_bundle.js`**: Pinned upstream JavaScript bootstrap from `macroquad 0.4.16` (`miniquad 0.4.11`), dual-licensed under MIT / Apache-2.0.

## Usage

Build and stage the bundle:
```bash
cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-wasm-game
```

Or run the local development server:
```bash
just wasm-serve
```
