---
name: rust-replay-differential-testing
description: Build independent oracles for behaviour that assertions cannot judge — small reference models, published external reference data, exhaustive enumeration of finite domains, golden corpora, live-versus-replay equivalence, and cross-target or cross-build determinism comparison. Use when correctness depends on a specification that lives outside the code, when an optimised implementation needs a slow obvious twin, when a system claims deterministic replay, or when a claim spans architectures, targets, or build profiles. Do not use where the implementation would be its own oracle, and do not import a production dependency purely to compare against.
---

# Rust replay and differential testing

An assertion can only check what its author already understood. A **differential oracle** checks
against something the author did not write: a second implementation, published data, another
architecture, or the same system's own past behaviour. This is the only category of evidence that
can catch a defect in the *specification as understood*, and it is routinely the cheapest strong
evidence available.

Read the nearest `AGENTS.md` and the normative architecture before adding a corpus or a
dependency.

## The four oracle shapes

| Shape | You compare against | Best when |
|---|---|---|
| **Reference model** | a deliberately slow, obvious second implementation you wrote | the rule is simple to state and hard to implement fast |
| **Published reference data** | numbers computed by someone else, decades ago | a standard corpus exists (perft, RFC vectors, published hashes) |
| **Exhaustive enumeration** | the whole finite domain | the state space is small enough to walk |
| **Self-differential** | the same system at another time, target, or build | determinism, replay, migration, cross-platform claims |

## 1. Reference models

Write the **most naive implementation you can stand**, structurally different from production, and
compare over generated or enumerated inputs.

Rules:

- **Different structure, not a copy.** If production uses bitboards, the model uses an array. If
  production uses `min_by_key` with a tuple key, the model sorts and scans with explicit `if`s.
  A model that shares the clever idea shares the bug.
- **Do not optimise the model.** Slowness is the point; it is what keeps it obviously correct.
- **Keep it in `tests/` or `#[cfg(test)]`**, never in the production path.
- **Compare acceptance as well as output.** A model that only checks the happy path misses the
  rejection semantics, which is usually where the divergence is.

Good candidates: legality/validation predicates, resolvers and selection rules (density, priority,
routing), clock and scoring arithmetic, canonical ordering.

Bounded exhaustive reference loops are the cheapest form of this and often beat a sampler:

```rust
for remaining in 1..=32 {
  for elapsed in 0..=40 {
    for increment in 0..=8 {
      let expected = if elapsed >= remaining { Flagged }
                     else { Ready(remaining - elapsed + increment) };
      assert_eq!(charge(remaining, elapsed, increment), expected);
    }
  }
}
```

No framework, no flakes, total over the interesting range.

## 2. Published reference data

The strongest oracle available, because it was computed **outside your codebase** by people
solving the same problem.

- Cite the source in the test.
- Commit the expected values as literals; never regenerate them from your own implementation.
- Choose the positions/cases that stress the hard paths, not just the easy ones.
- **Keep it in the per-PR suite if it is fast.** In a chess-like domain, a depth-3/4 perft suite
  runs in about a second and is frequently the *only* detector for a legality regression — a
  conformance suite, a determinism harness, and hundreds of self-play matches can all pass while a
  real rule is broken. Do not move it to nightly to save a second.
- Add a "divide" mode: on mismatch, report per-branch subtotals so the defect is localised in one
  run instead of a bisect.

**On importing a third-party implementation as an oracle:** it is legitimate to add one under
`[dev-dependencies]`, which does not enter the shipped graph. Weigh it against published data you
already have — if a standard corpus already provides an external oracle, a whole extra crate buys
little and costs dependency-policy and MSRV churn. Prefer published data; reach for a library
oracle only when a defect has escaped it.

## 3. Exhaustive enumeration of finite domains

If the reachable space is small, **walk all of it**. This subsumes sampling and bounded model
checking, needs no tooling, and runs in the ordinary suite.

