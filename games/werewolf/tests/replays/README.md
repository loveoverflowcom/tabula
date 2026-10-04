# Canonical Werewolf replay evidence

These server-side audit fixtures contain seeds and full accepted input streams.
Never stage, serve, bundle, or download them through the gameplay browser. They
are not projected playback artifacts (doc 05 §8.2, I-5/I-6).

Ordinary tests and replay verification only read these committed bytes. Every
fixture begins at `WerewolfRules::create` with a fixed seed, validated config,
and pseudonymous roster. `initial_snapshot` is absent; no `RawState` is forged.
Every accepted input carries a checkpoint, timers fire at their exact recorded
logical deadlines, and the stream stops at the ordinary terminal `EndMatch`.

| Fixture | Seats / inputs | Coverage | Pinned final state hash |
|---|---|---|---|
| `normal.tbr` | 20 / 86 | Three rounds, all six roles, protection and blind healing, Seer reports, Witch poison, same-batch Hunter retaliation, public faction standings including dead winners, village victory | `cd97e8a5d0e24207e0d31a03c5716d16d24b316e4edaa4cb453ed88d8e7cddf2` |
| `ties.tbr` | 12 / 27 | Equal private wolf attack candidates, pass, abstain and Unvote, equal 6/6 public vote with no elimination, two-round Dusk-cap draw | `dcd29b76176c4e617d9e8a2e6a6077d91783a387330e8448929d1dfc90931061` |
| `timeout.tbr` | 6 / 15 | Every phase expires without any player input, missing choices/ballots default, all seats survive, three-round Dusk-cap draw | `e4716b6d3c5f3072eb0ee8b83f5e3b291e2407549b8120582804249470d394eb` |

`games/werewolf/tests/replay.rs` requires rules version 2, the current generated
`RULES_HASH`, and an `Exact` replay verdict, in addition to all checkpoints,
terminal outcome, and the literal final hashes above. `CompatibleVersion`
is deliberately insufficient. Rules version 1 has no linked reducer/migration
and is explicitly rejected as unreplayable by the typed verifier.

## Verify without changing the corpus

```sh
cargo test -p tabula-game-werewolf --test replay
cargo xtask replay games/werewolf/tests/replays/normal.tbr --verify --diagnose
cargo xtask replay games/werewolf/tests/replays/ties.tbr --verify --diagnose
cargo xtask replay games/werewolf/tests/replays/timeout.tbr --verify --diagnose
```

Each CLI run must report `verdict: EXACT`, `status: VERIFIED`, every input
checkpoint checked, and matching recorded/actual terminal outcomes.

## Intentional update only

The writer is an explicit example, not a test or a corpus-writing verification
step. Prefer writing to a distinct temporary directory first:

```sh
cargo run -p tabula-game-werewolf --example write_replays -- /tmp/werewolf-replay-review
cmp games/werewolf/tests/replays/normal.tbr /tmp/werewolf-replay-review/normal.tbr
cmp games/werewolf/tests/replays/ties.tbr /tmp/werewolf-replay-review/ties.tbr
cmp games/werewolf/tests/replays/timeout.tbr /tmp/werewolf-replay-review/timeout.tbr
```

After a deliberate rules change, first decide the rules-version/migration
policy, inspect the changed hash and outcome evidence, then intentionally
replace the corpus and update the pinned test literals. A source-only identity
change can require refreshed headers even when the final state hashes agree.
Never regenerate goldens merely to conceal a failing historical verification.
