# Verification baseline

This directory documents development-only verification tools. Kani proof harnesses live beside
the real implementation under `#[cfg(kani)]`; no Kani runtime or dependency enters a normal
Tabula build, and no standalone synthetic proof package is maintained.

## Evidence kinds

| Evidence | What it means | What it does not mean |
|---|---|---|
| Unit/property tests | Sampled or generated examples exercise semantic behavior and edge partitions. | They do not examine every value in an unbounded input domain. |
| `cargo-mutants` | Plausible implementation changes are checked against the test oracle. | A mutation score is not a proof of the original implementation. |
| Kani | Exhaustive symbolic exploration of the finite modeled domains, with assertions checked by bounded model checking. | It proves only the stated harness model and toolchain assumptions, not all Tabula behavior. |

## Pinned tools

| Tool | Version | Purpose |
|---|---:|---|
| Kani | 0.67.0 | Bounded model checking / proof harnesses |
| cargo-nextest | 0.9.143 | Test runner used by cargo-mutants and repository gates |
| cargo-mutants | 27.1.0 | Mutation testing for test-oracle strength |

Install the pinned tools with:

```bash
just verification-install
```

Equivalent commands:

```bash
cargo install --locked kani-verifier --version 0.67.0
cargo kani setup
cargo install --locked cargo-nextest --version 0.9.143
cargo install --locked cargo-mutants --version 27.1.0
```

Kani owns its verifier toolchain under `~/.kani`; both tools are development commands rather than
workspace dependencies.

## Commands

```bash
just kani-core
just kani-tictactoe
```

These invoke Kani against the real packages:

```bash
cargo kani -p tabula-core
cargo kani -Z stubbing -p tabula-game-tictactoe
```

The TicTacToe command enables Kani's opt-in stubbing feature. Its transactional proof harnesses
execute the production `place` validation for every symbolic `SeatId`/cell pair and install a
total verifier-only replacement for the accepted outcome builder. Rejected calls return before
the replacement; accepted calls get the modeled board/turn transition without making CBMC model
the outcome's unrelated `SmallVec` destruction. The concrete
`concrete_opening_place_is_accepted` harness also exercises the real outcome builder for that
opening move.

Because `build.rs` hashes every `.rs` byte under `games/tictactoe/src/rules`, these `#[cfg(kani)]`
harnesses intentionally change `RULES_HASH`. A replay with the old hash and the same
`RULES_VERSION` is therefore classified as `CompatibleVersion`, not `Exact`, as designed by the
replay architecture. No architecture change is needed for this proof baseline.

Preview or run mutation testing for a package:

```bash
just mutants-list tabula-core
just mutants tabula-core
```

`.cargo/mutants.toml` keeps `test_tool = "nextest"`, matching the repository's ordinary test
runner. Mutation runs are intentionally opt-in and scoped to a named verification question.

Evidence from the exact scoped command on 2026-09-01:

| Mutant class | Result | Classification |
|---|---:|---|
| `LogicalTime::since` replacement | caught | Boundary regression test |
| `LogicalTime::plus` replacement | caught | Boundary regression test |
| `Millis::from_secs` replacement | unviable | The generated `Default::default()` body is not legal in this `const fn`; the compiling saturation behavior is covered by boundary tests and Kani |
| Four mutations inside `#[cfg(kani)]` harnesses | missed | Intentionally untested by ordinary nextest; those proof-only statements are exercised by Kani, not runtime tests |

The command therefore exits nonzero because cargo-mutants reports proof-only misses. This is
intentional evidence about the boundary between runtime mutation testing and verifier-only code,
not a suppressed score or a claim that all seven generated mutations are production survivors.

## Proof ledger

### `Millis::from_secs` is exact or saturating

- Invariant: conversion never wraps and is total.
- Harness: `crate::time::verification::millis_from_secs_is_exact_or_saturates`.
- Input domain: every `u64` seconds value.
- Assumptions: none about the input; the `checked_mul(1000)` oracle is independent.
- What is proved: fitting products equal the exact millisecond value; overflowing products equal
  `u64::MAX`; the production conversion has no arithmetic overflow in the modeled domain.
- What is not proved: other time-producing code or wall-clock conversion in platform shells.
- Command: `cargo kani -p tabula-core`.

### `LogicalTime::plus` cannot wrap