```text
DFS from the initial state over legal actions:
  at every reachable position p:
    assert enumerate_legal(p) == model_legal(p)
    for every action in the FULL action alphabet (legal and illegal):
       compare acceptance, resulting state, and terminality against the model
       on rejection: assert canonical bytes unchanged
```

Before choosing a heavier tool, compute the size. A 3×3 board game has 5 478 reachable positions;
a `(u8, u8)` argument pair is 65 536 cases. Both are microseconds. *Do not use a model checker
where a finite exhaustive model is possible.*

## 4. Self-differential: replay, targets, and builds

### Replay equivalence

```text
live:    create(config, roster, seed) then apply each input in order  -> final canonical hash
record:  the ordered inputs, with their logical times and indices
replay:  create(...) again, apply the recorded inputs                 -> final canonical hash
assert:  the hashes are equal, at every recorded checkpoint and at the end
```

Requirements that make this real rather than circular:

- **Record from real usage, not only from fixtures.** A corpus generated by the same code path that
  verifies it is a round trip, not a differential. Recording a human's session, or a randomized
  self-play run, is what makes the corpus an independent witness.
- **Commit the expected final hash as a literal** in the test, so a behaviour change fails loudly
  rather than regenerating.
- **Version the container and reject foreign versions loudly.** Decoding an old artefact into a
  plausible wrong value is worse than refusing: "unreplayable" is honest, a fake replay is not.
- **Keep the checkpoint density high enough to localise.** A final-hash-only mismatch tells you
  something broke; per-input checkpoints tell you where.
- **Classify evidence strength** in the report: exact divergence index, a window, final-only, or
  outcome-only. Do not report a window as an exact location.

### Cross-target and cross-build

The claim "same inputs ⇒ byte-identical state on every OS, architecture, native and WASM" cannot
be checked by any single-target assertion. The cheap closure:

```text
1. emit committed vectors on the reference target:
   (case id, seed, ordered inputs, per-checkpoint hash, final hash) as JSON
2. re-run the same vectors on each other target and compare
   - wasm32 via a wasm test harness
   - aarch64 via a CI matrix job
   - debug vs release, if the claim includes build profiles
3. fail CI on any divergence
```

Do this while the number of cases is small. Discovering a divergence with two games is far cheaper
than with five and a live datastore.

### Migration

For every stored version, keep an artefact and assert that the current code either replays it
exactly or **explicitly marks it unreplayable**. Both outcomes are correct; silently producing a
plausible wrong reconstruction is the one that destroys audit value.

## Corpus hygiene

- Store artefacts as **committed verification inputs**, not as test output. Ordinary tests read
  them; only an explicit, named regeneration command rewrites them.
- Review the binary diff and the reported hashes when regenerating, and say in the PR why the
  values changed.
- Cover the documented spread — a normal case, an edge case, and a failure/timeout case per domain
  — rather than three variations of the easy one.
- When a randomized run finds a failure, persist the failing case into the corpus. Put the
  file-writing in the CLI layer, not in the library: a test harness that mutates the repository is
  a harness nobody trusts.

## Choosing between this and its siblings

| Situation | Skill |
|---|---|
| The specification lives outside the code (a standard, a published corpus) | **this one** |
| The law is universal over a large but samplable space | `rust-property-testing` |
| The domain is unbounded arithmetic with an obvious independent oracle | `rust-kani` |
| The input is untrusted bytes and the claim is robustness | `rust-fuzzing` |
| You want to know whether existing assertions are strong | `rust-mutation-testing` |

## Report format

```text
Oracle:        reference model | published data (source) | exhaustive (N cases) | self-differential
Compared:      <what, over what domain>
Independence:  <why the oracle cannot share the implementation's bug>
Corpus:        <files, how generated, how regenerated>
Result:        pass/fail; on failure, the exact divergence index or the honest window
Not covered:   <targets, depths, or cases outside this run>
```
