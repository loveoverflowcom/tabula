# GDD · 0.1.0-d01

**DRAFT · UNBALANCED · owner approval PENDING.** All numeric choices are original
Tabula proposals, not extracted Age of War statistics. Research provenance is
in [RESEARCH](RESEARCH.md); exact semantics are in [RULES](RULES.md).

## 1. Promise and decision loop

A compact fantasy–technology army duel: read the opposing formation, keep a
front line, exploit a counter and pick a costly moment to evolve. Battles move
continuously. The player controls recruitment, investments and spells, not each
unit's walking/targeting. The rewarding decisions are **composition, tempo,
resource allocation and timing**. The mathematics explains these decisions;
the game does not ask arithmetic questions to permit a purchase.

One lane and two bases deliberately remove RTS pathfinding/micro overhead.
There is no second lane, manual unit movement, target-click micro, fog,
invisibility, polymorph, online PvP or shared platform currency. Match gold is
an isolated rulebook resource with no account, cash or cross-game value.

Human versus a disclosed Easy/Normal/Hard bot is the first product. Both sides
have identical content, gold/income, HP, caps, cooldowns, legality and public
information. Difficulty changes decision quality and cadence, never hidden
economy/health bonuses. See [QA plan](QA-PLAN.md).

Proposed median playtime is 8–12 minutes; hard end is 20 minutes. These are
targets to measure, not performance/balance results. Age 6 is an available
strategic destination, not a mandatory ending. Content review additionally
uses explicit per-age fixture starts so that a 10-minute win does not prevent
accepting ages 5/6.

## 2. Six ages, one world

| Age | Identity | Distinct tactical question to test |
|---|---|---|
| 1 Hoang sơ | Stone, timber, fire, beasts | Can an announced charge beat a light ranged line before capped medicine stabilizes it? |
| 2 Cổ đại | Bronze, sandstone, banners, engines | Is guard/volley attractive, and does shred plus a protected siege engine punish it? |
| 3 Phong kiến | Steel castles, cavalry, field medicine | Is slow heavy armor worth population, versus penetration and timed cavalry pressure? |
| 4 Huyền thuật | Snow forest, crystal, rune magic | Does arcane damage plus root/cleanse change the frontline, without permanent crowd control? |
| 5 Công nghiệp | Brass, steam, firearms | Do burst/reload and telegraphed artillery produce real attack windows for an enemy charge? |
| 6 Tương lai | Alloy, plasma, bounded drones | Do heat, pierce, finite shields and temporary drones change composition rather than multiply every stat? |

Each has six original units: frontline, ranged, tank, skirmisher, support and
siege. Their role names teach a vocabulary; costs, ranges, mitigation,
population, timing and skill bindings produce the differences. A skirmisher
does not teleport through a blocking front line. The 36-unit starting catalog
and individual exceptions live in [CONTENT](CONTENT.md), not a renderer.

Current and immediately preceding age remain recruitable. Older living units
and previously installed turrets survive. This makes evolving a trade-off:
new counters become available while a paid existing formation keeps its value.

Art target from #118 is 60–70% of humanoid roster presented as female, distributed
across armor/melee/ranged/cavalry/magic/engineering roles. The schema's
presentation hints are only D02 input; gender never changes combat statistics.
D02 must count its final humanoid denominator, review silhouette variety and
confirm the proportion. No character design or likeness is approved by D01.

## 3. Content boundaries

- 36 normal recruitable units; 16 reusable skill families
- Two commander spells per age, 12 designs; only two current-age slots shown
- Two turret branches per age, 12 designs; two physical sockets per base
- Two non-stackable researched stat technologies per age plus five age advances
- Two bounded summoned drone definitions, neither a normal recruitment button
- No skill-specific renderer logic, free-form physics engine or duplicated
  per-unit behavior tree

Skill cards must bind trigger, target/filter/priority, windup/contact/recovery,
range/AoE, costs/cooldown/duration, stacking/expiry, interruption, cleanse and
cue keys to an oracle. CONTENT is the readable dictionary; Rust catalog data
is the machine-readable baseline. The cards are reusable mechanisms with
original tuning; they are not proof that the reference games use those rules.

## 4. Economy and pacing brief

Baseline: 180 opening gold, 5 gold/s passive income, 24 population including
reserved recruits, one FIFO recruitment queue of five, one research slot and
two turret sockets. Buying a large siege/support piece delays another unit or
an age investment; population and training time prevent converting all saved
gold into an instantaneous army. Normal enemy deaths return only 15% of their
paid purchase cost, bounding deliberate feeding. Tech XP comes from active
logical time, not kills, so losing a battle does not also lock progression.

