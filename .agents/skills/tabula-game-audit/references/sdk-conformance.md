# SDK conformance layer

Read [doc 02](../../../../docs/architecture/02-game-module-and-sdk-design.md)
§§3–5, §§10–11 and §14, then the actual
[game API](../../../../crates/tabula-game-api/src/lib.rs) and
[fixture contract](../../../../crates/tabula-testkit/src/conformance/mod.rs).
Use the current API when an illustrative sketch is stale, while preserving
doc 00's normative invariants.

| Claim | Evidence to inspect |
|---|---|
| A usable module exists | `GameRules`/`GameModule`, real `State`/`Command`/`Event`/`View`/`ViewEvent`/`Config`, distinct projected types, `validate_config` and roster checks |
| Inputs have game-owned meaning | Every `Input` variant, stale/unknown timers, disconnect/reconnect, admin actions, terminal/repeated inputs; deliberate empty outcomes can be correct |
| Rejection is a total no-op | Canonical bytes before/after (R2), later legal probe and deterministic context/RNG behavior (R8), exact public-safe error reason |
| Effects are coherent | Timer set/cancel symmetry, logical deadlines, `EndMatch` once, standings; scope values and shell enforcement are separate claims |
| Capabilities describe implemented rules | Compiled metadata/capabilities vs `game.toml`, config ranges, spectator/substitution/preview/async promises and named consumers |
| Affordances never become authority | Stable `Enumerated` order/no duplicates, decodeable `Hints`, actor/phase gating, agreement with `apply`; independent legality evidence belongs in the rules layer |
| Fixtures exercise the contract | Nonempty legal script, changed state, real rejection/probe, reachable terminal state, alternate RNG seed where relevant; legitimate capability skip vs missing evidence |

`conformance!` expands to ordinary checks. Read
[commands](../../../../crates/tabula-testkit/src/conformance/commands.rs),
[terminal](../../../../crates/tabula-testkit/src/conformance/terminal.rs), or
the affected helper before attributing a claim to the macro. `check_legal`
currently tests only `Enumerated` at initial and post-script states; `Hints`
needs separate coverage. Hidden-information games additionally need
[security](hidden-information.md); a generic conformance pass does not imply it.

[Testkit's adversarial fixtures](../../../../crates/tabula-testkit/tests/)
test harness correctness. Use them when changing the harness instead of only
relying on well-behaved games.

Discover integration targets under `games/<slug>/tests/`, then run a real
conformance target, for example:

```bash
cargo nextest run -p tabula-game-chess --test conformance
cargo nextest run -p tabula-game-tiles --test conformance
```

Inspect the selected count and scenario skip output. An empty package can
build/test successfully without supporting game conformance. The common
`rules`, `bots`, `presentation`, `testkit` declarations prove no implementation.

Architecture gates are `cargo xtask check-deps`, `check-no-game-ids`, and
`check-manifests`, with different claims. The manifest checker currently does
not cross-check all compiled metadata/capabilities. `cargo xtask new-game`
remains unimplemented in [the dispatcher](../../../../xtask/src/main.rs),
despite documentation. Do not cite planned scaffold/registry/server paths as
tested support.
