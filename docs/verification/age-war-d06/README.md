# Age War D06 — scoped HUD/design evidence

2026-10-10 UTC · [#124](https://github.com/loveoverflowcom/tabula/issues/124),
roadmap [#118](https://github.com/loveoverflowcom/tabula/issues/118).
Base `develop@74589b491b0e6fec6f55ebc4cb2a0bbacf10ad85`.
**DRAFT / OWNER REVIEW PENDING. C01 #125 remains BLOCKED.**

[HUD/UX contract](../../games/age-war/D06-HUD-UX.md) owns the screen/input/asset
and authority decisions. [D05 ledger](../age-war-d05/README.md) owns source
verification, runtime packing, D01 timing, numeric audio and Xvfb evidence.
Neither ledger approves art/rights, runs gameplay or opens a phase.

## Outputs and retention

| Output | Location | Scope |
|---|---|---|
| Public source | `games/age-war/design-tools/hud-preview/` | HTML/CSS/MJS, layout/state/intent map, complete-frame sampler, private exporter/stager/reference compositor |
| Public HUD catalog | `hud-preview/catalog.generated.json` | exported from compiled D01; names/IDs/prices/pop/timing/spells/tech/tháp/age advance |
| Private review site and ZIP | `verification/age-war-d06/site`, `age-war-d06-private-preview.zip` | six ages,36units,460clips/13058frames; atlas pages/crops/art never enter Git |
| Private asset manifest | `site/assets/assets.json` | pack@version/AssetRef/source-page BLAKE3/bytes, regions/logical boxes/pivots, ordered frame times; icons marked DERIVED |
| Private PNG review | `site/review-compositions/` |72 Pillow art/layout compositions (6age×4viewport×3font), plus six-age sheet; **not browser screenshots** |
| Private receipts | `verification/age-war-d06/{pack-check,handoff}.json`, logs | exact scope/config/results; no private delivery index, source provenance or Drive reference in public docs |

ZIP is a private review artifact (approximately212MiB,875members), with CRC check.
It includes all-age review pages; production must load a bounded needed subset.
Its existence is not redistribution permission. Rebuild with the owner's input;
no upload/share/public deployment was performed in this continuation.

## Checks actually executed

Environment: Linux x86_64, pinned Rust1.96, Node24, Python3.14/Pillow/NumPy;
Xvfb/Mesa for the Rust pilot. The desktop browser inventory exposed no surfaces.

| Claim | Status / evidence | Residual |
|---|---|---|
| Pack binding, bytes+BLAKE3, D01 timing | PASS: Rust `check`,7packs;1102D01 assertions,0failures | art/motion quality not established by hashes |
| Exported motion is non-empty, in bounds and ordered | PASS: staging460clips/13058frames/188pages, exact size checks and decoded PNG dimensions | no browser pixels/playback evidence |
| Public catalog equals compiled D01 | PASS: fresh `hud-catalog` JSON equality | unbalanced draft, not runtime rules identity |
| Layout | PASS:12 viewport/font cases;44touch/36pointer,5queue targets, safe areas, lane protection,3enlarged economy lines; portrait explained | geometric tests do not measure browser fonts |
| Node suite | PASS:36tests (18marker/controller +18HUD/layout/storyboard/map),0ignored | DOM interaction paths not exercised |
| Python suite | PASS:21tests (19existing/runtime +2export/private-output tests),0ignored | synthetic tests + real export receipt, not owner review |
| Rust pilot unit tests | PASS:3executed,0ignored (parser, sampling, sparse/dense marker equality) | presentation-only |
| Final Rust render/audio smoke | PASS: Arcane844×390,d1,650fixed16.667msframes,52verified sounds loaded at volume0;42textures released to0resident | screenshot/cues exercise desktop Xvfb, not HTML/native mobile or listening |
| Portable core aggregate | **FAIL at I-9**: fmt PASS, all-target/all-feature workspace Clippy PASS,1415workspace tests passed/0failed/18ignored, check-deps31crates PASS | scanner also walks local `.worktrees/` and private `verification/`; no full-gate PASS |
| Base-relative I-9 | inherited FAIL: clean BASE and clean changed-source copy both report exactly4mobile test literals | no new I-9 violation in contributed source; scanner/mobile tests unchanged |
| Additional gates after aggregate stop | PASS:34manifests, generated mobile catalog freshness, raw-color check; cargo-deny advisories/bans/licenses/sources | not represented as later aggregate stages passing |
| Pillow compositions | generated72+sheet; six-age sheet and Arcane844×390 at200% inspected for lane/art/spacing | text overflow discovered and corrected; still not browser font/interaction QA |
| Browser screenshot/input/AT review | **BLOCKED**: `cua` IAB and Chrome unavailable, inventory `browsers:[]` | must open staged site and exercise mouse/key/touch/focus/dialogs/large text |
| Audio audition, rights/style/motion/VFX acceptance | PENDING / NOT_RUN by a person | use D05 pilot listening command and owner review |
| Android/iOS native gameplay/performance | BLOCKED / NOT_RUN, ADR-0043 adapters/artifacts/device acceptance | HTML/native desktop/CMP preview is no substitute |
| Gameplay conformance/replay/bot/balance | NOT_APPLICABLE to design tooling; NOT_IMPLEMENTED gameplay | required by C01–C04 after owner gate |

The layout/reference review exposed insufficient height for three economy lines
and an overflowing compact age label at200%. Economy now gets a taller pocket
outside the protected lane; compact enlarged age displays I–VI with the full
accessible label/detail sheet. Tests cover the height bound. Actual CSS font
metrics and event handling remain unverified until a browser is available.

The final pilot uses the existing approved raster multiplicative-identity tint
and `Color::with_alpha`; it adds no palette values. A shared production neutral
raster token remains part of C03's design review rather than expanding the token
schema/mobile adapter in this design slice.

## Aggregate failure attribution

`check-no-game-ids` scans ordinary ignored working directories. The current root
contains the owner's untracked `.worktrees/` plus private receipts/pages, so root
hits are not all contributed source. A temporary source-only copy under `/tmp`
was created from `git archive HEAD`, checked, then overlaid with only changed
tracked files and task source/docs (no art/verification or `.worktrees/`). Both
BASE and changed source report four literals in
`apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/DiscoveryCatalogTest.kt`
at95(three names) and120(one name). This reproduces the
[D01 inherited blocker](../age-war-d01/README.md#inherited-aggregate-blocker-and-check-selection).
No mobile edit, scanner exemption expansion or worktree cleanup is included.

The initial check additionally caught a new direct `smallvec` dev dependency
outside the app allow-list; it was removed. Pilot marker paths now collect into
the existing render type. Explicit fixture pack literals have the existing
line-scoped I-9 marker; they are example inputs, never production dispatch.
The mixer's common-pack filter now reads the bound pack reference. These fixes
were compiled/linted/tested again; failures are not silently labelled PASS.

## Reproduction and remaining review

```sh
cargo build -p tabula-game-client --example age_war_d05_pilot --release
target/release/examples/age_war_d05_pilot hud-catalog --report verification/age-war-d06/catalog-check.json
cargo test -p tabula-game-client --example age_war_d05_pilot
node --test --test-isolation=none games/age-war/design-tools/tests/*.test.mjs
python3 -m unittest discover -s games/age-war/design-tools/tests -p 'test_*.py' -v
python3 games/age-war/design-tools/hud-preview/stage_preview.py \
  --vfx-zip PRIVATE/<VFX auxiliary archive>.zip --out verification/age-war-d06
python3 -m http.server 8124 --bind 127.0.0.1 --directory verification/age-war-d06/site
just check
```

Open the site at `http://127.0.0.1:8124/?age=Arcane&size=844x390&font=2`.
Toolbar chooses age/state/size/font/effects/speed and starts motion explicitly.
All displayed HP/resources/queues/outcomes are named fixtures; controls log
proposed intents with `authority:false`, without running a JS combat reducer.
Review all states listed in `states.mjs`, not only the default scene. Capture
actual browser screenshots/video and input/focus/AT receipts separately.

Owner approval must name reviewer/date/commit/rules/art versions and exceptions.
Base art, full turret states, production HUD icons, SourceQA, motion/listening/
rights and COMPATIBILITY's authority/clock decision remain open. C01–C04 handoff
and their required tests are linked in the HUD contract; no issue is closed here.
