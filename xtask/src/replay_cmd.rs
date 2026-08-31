//! `cargo xtask replay <file>` — Phase 1 canonical replay verification.
//!
//! Static game selection belongs here temporarily, at the tooling edge. The
//! reusable replay library stays generic over `GameRules`; Phase 4's registry
//! will replace this dispatch when that phase is allowed to become real.

use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_core::StateVersion;
use tabula_testkit::{
    DivergenceLocation, PrefixPosition, ReplayDiagnosis, ReplayError, ReplayIdentity, ReplayRunner,
    ReplayVerdict, ReproducerAvailability, ValidatedReplay, VerifyReport,
};

pub(crate) fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(2);
    let path = args
        .next()
        .ok_or_else(|| usage("a replay file is required"))?;
    let mut at = None;
    let mut diagnose = false;
    let mut write_reproducer = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verify" => {}
            "--diagnose" => diagnose = true,
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
            "--write-reproducer" => {
                let destination = args
                    .next()
                    .ok_or_else(|| usage("--write-reproducer requires a destination path"))?;
                write_reproducer = Some(PathBuf::from(destination));
            }
            _ => return Err(usage(&format!("unknown argument {arg:?}"))),
        }
    }

    if write_reproducer.is_some() && !diagnose {
        return Err(usage("--write-reproducer requires --diagnose"));
    }
    if diagnose && at.is_some() {
        return Err(usage("--diagnose cannot be combined with --at"));
    }

    let path_ref = Path::new(&path);
    let game_id = ValidatedReplay::read(path_ref)
        .map_err(|error| format!("{path}: {error}"))?
        .header()
        .game_id
        .clone();
    match game_id.as_str() {
        "com.tabula.tictactoe" => verify::<tabula_game_tictactoe::TicTacToeModule>(
            path_ref,
            at,
            diagnose,
            write_reproducer.as_deref(),
        ),
        "com.tabula.chess" => verify::<tabula_game_chess::ChessModule>(
            path_ref,
            at,
            diagnose,
            write_reproducer.as_deref(),
        ),
        _ => Err(format!(
            "{path}: unsupported game {game_id}; Phase 1 tooling supports tictactoe and chess"
        )),
    }
}

fn verify<M: tabula_game_api::GameModule>(
    path: &Path,
    at: Option<StateVersion>,
    diagnose: bool,
    reproducer_destination: Option<&Path>,
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
                print_first_failing_evidence(&divergence);
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
    if diagnose {
        let diagnoses = report.diagnoses();
        if diagnoses.is_empty() {
            println!("diagnosis: NONE (all stored replay evidence agrees)");
        } else {
            for diagnosis in &diagnoses {
                print_diagnosis(diagnosis);
            }
        }
        if let Some(destination) = reproducer_destination {
            ensure_distinct_paths(path, destination)?;
            let first_diagnosis = diagnoses.first().ok_or_else(|| {
                "cannot write reproducer: replay has no divergence diagnosis".to_owned()
            })?;
            write_reproducer::<M>(&runner, first_diagnosis, path, destination)?;
        }
    }
    if report.is_verified() {
        Ok(())
    } else {
        Err(format!("{}: replay verification failed", path.display()))
    }
}

fn print_report(path: &Path, report: &VerifyReport) {
    let verdict = match report.verdict() {
        ReplayVerdict::Exact => "EXACT",
        ReplayVerdict::CompatibleVersion => "COMPATIBLE_VERSION",
        ReplayVerdict::NeedsMigration { .. } => "NEEDS_MIGRATION",
        ReplayVerdict::Unreplayable { .. } => "UNREPLAYABLE",
    };
    println!(
        "file: {}\ninputs: {}\ncheckpoints checked: {}\nfinal state hash: {}\nfinal hash checked: {}\nterminal outcome expected: {:?}\nterminal outcome actual: {:?}\nterminal outcome checked: {}\nevidence: state checkpoints, final state hash, terminal outcome\nverdict: {}",
        path.display(),
        report.inputs_replayed(),
        report.checkpoints_checked(),
        hex32(report.actual_final_state_hash().0),
        report.final_hash_checked(),
        report.expected_outcome(),
        report.actual_outcome(),
        report.outcome_checked(),
        verdict,
    );
    if report.divergences().is_empty() {
        println!("status: VERIFIED");
    } else {
        println!("status: DIVERGED");
        if let Some(divergence) = report.divergences().first() {
            print_first_failing_evidence(divergence);
        }
    }
}

