# Tiles (Carcassonne-like) — design placeholder

> **Status: PHASE 3 — NOT IMPLEMENTED.** This is a design placeholder, not
> rules documentation for an existing implementation. See
> [`games/tiles/src/lib.rs`](../../games/tiles/src/lib.rs) for the architecture
> sketch and [`docs/architecture/08-first-games-validation-plan.md`](../architecture/08-first-games-validation-plan.md)
> (Game C) for the validation role.

## Architectural role

Tiles is the **dynamic-spatial-state, deterministic-RNG, and secondary
hidden-information benchmark**. It stresses a growing board, incremental
feature scoring, camera-local presentation, and async-turn state without
making Werewolf's social and event-existence requirements its own.

## Information model

The canonical tile bag is ordered, and that order is not public:

| Value | Hidden from | Revealed when |
|---|---|---|
| Remaining tile-bag order | every player and spectator | never; only the next drawn tile is revealed |

The following are public:

- the number of tiles remaining;
- the tile currently drawn;
- every tile already drawn or placed, including its position and rotation.

`hidden_information = true` because the remaining order affects future outcomes.
The eventual `SecretModel` must declare the remaining bag order as authorised to
nobody, and Phase 3 must run the projection/security scan over reachable draws.
Tiles is a secondary hidden-information case: Werewolf remains the primary
benchmark for per-seat knowledge, secret event existence, and phased social
communication.

## Future scope

See `games/tiles/src/lib.rs` and doc 08 §4 for the rules, presentation, async
turn, and acceptance sketches. No implementation or final tile distribution is
locked by this placeholder.
