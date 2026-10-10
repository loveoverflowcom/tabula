# D06 private HUD review preview

Presentation/storyboard tooling for issue #124. Public source contains no art,
rig/provenance or Drive reference. It does not implement a game, simulate combat,
register Age War or embed a web view in CMP. Every displayed resource/HP/queue/
outcome is a review fixture; controls log semantic intents with `authority:false`.

[HUD/UX contract](../../../../docs/games/age-war/D06-HUD-UX.md) owns the design,
input/state/asset map and pending owner code gate. [Ledger](../../../../docs/verification/age-war-d06/README.md)
owns checks and limits. Browser screenshots/interactions are still required.

Build the D05 packs first (see the parent README). Then:

```sh
cargo build -p tabula-game-client --example age_war_d05_pilot --release
# Public catalog only: no private input is read by hud-catalog.
target/release/examples/age_war_d05_pilot hud-catalog --report games/age-war/design-tools/hud-preview/catalog.generated.json
python3 games/age-war/design-tools/hud-preview/stage_preview.py \
  --vfx-zip PRIVATE/<VFX auxiliary archive>.zip --out verification/age-war-d06
python3 -m http.server 8124 --bind 127.0.0.1 --directory verification/age-war-d06/site
```

Open `http://127.0.0.1:8124/?age=Arcane&size=844x390&font=1.3&state=queue`.
Review toolbar selects six ages, state fixtures, four reference sizes/portrait,
font100/130/200%, full/low/reduced VFX,0.5/1/2×, timeline and light/dark theme.
Motion starts explicitly; the page is silent. Listen separately through D05's
Rust pilot `--audio`. System Back, real host lifecycle, native performance and
screen-reader behavior remain outside the executable HTML scope.

`stage_preview.py` first runs the Rust pack binding/integrity/D01 timing check,
then copies density1 colour/VFX pages and exact regions/logical pivots/frame times.
It writes `site/assets/assets.json`, a private `handoff.json`, and
`age-war-d06-private-preview.zip` with CRC checked. In-repo output must be ignored.
It also renders 72 labelled Pillow reference compositions and a six-age PNG
sheet under `site/review-compositions/`. These share the layout geometry and
private art; they are not screenshots of HTML or evidence for browser fonts.
The ZIP contains all6age review art for private inspection, not a production
loading plan or permission to redistribute. Active preview loads only one age.
Base/turret complete states and final unit/spell HUD icons are missing; derived
idle/VFX icons are labelled in the manifest.

```sh
cd games/age-war/design-tools
node --test --test-isolation=none tests/*.test.mjs
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

Node24+, Python3.12+ with Pillow/NumPy; existing Rust build and private D05 input.
No npm dependencies. The layout checks prove geometry, not browser font metrics
or actual rendered pixels. Artwork/license approval and C01 gate remain pending.
