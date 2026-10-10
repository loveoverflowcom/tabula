# Age War offline presentation tools

Code-only support for D05 issue #123. This directory contains elapsed-time visual
marker dispatch and interrupt blending, bounded audio scheduling, constant-gain
review audio, and exported-media validation. It implements no gameplay, damage,
event ABI, playable registration, Macroquad importer or native host.

The marker controller is extracted from the authored offline pipeline. The pose
sampler is injected by the caller, so no authored rig or asset data is embedded.
Marker results always carry `authority: false`. A marker never awards a hit or
changes canonical combat. The actual source assets and complete authored pipeline
are delivered separately; this public package does not regenerate that artwork.

## Checks

Node 24 and Python 3.12:

```sh
npm run test:markers
npm run test:python
```

Tests use synthetic poses, cue metadata and codec streams. No art download or
third-party Node dependency is required. FFmpeg and FFprobe with H264/AAC support
are needed for the executable media and gain commands:

```sh
python3 core/audio_gain.py input.wav new-review.wav
python3 core/media_probe.py preview.mp4 --width 80 --height 60 --duration 1 --frames 60
```

Supply the real expected dimensions, duration and encoded frame count. The small
numbers above are synthetic examples. Encoded 60 FPS and successful decoding do
not establish browser playback, real-time frame rate, audio audition, visual art
acceptance, GPU residency or native-device performance.

## Runtime pack tools (`runtime/`)

Offline, game-owned tools for the owner's private D05 delivery. They read the
private index and archives from a directory you pass in and write private
outputs under ignored `target/` and `verification/` paths; nothing private is
committed. Python 3.12+ with Pillow and NumPy; FFmpeg for audio checks.

| Tool | Purpose |
|---|---|
| `verify_inputs.py` | Re-hash archives against the private index, test ZIP CRCs and member lists, report missing inputs (never a pass) |
| `build_runtime_packs.py` | Independently re-check the bake, repack per (unit, facing) at density 1/2, write integrity-protected sidecars and seal packs with `cargo xtask pack-assets --external` |
| `pilot_matrix.py` | Run the Rust presentation pilot (`apps/game-client/examples/age_war_d05_pilot`) across ages, speeds, effect modes, sizes, crowds, DPI and load/unload |
| `clip_sheets.py`, `coverage_matrix.py`, `audio_checks.py` | Pose sheets from the sealed packs; unit/skill → animation/VFX/SFX coverage; numeric audio, loop seams and scheduler limits |

[The D05 ledger](../../../docs/verification/age-war-d05/README.md) owns commands,
results and limits. The runtime tests use synthetic data only.

## Gates

D05 remains animation/design preview work. Final motion, joints/grips/sockets,
crowd readability, audio listening/rights, projected View/ViewEvent integration,
native importing and actual device budgets require separate acceptance. D06 may
compose design evidence; production gameplay and phase exits remain gated.

See the repository architecture contract, especially I-10/I-12, and the existing
Age War rules/schema documents. This tooling does not modify those contracts.

## D06 HUD review

[`hud-preview/`](hud-preview/README.md) stages a private HTML/canvas storyboard
from verified packs: all six ages, complete baked motion frames, 6-card tray,
queue, two spells, research/turrets, codex and state fixtures. Its controls log
intents only. The [HUD contract](../../../docs/games/age-war/D06-HUD-UX.md) and
[D06 ledger](../../../docs/verification/age-war-d06/README.md) retain the pending
owner/authority gate. Source is public; art, PNG reviews and ZIP stay ignored.
