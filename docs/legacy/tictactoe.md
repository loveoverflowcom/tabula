# Retired Phase-0 prototype: Tic-Tac-Toe

`games/tictactoe` (`tabula-game-tictactoe`) was retired from active code at
[commit e325ccc](https://github.com/loveoverflowcom/tabula/commit/e325ccc335df11d55e82e6fd9d495aa00736d42c).
It was the Phase-0 SDK/bootstrap example. It is no longer an authoring template
or a current verification target.

[The full historical lessons and verification report](https://github.com/loveoverflowcom/tabula/blob/cca6cbee2e7bca369a53403ad8927c59630c1589/docs/legacy/tictactoe.md)
remain in Git history. Its Kani/mutation counts apply to the retired implementation;
they do not establish current game readiness or transfer proof to another game.

| Retained lesson | Maintained source / current consumer |
|---|---|
| Pure synchronous rules; rejected input leaves canonical state unchanged | [Doc 02 game contract](../architecture/02-game-module-and-sdk-design.md), [engineering workflow](../../.agents/skills/tabula-engineering/SKILL.md), game conformance fixtures |
| Validate before commit; keep deserialization/construction boundaries explicit | Engineering [types](../../.agents/skills/tabula-engineering/references/types-as-proofs.md) and [boundary hardening](../../.agents/skills/tabula-engineering/references/boundary-hardening.md) references |
| Canonical state stays behind `project` and `view_event` | [Doc 00 I-5/I-6](../architecture/00-architecture-principles.md), [Tiles information model](../games/tiles.md) and its security suite |
| Preserve deterministic replay and independently check rule correctness | [Doc 05](../architecture/05-data-protocol-and-replay.md), Chess/Tiles goldens under `tests/replays/`, [game-audit workflow](../../.agents/skills/tabula-game-audit/SKILL.md) |
| Keep local interaction/presentation separate from authority | [Doc 04](../architecture/04-frontend-and-design-system.md), existing Chess/Tiles consumers of `apps/game-client` |

For a new game, follow [doc 02 §14](../architecture/02-game-module-and-sdk-design.md)
and the current [game inventory](../../games/README.md). `cargo xtask new-game`
remains planned until implemented; do not run a retired Tic-Tac-Toe selfplay or
proof target as a substitute for the new game's own fixtures.