- Invariant: addition is exact when representable and saturates otherwise.
- Harness: `crate::time::verification::logical_time_plus_is_exact_or_saturates`.
- Input domain: every pair of `u64` logical time and `u64` millisecond duration values.
- Assumptions: none.
- What is proved: `checked_add` success matches the result, failure yields `u64::MAX`, and the
  result is never less than the original logical time.
- What is not proved: callers' choices of logical timestamps or timer policy.
- Command: `cargo kani -p tabula-core`.

### `LogicalTime::since` cannot wrap

- Invariant: forward differences are exact and reverse differences are zero.
- Harness: `crate::time::verification::logical_time_since_never_wraps`.
- Input domain: every pair of `u64` `now` and `earlier` values.
- Assumptions: none; non-monotonic pairs are modeled rather than excluded.
- What is proved: ordered subtraction is exact, reverse order returns zero, and the result cannot
  exceed `now` as a wrapped duration would.
- What is not proved: the shell's monotonic timestamp construction.
- Command: `cargo kani -p tabula-core`.

### Field-level R2: rejected initial TicTacToe placement is transactional

- Invariant: a rejected placement leaves every modeled canonical state field unchanged (field-level R2 evidence).
- Harness: `crate::rules::verification::rejected_initial_place_preserves_state`.
- Input domain: every raw `u8` `SeatId` value and every raw `u8` cell against a valid initial state
  with seats `SeatId(7)` and `SeatId(42)`.
- Assumptions: the initial state is constructed through `State::new`; the production `place` call
  receives every symbolic input and the assertion is conditional on its actual `Err` result; the
  total verifier-only commit replacement models accepted calls only. Exhaustive state
  destructuring covers all current canonical fields. This is not a serialized-byte proof.
- What is proved: every actual `Err` returned by `place` over the modeled `u8`/`u8` domain preserves
  board, seats, turn, status, and timeout, without duplicating the legality predicate.
- What is not proved: all arbitrary reachable TicTacToe prefixes, or canonical encoding bytes.
- Command: `cargo kani -Z stubbing -p tabula-game-tictactoe`.

### Field-level R2: rejected TicTacToe placement after one valid move is transactional

- Invariant: a rejected placement remains a no-op across every modeled canonical state field after a valid opening move (field-level R2 evidence).
- Harness: `crate::rules::verification::rejected_second_place_preserves_state`.
- Input domain: every raw `u8` `SeatId` value and every raw `u8` cell after the known-valid prefix
  `SeatId(7)` places at cell `0`.
- Assumptions: the initial state is constructed through `State::new`; the production `place` call
  for the fixed prefix is modeled by the total verifier-only commit replacement, whose state
  update is asserted immediately; the symbolic second call is also the production `place` call
  and the assertion is conditional on its actual `Err` result. The separate
  `concrete_opening_place_is_accepted` harness checks that the real commit path accepts the same
  prefix and advances the real state. Exhaustive state destructuring covers all current canonical
  fields; this is not a serialized-byte proof.
- What is proved: every actual `Err` returned by the second `place` call over the modeled `u8`/`u8`
  domain preserves board, seats, turn, status, and timeout, without duplicating the legality
  predicate.
- What is not proved: R2 for every arbitrary reachable state or every `Input` variant.
- Command: `cargo kani -Z stubbing -p tabula-game-tictactoe`.

### Supporting concrete TicTacToe opening transition

- Invariant: the known-valid opening placement is accepted and advances the real state.
- Harness: `crate::rules::verification::concrete_opening_place_is_accepted`.
- Input domain: one canonical initial state, `SeatId(7)`, and cell `0`.
- Assumptions: none beyond the constructor's checked initial-state fixture.
- What is proved: the real `place()` outcome-building path accepts the opening and sets cell `0`
  to `Mark::X` while handing the turn to `SeatId(42)`.
- What is not proved: arbitrary accepted placements or their complete event/effect contents.
- Command: `cargo kani -Z stubbing -p tabula-game-tictactoe --harness concrete_opening_place_is_accepted`.

## Adoption rules

Before adding a proof, record the proposition, modeled domain, assumptions, independent oracle,
reproduction command, and residual behavior. Prefer private module-adjacent harnesses. Never add a
production API or a broad `kani::assume` solely to make a proof compile.

Kani and cargo-mutants remain opt-in local checks in this revision. They are not added to
`cargo xtask check`, and no GitHub Actions workflow is added.
