//! # `xtask` — repository automation
//!
//! **Phase 0, and doc 09 §7 says to build it FIRST — before `tabula-core`:**
//!
//! > 4. Write the enforcement FIRST: `deps.toml` + `xtask check-deps` +
//! >    `clippy.toml` + CI. Then deliberately add a forbidden dependency and
//! >    confirm CI fails. Remove it.
//!
//! That last sentence is the point. An enforcement mechanism that has never been
//! observed to fail is not known to work. Add `tokio` to `tabula-core`, watch CI
//! go red, then take it out.
//!
//! Pure Rust, no `make`, no shell scripts: cross-platform, typed, testable.
//! `justfile` is a human-facing wrapper around these commands and is never a
//! source of truth — anything CI depends on lives here.
//!
//! ## Commands (doc 01 §6.3)
//!
//! | Command | Purpose | Phase |
//! |---|---|---|
//! | `check-deps` | Resolve cargo metadata, assert the `deps.toml` matrix, regenerate the doc 00 §8.1 table and fail if it differs | 0 |
//! | `check-no-game-ids` | Grep `crates/` + `services/` for game id literals and `games::` imports outside the registry | 0 |
//! | `check-manifests` | `game.toml` == compiled `GameMetadata`/`GameCapabilities` | 0 |
//! | `new-game <slug>` | Scaffold a game crate from the template (doc 02 §10.1) | 0 |
//! | `selfplay <game>` | Bot-vs-bot matches with full invariant checking | 0 |
//! | `replay <file>` | Replay a `.tbr` locally and print the first divergence | 0 |
//! | `perft <depth>` | Chess move-generation counts | 1 |
//! | `gen-tokens` | `tokens.toml` → `tokens.css` + `generated.rs` + `tokens.json` | 2 |
//! | `check-no-raw-colors` | No hex literals or `Color::new(` outside `tabula-design` | 2 |
//! | `pack-assets <game>` | Build, hash, and manifest a game's asset pack | 3 |
//! | `gen-protocol-vectors` | Regenerate golden wire vectors — requires `--bump minor\|major` | 4 |
//! | `check-protocol` | Golden vectors match; the version-bump gate (I-13) | 4 |
//! | `db reset` / `db migrate` | Local Postgres lifecycle | 4 |
//! | `load --scenario <id>` | Run a load scenario, compare against the committed baseline | 4 |
//!
//! ## `check-deps` is the important one
//!
//! It must walk the **resolved** dependency graph per crate, not the declared
//! `[dependencies]` block. A transitive violation — crate A depends on B which
//! depends on `tokio` — has to fail exactly as loudly as a direct one, because
//! that is how I-1 actually gets broken in practice.
//!
//! It also **regenerates the doc 00 §8.1 table** from `deps.toml` and fails if
//! the committed docs differ. An architecture rule that lives only in prose is a
//! rule that will be broken within two months (doc 00 §8.2).
//!
//! ## Implementation sketch
//!
//! ```rust,ignore
//! let meta = cargo_metadata::MetadataCommand::new().exec()?;
//! let rules = deps::Matrix::load("deps.toml")?;
//! for pkg in workspace_members(&meta) {
//!     let resolved = transitive_deps(&meta, pkg);      // the RESOLVED graph
//!     for dep in resolved {
//!         if !rules.allows(pkg, dep) {
//!             bail!("I-1: {pkg} may not depend on {dep}\n  path: {}", explain_path(..));
//!         }
//!     }
//! }
//! ```
//!
//! Print the *path* — "tabula-core → foo → tokio" — not just the violation.
//! Without the path, the next step is a twenty-minute `cargo tree` session.

fn main() {
    let cmd = std::env::args().nth(1);

    match cmd.as_deref() {
        // TODO(phase 0): implement in this order, per doc 09 §7 step 4.
        // check-deps first: it is the one that proves the whole enforcement idea
        // works, and it is the one that guards every later crate.
        Some("check-deps") => todo!("doc 00 §8.2 — walk the RESOLVED graph against deps.toml"),
        Some("check-no-game-ids") => {
            todo!("I-9 — grep crates/ and services/, exempting tabula-registry")
        }
        Some("check-manifests") => todo!("doc 02 §4.3 — game.toml vs compiled metadata"),
        Some("new-game") => todo!("doc 02 §10.1 — scaffold from games/tictactoe"),
        Some("selfplay") => todo!("doc 02 §11.3 — the acceptance gate for Phase 0"),
        Some("replay") => todo!("doc 05 §8.3 — ReplayRunner::verify, print first divergence"),

        // Phase 2+
        Some("gen-tokens") => todo!("doc 04 §8.1"),
        Some("check-no-raw-colors") => todo!("doc 04 §8.2"),
        Some("pack-assets") => todo!("doc 04 §12"),

        // Phase 4+
        Some("gen-protocol-vectors" | "check-protocol") => todo!("doc 05 §9.2 — I-13 version gate"),
        Some("db") => todo!("sqlx migrate / reset"),
        Some("load") => todo!("doc 06 §10 — scenarios L1..L8"),

        other => {
            if let Some(c) = other {
                eprintln!("unknown command: {c}\n");
            }
            eprintln!(
                "usage: cargo xtask <command>\n\n\
                 phase 0:  check-deps  check-no-game-ids  check-manifests\n\
                           new-game <slug>  selfplay <game>  replay <file>\n\
                 phase 2:  gen-tokens  check-no-raw-colors\n\
                 phase 3:  pack-assets <game>\n\
                 phase 4:  gen-protocol-vectors  check-protocol  db  load\n\n\
                 See xtask/README.md and docs/architecture/01-stack-and-repository-plan.md §6.3."
            );
            std::process::exit(2);
        }
    }
}
