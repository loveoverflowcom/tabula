# Canonical replay corpus

These `.tbr` files are committed verification artifacts, not generated test
output. Ordinary tests and `cargo xtask replay` only read them; they never
rewrite the corpus.

The fixtures are regenerated intentionally with:

```text
cargo xtask replay-goldens
```

That command executes the typed `GameRules::create`/`apply` path, writes every
input checkpoint, and replaces every file in this directory. Review the binary
diff and the reported hashes before committing an update, and expect to bump the
game's `RULES_VERSION` if the hashes moved for a reason other than a deliberate
rules change.

The corpus contains:

| Fixture | What it pins |
|---|---|
| `chess-golden.tbr` | A short checkmate. |
| `chess-clock-golden.tbr` | A Fischer-clock timeout, with an `Input::Timer` at a recorded `LogicalTime`. |
| `tiles-golden.tbr` | One **complete** Tiles match: the deterministic shuffle, all 71 draws taken from it, every feature merge, completion scoring, follower returns, end-of-game partial scoring, and the final standings. |

Complete fixtures also store the terminal `MatchOutcome`, which the typed
verifier compares with the observed `Effect::EndMatch` outcome.

## A stale header downgrades the verdict rather than failing

`ReplayRunner::check` returns `Exact` only when the fixture's recorded
`rules_hash` equals the linked build's; a mismatch returns `CompatibleVersion`
and verification continues. So a fixture whose header went stale keeps passing
an `is_verified()`-only assertion while no longer being evidence that *this*
rules build produced it. `games/tiles/tests/replay.rs` therefore asserts the
verdict is `Exact`, which is what keeps "regenerate the corpus when the rules
change" enforced rather than remembered.

The Chess fixtures and Tiles fixture are current.


The Tiles fixture's input sequence is generated from the **rules** — first legal
placement in canonical order, claiming and passing on alternate opportunities —
and deliberately not from its bot. A golden that depended on bot policy would
have to be regenerated whenever the bot changed, which would defeat the point of
committing one.
