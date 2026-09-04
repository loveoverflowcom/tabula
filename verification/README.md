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
```

These invoke Kani against the real packages:

```bash
cargo kani -p tabula-core
```

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


## Adoption rules

Before adding a proof, record the proposition, modeled domain, assumptions, independent oracle,
reproduction command, and residual behavior. Prefer private module-adjacent harnesses. Never add a
production API or a broad `kani::assume` solely to make a proof compile.

Kani and cargo-mutants remain opt-in local checks in this revision. They are not added to
`cargo xtask check`, and no GitHub Actions workflow is added.
