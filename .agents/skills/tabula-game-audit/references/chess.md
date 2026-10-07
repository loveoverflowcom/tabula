# Chess audit rubric

Recheck implementation/features. Package: `tabula-game-chess`; default
`rules`, with real `bots`, `presentation` and optional `testkit`. Rules,
module, bots, presentation, conformance and committed replays exist. Online
runtime/rating operations are separate platform/phase claims; compiled
`async_turns` is currently disabled.

Authorities and evidence:

- [Doc 08 §2](../../../../docs/architecture/08-first-games-validation-plan.md)
  owns benchmark scope/failure signals/acceptance. There is no maintained
  `docs/games/chess.md` currently; use that section and explicit code/tests.
- [Module/capabilities](../../../../games/chess/src/lib.rs),
  [reducer](../../../../games/chess/src/rules/mod.rs),
  [move generation](../../../../games/chess/src/rules/movegen.rs),
  [state/FEN](../../../../games/chess/src/rules/state.rs),
  [clock](../../../../games/chess/src/rules/clock.rs).
- [Conformance](../../../../games/chess/tests/conformance.rs),
  [rules](../../../../games/chess/tests/rules.rs),
  [clocks](../../../../games/chess/tests/clocks.rs),
  [perft](../../../../games/chess/tests/perft.rs),
  [public projection controls](../../../../games/chess/tests/projection_control.rs),
  [replay/source hash](../../../../games/chess/tests/replay.rs),
  [bot tests](../../../../games/chess/tests/bot.rs),
  [presenter/tests/snapshots](../../../../games/chess/src/presentation/mod.rs).

| Surface | Sensitive edges |
|---|---|
| Legality | King safety, pins/evasions, castling rights/attacked transit, en passant exposing check, all promotions, unrepresentable FEN |
| Outcomes | Claimable vs automatic repetition/move-count draws, dead material, stalemate/checkmate, resignation/draw offers, terminal inputs; perft does not judge these |
| Clocks | Fischer/Bronstein, exact-zero/one-ms-before, overflow, timeout mating material, early/stale timers, rejection, disconnect clock continuation, terminal cancellation |
| Public projections | Both seats/spectator observe moves; deterministic public controls do not prove secrecy |
| Presentation | Drag/tap/keyboard equivalence, promotion chooser, cancellation/resize, input during motion, canonical locality, terminal HUD/a11y |

Choose relevant commands:

```bash
cargo nextest run -p tabula-game-chess --test conformance
cargo nextest run -p tabula-game-chess --test perft
cargo nextest run -p tabula-game-chess --test rules --test clocks
cargo nextest run -p tabula-game-chess --features bots,presentation
cargo xtask perft chess 4
```

Ordinary perft tests compare multiple published positions at practical depths.
Initial depth five is ignored by default; phase/release depth-five evidence
requires an explicit run:

```bash
cargo test -p tabula-game-chess --test perft -- --ignored
```

That exercises only the ignored test, not the ordinary multi-position suite.
See [replay/versioning](replay-and-versioning.md) for both Chess goldens and
[presentation](presentation-review.md) for UI limits.
