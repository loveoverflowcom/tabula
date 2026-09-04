---
name: rust-mutation-testing
description: Run and interpret cargo-mutants to measure whether a test suite's assertions actually kill plausible defects, scope campaigns to narrow packages or diffs, classify every survivor as a real gap, an equivalent mutant, unreachable, verifier-only, a tool limitation, or low-value, and convert real survivors into regression tests. Use when a module is stable and pure, when a suite looks green but its assertions are unproven, before trusting a "well-tested" claim, or when auditing verification quality. Do not use as a coverage target, on churning code, on async or I/O-heavy shells, or as evidence of correctness.
---

# Rust mutation testing

cargo-mutants "finds places where bugs can be inserted without causing any tests to fail". That is
**assertion strength**, not correctness. A 100%-killed crate can still be wrong about its
specification — mutation testing cannot invent an oracle you did not write.

Read the nearest `AGENTS.md` first. In this repository mutation testing is deliberately outside
`cargo xtask check`; it answers a named question about one package, on demand.

## When it is high leverage

| Good target | Why |
|---|---|
| A pure, stable domain core (rules, encoders, validators, resolvers) | mutants are meaningful and the suite is fast |
| A newly hardened trust boundary | proves the new checks are asserted, not just present |
| Code the team *believes* is well tested | the belief is the hypothesis under test |
| A diff, via `--in-diff` | cheap per-PR signal on exactly what changed |

| Poor target | Why |
|---|---|
| Async runtimes, sockets, database adapters | slow suites, timeouts dominate, mutants are mostly unviable |
| Code changing weekly | the survivor list is stale before it is triaged |
| Generated code, `Debug`/`Display` impls | high noise, low value |
| Anything as a percentage goal | the number is not the point; the classification is |

## Scope the campaign

```bash
cargo mutants --package <one-package> --list      # always look first: how many, and where?
cargo mutants --package <one-package>
cargo mutants --file 'src/rules/**' --package <p> # narrower
cargo mutants --in-diff <diff-file>               # PR-scoped
cargo mutants --shard 1/4                         # CI fan-out
cargo mutants --iterate                           # skip mutants caught in a previous run
```

Config lives in `.cargo/mutants.toml`; keys mirror the flags (`test_tool`, `exclude_re`,
`examine_re`, `exclude_globs`, `examine_globs`, `timeout_multiplier`, `additional_cargo_args`).
Set `test_tool = "nextest"` if that is the repository's runner, so mutation runs match ordinary
test runs.

**Scope to where the tests are.** `--package` runs only *that package's* tests. A mutant in a
low-level crate that only a downstream crate's tests would catch shows up as MISSED. That is
useful information — it says "this crate does not test its own contract" — but it is not the same
as "nothing catches this". Say which it is.

## The `cfg`-gated blind spot

cargo-mutants parses source, so `#[cfg(kani)]`, `#[cfg(fuzzing)]`, and similar blocks are mutated
even though they are never compiled into the test build. **Every such mutant survives trivially**,
and it does so in the modules that look most rigorously verified — inflating the apparent gap while
hiding the real one. In this repository, 24 of 110 survivors across two campaigns were
verifier-only noise.

Fix, and verify it:

```toml
# .cargo/mutants.toml
exclude_re = ["verification::"]   # the module name Kani harnesses live in
```

Then re-run `--list` and confirm the count dropped and no `verification::` entries remain. Because
this is a naming convention, document it wherever new harness modules are added.

## Classify every survivor — no exceptions

Write the classification down. An unclassified survivor list is worse than no campaign, because
next quarter nobody knows which entries were already judged.

| Class | Definition | Action |
|---|---|---|
| **REAL TEST GAP** | The mutant changes observable behaviour and nothing asserts the difference | Write the regression test. Prioritise by blast radius, not by ease. |
| **EQUIVALENT MUTANT** | The mutated program is semantically identical | Record why, in one line. Do not add a test. |
| **UNREACHABLE** | The branch cannot execute for any reachable input | Record why. Consider whether the branch should exist. |
| **VERIFIER-ONLY** | Inside a `cfg`-gated proof/fuzz module | Exclude by config; never "fix" with a test. |
| **TOOL LIMITATION** | Unviable substitutions, `Default` for a type with no meaningful default, lifetime-dependent returns | Ignore; do not contort the code. |
| **LOW-VALUE** | Trivial accessor, `Debug`/`Display`, a getter with no invariant | Usually ignore — **but read it once**: a `Debug` impl that redacts a secret is a security property, and its survival means nothing asserts the redaction. |

