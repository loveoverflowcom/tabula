# Tiles (Carcassonne-like)

> **Status: PHASE 3 — implemented (rules + presentation).** Async-turn
> *operations* (hibernation, push, surviving deploys) and the full Board Reader
> remain Phase 4+ and Phase 9 respectively.
> Code: [`games/tiles/`](../../games/tiles/). Validation role:
> [doc 08 §4](../architecture/08-first-games-validation-plan.md).

## Architectural role

Tiles is the **dynamic-spatial-state, deterministic-RNG, and secondary
hidden-information benchmark**. It stresses a growing board, incremental
feature scoring, camera-local presentation, and async-turn rules without making
Werewolf's social and event-existence requirements its own.

## Rules summary

A Carcassonne-like tile-placement game with **Tabula's own tile distribution**.
It is not a reproduction of any published set, and no test or acceptance
criterion depends on matching one.

```text
72 tiles: one start tile placed at the origin by `create`, plus a 71-tile bag.
Each turn: the seat on turn places the drawn tile on a free square that touches
the board and matches every shared edge, then the next tile is drawn.
A drawn tile with no legal square anywhere is discarded publicly and another
is drawn. The match ends when the bag is empty.
```

Each turn is two steps: place the tile, then claim one of its features or
decline. Claiming waits until after the placement because a follower placed now
can still score a feature *this* tile completed.

Edge terrains are **City**, **Road**, and **Field**; two tiles may sit side by
side only where the terrains facing each other are equal.

A tile is a list of *segments*, each naming a feature (City, Road, Monastery)
and the tile edges it reaches. That is what distinguishes "one city passing
through" from "two separate cities on opposite edges", which four edge letters
could not express. Edge terrain is derived from the segments, so the two can
never disagree.

### Followers and scoring

Each seat has seven followers. A follower may be placed only on a feature of
the tile just placed, and only on a feature **nobody has claimed yet** — a
feature gains a second owner by *merging* with an already-claimed one, never by
being claimed twice.

| Feature | Completed | Unfinished, at end of game |
|---|---|---|
| Road | 1 per tile | 1 per tile |
| City | 2 per tile, +2 per pennant | 1 per tile, +1 per pennant |
| Monastery | 9 (its own tile and all eight neighbours) | 1 + however many neighbours it has |

Every seat holding the **most** followers on a feature scores its full value;
ties share the value rather than splitting it. A completed feature scores
immediately, returns its followers, and is retired — so it cannot be counted
again at the end of the game. An unclaimed feature scores nothing and is
retired just the same.

Final standings rank seats by score, descending, with ties sharing a rank and
ranks dense from zero (the platform's own requirement — see
`MatchOutcomeError::NonContiguousRanks`).

### The feature graph

The design sketch said "incremental union-find". Implementation overturned the
noun: `find` with path compression **mutates**, and `project` and
`legal_commands` are read paths, so a compressing structure would make the
encoded state depend on query history rather than on the input stream — and the
graph lives in the state hash, where a representation that is not a function of
the semantic state stops being a divergence detector and starts being a
divergence source.

Tiles uses an explicit component registry merged by **minimum id** instead.
Reads never mutate; a component's contents are a set union and its id is a
minimum, so both are independent of the order a tile's four sides are processed
in; closure is a counter reaching zero. Only components adjacent to the new tile
are ever touched.
[`games/tiles/src/rules/feature.rs`](../../games/tiles/src/rules/feature.rs)
records all four representations compared and what decided it.

The whole-board flood fill that recomputes everything from scratch survives as
the **differential oracle**, not as production code: it runs after every
accepted input of complete matches at every seat count
([`tests/features.rs`](../../games/tiles/tests/features.rs)), which is the
honest place for "recompute from scratch".

### Deliberately out of scope

**Farms/fields are not a scorable feature.** `Field` remains an edge terrain so
adjacency matching is complete, but a field is never scored and never carries a
follower. Scoring farms needs sub-edge granularity — two field corners per tile
side — which multiplies the graph's representation without exercising a
contract that roads, cities, and monasteries do not already exercise. Recorded
as a decision, not an oversight.

Also out: expansions, rivers, custom boards, trading.

## Information model

### Secret, and from whom

| Value | Hidden from | Revealed when |
|---|---|---|
| Remaining tile-bag order | every player and every spectator | never; only the next drawn tile is revealed, and only by being drawn |