fn print_first_failing_evidence(divergence: &tabula_testkit::Divergence) {
    let logical_time = divergence
        .logical_time
        .map_or_else(|| "<unknown>".to_owned(), |time| time.0.to_string());
    println!(
        "first failing evidence: kind={:?} input_index={} logical_ms={} expected={} actual={} previous_checkpoint={} next_checkpoint={} rules_version={} rules_hash={} expected_outcome={:?} actual_outcome={:?}",
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

fn print_diagnosis(diagnosis: &ReplayDiagnosis) {
    let evidence = diagnosis.evidence();
    print!(
        "diagnosis: kind={} evidence_input_index={} expected={} actual={}",
        diagnosis.kind(),
        evidence.input_index,
        hex32(evidence.expected),
        hex32(evidence.actual),
    );
    match diagnosis.location() {
        DivergenceLocation::Exact(exact) => println!(
            " location=EXACT input_index={} previous_verified={}",
            exact.input_index().0,
            exact.previous_verified().0,
        ),
        DivergenceLocation::Window(window) => println!(
            " location=WINDOW after_verified={} at_or_before={} first_failing_evidence={}",
            format_input_index(window.after_verified()),
            window.at_or_before().0,
            window.first_failing_evidence().0,
        ),
        DivergenceLocation::FinalEvidenceOnly(final_only) => println!(
            " location=FINAL_ONLY after_verified={} final_input={}",
            format_input_index(final_only.after_verified()),
            format_input_index(final_only.final_input()),
        ),
    }
}

fn write_reproducer<M: tabula_game_api::GameModule>(
    runner: &ReplayRunner<M::Rules>,
    diagnosis: &ReplayDiagnosis,
    source: &Path,
    destination: &Path,
) -> Result<(), String> {
    match runner.reproducer(diagnosis) {
        ReproducerAvailability::Available(replay) => {
            let frames = replay.frames().len();
            let bytes = replay
                .to_bytes()
                .map_err(|error| format!("cannot encode reproducer: {error}"))?;
            fs::write(destination, bytes)
                .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
            println!(
                "reproducer: wrote {} (frames={}, failing_evidence_input={})",
                destination.display(),
                frames,
                diagnosis.evidence().input_index,
            );
            Ok(())
        }
        ReproducerAvailability::OriginalReplayIsMinimal => {
            let bytes = fs::read(source)
                .map_err(|error| format!("cannot read {}: {error}", source.display()))?;
            fs::write(destination, bytes)
                .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
            println!(
                "reproducer: copied original replay to {} (original replay is already minimal)",
                destination.display()
            );
            Ok(())
        }
        ReproducerAvailability::InsufficientEvidence { reason } => Err(format!(
            "cannot write reproducer for {}: {reason}",
            diagnosis.kind()
        )),
    }
}

fn ensure_distinct_paths(source: &Path, destination: &Path) -> Result<(), String> {
    let source_canonical = fs::canonicalize(source)
        .map_err(|error| format!("cannot resolve source {}: {error}", source.display()))?;
    let destination_canonical = match fs::canonicalize(destination) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = destination.parent().unwrap_or_else(|| Path::new("."));
            let parent = fs::canonicalize(parent).map_err(|parent_error| {
                format!(
                    "cannot resolve destination directory {}: {parent_error}",
                    parent.display()
                )
            })?;
            let name = destination
                .file_name()
                .ok_or_else(|| format!("destination {} has no file name", destination.display()))?;
            parent.join(name)
        }
        Err(error) => {
            return Err(format!(
                "cannot resolve destination {}: {error}",
                destination.display()
            ));
        }
    };
    if source_canonical == destination_canonical {
        return Err("reproducer destination must differ from the source replay".to_owned());
    }
    Ok(())
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

fn format_input_index(value: Option<tabula_core::InputIndex>) -> String {
    value.map_or_else(|| "<none>".to_owned(), |value| value.0.to_string())
}

fn usage(reason: &str) -> String {
    format!(
        "{reason}\nusage: cargo xtask replay <file> [--verify] [--at N] [--diagnose] [--write-reproducer PATH]"
    )
}
