# Determinism, replay, and versioning layer

Read doc 00 I-2–I-4, I-8 and I-16,
[doc 05 §§7–10](../../../../docs/architecture/05-data-protocol-and-replay.md),
and the [corpus contract](../../../../tests/replays/README.md). For advanced
methods use [shared replay/differential guidance](../../tabula-engineering/references/replay-differential-testing.md).

| Claim | Evidence |
|---|---|
| Current reducer is deterministic | Same seed/config/roster/ordered input and logical time produce identical canonical state/events/effects; rejection preserves later RNG |
| Serialization/checkpoints are stable | Canonical round trips and recorded per-input hashes; read paths cannot mutate encoded state |
| Historical behavior reproduces | Committed `.tbr`, pinned hashes/outcome and game/rules identity; current-build-vs-current-build reruns cannot replace it |
| Rules identity covers authority | `RULES_VERSION`, generated `RULES_HASH`, `build.rs` source discovery, manifest/compiled metadata, bot/presentation exclusion and independent source-hash tests |
| Old versions have a disposition | Implemented migration or explicit `Unsupported`; distinguish incompatible version, malformed input and missing artifact |
| Target outputs agree | Executed identical-byte/hash comparisons across the claimed targets; compilation alone is build compatibility |

Current fixtures are `chess-golden.tbr` (checkmate),
`chess-clock-golden.tbr` (logical-time timeout) and `tiles-golden.tbr` (complete
placement/follower/scoring match):

```bash
cargo xtask replay tests/replays/chess-golden.tbr --verify --diagnose
cargo xtask replay tests/replays/chess-clock-golden.tbr --verify --diagnose
cargo xtask replay tests/replays/tiles-golden.tbr --verify --diagnose
```

Inspect verdict and coverage as well as exit status. A rules-source-hash
mismatch can yield `CompatibleVersion` while verification continues; this
is not `Exact` provenance. Sparse checkpoints allow only windowed/final-only
evidence. `--at` can reconstruct without verifying an exact prefix; distinguish
`POSITION_RECONSTRUCTED` from `PREFIX_VERIFIED`. See
[diagnostics](../../../../crates/tabula-testkit/src/replay/diagnostics.rs) and
[CLI output](../../../../xtask/src/replay_cmd.rs).

Preserve original artifacts. A derived reproducer needs an explicit different
destination: `--diagnose --write-reproducer /tmp/<descriptive-name>.tbr`.
Do not rewrite the committed oracle or minimize away recorded evidence.
`cargo xtask replay-goldens` intentionally replaces the corpus; use it only
for a requested corpus/rules update after explaining the compatibility
decision and reviewing hashes/diffs.

The CLI supports Chess/Tiles, not Caro/Werewolf. Protocol vectors,
`check-protocol`, DB commands and load testing are future dispatcher branches.
They are not executable version/security evidence. Game-rules version and
wire-protocol version decisions are distinct.