`hidden_information = true`, because that order determines every future draw.
The `SecretModel` in [`games/tiles/src/rules/secret.rs`](../../games/tiles/src/rules/secret.rs)
declares it authorised to **nobody**, and the crate expands
`tabula_testkit::projection_security!` alongside `conformance!`. Being the
*secondary* hidden-information benchmark (Werewolf is the primary one, for
per-seat knowledge and event non-existence) is not an exemption from either
obligation.

### Public

The board — every placed tile with its rotation — the tile currently drawn, the
tiles discarded as unplaceable, the number of tiles remaining, whose turn it is,
and whether the match is paused.

### Intentionally derivable

A player who counts the tiles already placed and discarded knows exactly which
*multiset* remains in the bag; the distribution is published in `TILE_SET`. That
is card-counting, it is part of the game, and it is not a leak. What is secret is
the **order**, and no amount of counting reveals it.

### Deliberately NOT derivable

Nothing in any projection depends on the remaining order. In particular, the
`View` carries a remaining-tile **count** and not a collection, so there is no
field a later refactor could widen back into a sequence, and no ordering,
length, or checksum anywhere in the projection changes when the bag is permuted.
Do not add a "next tile preview" affordance — it would.

### Replay evidence

`tests/replays/tiles-golden.tbr` is a complete committed match. It is the only
piece of replay evidence here that survives a *code change*: the conformance
suite, the per-checkpoint replay property, and self-play all compare the current
code against itself. The golden's final state hash is a literal in
`games/tiles/tests/replay.rs`, so a rules change that alters the shuffle, a
draw, a merge, a score, or the standings fails loudly and forces an explicit
`RULES_VERSION` decision (doc 02 §11.4).

### Verification, and its honest limit

Two oracles, because they catch different things.

**Containment** (`assert_no_leaks` / `assert_no_event_bypasses_redaction`, via
`projection_security!`) scans every reachable step of a real trace for the
secret's bytes in any unauthorized `View` or `ViewEvent`. Tiles declares two
sequence tokens — the whole remaining order, and the next few draws in draw
order — and declares them **only while the bag holds at least four tiles**.

That threshold is the honest limit, and it is asserted rather than merely
written down. A single remaining tile encodes to about two bytes, and two bytes
occur in a `View` full of tile kinds and coordinates constantly; a token that
short would report leaks that are not leaks, and a scan nobody believes is worse
than no scan. This is the answer to the `TODO(phase 3)` on `Secret::tokens`
asking the first real hidden-information game what a token *is*: for an ordered
secret, a sequence — never one token per hidden value.

**Noninterference** closes the gap. Permuting the remaining bag (two different
permutations, each checked to leave a state the validator still accepts) must
leave every unauthorized viewer's `View` and `ViewEvent` byte-identical. That
property holds for a bag of one as readily as for a bag of seventy, and it also
catches a *derived* leak — a length, an ordering, a count that moved — which no
containment scan can see. Both directions are controlled: a draw, which is a
public change, must be visible to every viewer, and two distinct public draws
must stay distinct.

## Balance and configuration

| Knob | Meaning |
|---|---|
| `turn_deadline_ms` | Milliseconds a seat has to complete its turn. `0` disables the deadline and is what local hot-seat play uses. Any nonzero value must be at least 5 000. |

Live play and async play differ **only** in this number. `apply` reads nothing
but `ctx.now`, so a 60-second deadline and a 24-hour one take the same code
path; that is the payoff of `LogicalTime`, and it is why `async_turns` can be
declared from Phase-3 rules rather than promised.

When the deadline fires, the rules resolve the turn themselves: the drawn tile
goes on the **first legal square in canonical order**, at that square's lowest
legal rotation. "First in canonical order" is a rule any observer can reproduce
and that no seat can steer — the alternative (skipping the turn) would let a
seat improve its position by timing out.

`pausable = true` is implemented: `Admin(Pause)` disarms the deadline and
refuses play; `Admin(Resume)` grants a *full fresh* deadline rather than the
remainder, which is the generous reading and the one an async match wants.
Pausing twice, or resuming when not paused, is a no-op rather than an error.

`client_preview = false`, deliberately. Doc 02 §7.2 defines it as "folding the
`ViewEvent` stream onto a previous `View` lands on the same `View` that
`project` returns", and the testkit has no oracle for that property yet.
Declaring `true` would be an unverified claim.

