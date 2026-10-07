# Caro audit rubric

Recheck status. Package: `tabula-game-caro`; `rules`, `bots`, `presentation`
and `testkit` are declared but this is a **design placeholder**. No
`GameRules`, `GameModule`, `game.toml`, conformance fixture, game tests, bot,
presenter or replay exists. A build or empty test run is not correctness
evidence.

Maintained authorities:

- [Doc 08 §3](../../../../docs/architecture/08-first-games-validation-plan.md):
  SDK-friction benchmark, scope/failure signals/acceptance.
- [Game design placeholder](../../../../docs/games/caro.md): perfect
  information and explicitly unresolved variant.
- [Crate sketch](../../../../games/caro/src/lib.rs) and
  [features](../../../../games/caro/Cargo.toml): plans, not implementations.

For today's audit, report existing surfaces, unavailable claims and doc/code
contradictions. Do not manufacture a conformance target or count unsupported
`xtask selfplay caro`/replay as evidence. Implementation remains subject to
phase gates and requested scope.

If implementation is requested, settle the variant before judging freestyle
vs forbidden first-player moves, exact-five vs overlines, or blocked ends.
Then derive these claim families from the maintained specification:

| Claim | Rubric |
|---|---|
| Line detection | Four directions, edges/corners, intersections, variant-specific overlines/blocked ends; simple whole-board oracle |
| Transitions | Occupied/out-of-range cells, actor/turn, resignation, full-board draw, terminal/repeated inputs, transactional rejection |
| SDK friction | Large fixed-board `Enumerated` affordances/legality; no core/API or game-specific platform behavior beyond mechanical registration |
| Public projection | Distinct `View`, actor affordances, spectator visibility; no fake `SecretModel` |
| Presentation | Tap/coordinate/keyboard placement, readable grid, local hover/focus/win line; motion stays outside canonical state |

These dimensions are not settled rules. Future findings/tests should cite the
chosen maintained variant rather than expanding this into a second rulebook.
