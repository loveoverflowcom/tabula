---
name: rust-fuzzing
description: Decide whether a Rust surface needs coverage-guided fuzzing, then build cargo-fuzz/libFuzzer targets for parsers, decoders, and containers that consume untrusted bytes — with a seeded corpus, a dictionary where the format is textual, resource bounds, crash minimisation, and minimised crashes promoted into the deterministic test suite. Use when a surface accepts attacker-influenced bytes and the interesting failures are panics, hangs, or resource exhaustion rather than wrong answers. Do not use for pure functions over small typed domains, where a property test or an exhaustive model gives a stronger oracle, or as a substitute for validating that correct input is accepted.
---

# Rust fuzzing

Fuzzing answers one question: *given arbitrary bytes, does this surface panic, hang, or consume
unbounded resources?* It does **not** tell you whether correct input is accepted, or whether the
output is right. Reach for it only where that question is the interesting one.

Read the nearest `AGENTS.md` first: fuzzing needs a nightly toolchain and a separate cargo
workspace, both of which may be policy decisions in a repository that pins its toolchain.

## Gate: does this surface need a fuzzer?

Both must be true.

1. **The bytes are untrusted** — they come from a network peer, a CDN, a user-supplied file, a
   cache, or another process. "A future phase will make it untrusted" is not yet a reason.
2. **A fuzzer gives a *different* oracle** than the cheaper tools. Fuzzing beats a property test
   when the failure mode is a crash, a hang, or an allocation, and when the input is *malformed* in
   ways a typed generator cannot express.

| Surface | Fuzz? | Instead |
|---|---|---|
| Binary container / archive decoder (length prefixes, checksums, compression) | **yes** | — |
| Text config/manifest parser over untrusted data (TOML/JSON/YAML) | **yes** | — |
| Wire protocol decoder | **yes** | — |
| Non-self-describing binary decode into a typed struct | **yes**, targeted per type | — |
| Short-string validators (paths, ids, versions) | no | property test against a naive reference validator — it checks acceptance too |
| A reducer over a typed command enum | no | state-machine property test (`rust-property-testing`) |
| Pure arithmetic with an independent oracle | no | `rust-kani` |
| A move generator with published reference counts | no | `rust-replay-differential-testing` |

**Do not fuzz pure functions that property tests cover more efficiently unless fuzzing provides a
different oracle.** A no-panic result on a function whose real risk is a wrong answer is a green
tick that means nothing.

## Target design

Keep the target thin, deterministic, and free of I/O:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The claim: decoding arbitrary bytes must not panic and must bound its allocations.
    if let Ok(value) = my_crate::Container::read(data) {
        // Optional, and worth it: an accepted value must satisfy its own invariants.
        let _ = value.checked_invariants();
    }
});
```

Rules:

- **One target per surface.** A target that fuzzes three parsers reports coverage you cannot
  attribute.
- **No filesystem, no network, no time, no global state.** A nondeterministic target cannot
  reproduce a crash.
- **Assert invariants on accepted values**, not just absence of panic. "It decoded" is weak;
  "it decoded and its declared length matches its actual length" is a real oracle.
- **`debug_assert!`s are on in fuzz builds.** Use them; they turn silent corruption into a crash.

### Structure-aware generation — when *not* to use it

For malformed-input targets (containers, protocol frames), feed raw bytes. The whole point is
input the type system would reject.

Use `arbitrary`-derived structured generation only when the target is a *typed* API — for example
fuzzing a command sequence rather than a byte stream. Note that for mutation quality,
`fuzz_mutator!`-based structure-aware mutation has been measured to outperform `Arbitrary`-based
generation; if you go structure-aware and coverage plateaus, that is the next lever.

## Corpus, dictionary, bounds

- **Seed the corpus from real artefacts** already in the repository: committed golden files, test
  fixtures, manifest literals. A seeded corpus reaches interesting code in minutes rather than
  days.
- **Commit the minimised corpus**, not the raw one. `cargo fuzz cmin` first.
- **Dictionaries pay for textual formats.** List the keywords, keys, and enum values
  (`pack`, `version`, `hash`, `priority`, `resources`, …). They rarely pay for binary formats with
  a magic header and length prefixes — the fuzzer finds that structure quickly on its own.
- **Set resource bounds explicitly.** "Does not panic" is insufficient if a 200-byte input can
  allocate megabytes or spin for seconds:

```bash
cargo fuzz run <target> -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536 -timeout=10
```

  If the surface itself declares caps (max decompressed size, max element count, max window size),
  those caps are exactly what the fuzzer will attack — and finding the cap nobody wrote is the
  point.

## Crash handling — the part that makes fuzzing durable

1. `cargo fuzz tmin <target> <crash-file>` to minimise.
2. **Promote the minimised input into the ordinary deterministic test suite** as a fixture with an
   explanatory name — not into the fuzz corpus alone. The fuzz corpus is not run on every PR; the
   test suite is.
3. Fix, then keep both the regression test and the corpus entry.

## CI placement

| Where | What | Why |
|---|---|---|
| **Every PR** | `cargo fuzz build` only | A fuzz target that stops compiling is the most common way fuzzing dies. Never *run* a fuzzer in PR CI — it is nondeterministic and time-boxed. |
| **Nightly** | `cargo fuzz run <target> -- -max_total_time=<n>` per target, with the committed corpus | bounded, attributable |
| **Release / phase exit** | a longer campaign, plus a corpus refresh | |

**Never leave a CI job invoking a fuzz target that does not exist.** A permanently red scheduled
job trains everyone to ignore scheduled jobs, which costs more than the fuzzing was worth.

## Layout

```text
fuzz/                              # its own workspace; excluded from the main one
  Cargo.toml
  fuzz_targets/
    <surface_a>.rs
    <surface_b>.rs
  corpus/<surface_a>/              # committed, minimised
  dictionaries/<surface_b>.dict    # only for textual formats
```

Pin the fuzz workspace's toolchain explicitly if the repository pins a stable channel.

## Worked example: a versioned binary replay/archive container

A container with a magic header, a format version, length-prefixed sections, a checksum trailer,
compression, and explicit caps on decompressed size, section size, element count, and window size
is close to an ideal first target:

- the caps are asserted today by hand-written tests, which cover the caps someone thought of;
- the failure modes are hangs and allocation, not wrong answers;
- a corpus already exists (the committed golden files);
- the surface is genuinely reachable by untrusted input the moment a support tool opens a
  user-supplied file.

Second target: the text manifest parser, with a dictionary of its keys. Third, later: the wire
protocol decoder, when it exists.

## What *not* to fuzz in a deterministic-core codebase

Deterministic RNGs, canonical hash functions, logical-time arithmetic, and rule engines over typed
enums. They are pure functions over small typed domains with better oracles already available
(frozen vectors, bounded model checking, published reference data). Fuzzing them burns nightly
minutes to rediscover nothing.

## Report format

```text
Target:        <name>  Surface: <function>  Untrusted source: <where the bytes come from>
Oracle:        no panic | no panic + invariant | differential against <x>
Corpus:        seeded from <n> artefacts, minimised to <m>
Bounds:        -max_len, -rss_limit_mb, -timeout
Run:           <duration>, <execs/s>, <new coverage>
Crashes:       <n>; minimised and promoted to tests as <names>
Not covered:   <what this target does not reach>
```
