# Werewolf: actual runtime review

Ten original PNGs were captured from the existing built Macroquad/WASM local simulator in actual Chromium, then hash/dimension-checked and visually inspected. The capture workflow succeeded; visible UI review found defects.

- Actual source: `24bfa7567110e83e0879bda12f141b7af47e9b11`; tree `2c9f9d16ff1de2db48baa435af2b3034ac9cd0f0`
- Game/presentation/runtime behavior is unchanged from develop `9642e4a60a8041bde3652d96dae0d1544bfaec3a`; the isolated evidence branch adds test tooling only
- Actual run: https://github.com/loveoverflowcom/tabula/actions/runs/37390657748
- Chromium 151.0.7922.34 / Playwright 1.62.0, headless ANGLE/SwiftShader, disposable first-party loopback server on GitHub-hosted Ubuntu 22.04
- Desktop 1200x880 and separate fresh narrow 390x844 CSS pixels; DPR1, dark theme
- Exact WASM: 1,200,199 bytes; SHA256 `d3d7011d9051ab666b1a7e58e0bd958dfbf26ef07d614f209f58482bc439c019`
- UTC capture window: 2026-10-05 23:51:55 through 23:54:38
- Only anonymous disposable Người N simulator seats or public outsider views; no real accounts or credentials

## Findings

1. Role-front sprites and text, private information, public logs and terminal draw text are vertically inverted inside clipping scopes in the tested browser. The concealed back and un-clipped controls are upright. Candidate: `crates/tabula-render-macroquad/src/draw.rs:189-214` and the screen-camera Y convention. No production fix was made; native/other platforms were not executed.
2. At 390x844, the host Leave setup / Keyboard help buttons overlap simulator footer warnings. Candidate: `.runtime-access` in `apps/game-client/web/standalone.css` and the Werewolf footer. This is browser responsive evidence, not physical mobile or CMP embedding acceptance.

The original PNGs are unchanged. Temporary OCR-only crops, vertical flips and grayscale/upscaling were used to read the actual buggy pixels; no published image or game asset was corrected.

## Scope

The operator chooses each isolated local seat, explicitly reveals its own card, selects actual legal targets and submits through the real UI. One wolf attack and one witch poison produce two public dawn deaths. One living seat submits a ballot for seat 6. Normal logical-deadline controls then reach a real round 10 draw. A separate fresh narrow document captures the own-card and table tabs.

This is the isolated-seat local simulator. Real online multiplayer, authenticated seat privacy, chat/voice/native audio, persisted resume/replay, physical mobile touch, Android/iOS CMP WebView embedding, production rollout and complete accessibility were NOT_RUN.

`screenshots-provenance.json` and `browser-result.json` are original capture outputs. `inspection-receipt.json` is a separate post-capture visual/byte verification receipt; it does not rewrite capture provenance. The earlier partial failed run remains under `../62d2c742/`, with its original failed verdict, and is not relabelled PASS.