### `StateSizeClass`, measured

Doc 02 §12.4 estimated a full Tiles board at 30–120 KB and doc 03 §9.2 made
Tiles the worked example for the `Medium` snapshot class on that basis.
`games/tiles/tests/state_size.rs` plays complete matches at every supported seat
count and measures the canonical encoding:

| Position | Canonical bytes |
|---|---|
| Opening (board of one, full bag) | ~150 |
| Full board, no feature graph (Part 1 of this work) | ~307 |
| Full board, with the feature graph and followers | **~1 677** |

So Tiles is **`Small`**, two orders of magnitude below the estimate, and the
declared class follows the measurement in both `src/lib.rs` and `game.toml`.
The test asserts the declared class *is* the measured one, so a future tile-set
or state-shape change cannot quietly leave the declaration stale.

The consequence for doc 03 §9.2 is recorded there: **no game in the portfolio
occupies `Medium` yet.** A class nobody occupies is an honest row; a class
assigned from a guess sets a snapshot cadence nobody measured. What Tiles does
still validate is the class as a *mechanism* — its state grows about fivefold
over a match while chess's barely moves.

## Presentation and controls

Everything interactive is presentation-local (I-10): camera pan and zoom, the
rotation of the tile being previewed, the keyboard cursor, hover, and the drag
in progress. `tests/presentation.rs`'s camera property drives one logical
interaction — "tap the first legal square" — from five different camera
positions and zoom levels, so each takes a different path through the pointer
mapping, and requires the resulting canonical state to be byte-identical.

| Input | Does |
|---|---|
| Left drag | Pan (past a 6 px threshold; a release after a drag never places) |
| Left click | Place at that square, or claim a follower slot on the tile just laid |
| Right click | Rotate the tile in hand |
| Arrows | Move the board cursor; the camera follows it off-screen |
| Tab | Jump the cursor to the next square where the tile fits at this rotation |
| Space | Rotate the tile in hand |
| Enter | Place at the cursor, or claim |
| Escape | Pass on a follower |
| On-screen | Zoom in/out, recenter, rotate, pass |

The whole placement step is reachable from the keyboard, which doc 04 §10.3
makes mandatory rather than optional.

### The camera, and the HUD

`RenderList` carries exactly **one** camera, and the backend applies it to every
draw after that draw's local transforms (doc 04 §5.3). A screen-fixed HUD
therefore cannot be expressed by declining to use the camera — it has to undo
it. Tiles emits board geometry in world units with the real camera on the list
and wraps the HUD in a `PushTransform` of the camera's inverse; the composition
is exactly the identity, so HUD text keeps its size at every zoom while board
text scales with the board. `tests/presentation.rs` asserts the HUD occupies the
same logical rectangle at every zoom level.

This is worth recording because the alternative — leaving the camera at identity
and transforming board geometry in the presenter — also works and is what a game
without a HUD would do. Using the camera field for real is what makes Tiles the
camera benchmark rather than a game that happens to draw a grid.

## Art direction

`accent = "#7A9E5B"`, `mood = "calm"`. Tiles render procedurally from semantic
design tokens rather than from a sprite pack: a tile is a surface rectangle, a
city edge a solid band against that border, a road edge a bar reaching the tile
centre, a monastery a block in the middle, a pennant a dot, and a follower a
disc in its seat's colour. Adjacency is therefore readable at a glance — a city
that continues across two tiles looks continuous — which matters more for a
Phase-3 slice than art does. The asset pack is declared (`tiles@0.1.0`) for the
catalog and for Phase 9's real art, and nothing in the Phase-3 presenter depends
on it.

## Open questions

- The tile distribution is tuned for a playable 72-tile game, not balanced by
  play-testing. Changing it changes `RULES_VERSION`, because a different bag is
  a different game.
- Whether the turn deadline should skip the turn instead of auto-placing is a
  game-design decision; auto-placing was chosen because skipping rewards timing
  out. Revisit with real async play in Phase 9.
- The deadline auto-resolution *declines* the claim rather than claiming
  greedily. Declining is the neutral choice — a follower is a resource, and
  spending one on a seat's behalf is a bigger decision than placing the tile it
  was already required to place.
- A follower may only be placed on the tile just laid, which is the classic
  rule. It also means a seat that draws no claimable feature for several turns
  simply banks its followers; no catch-up mechanism was added.
