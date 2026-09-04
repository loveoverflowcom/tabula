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

Edge terrains are **City**, **Road**, and **Field**; two tiles may sit side by
side only where the terrains facing each other are equal.

A tile is a list of *segments*, each naming a feature (City, Road, Monastery)
and the tile edges it reaches. That is what distinguishes "one city passing
through" from "two separate cities on opposite edges", which four edge letters
could not express. Edge terrain is derived from the segments, so the two can
never disagree.

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
count and measures the canonical encoding. **The estimate was wrong by two
orders of magnitude**, and the declared class follows the measurement, not the
estimate.

The consequence for doc 03 §9.2 is recorded there: no game in the portfolio
occupies `Medium` yet. A class nobody occupies is an honest row; a class
assigned from a guess sets a snapshot cadence nobody measured.

## Art direction

`accent = "#7A9E5B"`, `mood = "calm"`. Tiles render procedurally from semantic
design tokens — terrain fills, a pennant marker, and follower discs in seat
colours — rather than from a sprite pack. The asset pack is declared
(`tiles@0.1.0`) for the catalog and for Phase 9's real art, and nothing in the
Phase-3 presenter depends on it.

## Open questions

- The tile distribution is tuned for a playable 72-tile game, not balanced by
  play-testing. Changing it changes `RULES_VERSION`, because a different bag is
  a different game.
- Whether the turn deadline should skip the turn instead of auto-placing is a
  game-design decision; auto-placing was chosen because skipping rewards timing
  out. Revisit with real async play in Phase 9.
