# Age War — design-only D01 scaffold

**PHASE D01. C01 remains blocked by D01–D06/owner approval and the authority
compatibility decision.** This is not a playable game or production module.

Start with [the design entry](../../docs/games/age-war.md), then RULES/CONTENT.

| Source | Implemented design surface |
|---|---|
| `src/schema.rs` | Private integer-unit/ID constructors, raw descriptor validation and borrow-bound validated witness |
| `src/catalog.rs` | Original **UNBALANCED** 36-unit/16-family/12-spell/12-turret/12-tech starting catalog, five advances and two bounded drone models |
| `src/math.rs` | Pure integer damage/TTK/EHP/efficiency/grant/refund/occupancy arithmetic examples |
| `src/phase_gate.rs` | Typed errors for unavailable simulation, AI and migration; success type `Infallible` |
| `tests/catalog.rs` | Inventory, references, bounds and hostile raw descriptors |
| `tests/design_math.rs` | 576 literal analytical rulebook contact cells, excluding gameplay/skills/economy |

There is no `GameRules`, `GameModule`, `GameBot`, canonical State/wire type,
renderer/presenter, `game.toml`, registry/discovery entry, runtime availability,
replay support or asset pack. Features preserve existing game crate shape;
enabling `rules`, `bots`, `presentation` or `testkit` only compiles this scaffold
and permitted optional dependencies. Nothing opens gameplay.

Descriptors are authored Rust literals. Public raw descriptor fields permit
negative validation tests; use `validate()` before treating cross-field facts
as coherent. Whole-catalog ID/version resolution is separately tested. There
is no serde/JSON/TOML loader and no decoder/serialization bypass promised here.
Design revision1 is not an SDK RulesVersion. Do not claim canonical migration,
transactional game reduction or cross-target replay from these tests.

## Focused commands

Run from the workspace root with pinned Rust1.96 and a bounded job count:

```sh
CARGO_BUILD_JOBS=2 cargo test -p tabula-game-age-war --no-default-features
CARGO_BUILD_JOBS=2 cargo test -p tabula-game-age-war --all-features
CARGO_BUILD_JOBS=2 cargo clippy -p tabula-game-age-war --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo xtask check
```

The last command is the repository aggregate gate, not replaced by the focused
checks. [Verification](../../docs/verification/age-war-d01/README.md) owns exact
executed results/limits. `conformance!` has no game fixture in D01; it becomes a
mandatory real-game acceptance gate before C01 completes. Planned 1,000/10,000
match campaigns, human playtests, visual/performance and native-mobile checks
are not executed by the commands above.
