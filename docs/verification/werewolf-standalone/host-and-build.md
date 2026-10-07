# Local host and target evidence

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-standalone).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


The separately compiled `tabula-werewolf-client` is an explicitly labelled isolated-seat
simulator. The operator may select each seat or a public outsider view on the same device.
It is not authenticated hot-seat privacy, online multiplayer or a substitution-bot table.

## Build / run

```bash
cargo run -p tabula-game-client --no-default-features --features werewolf --bin tabula-werewolf-client -- --seats 12
cargo build -p tabula-game-client --no-default-features --features web-werewolf --bin tabula-werewolf-client --target wasm32-unknown-unknown --profile wasm-release
cargo xtask stage-wasm-game --game werewolf
python3 -m http.server 8768 --bind 127.0.0.1 --directory target/tabula-web-werewolf
```

The old Chess browser command `--no-default-features --features web` remains compatible.
Native defaults retain the existing Chess/Tiles set. The explicit Werewolf web feature links
Werewolf alone, without Chess, Tiles or Leptos in its normal target dependency graph.
Registry launch support and normal dashboard behavior are unchanged; rollout stays disabled.

## Interaction / lifecycle

- Night/Day/Vote are valid five-minute fixed simulator windows; Dawn/Dusk two seconds,
  cap ten rounds. These are host configuration, not changes to domain defaults
- Next phase advances the real logical deadline. A recorded Timer enters the same
  pure reducer path as elapsed time. Early all-action completion does not close a window
- Each seat uses `LocalMatch::set_viewer` after clearing private-local state. Outsiders
  cannot issue player commands. No raw state, seed, canonical counters or audit view is shown
- Role fronts and private target controls appear only after deliberate own-card reveal.
  Escape, seat/public switching, focus loss, phase/lifecycle changes and reset conceal them
- Enter/Space held across conceal or fresh matches must release before activation
- The browser immediately adds an opaque role-independent shield on focus/visibility loss
  or host dialogs. It removes that shield only after Rust submits and flushes a concealed
  frame, then requests the bounded `tabula-concealed.txt` virtual acknowledgment. Generation
  checks reject stale acknowledgments. Merely returning focus does not uncover a stale frame
- Startup and renderer-recovery frames also acknowledge only after successful flush. A focus
  loss plus reactivation in one batch is forcibly concealed before the acknowledged frame
- The three virtual configuration/readiness/conceal files are never network/cache payloads.
  No private DOM role, CSS background-role URL or canonical replay data is exposed
- Refresh/retry/page restoration starts a fresh match. Existing generation/abort teardown,
  verified public-cache budgets, HTTP integrity and explicit resource retry remain in force

## Executed checks and limitations

The host Node suite currently passes 90 tests, including existing loader failures, cancellation,
cache denial/corruption, repeated navigation, stale callbacks, missing imports/SRI, runtime
mismatch, and the new privacy shield/ack lifecycle, startup interruption and safe recovery-frame acknowledgment. These are mocked browser component tests.
Four generic `LocalMatch` integration tests pass on real Werewolf rules: private action public
projection/render noninterference, fixed deadlines and Timer stream, deterministic completed
all-timeout matches, fresh reset and isolated perspective switching.

Native debug and optimized WASM builds executed successfully. The final selected artifact is
1,202,983 bytes raw, 434,147 gzip9; its exact
SHA256 and complete resource inventory are in loading-receipt.json.
Staging validates and content-versions 18 runtime payloads (WASM, three fonts, 14 density images),
with only seven selected-density card assets loaded by the Rust host. Both densities' total
encoded art is 1,639,008 bytes; no original all-role atlas is linked or served.

Real native launch failed with `XOpenDisplay() failed`: this execution shell has no DISPLAY
or X11 socket. The permitted dot cloud browser preview to `http://127.0.0.1:8768/index.html`
failed with `net::ERR_BLOCKED_BY_CLIENT`. No alternate route was used to evade the denial.
Therefore native/browser screenshots, real GPU upload/input, mobile 390×844/320×568 pixels,
200% browser zoom, BFCache behavior, assistive-technology and final GPU reclamation are
**BLOCKED / NOT_RUN**, not passed. Approved design-reference pixels are not runtime captures.
The opt-in source is reviewable; rollout acceptance remains open until those gates are met.