Age advance costs 250/400/600/850/1150 gold, requires total XP
60/140/240/360/500 and occupies research for five seconds. Gold alone has a
650-second opportunity cost for all five advances before purchases/bounty.
This is an explicit pacing risk to investigate; it is not a claim that every
normal match naturally displays all ages. Start-at-age tests cover the full
content while self-play and humans decide whether the curve is too long.

Refunds and age transitions have finite, written rules. There is no free base
heal, cooldown reset, conversion of old units into stronger ones, summon bounty
farm or full refund after deployment. Anti-stall and timeout are public rules
applied identically to both sides.

## 5. Presentation/asset handoff, not asset production

The three [owner references](https://github.com/loveoverflowcom/tabula/issues/118#issuecomment-6057763344)
are key art/cinematic concepts. Their large portraits, effects, bridge and
background density do not establish actual gameplay, collision, sprite anchors
or redistribution rights. This PR copies none of those images into runtime
assets and grants no license.

For D02–D06:

1. Smooth readable 2D, depth/material weight and original characters; female
   characters across tactical roles. Approve gameplay-scale silhouettes before
   multiplying the roster.
2. Separate background/parallax/ground/props, six bases, sockets/turrets, unit
   sprites, cues/VFX, SFX and HUD. Every animation has attack/cast anchors and
   idle/move/attack/cast/hit/death coverage, right/left orientation checks and
   interruption behavior.
3. Telegraphs must identify target area, impact deadline and affected status
   without relying only on color or sound. Reduce motion/audio modes change
   presentation only; authority still applies the same hit/cooldown.
4. Mobile landscape and desktop: six current + six preceding-age recruits
   separated by an age switch, two spell buttons, two sockets, one research
   panel, population/queue/base HP. Never show 36 recruitment buttons together.
   Min-range, counter role, cooldown and missing-cost reasons have tooltips and
   keyboard/a11y equivalents.
5. M3 Expressive semantic tokens belong to shell/HUD; the battlefield must remain
   a legible game scene. `present(View)` emits RenderList; Macroquad does not
   change HP, damage, queues, phase or outcomes.
6. Cue keys in data are obligations for D05, not present assets. Store editable
   source/export/rights manifests in game-owned packs. Lazy-load the selected
   current/adjacent age, do not preload six ages at the Tabula dashboard.

Web/desktop are the first gameplay acceptance targets after the gate. WASM
compilation and CMP desktop previews do not prove native Android/iOS. Native
hosting remains a separate dependency; no mobile WebView fallback is proposed.

## 6. Ownership and next step

Rules/data stay in the pure Rust game owner. SDK, ordering, projection dispatch,
timer mechanism, pack delivery and lifecycle retain existing owners. The
compile-only D01 crate implements no runtime traits and is not in any catalog.
The proposed 20 Hz simulation requires the narrow decision in
[COMPATIBILITY](COMPATIBILITY.md); determinism does not imply board-runtime
compatibility. No ADR register or invariant is silently changed.

The next deliverable is D02 art direction, not C01 gameplay. The following
finite review questions need explicit owner decisions against version
`0.1.0-d01`; a merged design draft is not automatically an approval:

| ID | Decision requested | Proposed baseline | Status |
|---|---|---|---|
| Q1 | Scope and content count | Six ages × six units, 16 families, two spells/turrets per age | PENDING |
| Q2 | Pacing/economy | 8–12 min target, 20 min maximum, XP/age-cost curve in RULES | PENDING, UNBALANCED |
| Q3 | Recruitment across ages | Current + previous age; all living old units persist | PENDING |
| Q4 | Skill budgets/control | Lifetime heal/shield caps, root/immunity, drone caps and no hidden micro | PENDING |
| Q5 | Fairness/end rules | Same AI resources, symmetric fatigue, simultaneous-base draw and exact timeout score | PENDING |
| Q6 | Authority exception | Offline-only PVE, four exact Timer inputs per host pump, no new batch ABI | PENDING architecture review |
| Q7 | D02 visual brief | Original smooth fantasy–technology art, female humanoid target 60–70%, licenses required | PENDING D02 |
| Q8 | Platforms and execution gate | Web/desktop first, native mobile separately blocked; D01–D06 approval before C01 | PENDING |

Any accepted change gets an explicit versioned decision and updates catalog,
rules and oracles together. Balance sweeps and performance/human acceptance
remain NOT_RUN/NOT_IMPLEMENTED as described in QA and verification.
