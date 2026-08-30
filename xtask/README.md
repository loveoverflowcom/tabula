# xtask

Repository automation, in Rust. Run as `cargo xtask <command>`, or through the
`justfile` wrapper.

**Build this before anything else.** Doc 09 §7 step 4:

> Write the enforcement FIRST: `deps.toml` + `xtask check-deps` + `clippy.toml` +
> CI. Then deliberately add a forbidden dependency and confirm CI fails.
> Remove it.

That confirmation step is not optional theatre. An enforcement mechanism nobody
has watched fail is not known to work.

## Setup

Add to `.cargo/config.toml` so `cargo xtask` works from anywhere in the tree:

```toml
[alias]
xtask = "run --package xtask --"
```

## Commands

### Phase 0 — enforcement and the game SDK

| Command | What it does |
|---|---|
| `check-deps` | Walks the **resolved** cargo metadata graph per crate and asserts the `deps.toml` matrix. Also regenerates the doc 00 §8.1 table and fails if the committed docs differ. Enforces I-1 and I-15. |
| `check-no-game-ids` | Greps `crates/` and `services/` for game id literals and `games::` imports. `tabula-registry` is the only exemption. Enforces I-9. |
| `check-manifests` | Asserts `game.toml` equals the compiled `GameMetadata`/`GameCapabilities`. |
| `new-game <slug> [--seats N] [--category C]` | Scaffolds a game crate from the `games/tictactoe` template, including `clippy.toml` and `tests/conformance.rs`. |
| `selfplay <game> [--matches N]` | Bot-vs-bot matches with determinism, projection, and termination checking. Failing seeds are written to `tests/replays/<game>/regressions/`. |
| `replay <file> \| --all [--verify]` | Replays a `.tbr` and prints the first divergence with its input index. |

### Phase 1+

| Command | Phase | What it does |
|---|---|---|
| `perft <depth>` | 1 | Chess move-generation node counts, against published positions |
| `gen-tokens` | 2 | `tokens.toml` → `tokens.css` + `generated.rs` + `tokens.json`. Outputs are committed; CI fails if stale. |
| `check-no-raw-colors` | 2 | No hex literals or `Color::new(` outside `tabula-design` |
| `pack-assets <game>` | 3 | Builds, hashes, and writes a pack manifest |
| `gen-protocol-vectors --bump minor\|major` | 4 | Regenerates golden wire vectors, bumps `PROTOCOL_VERSION`, appends to `protocol-changelog.md` |
| `check-protocol` | 4 | Golden vectors round-trip; the I-13 version gate |
| `db reset` / `db migrate` | 4 | Local Postgres lifecycle |
| `load --scenario L1..L8` | 4 | Load scenarios against the committed baseline |

## Design notes

- **Pure Rust, no shell.** Cross-platform, typed, and testable. Contributors on
  Windows are not second-class.
- **`just` is a wrapper, never a source of truth.** Anything CI depends on lives
  here, so CI and a developer's machine run the same code.
- **Tooling is exempt from the dependency matrix.** `xtask` may depend on
  anything; it is never linked into a shipped binary.
- **Print the path, not just the violation.** `check-deps` failing with
  "tabula-core may not depend on tokio" costs twenty minutes of `cargo tree`.
  Failing with `tabula-core → foo 1.2 → tokio` costs none.
