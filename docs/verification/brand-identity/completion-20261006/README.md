# Issue 91 completion — 2026-10-06

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/brand-identity/completion-20261006).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


This follow-up integrates PR90's action palette with PR95's T Portal identity.
The runtime source is `ad2677a`; `fa7d4ad` merges the completed PR90 from develop
with an identical tree. The original implementation record in the parent README
is historical; the checks below supersede its pending/blocked statements only
for the named targets.

| Claim / owner | Oracle and executed check | Result | Remaining scope |
|---|---|---|---|
| One authored theme, ADR-0027 | `cargo xtask check`; source/adapter contrast check | PASS: 1,270 tests, 18 existing ignored doc examples; 684 scalar mappings, 16 filled-label and 24 primary/surface pairs | Ignored docs are not tests passed |
| Shared geometry, provenance and launcher resources | `python3 -m unittest discover -s tools/tests -p test_brand_identity.py -v` | PASS: 6 tests, exact source/output hashes, vector outlines, alpha and 16/24/32/48/64 portal pixels, OS safe masks | Fresh Inkscape rerasterization NOT_RUN here: Inkscape unavailable; committed exports unchanged from original verified run |
| Web identity / I-10 | Actual Trunk online-feature `wasm-release` build and the capture helper against the loopback server | PASS: 32 cases, four themes × 320/390/768/1440 × vi/en; keyboard Tab reaches brand, Escape returns drawer focus, one accessible name, reduced motion, unclipped canonical ratio; zero page errors | This matrix checks brand, not all page typography or gameplay |
| Shell loading budget | Actual emitted WASM length and SHA-256 in `runtime.json` | PASS: 739,221 bytes < unchanged 900,000-byte cap | No network performance claim |
| Mobile shell and package wiring | `./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` after real `stage-mobile-game` | PASS: 52 shared tests, 14 desktop Compose tests including 8 brand theme/width cases; debug APK assembled | Desktop Compose screenshots are not Android WebView, installed launcher or iOS execution |
| Existing browser host behavior | `node --test apps/game-client/web/tests/*.test.cjs`; dashboard helper tests | PASS: 122 host tests; 17 helper tests | Helpers are not browser interaction evidence |
| Existing pack integrity and lazy setup | Three fresh packs compared byte-for-byte with fixtures; built/staged Werewolf checked against approved #84 inventory | PASS: Werewolf 0.2.0's 18 PNGs / 3,203,783 bytes and all hashes; 22 runtime resources; unchanged WASM caps; 3 negative controls reject image/preload requests disguised as brand or game files | Static inventory is not a request waterfall |

Five actual web PNGs and two actual desktop Compose PNGs are archived at the pinned commit;
all seven were visually inspected for open portal, consistent outlined wordmark,
readable scheme colors and no logo clipping. Their SHA-256 hashes are in
`runtime.json`. Desktop web captures intentionally show keyboard focus.
The 320px drawer capture uses ordinary browser text size; earlier 200% text,
standalone header/loader and before images remain source-pinned in
[issue 91's runtime evidence](https://github.com/loveoverflowcom/tabula/issues/91#issuecomment-6016172714).

## Reproduce web captures

Build actual artifacts, then run the existing local server:

```sh
CARGO_TARGET_DIR=/path/to/shared-target cargo build -p tabula-game-client --no-default-features --features web --target wasm32-unknown-unknown --profile wasm-release
(cd apps/web && NO_COLOR=true TABULA_PLAY_BASE=/play CARGO_TARGET_DIR=/path/to/shared-target trunk build --release --cargo-profile wasm-release --features online)
CARGO_TARGET_DIR=/path/to/shared-target cargo xtask stage-local-play
python3 tools/serve-local-shell.py --port 8191
```

In a second terminal, using Python with Playwright 1.62 and Chrome installed at
`/opt/google/chrome/chrome`, run `python3 tools/verification/capture-brand-identity.py`.
The harness uses real builds, media preferences and keyboard input; no injected
styles, modified responses, account credentials or game state.

## Merge scope and residuals

PR90 merged into develop as `1a82ff8`. PR95 preserves the same action palette
and adds independent `brand-*` roles. Design01 layout/hierarchy (#87), game art,
account avatars, rules and phase gates remain unchanged. CI now exercises the
brand source/pixel tests and verifies the generated Kotlin adapter too.

The online continuity job was already failing on develop before these PRs.
[Run 37456929975](https://github.com/loveoverflowcom/tabula/actions/runs/37456929975)
passed the real two-browser Chess game and durable PostgreSQL verdict, then
failed with `browser_timeout` in continuity/rotation after seven completed fault
partitions. That failure is not relabeled PASS or bypassed; these branding PRs
do not claim to repair online recovery. Branch protection API reported no
protection and the repository ruleset list was empty when inspected.

Native window/Dock pixels, installed mobile launcher behavior and physical
Android/iOS execution remain NOT_RUN. The optional desktop launcher and separate
About/splash screens do not exist; no phase is opened to create them.
