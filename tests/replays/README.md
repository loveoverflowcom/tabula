# Canonical replay corpus

These `.tbr` files are committed verification artifacts, not generated test
output. Ordinary tests and `cargo xtask replay` only read them; they never
rewrite the corpus.

The fixtures are regenerated intentionally with:

```text
cargo xtask replay-goldens
```

That command executes the typed `GameRules::create`/`apply` path, writes every
input checkpoint, and replaces the three files in this directory. Review the
binary diff and the reported hashes before committing an update.

The corpus contains one complete Tic-Tac-Toe match, one short Chess checkmate,
and one Chess Fischer-clock timeout containing an `Input::Timer` at a recorded
`LogicalTime`.
