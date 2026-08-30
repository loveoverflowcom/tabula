# Per-game design notes

One file per game: `docs/games/<slug>.md`. Required before a game ships
(doc 08 §7.1).

## What goes in it

### 1. Rules summary

Enough that someone reviewing the code can tell whether it implements the game
correctly, without owning the physical box. Cite the edition or variant.

### 2. The information model — **the part that matters**

**Mandatory for every game with `hidden_information = true`** (doc 02 §7.1).

The `SecretModel` scanner catches *direct* leaks: a whole hand serialised into a
spectator's view, a role map sent wholesale. It runs on every PR and it is good
at what it does.

It cannot catch **derived** secrets — where two individually-public values
combine to reveal a hidden one. The classic: exact deck count + all visible
discards + your own hand = the opponent's hand, in a 52-card game.

So this section states, explicitly:

```markdown
## Information model

### Secret, and from whom
| Value | Hidden from | Revealed when |
|---|---|---|
| Deck order | everyone | match end (via salt reveal) |
| Seat N's hand | every other seat, all spectators | cards are played |
| Shuffle salt | everyone | match end |

### Public
Hand counts, the current trick, pass state, finishing order, the deck commitment.

### Intentionally derivable
A player who tracks every discard can narrow the remaining distribution. This is
counting, it is part of the game, and it is not a leak.

### Deliberately NOT derivable
Nothing in any projection distinguishes "seat 2 passed because they could not
beat the trick" from "seat 2 passed by choice". Do not add a `could_have_played`
hint to the view — it would.
```

That last kind of entry is the valuable one. It records a decision that is
invisible in the code and would otherwise be quietly undone by a future
"helpful UI hint" PR.

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
| `tictactoe.md` | Phase 0 — the template. Trivially: nothing is secret. |
| `chess.md` | Phase 1 |
| `cards.md` | Phase 3 — **the important one.** Hidden hands, server RNG, the deck commitment. |
| `werewolf.md` | Phase 3 — roles, night actions, and event *non-existence*. |
| `tiles.md` | Phase 3 — bag order secret, count public. |
