# Verification tooling

This directory contains **development-only verification infrastructure**. Nothing here is linked
into Tabula's shipped binaries or deterministic rules dependency graph.

This first slice deliberately establishes the tooling boundary only. It does **not** add Kani
proofs to game rules and it does **not** change gameplay behavior. Real proof harnesses should land
in later focused PRs beside the invariant they verify.

## Pinned tools

| Tool | Version | Purpose |
|---|---:|---|
| Kani | 0.67.0 | bounded model checking / proof harnesses |
| cargo-mutants | 27.1.0 | mutation testing: verify that tests actually reject plausible bugs |

Install the pinned versions with:

```bash
just verification-install
```

Equivalent commands:

```bash
cargo install --locked kani-verifier --version 0.67.0
cargo kani setup
cargo install --locked cargo-mutants --version 27.1.0
```

Kani owns its verifier toolchain under `~/.kani`; it is not a workspace dependency. cargo-mutants
is also a Cargo subcommand, not a dependency of any Tabula crate.

## Commands

Verify that the Kani installation and repository plumbing work:

```bash
just kani-smoke
```

The smoke package under `verification/kani-smoke` is intentionally a nested standalone workspace.
That prevents Kani-only code from entering the root Cargo graph or `deps.toml` policy surface.

Preview the mutants for one package without running them:

```bash
just mutants-list tabula-core
```

Run mutation testing for one package:

```bash
just mutants tabula-core
just mutants tabula-game-api
just mutants tabula-game-tictactoe
just mutants tabula-game-chess
```

Package scope is intentional. There is no default whole-workspace mutation command: mutation
runs are expensive and should answer a named verification question.

## Adoption rules

### Kani

Before adding a real `#[kani::proof]` harness, record:

1. the invariant/proposition being checked;
2. the bounds and assumptions used by the model;
3. why existing type checks, unit tests, property tests, or self-play are insufficient;
4. the reproduction command;
5. the residual behavior outside the model.

Prefer module-adjacent `#[cfg(kani)]` harnesses when the proof needs private implementation detail.
Do not export production APIs only to make a verifier able to reach them.

Good future Tabula targets include transactional rejection, bounded state-transition invariants,
index/coordinate arithmetic, and totality/no-panic properties over hostile bounded input.

### cargo-mutants

A surviving mutant is evidence to investigate, not an instruction to add a brittle test. For each
survivor, classify it as one of:

- a real oracle/test gap;
- equivalent behavior;
- unreachable/dead code that should be removed;
- intentionally untested behavior with a documented reason.

Do not blanket-skip survivors just to make a mutation score green.

## CI policy for this slice

Kani and cargo-mutants are **opt-in local verification commands** in this PR. They are not added to
`cargo xtask check` and no GitHub Actions workflow is added. Once useful real proof harnesses and a
stable mutation budget exist, CI ownership can be introduced in a separate PR with an explicit
runtime/quota decision.
