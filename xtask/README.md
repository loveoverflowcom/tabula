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
| `check` | Runs `fmt`, `clippy`, `test`, `check-deps`, `check-no-game-ids`, `check-manifests`, then `cargo deny check`, in that order, stopping at the first failure. The one command a PR needs to be confident in; see AGENTS.md §5. |
| `check-deps` | Walks the **resolved** cargo metadata graph per crate and asserts the `deps.toml` matrix: direct dependencies come from the crate's allow-list, banned/forbidden-category crates cannot be reached transitively (with the path printed), and dependency direction respects the tier ordering. Enforces I-1 and I-15. |
| `check-no-game-ids` | Scans the tree for a game id appearing as a whole word (case-insensitive, `_`/`-` count as separators) outside its own game package, `tabula-registry`, `xtask`, test fixtures, manifests, or docs/comments. Enforces I-9. |
| `check-manifests` | Validates every workspace `Cargo.toml` (workspace-field inheritance, no wildcard registry versions, internal crates referenced via `{ workspace = true }`, the `rules`/`presentation`/`bots`/`testkit` feature shape for game crates) and, for games that have one, `game.toml`'s schema (required fields, the `com.tabula.<id>` convention, enum-valued capabilities). Does **not** yet cross-check against the compiled `GameMetadata`/`GameCapabilities` statics — that needs the `metadata_from_manifest!` proc macro (doc 02 §10.2), which does not exist yet. |
| `new-game <slug> [--seats N] [--category C]` | Scaffolds a game crate from the `games/tictactoe` template, including `clippy.toml` and `tests/conformance.rs`. |
| `selfplay <game> [--matches N] [--seed N\|HEX] [--match-index N] [--max-inputs N] [--clock fischer\|bronstein\|none]` | Deterministic bot-vs-bot matches with projection, timer, transactional, and termination checks. Failures print reproducible seed/match/input coordinates; the command does not mutate the repository. |
| `replay <file> [--verify] [--at N]` | Verifies a canonical `.tbr`, compares every checkpoint and the final hash, and prints the first divergence with its input index. `--at N` seeks to an accepted-input state version. |
| `replay-goldens` | Intentionally regenerates the committed Phase 1 corpus under `tests/replays/`; ordinary tests never rewrite it. |

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
