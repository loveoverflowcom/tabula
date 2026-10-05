# Actual Werewolf runtime screenshots

This isolated evidence branch builds the existing `web-werewolf` binary and
stages its genuine Macroquad/WASM resource bundle. It does not edit game rules,
presentation, platform behavior, phase gates, or production configuration.

The GitHub-hosted browser drives ordinary pointer controls on the local
isolated-seat simulator. The fixture uses the runtime's twelve anonymous
`Người N` seats; none represents an account or a real person. Screenshots show
one deliberately selected test seat's authorized view, or the public view.
Temporary OCR images are never uploaded. No canonical state, seed, account
credential, network game, chat, voice, CMP embedding or physical mobile
acceptance is claimed.

The script records source commit/tree, a clean-checkout check before outputs,
WASM/resource hashes, actual browser engine/version, viewport/DPR, UTC capture
time, ordinary input sequence, PNG dimensions and hashes. Original captured
PNGs are preserved. Pixel inspection is a separate later receipt, and is
required before appending this evidence to issue #82.

Run: `python3 tests/werewolf-runtime/capture.py` after the exact build/stage
commands in `.github/workflows/werewolf-runtime-evidence.yml`. Official
Playwright, Pillow and Vietnamese Tesseract OCR are prerequisite test tools.
