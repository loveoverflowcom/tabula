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
//! | `check` | Run every gate below (plus fmt/clippy/test/cargo-deny) in fail-fast order — the one command a PR needs to be confident in | 0 |
//! | `check-deps` | Walk the resolved cargo metadata graph, assert the `deps.toml` matrix (I-1, I-15) | 0 |
//! | `check-no-game-ids` | Scan the tree for game id literals outside their game package, the registry, tests, manifests, and docs (I-9) | 0 |
//! | `check-manifests` | Validate workspace `Cargo.toml`s (inheritance, no wildcard versions, `{ workspace = true }` over duplicated paths, game feature shape) and `game.toml` schemas | 0 |
//! | `new-game <slug>` | Scaffold a game crate from the template (doc 02 §10.1) | 0 |
//! | `selfplay <game>` | Bot-vs-bot matches with full invariant checking | 0 |
//! | `replay <file>` | Replay a `.tbr` locally; `--diagnose` prints evidence strength | 0 |
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

mod check_cmd;
mod colors_cmd;
mod deps_cmd;
mod deps_policy;
mod game_ids_cmd;
mod game_ids_policy;
mod graph;
mod manifest_cmd;
mod manifest_policy;
mod pack_assets_cmd;
mod perft_cmd;
mod replay_cmd;
mod replay_goldens_cmd;
mod selfplay_cmd;
mod tokens_cmd;

fn main() {
    let cmd = std::env::args().nth(1);

    match cmd.as_deref() {
        Some("check") => {
            if !check_cmd::run() {
                std::process::exit(1);
            }
        }
        Some("check-deps") => match deps_cmd::run() {
            Ok(true) => {}
            Ok(false) => std::process::exit(1),
            Err(err) => {
                eprintln!("check-deps: {err}");
                std::process::exit(2);
            }
        },
        Some("check-no-game-ids") => match game_ids_cmd::run() {
            Ok(true) => {}
            Ok(false) => std::process::exit(1),
            Err(err) => {
                eprintln!("check-no-game-ids: {err}");
                std::process::exit(2);
            }
        },
        Some("check-manifests") => match manifest_cmd::run() {
            Ok(true) => {}
            Ok(false) => std::process::exit(1),
            Err(err) => {
                eprintln!("check-manifests: {err}");
                std::process::exit(2);
            }
        },
        Some("new-game") => {
            unimplemented_command("new-game", "doc 02 §10.1 — scaffold from games/tictactoe")
        }
        Some("selfplay") => match selfplay_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("selfplay: {err}");
                std::process::exit(2);
            }
        },
        Some("replay") => match replay_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("replay: {err}");
                std::process::exit(1);
            }
        },
        Some("replay-goldens") => match replay_goldens_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("replay-goldens: {err}");
                std::process::exit(1);
            }
        },

        // Phase 1
        Some("perft") => match perft_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("perft: {err}");
                std::process::exit(2);
            }
        },

        // Phase 2+
        Some("gen-tokens") => match tokens_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("gen-tokens: {err}");
                std::process::exit(2);
            }
        },
        Some("check-no-raw-colors") => match colors_cmd::run() {
            Ok(true) => {}
            Ok(false) => std::process::exit(1),
            Err(err) => {
                eprintln!("check-no-raw-colors: {err}");
                std::process::exit(2);
            }
        },
        Some("pack-assets") => match pack_assets_cmd::run() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("pack-assets: {err}");
                std::process::exit(2);
            }
        },

        // Phase 4+
        Some("gen-protocol-vectors") => {
            future_command("gen-protocol-vectors", 4, "doc 05 §9.2 — I-13 version gate")
        }
        Some("check-protocol") => {
            future_command("check-protocol", 4, "doc 05 §9.2 — I-13 version gate")
        }
        Some("db") => future_command("db", 4, "doc 06 §3 — sqlx migrate / reset"),
        Some("load") => future_command("load", 4, "doc 06 §10 — scenarios L1..L8"),

        other => print_usage_and_exit(other),
    }
}

fn print_usage_and_exit(other: Option<&str>) -> ! {
    if let Some(c) = other {
        eprintln!("unknown command: {c}\n");
    }
    eprintln!(
        "usage: cargo xtask <command>\n\n\
         local gate:  check   (fmt, clippy, test, check-deps, check-no-game-ids,\n\
                                check-manifests, cargo-deny, in that order)\n\n\
         phase 0:  check-deps  check-no-game-ids  check-manifests\n\
                   new-game <slug>  selfplay <game>  replay <file> [--verify] [--at N] [--diagnose] [--write-reproducer PATH]\n\
                   replay-goldens (intentional fixture regeneration)\n\
         phase 1:  perft chess [depth]\n\
         phase 2:  gen-tokens  check-no-raw-colors\n\
         phase 3:  pack-assets <game>\n\
         phase 4:  gen-protocol-vectors  check-protocol  db  load\n\n\
         See xtask/README.md and docs/architecture/01-stack-and-repository-plan.md §6.3."
    );
    std::process::exit(2);
}

pub fn unavailable_message(command: &str, planned_phase: u8, doc_ref: &str) -> String {
    format!(
        "{command} is not available in Phase 2 (planned activation: Phase {planned_phase}, {doc_ref})"
    )
}

pub fn unimplemented_message(command: &str, doc_ref: &str) -> String {
    format!("{command} is not yet implemented ({doc_ref})")
}

fn future_command(command: &str, planned_phase: u8, doc_ref: &str) -> ! {
    eprintln!("{}", unavailable_message(command, planned_phase, doc_ref));
    std::process::exit(2);
}

fn unimplemented_command(command: &str, doc_ref: &str) -> ! {
    eprintln!("{}", unimplemented_message(command, doc_ref));
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_commands_produce_intentional_messages_without_panic() {
        let msg = unavailable_message("check-protocol", 4, "doc 05 §9.2 — I-13 version gate");
        assert_eq!(
            msg,
            "check-protocol is not available in Phase 2 (planned activation: Phase 4, doc 05 §9.2 — I-13 version gate)"
        );

        let msg = unavailable_message("pack-assets", 3, "doc 04 §12");
        assert_eq!(
            msg,
            "pack-assets is not available in Phase 2 (planned activation: Phase 3, doc 04 §12)"
        );

        let msg = unimplemented_message("new-game", "doc 02 §10.1 — scaffold from games/tictactoe");
        assert_eq!(
            msg,
            "new-game is not yet implemented (doc 02 §10.1 — scaffold from games/tictactoe)"
        );
    }
}
