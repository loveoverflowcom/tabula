# RULES · 0.1.0-d01

**DRAFT / original Tabula proposal / UNBALANCED.** This is a specified rulebook,
not an implemented reducer. [CONTENT](CONTENT.md) owns individual baseline
unit/card/turret/tech values. Architecture doc 00 continues to win. The clock
and local-authority exception need [owner review](COMPATIBILITY.md).

## R0. Units, numeric domains and state ownership

Canonical logical quantum is 50 ms: 20 ticks/s. All due times are integer tick
numbers. Coordinates use integer q, 1000 q = one battlefield world unit;
velocities are q/tick. Percentages use basis points (bp), 10000 bp = 100%.
HP, shield, damage, gold, population and XP are integers. Products use checked
widened intermediates; overflow rejects a command/config before mutation.
Floats, render frame delta, animations, wall clock, OS randomness and unordered
iteration have no role in canonical outcomes (I-1–I-4, I-10/I-11).

Definitions and constants in this rulebook are normative **within the draft**:

| Quantity | Baseline / bound |
|---|---|
| Field, facing base edges | x ∈ [0,100000] q; left face 5000, right face 95000 |
| Recruitment centers | 7000 / 93000 q, adjusted inward only enough to fit footprint |
| Base HP / type mitigation | 3000 HP; physical 2000 bp, arcane 1000 bp, siege 0 bp |
| Gold / population / queue | 180 start; 5/s income; gold cap 20000; pop cap 24; queue 5 |
| Cumulative tech XP | 1 per complete active logical second; cap 1200; never spent |
| Normal enemy bounty | floor(paid purchase gold × 1500 / 10000) |
| Research | One slot, concurrent with recruitment; age advance 100 ticks |
| Turret sockets | Two per base; construction/refit occupies that socket |
| Heal budget / shield grants | Per living normal-unit lifetime: 35% / 50% of spawn max HP |
| Slow / root / knockback | Slow reduction ≤4000 bp; root ≤12 ticks; displacement ≤2000 q |
| Control immunity | Root/knockback ends ⇒ 40 ticks of shared control immunity |
| Summons | Two drones/cast, 240-tick life, 4/owner and 8/match, 1 pop each |
| Projectile/status/event bounds | ≤128 in-flight shots/match, ≤16 status instances/entity and ≤512 pending semantic deadlines/match |
| Game time | Median target 8–12 min (not measured); fatigue starts 12 min; hard limit 20 min |

Normal recruit population and drone population share the 24 cap. Queued units
reserve population on accepted purchase. Structures use sockets, not population.
No basic attack has random hit/miss/crit; all randomness, if a future accepted
rule adds it, must use named seeded DetRng domains and bump rules identity.

Future State must contain only rule-owned facts: tick, phase/outcome, players'
resources/queues/research, entities with immutable spawn definition/version,
positions/HP/statuses/skill deadlines, persistent commander slot deadlines,
pending impacts/telegraphs and deterministic IDs. Rendering/camera/interpolation
remain Local presentation. Use Tabula `SeatId`, not a second player-index scheme.
Entity identity is `(owner SeatId, owner-local monotonic spawn ordinal)`;
research/queue/cast/impact IDs also monotonically increase without reuse.

