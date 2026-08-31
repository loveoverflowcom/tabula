//! `cargo xtask replay <file>` — Phase 1 canonical replay verification.
//!
//! Static game selection belongs here temporarily, at the tooling edge. The
//! reusable replay library stays generic over `GameRules`; Phase 4's registry
//! will replace this dispatch when that phase is allowed to become real.

use std::path::Path;

use tabula_core::StateVersion;
use tabula_testkit::{
    PrefixPosition, ReplayError, ReplayIdentity, ReplayRunner, ReplayVerdict, ValidatedReplay,
    VerifyReport,
};

pub(crate) fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(2);
    let path = args
        .next()
        .ok_or_else(|| usage("a replay file is required"))?;
    let mut at = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verify" => {}
            "--at" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--at requires a state version"))?;
                at = Some(StateVersion(
                    value
                        .parse()
                        .map_err(|_| usage("--at must be an unsigned integer"))?,
                ));
            }
            _ => return Err(usage(&format!("unknown argument {arg:?}"))),
        }
    }

    let path_ref = Path::new(&path);
    let game_id = ValidatedReplay::read(path_ref)
        .map_err(|error| format!("{path}: {error}"))?
        .header()
        .game_id
        .clone();
    match game_id.as_str() {
        "com.tabula.tictactoe" => verify::<tabula_game_tictactoe::TicTacToeModule>(path_ref, at),
        "com.tabula.chess" => verify::<tabula_game_chess::ChessModule>(path_ref, at),
        _ => Err(format!(
            "{path}: unsupported game {game_id}; Phase 1 tooling supports tictactoe and chess"
        )),
    }
}

fn verify<M: tabula_game_api::GameModule>(
    path: &Path,
    at: Option<StateVersion>,
) -> Result<(), String> {
    let identity = ReplayIdentity::from_module::<M>();
    let mut runner = ReplayRunner::<M::Rules>::open(path, identity)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    if let Some(target) = at {
        return match runner.seek(target) {
            Ok(position) => {
                let (status, evidence) = match position {
                    PrefixPosition::Verified(evidence) => ("PREFIX_VERIFIED", evidence),
                    PrefixPosition::Reconstructed(evidence) => ("POSITION_RECONSTRUCTED", evidence),
                };
                println!(
                    "game: {}\nrules version: {}\nstate version: {}\nstate hash: {}\ncheckpoints checked: {}\nfinal hash checked: {}\nterminal outcome checked: {}\nstatus: {}",
                    runner.header().game_id,
                    runner.header().rules_version.0,
                    evidence.state_version.0,
                    hex32(evidence.state_hash.0),
                    evidence.checkpoints_checked,
                    evidence.final_hash_checked,
                    evidence.outcome_checked,
                    status,
                );
                Ok(())
            }
            Err(ReplayError::PrefixDivergence { divergence }) => {
                println!("file: {}\nstatus: PREFIX_DIVERGED", path.display());
                print_divergence(&divergence);
                Err(format!(
                    "{}: replay prefix verification failed",
                    path.display()
                ))
            }
            Err(error) => Err(format!("{}: {error}", path.display())),
        };
    }

    let report = runner
        .verify()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    print_report(path, &report);
    if report.is_verified() {
        Ok(())
    } else {
        Err(format!("{}: replay verification failed", path.display()))
    }
}

fn print_report(path: &Path, report: &VerifyReport) {
    let verdict = match &report.verdict {
        ReplayVerdict::Exact => "EXACT",
        ReplayVerdict::CompatibleVersion => "COMPATIBLE_VERSION",
        ReplayVerdict::NeedsMigration { .. } => "NEEDS_MIGRATION",
        ReplayVerdict::Unreplayable { .. } => "UNREPLAYABLE",
    };
    println!(
        "file: {}\ninputs: {}\ncheckpoints checked: {}\nfinal state hash: {}\nfinal hash checked: {}\nterminal outcome expected: {:?}\nterminal outcome actual: {:?}\nterminal outcome checked: {}\nevidence: state checkpoints, final state hash, terminal outcome\nverdict: {}",
        path.display(),
        report.inputs_replayed,
        report.checkpoints_checked,
        hex32(report.actual_final_state_hash.0),
        report.final_hash_checked,
        report.expected_outcome,
        report.actual_outcome,
        report.outcome_checked,
        verdict,
    );
    if report.divergences.is_empty() {
        println!("status: VERIFIED");
    } else {
        println!("status: DIVERGED");
        if let Some(divergence) = report.divergences.first() {
            print_divergence(divergence);
        }
    }
}

fn print_divergence(divergence: &tabula_testkit::Divergence) {
    let logical_time = divergence
        .logical_time
        .map_or_else(|| "<unknown>".to_owned(), |time| time.0.to_string());
    println!(
        "first divergence: kind={:?} input_index={} logical_ms={} expected={} actual={} previous_checkpoint={} next_checkpoint={} rules_version={} rules_hash={} expected_outcome={:?} actual_outcome={:?}",
        divergence.kind,
        divergence.input_index,
        logical_time,
        hex32(divergence.expected),
        hex32(divergence.actual),
        format_option(divergence.previous_checkpoint),
        format_option(divergence.next_checkpoint),
        divergence.rules_version.0,
        hex32(divergence.rules_hash),
        divergence.expected_outcome,
        divergence.actual_outcome,
    );
}

fn hex32(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}

fn format_option(value: Option<u64>) -> String {
    value.map_or_else(|| "<none>".to_owned(), |value| value.to_string())
}

fn usage(reason: &str) -> String {
    format!("{reason}\nusage: cargo xtask replay <file> [--verify] [--at N]")
}
