# Rules and oracle layer

Start from the maintained game spec and declared variant; map each changed
rule to a failure mode and an oracle that does not call production code to
generate its own expectation. For advanced oracle design read
[replay/differential techniques](../../tabula-engineering/references/replay-differential-testing.md)
or [properties](../../tabula-engineering/references/property-testing.md) as
needed.

Select partitions from actual constructors, enums and spec boundaries:
legal/illegal actors and phases, time/numeric edges, ties, repeated inputs,
terminal state, empty/full resources, hostile coordinates and ordering.

| Game | Existing oracle and honest claim |
|---|---|
| Chess | [Published perft fixtures](../../../../games/chess/tests/perft.rs) compare independent node counts on several positions; they test move generation, not clocks, draw claims, all adjudication or UI |
| Tiles | [Whole-board `recompute`](../../../../games/tiles/src/rules/feature.rs) constructs a separate graph, compared after inputs in [features tests](../../../../games/tiles/tests/features.rs); scoring examples and follower/outcome laws add different claims |
| Caro | [Variant decision](../../../../docs/games/caro.md) remains open and no reducer exists; choose the variant before judging overlines, blocked ends or first-player restrictions |
| Werewolf | [Maintained W-D decisions](../../../../docs/games/werewolf.md) define future reducer oracles; current [config tests](../../../../games/werewolf/tests/config.rs) pin ClassicV1 counts/bounds, not night/vote resolution |

Tiles also has a finite placement oracle in
[placement.rs](../../../../games/tiles/src/rules/placement.rs). Comparing
`legal_commands` with `apply` is useful consistency evidence, not independent
legality evidence. Its graph oracle shares tile definitions; record those
trusted inputs and shared scoring logic.

Self-play is a transition driver. Record seed, match/input coordinates, seat
counts, clock/deadline mode, termination bound and exercised branches. Ten
games do not satisfy a 100k campaign; 100k does not prove rule correctness.
Current [command support](../../../../xtask/src/selfplay_cmd.rs) is Chess and
Tiles only. Bounded examples:

```bash
cargo xtask selfplay chess --matches 10 --seed 47
cargo xtask selfplay tiles --matches 10 --seed 47 --seats 5
```

Preserve a minimal deterministic counterexample and independent expectation
before fixing a defect. Mutation, fuzzing and bounded proofs are optional
escalations for named uncertainty, not mandatory audit campaigns. Shared
[verification guidance](../../tabula-engineering/references/verification-testing.md)
selects the cheapest adequate evidence.