Information model: both sides' gold, XP, queues/research, spell deadlines and
visible combat facts are public in distinct View types. Seed/internal heap and
future AI decisions are excluded. See [the complete model](../age-war.md#rules-and-information-model);
public gameplay does not permit canonical State to cross I-5/I-6.

## R1. Ordered semantic input and tick pipeline

Player/bot commands are Recruit, CancelRecruit, Research, AdvanceAge,
Build/Refit/SellTurret, Cast(slot, targetQ), Resign. They are semantic intents,
not movement/frame updates. No client command can advance time, edit HP or
choose a hidden hit result. At acceptance validate actor, phase, IDs/version,
all costs/caps/coordinates/targets and every dependent condition; only then
commit reservations. Rejection is a total no-op, including State/context/RNG
obligations (R2/R8). Duplicate transport requests use platform idempotency,
not another side channel.

Accepted intents carry authoritative input order and enter the next tick:
`effective_tick = last_completed_tick + 1`. No retroactive use of an old
telegraph/cooldown. Two commands at the same logical timestamp retain platform
input order; a timer already logged first remains first. The host must not
catch up past an already accepted command's effective tick. If a cooldown
expires next tick but not now, a current Cast is rejected, not secretly queued.

One scheduled Tick Timer represents exactly one 50-ms step. A host pump may
dispatch at most four such existing inputs; it does not skip/internal-merge
steps or turn four successes into one state version. A timer ID is game-scoped;
canonical state keeps its expected tick/deadline and generation. Duplicate,
stale, premature, wrong-phase timers are rejected without mutation. See
COMPATIBILITY for current SDK gaps rather than assuming this contract exists.

Within a tick use these phases, preserving stable keys for every list:

1. Expire statuses whose `expiry_tick ≤ t`, release expired drone pop and mark
   hard-limit/fatigue due facts. Dead-at-start entities do not act. Capture the
   living-at-start actor set; newly spawned entities cannot attack this tick.
2. Complete research/age changes and paid queue/construction entries; apply
   accepted semantic commands in their already reserved order. New spells/tech
   are usable from the following tick. Deployment must pass footprint checks.
3. Resolve due cleanse, control and buff applications from living-at-start
   actors/committed impacts. Cleanse precedes a new hostile status in this tick.
   Apply root/knockback eligibility and interruptions; then solve movement.
4. Choose/validate attack and autocast targets from the post-movement snapshot;
   begin allowed windups and complete already due windups. Launch homing basic
   shots/ground telegraphs; collect melee, due projectile, DoT and skill hit
   intents. A due contact from an interrupted windup is excluded.
5. Aggregate eligible capped heal/shield grants, then damage per target from
   this common snapshot. No entity-order mutation may alter another actor's
   same-tick eligibility. Consume shields and assign final HP simultaneously.
6. Remove all newly dead entities together, release population, interrupt
   remaining windups and settle at most one death reward per normal unit.
   Already airborne committed shots persist; a targeted dead entity fizzles.
7. Test both bases before every other end rule. Emit one terminal outcome; then,
   if still live, grant this tick's passive income/XP, evaluate timeout, finish
   checkpoint/projection facts and schedule the next exact tick.

If two living-at-start units each complete lethal contacts at t, both contacts
count and both die. There is no lower-ID “kill first, cancel opponent” advantage.
An earlier tick's death cancels its later windup. An airborne bombard remains
after caster death. Dead or unspawned entities cannot create a fresh cast.
This pipeline is an independent oracle specification; D01 does not implement it.

## R2. Collision, reach and targets

Commander Cast points are canonical positions within `[5000,95000]` q and
cards have 100000-q range from their own base facing edge, covering the lane.
The selected point anchors the card's area; eligible target footprints must
intersect it. Zero-radius individual cards require a containing footprint.
Within that area apply the card's normal target filter/priority/max-target cap.
An empty/forbidden cast target rejects before cost/cooldown; ground telegraphs
may target an empty area by their explicit GroundPoint contract. Ground summon
requires a non-overlapping legal footprint as well as cap/pop reservations,
does not place drones beyond an enemy blocker or teleport through a formation.

The lane is a one-dimensional ordered formation. Each normal unit/drone has a
center and a half-width from data. Occupied footprints may touch but never
overlap or pass through a living ally/enemy; no multi-lane flank or hidden
teleport. Corpses are visual and cease blocking after the death phase.

Movement is toward the enemy base only, except a legal knockback. Solve the
two opposing front bodies together, then followers front-to-back clamped by
the newly solved ally ahead. If opposing advance would cross, allocate the
remaining nonnegative gap proportionally to their desired velocities using
integer division. A remaining single q goes to left on even ticks/right on odd
ticks, never to a zero-velocity side; this is at most 1 q and is side-swapped
in mirror oracles. Stationary windup/root actors have zero desired velocity.
Allies cannot overtake; a wider follower stops at its required footprint gap.
Spawn waits, with pop already reserved, if its footprint is blocked. Age
advance does not teleport, reorder or shrink a formation.

Distance means nonnegative gap between nearest occupied surfaces, not sprite
pixels. Basic acquisition includes the nearest enemy normal unit/drone in
forward order; target candidate must be inside reach and outside min-reach.
Tie key: `(surface distance, target owner-local ordinal)`; IDs are stable,
not container iteration. Normal attacks prefer a reachable lane enemy before
structures. If none is in reach, move toward the front without crossing it.

When no lane enemy is legally reachable, structures in reach can be attacked:
turret sockets first by `(distance, socket index)`, then the base. Siege-family
cards may prefer structures, but cannot bypass an occupied enemy footprint to
walk there. Two turret sockets are attached at the base facing edge; they do
not create moving lane bodies. Their individual HP is targetable, and zero-HP
turrets stop firing after the simultaneous death phase. A base death removes
its sockets and ends the match even if turret HP remains.

Support filters differ explicitly: Heal chooses damaged normal ally with the
lowest `current_hp/max_hp` ratio, compared by cross-multiplication, then nearest,
then ordinal; it skips full HP or exhausted lifetime budgets. Shield chooses a
normal ally with incoming committed threat, then lowest HP ratio and same ties.
Cleanse prefers root, then slow, then hostile DoT/shred; ties oldest application,
then status ID. Summons/structures are never healing/shield targets. Offensive
AoE/chain candidates sort by distance from impact/previous hop, then ordinal,
and stop at their card target cap. No arbitrary “all enemies” iteration.

Each unit has **one** action channel for basic attack or an autocast, with one
windup/recovery and one committed target. They never run concurrently to create
two attacks in one animation. Passive OnAttackContact modifiers attach to that
one basic hit; they do not launch another action. When idle, choose the first
eligible ready autocast in this family priority: Dispel, Heal, Shield, Root,
Slow, Knockback, Summon, Bombard, Chain, Volley, BurstHeat, Charge, Guard; same
family ties card ID. If none is eligible, choose basic attack. A started
windup/recovery completes or is explicitly interrupted, never freely replaced.
Cooldowns remain independent absolute deadlines, so filling an action channel
does not reset another skill. Guard's duration buff is not a second attack.

Basic attack starts only with a legal target and ready cycle. Attack data gives
windup w and recovery r; period is `w+r`. Movement stops in windup/recovery unless
the card explicitly says mobile. On melee contact recheck target alive and
reach/min-reach; otherwise whiff with cycle spent. There is no free retarget.

## R3. Projectiles and impact commitment

A basic ranged projectile commits target entity ID, damage/modifier snapshot,
launch position and arrival tick at release:
`arrival = release + ceil(surface_distance_at_release / speed_q_per_tick)`.
At least one travel tick for a ranged shot. Basic shots home to that committed
living target; moving away does not dodge them. The trajectory and arrow curve
are presentation only. If the entity dies/despawns, the shot fizzles without
retargeting. It cannot hit a unit “behind” because the visual sprite crossed it.
This limited abstraction avoids adding a physics engine.

Bombard cards instead commit a ground center, radius, cap and announced impact
tick. They do not home. At impact choose living enemies whose surfaces intersect
the closed area `[center-radius, center+radius]`; movement/control can change
occupancy. Edge equality counts. Splash hits each chosen target once; ties and
target cap are stable. Friendly fire is absent in this version. Chain never
revisits a target, attenuates damage separately at each hop and stops when no
eligible new target is in hop range. Volley has an explicit cap, never one shot
per enemy in an unbounded cluster.

Pending impacts/status expiry are sorted by `(due_tick, phase, cast_id,
subevent_ordinal)`. A later accepted operation cannot relabel an old impact as
new-age damage or change its snapshot. Damage sent before tech research still
uses the old attack modifier; new attacks use the new one.

Bounds are real rules, not a performance-dependent random drop: validate a
player cast's full maximum reservations before taking gold/cooldown. An
autocast with insufficient summon/projectile/event capacity waits and spends
nothing. A basic attack with no shot reservation waits before windup; once
accepted its reserved slot cannot vanish. No silent overflow, erased impact
or changing target cap based on FPS. C01 must account conservatively for event
reservations; the exact storage mechanism remains unimplemented.

## R4. Damage and modifier order

Three damage kinds: Physical, Arcane, Siege. Mitigation is kind-specific and
capped at 8000 bp. “Tank” is not immunity, and arcane is not unconditionally true
damage. A siege attack against a base has 15000 bp structure multiplier;
other attacks default 10000 bp unless a card specifies an action multiplier.
No hidden role rock–paper–scissors multiplier is added: counters arise from
visible timing/range/armor/pop/cost and skills.

For an individual committed positive hit, floor at **each** division:

1. `d1 = floor(base_damage × (10000 + attack_tech_bp) / 10000)`; aggregate
   researched attack bonus cap is 3000 bp.
2. `d2 = floor(d1 × action_multiplier_bp / 10000)`; action multiplier domain
   is 5000–20000 bp. Chain's declining packet damage is computed before this
   pipeline, not by abusing the multiplier minimum. An active guard's outgoing
   penalty also scales this action multiplier: floor(card multiplier ×
   (10000−outgoing_penalty_bp)/10000), then validate the final multiplier.
3. `m = clamp(kind_armor_bp + defense_tech_bp - active_shred_bp, 0, 8000)`;
   defense tech aggregate cap is 1500 bp, strongest shred only.
4. `m_after_pierce = floor(m × (10000 - pierce_bp) / 10000)`.
5. `d3 = floor(d2 × (10000 - m_after_pierce) / 10000)`.
6. `d4 = floor(d3 × (10000 - eligible_guard_bp) / 10000)`. Guard has two finite
   hit charges and reduces direct Physical/Arcane only, never DoT or Siege.
7. `hit_damage = max(1, d4)` for a positive committed packet. Miss/fizzle is
   an absent packet, never converted to 1 damage. Zero base damage remains 0.

Attack tech buffs normal unit basic attacks, not healing, DoT already applied,
spells, drones, turrets or base stats. Defense tech applies to normal-unit
mitigation, not base/turret/drone stats. Source-less fatigue is true base loss.

When guard can absorb only K contacts, choose the K largest eligible `d3`
packets this tick; equal damage ties `(cast_id, subevent_ordinal)`. Do not let
arbitrary processing order spend guard on a tiny first hit. Shield is a generic
pool: after per-hit mitigation/guard, sum packets with checked arithmetic,
absorb `min(pool,total_damage)`, then apply the remainder to HP. Shield does not
earn heal budget and expires independently.

Heal/shield grants this tick are sorted by stable cast ID, bounded by remaining
lifetime grant budgets, then current HP deficit/shield cap. Every granted HP
or shield point counts; refreshing does not restore budgets. Shield cap at one
time is 25% of spawn max HP, lifetime grants 50%. Heal lifetime grants 35%.
Over-heal or overflow shield is not granted/countable; a cast with no eligible
target does not start. Grants resolve before this tick's damage, so a shield
already due can save an ally; a caster killed this tick can finish an already
due eligible heal. A unit dead before the tick cannot be resurrected.

Final HP is `max(0, min(max_hp, hp + allowed_heal) - unshielded_damage)` for all
targets simultaneously. Overkill is wasted; it does not spill to the next
body/base. Death events, bounty and population release occur once, at R1 phase 6.

## R5. Skills, status and anti-loop limits

Full cards are [CONTENT](CONTENT.md); these shared laws constrain every family:

- Passive triggers consume declared budgets. Autocast checks a complete legal
  target/capacity before beginning. Player Cast reserves gold and updates the
  persistent commander slot deadline on acceptance. Skill cost/cooldown are
  consumed on start, not only on success; interruption/whiff refunds neither.
- Cooldown readiness is an absolute tick (`t >= ready_at`). Every accepted
  cast uses the captured card/version; later age/research does not recalculate it.
  Two commander slots persist across all six ages. Changing the offered card
  retains `ready_at`; newly offered spells cannot reset an older cooldown.
- Status lifetime is half-open `[apply_tick, expiry_tick)`. Expiry phase runs
  before any contact at expiry. DoT first pulses after its interval and never
  at expiry. Reapply one same-family DoT per source: take strongest packet,
  refresh to later expiry, preserve the next pulse (no immediate reset hit).
  At most three hostile DoT sources/target; overflow replaces the weakest only
  if stronger, with stable oldest/source-ID ties. Poison/burn are the same
  family with different cue keys, not two new stacking loopholes.
- Shred/slow: strongest magnitude only; refresh later expiry without addition.
  Slow leaves at least 60% of base desired move speed; floor to integer q/tick,
  then apply collision clamps. Slow changes movement, not attack cooldown.
- Root prevents movement and interrupts charge, not ordinary basic/heal
  windup. Root ≤12 ticks. Root and knockback share control immunity: while
  controlled, an extra root/knockback cannot extend it; at ending/cleanse set
  immunity through `end_tick + 40`. Buff expiration/age does not erase immunity.
  A resisted card still spends its attack/cast budget and cues “resisted”.
- Knockback interrupts attack/autocast windup, displaces ≤2000 q away from
  source, clamps against own base/ally footprint and never crosses a body.
  Apply once per target/tick, strongest displacement; root-only movement block
  does not suppress a valid first knockback. Immunity prevents repeated locks.
- Cleanse removes only card-listed hostile statuses and their future pulses,
  not airborne projectiles, immunity, positive buffs, lifetime heal/shield
  budgets or cooldowns. D01 has defensive ally-negative cleanse only; it cannot
  remove an enemy shield. No buff stealing. Priority is R2's explicit order.
  Shield expiry/depletion never restores grant budget.
- Guard finite charges, shield finite grant budgets, summons finite life and
  control immunity prevent infinite defense/chain-stun. Burst heat cannot be
  reset by age/cleanse. In burst_heat, data defines shots, intervals, heat
  per burst, heat cap and recovery/cooling; all shots are reserved before the
  burst. A cooling unit cannot bypass reload by changing target.
- Summon requires two free drone-cap slots and two free population points;
  else autocast waits / player Cast rejects transactionally. Exactly two drone
  entities are created, each costing one population, lasting 240 ticks. Drone
  data is fixed, attack-only; drones cannot summon/heal/shield/earn tech XP or
  receive healing/shields. Killed or expired drones yield zero bounty. No
  summon consumes/refunds a normal-unit recruitment purchase.

Telegraphs must have stable cue ID, position, start/due tick and cancellation
reason. An animation's last frame neither causes nor delays a hit. Required
attack/cast/hit/status/immune/death cues are D05 obligations, not assets in D01.

## R6. Economy, population, recruitment and refunds

Income and XP accrue only for complete active logical seconds. Use accumulated
tick remainder: every 20 active ticks grants 5 gold and 1 total XP, independent
of renderer/host pumps. A player must never gain twice because they pause,
restart a frame or change age. Gold cap 20000/XP cap1200 deliberately clips
overflow income; no hidden resource debt.

Recruitment is one FIFO of at most five per side. Purchase pays current-age or
previous-age unit cost and reserves population immediately. Immutable queue
entry includes purchase ID, unit definition/version, paid gold, reserved pop,
remaining training ticks. Only the head trains. Completion waits for a free
spawn footprint with no extra charge; completed blocked head is still a head.
Deploy at most one unit per tick, with a 4-tick barracks deployment cooldown.
New deployment moves next tick and cannot attack in its spawn tick.

Cancel a not-yet-head entry: return 100% of its paid gold and release reserved
population. Cancel head whose training started, including blocked completion:
floor(75% paid gold), release reserved population. No deployed unit sale or
refund; cancellation cannot earn XP, bounty or restore spell/research time.
Cancelled entries never later complete; old IDs remain retired. An entry
becomes head at tick processing, so commands at a boundary use the logged order.

For an opposing normal unit killed by an enemy-owned committed attack/skill,
award the opposing owner floor(15% of that unit's paid purchase cost), once.
Attribution for diagnostics chooses greatest post-mitigation per-source damage
in that death tick, ties earliest cast ID; ownership, not a projectile entity,
receives the gold. Two players imply one opposing owner, avoiding kill-steal.
Drone/turret/base death, expiration, cancel/despawn, friendly/no-source damage
give no bounty. Source drone/turret kills of a paid normal enemy still credit
their owner, but their own deaths never generate gold. Kill XP is zero.

Example: paid 80-gold troop dies ⇒ 12 bounty. Buying/cancelling a waiting troop
returns exactly 80 (zero profit); a started troop returns 60 (20 loss).
Enemy feeding recovers only 12 to the opponent, never 80 to the buyer. Summon
loops create no kill gold because drone targets have zero bounty.

## R7. Research, age transition and structures

Only research slot is mutually exclusive: age advance and stat research cannot
overlap. Recruitment/combat/income continue while it runs. Age prerequisite
uses cumulative XP, not a second spendable balance:

| To age | Required current age | Total XP ≥ | Gold cost | Duration |
|---|---|---:|---:|---:|
| 2 Ancient | 1 | 60 | 250 | 100 ticks |
| 3 Feudal | 2 | 140 | 400 | 100 ticks |
| 4 Arcane | 3 | 240 | 600 | 100 ticks |
| 5 Industrial | 4 | 360 | 850 | 100 ticks |
| 6 Future | 5 | 500 | 1150 | 100 ticks |

No age skip/reversal. Pay at accepted start. Research cannot be cancelled for
a refund; Resign/terminal state ends it without resource payout. Current-age
and preceding-age stat techs may be researched once each, IDs never repeat.
Baseline each age has attack +1000 bp and defense +250 bp; aggregate caps
attack +3000 / defense +1500. A tech beyond the effective cap is rejected,
not a paid useless no-op. Completion modifies eligible future contacts only;
no current/max HP gain and no retroactive launched damage.

At exact age completion:

```text
Live(age A, advance paid/in progress)
  -> Live(age A+1, new offers, same remaining match state)
Living units     -> SAME definition/version, HP, position, statuses, cooldowns
Queued recruits -> SAME paid definition/version, progress, reserved pop/order
Turrets          -> SAME socket/model/HP/reload/shot commitments
Commander slots  -> SAME ready_at deadlines; offered card changes next tick
Pending impacts  -> SAME snapshots, due tick, target/point, caster ownership
Base             -> SAME current/max HP and mitigation; cosmetic model changes
Gold/total XP    -> SAME except the already paid cost; total XP never reset
Research flags   -> Advance completes once; old tech flags remain and caps apply
```

On transitioning from age2 to3, age1 disappears only from **new purchases**;
already living/queued age1 units survive. A blocked age1 queue head does not
auto-cancel/re-price. Root/DoT/heat/immunity and heal/shield lifetime counters
survive; no free heal, instant cooldown reset, summon refresh or queue cloning.
Terminal-before-completion means no age-change effect occurs afterward.

Turrets are two per base, each from one of the current-age two branches. They
need gold, a free socket and the catalog's construction time. Unfinished turret
does not shoot and cannot shield the base. It is targetable with its current
construction HP and yields no kill reward. A slot is locked during construction.
There is no unrestricted building/placing/moving tower RTS.

Refit replaces a surviving turret only into the same branch's current-age
model; pay full target model cost, use its build time and stop firing meanwhile.
Current HP on completion is `min(old_current_HP, new_max_HP)`; never a free
repair. Cooldown readiness is `max(old_ready_at, completion_tick)`; old airborne
shots keep old damage. A destroyed or sold turret cannot finish an old refit.
Sell completes immediately only outside construction/refit, refunds
`floor(paid_model_cost × 5000 × current_HP / (10000 × max_HP))`, cancels future
windups, removes the socket entity; already airborne shots persist. Selling
full HP returns half, damaged returns less. Rebuilding/refitting cannot heal
for free or reset a shot already on cooldown. Data power is UNBALANCED.

## R8. End, pause, anti-stall and timeout

Destroy one base in R1's simultaneous damage phase ⇒ other side wins. Both
bases reach zero in that same tick ⇒ Draw, regardless of entity/cast order.
Base destruction takes precedence over hard timeout. Resign accepted before a
scheduled fatal tick produces a decisive loss immediately. Admin Cancel yields
an aborted result, no win. Terminal state accepts no recruit/cast/timer/research
and returns one EndMatch effect only; duplicate ending inputs cannot repay or
change outcome.

At 12 active minutes (tick14400), visible symmetric fatigue begins, unrelated
to whether a player has dealt damage recently. Once per active second each
living base loses true HP:
`fatigue = min(12, 2 + floor((elapsed_seconds - 720) / 60))`.
Base loss applies with that second's simultaneous hit batch, ignores armor/
shield, earns no bounty and never resets on damage, pause or age. It pressures
turtle without a secret AI advantage; possible draw frequency is a balance
risk to measure. No punitive income boost to the winning side.

At 20 minutes (tick24000), after any due combat/base-death result, compare
base HP fraction by exact cross multiplication. Higher fraction wins; equality
is Draw. Base max HP is always3000 in this draft, so current HP suffices, but
the general oracle preserves the fraction rule. Gold, army value, XP, age and
unit count are not arbitrary timeout tie-breakers. A UI must announce fatigue
and the approaching hard limit.

Offline PVE pause is a host-authorized Admin input, not a player clock edit.
While paused, no logical time, income, XP, queues, heat, cooldown, statuses,
drone lifetime or fatigue advance. Resume rebases host wakeup timing without
altering canonical remaining ticks; no background catch-up reward. App surface
loss/background should request that same pause through a supported host path,
not mutate State in a renderer. Seat absence/substitution/force-end semantics
need the compatibility/SDK implementation review; they are not invented as an
online feature here.

## R9. Independent oracle obligations

The arithmetic-only D01 tests exercise [MATH](MATH.md), not this pipeline.
C01 must implement independently stated fixed fixtures including:

| Oracle | Literal expected fact |
|---|---|
| Same-tick exchange | Two 50-HP units with due 50-damage contacts both die; no lower-ID survivor |
| Base tie | Two 1-HP bases with same-tick lethal contacts ⇒ exactly one Draw outcome |
| Contact motion | Front surfaces cannot cross; total closing displacement ≤ prior gap; ally order remains |
| Projectile overkill | Two homing shots at one target; dead target ⇒ second shot fizzles, never hits next unit |
| AoE edge | Target surface exactly at radius included; radius+1 q excluded; no cap+1th hit |
| Status expiry | DoT applied at0, interval4, expiry12 pulses4,8 only; no pulse12 |
| Root immunity | Root ends12; root at20 resisted; first eligible new root at52 |
| Heal cap | Spawn HP200 ⇒ lifetime heal≤70; prior heal65 ⇒ next grant≤5 |
| Shield budget | Spawn HP200 ⇒ pool≤50; lifetime grants≤100 even after dispel/expiry |
| Summon cap | Owner has3 drones: two-drone Cast rejects with exact no-op; owner2 may reach4 |
| Refund | Paid80 waiting/head refunds80/60; deployed cannot cancel |
| Age preservation | Base1370HP, slotready900, oldqueuedcost80 ⇒ transition preserves all three exactly |
| XP clock | 19 active ticks zero XP/income; tick20 grants1/5; pause adds none |
| Timeout equality | Equal fractions Draw even if one side is richer/higher age |
| Terminal idempotence | Retry ending input gives no second reward, state change or EndMatch |

Use independent slow motion/geometry and algebraic oracles for the optimized
implementation. Same-build replay is additional determinism evidence, not a
proof these rules are correct. All gameplay oracles above remain planned here.
