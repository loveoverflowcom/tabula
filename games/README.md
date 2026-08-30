# games/

One crate per game. Each is feature-split into `rules` / `bots` / `presentation`
so the server compiles a game with no renderer and the client compiles it with no
database (doc 01 §5.1 rule 3).

```bash
cargo xtask new-game <slug> --seats 2 --category abstract
```

**The target: a playable, networked, spectatable, replayable game in one crate,
under 300 lines, with zero platform changes.**

If adding a game requires editing anything under `crates/` or `services/`, that
is a platform bug — report it rather than working around it.

## The five reference games (doc 08)

Each exists to stress a different dimension of the contract. They are not a
product roadmap; they are a test matrix that happens to be playable.

| Game | Phase | What it proves | Hardest contract stressed |
|---|---|---|---|
| [`tictactoe`](tictactoe) | 0 | The SDK works at all. **The template.** | none — it is the smoke test |
| [`chess`](chess) | 1 | Complex legality, clocks, ratings, async turns | clocks + `legal_commands` enumeration |
| [`cards`](cards) | 3 | Hidden hands, server RNG, delayed spectators | projection + RNG secrecy |
| [`werewolf`](werewolf) | 3→7 | Phases, scoped chat, event **non-existence** | `view_event → None` + scopes |
| [`tiles`](tiles) | 3→9 | Large dynamic state, camera, async turns | state size + snapshot cost |

## What one contract absorbs (doc 02 §12.5)

| Dimension of variation | Absorbed by |
|---|---|
| 2 vs 20 players | `SeatSpec`, `SeatId` |
| Public vs hidden state | `project` / `view_event`, `SecretModel` |
| Strict turns vs phases vs simultaneous | `TurnModel` + game-owned phase state |
| Clocks vs phase timers vs 24 h deadlines | one mechanism: `Effect::SetTimer` + `Input::Timer` |
| Chat trivial vs chat as a core rule | `ChatPolicy` + `Effect::SetChatScopes` |
| Voice irrelevant vs essential | `VoiceRequirement` + `Effect::SetVoiceScopes` |
| Ranked vs social | `RankedSupport` + the platform rating service |
| Tiny vs medium state | `StateSizeClass` → snapshot policy |
| Disconnect fatal vs irrelevant | `Input::Seat` + the game's own handling |
| Bot substitution fine vs forbidden | `SubstitutionPolicy` |

**No platform code branches on which game it is running.** Every difference above
is either a declarative capability the platform reads, or a behaviour the game
implements behind the same five functions.

## New-game checklist (doc 02 §14)

```text
[ ] game.toml complete; xtask check-manifests passes
[ ] State/Command/Event/View/ViewEvent/Config defined; View is a DISTINCT type
[ ] RULES_VERSION set; migrate() implemented or explicitly Unsupported
[ ] apply(): validate-then-mutate; EVERY Input variant handled (Timer/Seat/Admin too)
[ ] Effects: timers set and cancelled symmetrically; EndMatch emitted exactly once
[ ] project(): all four Viewer cases considered, spectators EXPLICITLY
[ ] view_event(): every Event variant decided per viewer (Some / degraded / None)
[ ] SecretModel implemented if hidden_information = true
[ ] legal_commands(): Enumerated or Hints (unlocks bots, UI hints, fuzzing)
[ ] describe(): a11y text for the board and the current turn/phase
[ ] Bot: at least Trivial (free via legal_commands)
[ ] tests/conformance.rs present; suite green
[ ] >= 3 golden replays committed (a normal game, an edge case, a timeout)
[ ] docs/games/<slug>.md: rules summary + INFORMATION MODEL
[ ] Presentation: RenderList only; no direct renderer calls; motion tokens used
[ ] Asset pack built and hashed; no assets in the binary beyond a placeholder
[ ] Registered in register! behind a per-game cargo feature
```

## Anti-patterns that will get a PR rejected (doc 02 §13)

| You wrote | Do instead |
|---|---|
| `std::time::Instant` in rules | `ctx.now` |
| `rand::thread_rng()` for a shuffle | `ctx.rng.stream(DOMAIN)` |
| `HashMap` in `State` | `BTreeMap` |
| `View { hand: Option<Vec<Card>> }` set to `None` | `HandSummary { count }` — model the knowledge |
| `Ok` with no events for an illegal command | `Err(RuleError)` |
| Mutating, then validating | Validate fully, then mutate |
| Animation/tween state in `State` | `GamePresentation::Local` |
| The whole rulebook in one `apply` match arm | Sub-functions per phase |
| Reading a clock to detect timeouts | `Effect::SetTimer` + `Input::Timer` |
| Secrets in `RuleError::detail` | Codes only; detail must be public-safe |
| A `Command::Debug*` variant | `#[cfg(test)]`, excluded from the decoder |
| One event per pixel of feedback | Semantic events; presentation elaborates |
| Calling `legal_commands` from `apply` for authority | `apply` decides for itself |
| A game-side "player index" beside `SeatId` | `SeatId` throughout |
