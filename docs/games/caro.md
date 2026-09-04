# Caro (Gomoku-style) — design placeholder

> **Status: PHASE 3 — NOT IMPLEMENTED.** This is a design placeholder, not
> rules documentation for an existing implementation. See
> [`games/caro/src/lib.rs`](../../games/caro/src/lib.rs) for the architecture
> sketch and [`docs/architecture/08-first-games-validation-plan.md`](../architecture/08-first-games-validation-plan.md)
> (Game B) for the validation role.

## Architectural role

Caro is the **simple real product game and SDK-friction benchmark**. It sits
between `tictactoe` (a tiny internal SDK smoke test) and `chess` (the complex
correctness benchmark) on a three-rung ladder:

```text
tictactoe  — tiny example, proves the SDK works at all
    ↓
caro       — simple product game, proves a second real game is cheap to add
    ↓
chess      — complex product game, proves the contract survives real depth
```

Caro is **not** tic-tac-toe renamed, and it does not replace tic-tac-toe as
the template. It exists specifically to measure whether adding an
independent, real, product-shaped game requires any platform change — the
claim in `AGENTS.md` §7 ("zero platform changes").

## Information model

Not applicable. Caro has `hidden_information = false` — every seat sees the
full board. No `SecretModel` is required (doc 02 §7.3).

## Variant decision — still open

This document deliberately does **not** settle:

- Freestyle Gomoku vs. a Renju-style restriction on the first player's moves.
- Exact board size (a larger board such as 15×15 is the expected direction,
  but the final dimensions are a game-design decision, not an architecture
  one).
- Whether board size is configurable per match or fixed per ruleset.

These are marked `TBD during implementation` (Phase 3) and are out of scope
for the reference-game portfolio realignment that created this placeholder.

## Future scope

See `games/caro/src/lib.rs` for the IN/OUT scope sketch: fixed-board
placement, four-direction win-line detection, local play now, online play
later. No AI engine beyond a trivial/easy bot; no tournament opening
protocols.