**Timeouts are a finding, not a nuisance.** A mutant that makes the suite hang usually means an
unbounded loop whose termination depends on an unproven invariant. In this repository three
timeouts all landed on one rejection-sampling loop and revealed that its termination condition was
asserted nowhere — a liveness hazard for code that runs inside a server task.

## Convert real survivors into tests

For each REAL TEST GAP, write the smallest deterministic test that the mutant fails and the
original passes. Two rules:

1. **Assert the property, not the mutant.** If `zone = 2^32 - (2^32 % n)` mutated to `+` survives,
   do not test "the result is not `2^32 + x`". Test the property the code exists for: *`zone` is
   the largest multiple of `n` not exceeding 2³²*. That kills the whole family.
2. **Put the regression in the ordinary suite**, so it runs on every PR. The mutation campaign is
   the discovery mechanism; the test is the durable artefact.

Re-run the campaign after the fixes and record the new numbers.

## Worked example from this repository

`cargo mutants --package tabula-core` → 133 mutants: 69 caught, 30 missed, 31 unviable, 3 timeouts.
Triage of the 30:

- **4 VERIFIER-ONLY** — inside `#[cfg(kani)] mod verification`. Excluded by config.
- **2 REAL, high value** — the rejection zone in `DetRng::below`. Confirmed by a standalone
  program: the mutant removes all rejection and restores modulo bias in every shuffle and dice
  roll the platform will ever make.
- **3 REAL, high value** — the `shuffle` length guard. `len < 2` → `len == 2` makes a **2-element
  shuffle the identity**, and the existing `shuffle_is_a_permutation` test cannot see it because
  the identity is a permutation.
- **1 REAL** — `SeatRoster::get`'s `==` → `!=` returns a different seat; that accessor feeds seat
  validation in both games.
- **1 REAL, security-adjacent, weak mutant** — `MatchSeed`'s `Debug` impl. The mutant still
  redacts, so it proves little; its survival reveals that *no test asserts the redaction at all*,
  and "a seed in a log line is a leaked deck" is a stated security property.
- **1 REAL, cross-package** — `Viewer::seat -> None`. Downstream game tests would catch it; the
  package-scoped run does not. Report it as "not covered by this crate's own tests".
- The rest: accessors and `Display` impls (LOW-VALUE), one `||`→`&&` requiring a slice longer than
  2³² (UNREACHABLE).

Total remediation: roughly 25 lines of tests plus one Kani harness. That is the shape of a good
outcome — a short, specific, high-value list, not a percentage.

## What mutation testing is *not* evidence for

It cannot find a defect your specification never mentioned. In this repository, deleting chess's
castling-through-check guard is a genuine rule violation that the conformance suite, the
determinism harness, and 200 self-play matches all missed; only **perft against published node
counts** caught it. Mutation testing would have reported the same blind spot as a survivor only if
perft were already in the test set.

Use mutation testing to find *unasserted behaviour*, then use
`rust-verification-testing` to choose the right oracle for that behaviour —
`rust-replay-differential-testing` when the specification lives outside the code,
`rust-property-testing` when the law is universal, `rust-kani` when the domain is unenumerable.

## Report format

```text
Package: <p>   Command: <exact>
N mutants: C caught, M missed, U unviable, T timeouts, in <time>
Survivors:
  REAL TEST GAP     <file:line> <mutation>  -> test added: <name>
  EQUIVALENT        <file:line> <mutation>  -> why
  UNREACHABLE       <file:line> <mutation>  -> why
  VERIFIER-ONLY     <n> mutants in <module> -> excluded via exclude_re
  LOW-VALUE         <n> accessors/Debug impls
Timeouts:           <file:line> -> what unbounded construct they reveal
After remediation:  C' caught, M' missed
```
