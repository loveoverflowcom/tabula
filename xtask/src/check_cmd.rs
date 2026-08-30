//! `cargo xtask check` — the single local entrypoint (Phase 0 deliverable).
//!
//! Runs the gates that matter, in the order that fails fastest first (same
//! philosophy as the `justfile`'s `check` recipe), and stops at the first
//! failure so the developer fixes one thing at a time instead of reading a
//! wall of unrelated output.
//!
//! `fmt`, `clippy`, and `test` shell out to `cargo` (nothing else can run
//! them). `check-deps`, `check-no-game-ids`, and `check-manifests` run
//! in-process — they are this same binary, so there is no reason to pay for
//! a second `cargo run` per check. `cargo-deny` shells out to a separate,
//! optionally-installed binary; a missing binary is reported as a failure
//! with the install command, not silently skipped, because a check nobody
//! can prove ran is not a check.

use std::process::Command;

enum Step {
    Shell {
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        /// Printed on failure, in addition to the subprocess's own output.
        /// `cargo`'s "no such command" and a real lint failure both exit
        /// non-zero indistinguishably from here, so this is shown either way
        /// rather than trying to guess which one happened.
        failure_hint: Option<&'static str>,
    },
    InProcess {
        label: &'static str,
        run: fn() -> Result<bool, String>,
    },
}

pub fn run() -> bool {
    let steps: &[Step] = &[
        Step::Shell {
            label: "cargo fmt --all -- --check",
            program: "cargo",
            args: &["fmt", "--all", "--", "--check"],
            failure_hint: None,
        },
        Step::Shell {
            label: "cargo clippy --workspace --all-targets --all-features -- -D warnings",
            program: "cargo",
            args: &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
            failure_hint: None,
        },
        Step::Shell {
            label: "cargo test --workspace",
            program: "cargo",
            args: &["test", "--workspace"],
            failure_hint: None,
        },
        Step::InProcess {
            label: "check-deps",
            run: || crate::deps_cmd::run().map_err(|e| e.to_string()),
        },
        Step::InProcess {
            label: "check-no-game-ids",
            run: || crate::game_ids_cmd::run().map_err(|e| e.to_string()),
        },
        Step::InProcess {
            label: "check-manifests",
            run: || crate::manifest_cmd::run().map_err(|e| e.to_string()),
        },
        Step::Shell {
            label: "cargo deny check",
            program: "cargo",
            args: &["deny", "check"],
            failure_hint: Some(
                "if this says \"no such command: deny\", install it with:\n    cargo install cargo-deny --locked\n  see: https://embarkstudios.github.io/cargo-deny/",
            ),
        },
    ];

    for step in steps {
        let label = step_label(step);
        println!("\n=== xtask check: {label} ===");
        let ok = match step {
            Step::Shell {
                program,
                args,
                failure_hint,
                ..
            } => run_shell(program, args, label, *failure_hint),
            Step::InProcess { run, .. } => match run() {
                Ok(ok) => ok,
                Err(err) => {
                    eprintln!("{label}: {err}");
                    false
                }
            },
        };
        if !ok {
            eprintln!("\nxtask check: FAILED at `{label}`");
            return false;
        }
    }

    println!("\nxtask check: all gates passed");
    true
}

fn step_label(step: &Step) -> &'static str {
    match step {
        Step::Shell { label, .. } | Step::InProcess { label, .. } => label,
    }
}

fn run_shell(program: &str, args: &[&str], label: &str, failure_hint: Option<&str>) -> bool {
    let ok = match Command::new(program).args(args).status() {
        Ok(status) => status.success(),
        Err(err) => {
            eprintln!("{label}: failed to launch `{program}`: {err}");
            false
        }
    };
    if !ok {
        if let Some(hint) = failure_hint {
            eprintln!("{label}: {hint}");
        }
    }
    ok
}
