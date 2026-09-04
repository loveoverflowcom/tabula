# Per-game design notes

One file per game: `docs/games/<slug>.md`. Required before a game ships
(doc 08 §7.1).

## What goes in it

### 1. Rules summary

Enough that someone reviewing the code can tell whether it implements the game
correctly, without owning the physical box. Cite the edition or variant.

### 2. The information model — **the part that matters**

**Mandatory for every game with `hidden_information = true`** (doc 02 §7.1). In
the current portfolio that includes `werewolf` and `tiles` — `tictactoe`,
`chess`, and `caro` are perfect information. Werewolf is the primary
hidden-information benchmark; Tiles is the secondary case for its secret
tile-bag order and public count.

The `SecretModel` scanner catches *direct* leaks: a whole role map serialised
into a spectator's view, a secret night action sent wholesale, or the remaining
Tiles bag order sent to a player. It runs on every PR and it is good at what it
does.

It cannot catch **derived** secrets — where two individually-public values
combine to reveal a hidden one. The classic, from a hidden-hand game: exact
deck count + all visible discards + your own hand = the opponent's hand.

So this section states, explicitly (werewolf example):

```markdown
## Information model

### Secret, and from whom
| Value | Hidden from | Revealed when |
|---|---|---|
| Role assignment | every seat until death, all spectators | a seat dies and its role is revealed |
| A night action's existence | every seat but the actor | never, unless the ruleset reveals it |
| Doctor's `Saved` outcome | every seat but the doctor | dawn, if the ruleset reveals saves |

### Public
Phase, alive/dead seats, vote tallies (in most rulesets), revealed roles.

### Intentionally derivable
A player who tracks voting patterns and claims can build a suspicion model.
This is deduction, it is part of the game, and it is not a leak.

### Deliberately NOT derivable
Nothing in any projection distinguishes "no night action happened" from "a
night action happened but is not yet resolved" for an unauthorized viewer —
`view_event` returns `None` for both. Do not add a `something_happened` hint
to the view — it would.
```

That last kind of entry is the valuable one. It records a decision that is
invisible in the code and would otherwise be quietly undone by a future
"helpful UI hint" PR.

Tiles has a smaller information model, but it is still a real one:

```markdown
## Information model

### Secret, and from whom
| Value | Hidden from | Revealed when |
|---|---|---|
| Remaining tile-bag order | every player and spectator | never; only the next drawn tile is revealed |

### Public
Bag count, the tile currently drawn, and all placed/drawn tiles.

### Verification
`SecretModel` declares the remaining bag order as authorised to nobody. The
Phase-3 projection scan must cover reachable draws while preserving the public
count and the drawn tile.
```

### 3. Balance and configuration

What the config knobs do, which combinations are supported, why the defaults are
the defaults. Werewolf's role sets per seat count belong here.

### 4. Art direction

Palette intent, the `[theme]` accent and `mood` from `game.toml`, animation
character. Enough that a second artist stays consistent with the first.

### 5. Open questions

Rules edge cases you decided by fiat. Future you will want to know it was a
decision rather than an oversight.

## Files

| Game | Status |
|---|---|
| `tictactoe.md` | Phase 0 — internal SDK smoke test / template. Trivially: nothing is secret. Not a product/reference game. |
| `chess.md` | Phase 1 — correctness benchmark: complex legality, clocks, deterministic replay. |
| `caro.md` | Phase 3 — design placeholder. Simple real product game, large fixed board, SDK-friction benchmark. Perfect information; no information model needed. |
| `tiles.md` | Phase 3 — design placeholder. Carcassonne-like: deterministic tile-bag RNG, dynamic spatial state, incremental scoring. Bag order secret, count public. |
| `werewolf.md` | Phase 3 (rules/headless) → Phase 7 (social) — **the important one.** Roles, night actions, and event *non-existence*. |
