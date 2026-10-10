# Age War D05 — runtime-verifiable presentation set

Date: 2026-10-10 UTC. Issue [#123](https://github.com/loveoverflowcom/tabula/issues/123),
roadmap [#118](https://github.com/loveoverflowcom/tabula/issues/118).
Source base: `develop@74589b491b0e6fec6f55ebc4cb2a0bbacf10ad85`.
**Animation/design presentation work. Owner art, motion, audio and rights review PENDING.
No combat rule, `GameRules`, registry entry or gameplay is implemented. C01 stays blocked by D06.**

This ledger records what was executed against the owner's private D05 delivery
(reconstructed export v05). The delivery archives, their index, rig/provenance
records, Drive locations and every generated pack or image stay outside Git
(`verification/` and `target/` are ignored). Only code, schemas and aggregate
results are public.

## What was delivered here

| Output | Location | Public? |
|---|---|---|
| Input verifier, per-age runtime pack builder, pose sheets, coverage, audio checks, pilot matrix | [`games/age-war/design-tools/runtime`](../../../games/age-war/design-tools/runtime) | yes (code only) |
| Generic external-source pack sealing | `cargo xtask pack-assets --external` ([pack_assets_cmd.rs](../../../xtask/src/pack_assets_cmd.rs)) | yes |
| Presentation pilot through the existing asset/`RenderList`/Macroquad path | [`apps/game-client/examples/age_war_d05_pilot`](../../../apps/game-client/examples/age_war_d05_pilot) | yes (code only) |
| Seven sealed packs: `age-war-{primitive,ancient,feudal,arcane,industrial,future}@0.5.0`, `age-war-common@0.5.0` | `target/age-war-d05/packs` | **no** |
| Runtime manifest (unit/skill → clips/VFX/SFX, sockets, markers, timing), coverage CSVs, reports, pose sheets, pilot captures | `verification/age-war-d05/evidence` | **no** |

The pack game id `com.tabula.agewar` is provisional: `GameId` has no `-`, and
nothing registers or lists the game. Runtime resources are logical `AssetRef`s:
`unit/<unit>/<facing>/<clip>/<NNN>`, `mask/...`, `vfx/<recipe>/<team>/<phase>/<n>`,
`sfx/<id>`, `scene/backdrop`, `meta/clips`, `meta/vfx`, `meta/sfx`.

## Inputs and SourceQA

| Item | Status | Evidence / residual |
|---|---|---|
| Delivery index current files (27: contracts, audio, VFX, 6 preview inputs, 6 runtime candidates, 12 preview videos) | PASS | exact bytes + SHA-256 vs the index; ZIP CRC; every self-declared archive member re-hashed (0 mismatches/missing) |
| Original S02 SourceRig/CommonContract archives (7) | NOT_RUN | not downloaded: rebuilding art was out of scope; the runtime set does not consume them |
| **SourceQA archive** | **MISSING** | index declares `NO_VERIFIED_REMOTE_FILE` (195 local members never uploaded) |
| SourceQA's complete runtime clip index | PASS (reconstructed) | the six per-age clip indexes, concatenated in age order and serialised as JSON (indent 2), reproduce the index-declared SHA-256 exactly: 460 clips / 13,058 frames |
| SourceQA's own QA reports (ground, motion-extended 10,651, independent atlas 365,624, 18 marker + 6 gain-report tests, sampler parity 27,600) | NOT_RUN / unverifiable | reports absent; the overlapping claims were re-proven independently below, the rest stay unverified |

Re-proven independently from the delivered bytes (builder, 179,362 PASS / 4 FAIL):
page and mask SHA-256; frame rectangles in bounds and non-overlapping; exact 2 px
edge extrusion of every colour and mask frame; mask alpha ⊆ colour alpha;
trim/pivot consistency; 24 Hz grid plus exact poses at markers and one-shot ends;
two facings with identical clip sets and frame counts; ground penetration ≤ 1 px
(worst 1 px, matching the delivery's claim); `marker_authority = false`.
The 4 FAIL are `sand_rider` (mounted) run/charge clips in both facings: no ground
contact sockets, so planted-foot speed and slide cannot be measured.

## Runtime packs (density 1 = desktop 1:1 at the D03 stage; density 2 = delivered bake)

| Age | Clips | Frames | Files | Encoded d1 / d2 / other MiB | Decoded d1 / d2 MiB (all 12 unit-facings) | d1 per unit-facing MiB | d1 one side (6 units) MiB |
|---|---:|---:|---:|---|---|---|---:|
| Primitive | 74 | 2,054 | 93 | 20.7 / 47.9 / 1.8 | 76 / 200 | 4.6–8.0 | 38.0 |
| Ancient | 74 | 2,076 | 92 | 26.8 / 48.5 / 1.6 | 104 / 191 | 5.0–13.3 | 52.0 |
| Feudal | 76 | 2,158 | 91 | 22.1 / 42.2 / 1.7 | 89 / 161 | 5.3–14.1 | 44.3 |
| Arcane | 78 | 2,210 | 90 | 23.7 / 42.3 / 1.7 | 91 / 158 | 4.0–14.5 | 45.3 |
| Industrial | 76 | 2,220 | 93 | 23.3 / 34.2 / 1.8 | 113 / 143 | 4.7–16.2 | 56.4 |
| Future | 82 | 2,340 | 93 | 23.3 / 31.8 / 1.8 | 105 / 127 | 4.7–17.0 | 52.5 |

Common pack: 223 files (40 two-team VFX phase sheets, 181 shared SFX, sidecars).
Decoded figures are RGBA8 before mips/driver overhead, not GPU residency.

Design decisions, all presentation-only and reversible:

- **Packed per (unit, facing).** A side shows one facing of each unit, so the
  loader fetches only the groups on the field. Whole-age packing (the delivered
  layout) forces ~200 MiB decoded per age.
- **Density 1** resamples each frame (premultiplied Lanczos, never upsampled) so
  the unit's measured body equals its D01/D04 desktop display height in logical
  units. Density 2 keeps the delivered bake pixel-exact (checked) and is declared
  only where it adds ≥ 1.33 texels/unit; constructs whose bake is smaller than
  display (`k > 1`) keep one density and are drawn upscaled.
- **Team paint** uses a trimmed mask atlas whose RGB is the cloth luminance and
  whose alpha is `min(mask, colour)`; a team tint multiplies it. Colliders and
  D01 footprints are unchanged by any sprite size.
- Each age pack carries the delivered VFX sheets its units and spells can cue
  (17–22 sheets), because the renderer binds one pack at a time (gap G1 below).

## Pilot and oracle checks

Commands under [reproduction](#reproduction). Windowed runs used `xvfb-run`
with Mesa software GL; frame times are **not** device performance.

| Claim | Status | Evidence |
|---|---|---|
| Every pack file passes manifest parse, binding and size+BLAKE3 through `tabula-assets` | PASS | pilot `check`: 6 age packs + common; ~35–48 ms verify per age pack |
| Clip timing equals the compiled D01 catalog | PASS | 1,102 checks, 0 failures: attack windup/period and marker kind (`contact` for melee, `release` for projectiles) for 72 unit-facings; 45 timed skills × 2 facings windup/period/D01 contact cue key; 24 passive skills reuse attack; roster, roles, hints, skill bindings |
| Walk/attack/skill/hit/death in two facings render through `RenderList` + Macroquad | PASS (Xvfb) | six lineup runs, both teams facing each other; pose sheets per age at 1× and 2× |
| Speed independence 0.5×/1×/2× | PASS | battlefield pixels identical to 1× at 6 shared presentation times; cue counts equal |
| Effects full/low/reduced change visuals only | PASS | identical cue streams (attack/hit/skill/spell) in lineup and 100-actor runs |
| Planted-foot consistency (socket level) | PASS / PARTIAL | slide ≤ 0.021 units for all walk/run clips except `sand_rider` (NOT_VERIFIABLE); pixel-level foot plant not measured |
| Real play size | PASS (Xvfb) | 844×390, 1024×768, 1280×720, 1920×1080 logical; ~40-unit bodies at 844×390 |
| Crowd | PASS (Xvfb) | 48 actors at the D01 population cap in one file of touching D01 footprints (CPU p50/p95 2.3/3.8 ms); 100-actor beyond-cap stress (5.2/7.4 ms, 234 sprites) |
| Load / unload | PASS | lineup (12 unit-facings + VFX + scene): ~42 files, ~0.37 s verify+decode+upload, 117 MiB resident; release → 0 resident in all 12 cycle steps; RSS bounded (peak 303 MiB d1, 412 MiB d2) with no growth across six ages |
| High-DPI density | PASS (selection only) | DPI 2 and phone 844×390@3 select density 2/3→2: 213 MiB resident, ~0.67 s; Xvfb lacks a high-DPI framebuffer, so those pixels are not a device render |
| Audio numeric | PASS | 305/305 WAV format/count/peak/DC; 305/305 OGG decode within 25 ms of WAV, loops sample-exact; 23/23 loop seams ≤ 1.25× the 99th-percentile step (worst 1.03) |
| Audio voice limits | PASS (simulation) | public scheduler over real pilot cue logs: 100-actor stress 420 cues → 167 accepted, 253 coalesced, peak 16 voices (cap 24) |
| Audio playback path | PASS (silent) | pilot `--audio --audio-volume 0`: 52 verified sounds decoded by Macroquad, loops started/stopped |
| 12 delivered preview videos | PASS | public `media_probe`: 132 checks; dense-video dimensions/duration are self-referenced |
| Public design-tool tests | PASS | Node 18; Python 19 (11 original + 8 runtime-tool tests) |
| Listening / audition | NOT_RUN | no person listened; run the pilot with `--audio` |
| Motion, art, VFX readability, rights | PARTIAL / PENDING | owner review; see findings |
| Browser, Android/iOS native, GPU residency, device FPS | NOT_RUN | not in this scope; mobile native GameHost remains blocked (ADR-0043) |

## Findings for D06 / C03

| ID | Finding | Consequence |
|---|---|---|
| G1 | `SpriteAssetCache::bind_pack` holds one manifest; rebinding retires textures | current + previous age + common cannot be resident together; C03 needs multi-pack residency or a per-match composed pack |
| G2 | The renderer picks texture density from DPI (`density_for_dpi`), not from on-screen texel need | a phone (DPI ~3) on a 0.44× stage requests the 1.6–1.9 texel/unit bake: ~2× memory for no visible gain; needs a density policy input |
| G3 | Baked clips switch with a hard cut | the D05 preview's 120 ms rig blends are not reproducible from sprites; accept cuts or add short cross-fades |
| G4 | `AudioSink` is one-shot; Macroquad `stop_sound` has no fade | the 120 ms loop cancel and 12 ms steal fades of the D05 mix policy cannot be expressed |
| G5 | Team cloth tint is barely visible at play size | side identity relies on the shape-coded ground marker (circle/diamond) and facing |
| G6 | Delivered VFX contact bursts are thin and faint at play size | owner readability review; the 2 units-per-texel VFX scale is a pilot assumption |
| G7 | Hit reactions of several constructs barely differ from idle; `sand_rider` has no ground contacts | motion review items, not runtime defects |
| G8 | RSS is not returned to the OS after release (allocator/driver), although residency counters reach 0 | budgets must use residency accounting plus a measured process ceiling |

## Reproduction

Private inputs: download the delivery index and the files it lists into one
private directory (outside Git). Python 3.12+ with Pillow and NumPy, FFmpeg,
Rust 1.96, `xvfb-run`.

```sh
python3 games/age-war/design-tools/runtime/verify_inputs.py \
  --index PRIVATE/index.json --inputs PRIVATE --out verification/age-war-d05/evidence/input-verification.json
python3 games/age-war/design-tools/runtime/build_runtime_packs.py \
  --index PRIVATE/index.json --inputs PRIVATE --stage target/age-war-d05/stage \
  --evidence verification/age-war-d05/evidence --seal target/age-war-d05/packs
cargo build -p tabula-game-client --example age_war_d05_pilot --release
python3 games/age-war/design-tools/runtime/pilot_matrix.py --out verification/age-war-d05/evidence/pilot
python3 games/age-war/design-tools/runtime/clip_sheets.py --packs target/age-war-d05/packs --out verification/age-war-d05/evidence/poses
python3 games/age-war/design-tools/runtime/audio_checks.py --audio-zip PRIVATE/<OriginalAudio archive> \
  --pilot verification/age-war-d05/evidence/pilot --out verification/age-war-d05/evidence/audio-checks.json
python3 games/age-war/design-tools/runtime/coverage_matrix.py --evidence verification/age-war-d05/evidence
```

Owner listening session (plays through the default output device):

```sh
cargo run -p tabula-game-client --example age_war_d05_pilot --release -- \
  run --packs target/age-war-d05/packs --age Arcane --audio --audio-volume 0.6 --frames 1200
```

The builder decodes one delivered page at a time (peak RSS 428 MiB for all six
ages); the ~2.375 GiB decoded colour+mask bake is never loaded together.
Rerunning produces the same pack plan from the same inputs; PNG encoding depends
on the Pillow version, so pack hashes are pinned per toolchain, not globally.
