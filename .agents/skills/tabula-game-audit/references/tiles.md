# Tiles audit rubric

Recheck status/features. Package: `tabula-game-tiles`; default `rules`, with
real `bots`, `presentation` and optional `testkit`. Phase-3 rules/module,
follower scoring, presentation, conformance, SecretModel/security and a
complete committed replay exist. Async-turn rules exist; hibernation/push/
deploy recovery, network enforcement, full Board Reader, camera rotation and
wheel/pinch zoom are deferred claims. State size is measured **Small**, not
the old Medium estimate.

Authorities and evidence:

- [Game spec/information model](../../../../docs/games/tiles.md),
  [doc 08 §4](../../../../docs/architecture/08-first-games-validation-plan.md),
  [module/capabilities](../../../../games/tiles/src/lib.rs),
  [reducer](../../../../games/tiles/src/rules/mod.rs),
  [state validator](../../../../games/tiles/src/rules/state.rs),
  [placement](../../../../games/tiles/src/rules/placement.rs),
  [graph/recompute](../../../../games/tiles/src/rules/feature.rs),
  [scoring](../../../../games/tiles/src/rules/scoring.rs),
  [bag secret model/tests](../../../../games/tiles/src/rules/secret.rs).
- [Rules tests](../../../../games/tiles/tests/rules.rs),
  [graph/scoring oracle tests](../../../../games/tiles/tests/features.rs),
  [reachable-state properties](../../../../games/tiles/tests/determinism.rs),
  [conformance](../../../../games/tiles/tests/conformance.rs),
  [replay](../../../../games/tiles/tests/replay.rs),
  [size](../../../../games/tiles/tests/state_size.rs),
  [presentation/tests/snapshots](../../../../games/tiles/src/presentation.rs).

| Surface | Sensitive edges |
|---|---|
| Placement | Rotations/edge match, adjacency/frontier, occupied/disconnected/extreme coordinates, unplaceable draws, placement vs claim step |
| Graph | Canonical merging, open edges/pennants/monastery neighbours, equality to recompute, read paths preserve bytes |
| Followers/scoring | Claim availability/ownership, majority/ties, conservation/return, completion once, final partial points/standings |
| Affordances | Placement `Hints` decode; claim `Enumerated`; generic conformance skips `Hints`, so game-specific laws matter |
| Secrecy/RNG | All clients lack remaining order; multiset/count public, deterministic shuffle/draws, explicit short-bag scan limit |
| Time/lifecycle | Deadline auto-resolution, stale timers, pause/resume, terminal cancellation; long deadlines prove rules, not async operations |
| Presentation | Screen/world mapping, pan vs tap, rotation/claim keyboard, HUD transform, identical state from different cameras |

Command choices:

```bash
cargo nextest run -p tabula-game-tiles --test conformance
cargo nextest run -p tabula-game-tiles --test features --test determinism --test rules
cargo nextest run -p tabula-game-tiles --lib -E 'test(rules::secret::tests)'
cargo nextest run -p tabula-game-tiles --features bots,presentation
cargo xtask selfplay tiles --matches 10 --seed 47 --seats 5
cargo xtask replay tests/replays/tiles-golden.tbr --verify --diagnose
```

The graph is a canonical component registry, not path-compressing union-find.
`recompute` is the graph oracle; shared tile definitions/scoring inputs remain
trusted. Farms are deliberately unscored; the tile set is Tabula's, not a
published Carcassonne distribution. Those variant choices are not defects.
See [security](hidden-information.md), [replay](replay-and-versioning.md) and
[presentation](presentation-review.md) for evidence limits.
