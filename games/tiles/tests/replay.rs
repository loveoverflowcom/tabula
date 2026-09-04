//! Replay evidence that survives a code change.
//!
//! # Why a committed corpus, when three other things already check replay
//!
//! The conformance suite re-runs a script three times, a property test compares
//! live and replayed state at every checkpoint, and self-play runs each match
//! twice and diffs the whole semantic trace. All three compare **the current
//! code against itself**, so all three stay green through a rules change that
//! silently alters historical behaviour.
//!
//! `tests/replays/tiles-golden.tbr` is the one that does not: it is a whole
//! match — the shuffle, all 71 draws, every merge, completion scoring, follower
//! returns, end-of-game partial scoring, and the final standings — with its
//! final state hash committed here as a literal. A rules change that alters any
//! of that fails this test with a precise diff, forcing an explicit
//! `RULES_VERSION` bump and a migration decision (doc 02 §11.4).
//!
//! The golden is regenerated only by `cargo xtask replay-goldens`, never by a
//! test: a harness that rewrites its own expectations is a harness nobody
//! trusts.

use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_core::canonical_decode;
use tabula_game_api::{GameRules, Input};
use tabula_game_tiles::{Command, Config, TilesModule, TilesRules};
use tabula_testkit::{ReplayIdentity, ReplayRunner, ValidatedReplay};

fn replay_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/replays/{name}"))
}

/// **The durable oracle.** The committed bytes must replay to the committed
/// hash, through the ordinary typed rules functions.
#[test]
fn the_committed_tiles_replay_reproduces_its_recorded_final_hash() {
    let path = replay_path("tiles-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("the committed replay must decode");

    // A whole match: every tile in the bag reached the board or the discards.
    assert!(
        replay.frames().len() > 100,
        "the golden holds only {} frames; a whole 71-tile match cannot fit in that, \
         so the corpus no longer covers what this file claims it does",
        replay.frames().len()
    );

    let runner =
        ReplayRunner::<TilesRules>::open(&path, ReplayIdentity::from_module::<TilesModule>())
            .expect("the committed replay must match the typed runner");

    // **Exact**, not merely `CompatibleVersion`. A differing `rules_hash` in
    // the header downgrades the verdict rather than failing
    // (`ReplayRunner::check`), so a golden whose header went stale would keep
    // passing a `is_verified()`-only assertion while no longer being evidence
    // that *this* rules build produced it. Asserting the verdict is what keeps
    // "regenerate the corpus when the rules change" enforced rather than
    // remembered. The committed tic-tac-toe golden is currently in exactly
    // that state; see the PR notes.
    assert_eq!(
        runner.check(),
        tabula_testkit::ReplayVerdict::Exact,
        "the golden's recorded rules hash no longer matches this build; \
         regenerate it with `cargo xtask replay-goldens` and decide whether \
         RULES_VERSION should move"
    );

    let mut runner = runner;
    let report = runner.verify().expect("replay execution must succeed");
    assert!(report.is_verified(), "{report:?}");
    assert_eq!(report.expected_outcome(), report.actual_outcome());
    assert!(
        report.expected_outcome().is_some(),
        "the golden must be a terminal match, so it pins the standings too"
    );
    assert_eq!(
        report.actual_final_state_hash().0,
        [
            0xe0, 0xfd, 0xee, 0xb3, 0x2a, 0xb5, 0xf6, 0x6f, 0x1c, 0x31, 0xde, 0x8d, 0xd4, 0x1f,
            0x35, 0xe2, 0x74, 0xae, 0x9a, 0x7a, 0xe3, 0xab, 0xaf, 0xd0, 0x24, 0x89, 0xb0, 0x50,
            0xff, 0x7a, 0x7c, 0xab,
        ]
    );
}

/// The golden carries both halves of a turn, so a change to either the
/// placement step or the claim step moves the hash.
#[test]
fn the_committed_tiles_replay_covers_placements_claims_and_passes() {
    let path = replay_path("tiles-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("the committed replay must decode");

    let mut placements = 0usize;
    let mut claims = 0usize;
    let mut passes = 0usize;
    for frame in replay.frames() {
        let input: Input<Command> = canonical_decode(&frame.input)
            .expect("every golden frame must use canonical input encoding");
        match input {
            Input::Player {
                command: Command::PlaceTile { .. },
                ..
            } => placements += 1,
            Input::Player {
                command: Command::PlaceMeeple { .. },
                ..
            } => claims += 1,
            Input::Player {
                command: Command::SkipMeeple,
                ..
            } => passes += 1,
            _ => {}
        }
    }

    assert!(
        placements > 60,
        "only {placements} placements in the golden"
    );
    assert!(claims > 0, "the golden never claims a follower");
    assert!(passes > 0, "the golden never passes on a follower");

    let config: Config = canonical_decode(&replay.header().config)
        .expect("the golden config must use canonical encoding");
    assert_eq!(
        config.turn_deadline_ms, 0,
        "the golden is the no-deadline configuration; a deadline would add timer frames"
    );
    assert_eq!(replay.header().roster.len(), 3);
}

// ---------------------------------------------------------------------------
// The rules-source hash oracle
// ---------------------------------------------------------------------------

/// `RULES_HASH` comes from `build.rs`. This recomputes it here, independently,
/// from the same documented preimage — so a build script that quietly stopped
/// covering part of the rules subtree would be caught rather than trusted.
#[test]
fn the_rules_hash_matches_an_independent_recomputation_over_the_rules_subtree() {
    let sources = independent_rules_sources();
    assert!(!sources.is_empty());
    assert_eq!(TilesRules::RULES_HASH, independent_rules_hash(&sources));
}

#[test]
fn every_rules_subtree_file_is_discovered_recursively_and_in_order() {
    let root = rules_root();
    let sources = independent_rules_sources();
    let source_paths: Vec<_> = sources.iter().map(|(path, _)| path.clone()).collect();
    let mut filesystem_paths = Vec::new();
    collect_rust_paths(&root, &root, &mut filesystem_paths);

    assert_eq!(filesystem_paths.len(), source_paths.len());
    assert!(filesystem_paths
        .iter()
        .all(|path| source_paths.contains(path)));
    assert!(
        source_paths.windows(2).all(|paths| paths[0] < paths[1]),
        "the preimage must be built in a stable order or the hash is not reproducible"
    );
    assert!(source_paths
        .iter()
        .all(|path| !path.is_absolute() && !path.starts_with("..")));
}

/// Changing one byte of canonical rules source must change the hash. Without
/// this, a hash that ignored file contents would satisfy the test above.
#[test]
fn a_change_to_canonical_rules_source_changes_the_hash() {
    let sources = independent_rules_sources();
    let before = independent_rules_hash(&sources);

    let mut mutated = sources.clone();
    mutated
        .last_mut()
        .expect("the rules subtree is not empty")
        .1
        .push(b'\n');
    assert_ne!(before, independent_rules_hash(&mutated));

    // And so must a rename, since the path is part of the preimage.
    let mut renamed = sources;
    renamed
        .last_mut()
        .expect("the rules subtree is not empty")
        .0 = PathBuf::from("renamed.rs");
    assert_ne!(before, independent_rules_hash(&renamed));
}

/// Canonical rules must not reach into the noncanonical halves of the crate.
///
/// Narrower than chess's equivalent check, deliberately: Tiles' rules subtree
/// *does* contain `crate::`, because the information model
/// (`rules/secret.rs`) names `TilesModule` from its own `cfg(test)` block to
/// assert the module declares the hidden information it has. What must never
/// appear is a dependency on `bot` or `presentation` — those are the halves the
/// rules hash deliberately excludes, so a rules file that used one would make
/// the hash under-cover its own inputs.
#[test]
fn canonical_rules_do_not_depend_on_the_bot_or_presentation_halves() {
    for (relative, bytes) in independent_rules_sources() {
        let source = std::str::from_utf8(&bytes).expect("Rust source must be UTF-8");
        for (line_number, line) in source.lines().enumerate() {
            // Prose in a doc comment may name them; code may not.
            let code = line.split("//").next().unwrap_or("");
            for forbidden in [
                "crate::bot",
                "crate::presentation",
                "super::bot",
                "super::presentation",
                "#[path",
                "include!",
            ] {
                assert!(
                    !code.contains(forbidden),
                    "canonical rules source {}:{} uses {forbidden}",
                    relative.display(),
                    line_number + 1
                );
            }
        }
    }
}

/// The noncanonical halves really are outside the hashed subtree, so the
/// exclusion above is a fact about the layout rather than a hope.
#[test]
fn the_bot_and_presentation_halves_live_outside_the_hashed_subtree() {
    let canonical_root = rules_root();
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for source in ["src/lib.rs", "src/bot.rs", "src/presentation.rs"] {
        let path = manifest_root.join(source);
        assert!(path.is_file(), "{source} must exist");
        assert!(
            !path.starts_with(&canonical_root),
            "{source} must remain outside {}",
            canonical_root.display()
        );
    }
}

// ---------------------------------------------------------------------------
// The two-step turn is visible in the recorded stream
// ---------------------------------------------------------------------------

/// Every claim decision in the golden belongs to the seat that placed the tile
/// immediately before it.
///
/// The converse does **not** hold and must not be asserted: a turn whose tile
/// offers nothing claimable ends at the placement, so a placement may be
/// followed directly by the next seat's placement. Asserting the stronger
/// property would be asserting a rule the game does not have.
///
/// A rules change that collapsed the claim step would still produce a
/// legal-looking command stream, and the final-hash test would say *something*
/// broke without saying what. This says what.
#[test]
fn the_golden_stream_shows_a_claim_decision_by_the_seat_that_just_placed() {
    let path = replay_path("tiles-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("the committed replay must decode");

    let mut awaiting_claim_from: Option<tabula_core::SeatId> = None;
    let mut turns_with_a_claim_step = 0usize;
    for frame in replay.frames() {
        let input: Input<Command> = canonical_decode(&frame.input)
            .expect("every golden frame must use canonical input encoding");
        let Input::Player { seat, command } = input else {
            continue;
        };
        match command {
            Command::PlaceTile { .. } => awaiting_claim_from = Some(seat),
            Command::PlaceMeeple { .. } | Command::SkipMeeple => {
                assert_eq!(
                    awaiting_claim_from,
                    Some(seat),
                    "a claim decision by seat {seat:?} did not follow that seat's own placement"
                );
                awaiting_claim_from = None;
                turns_with_a_claim_step += 1;
            }
        }
    }

    assert!(
        turns_with_a_claim_step > 30,
        "only {turns_with_a_claim_step} turns reached the claim step"
    );
    // A turn whose tile offered nothing to claim ends at the placement, so the
    // last frame may legitimately be a placement.
    assert!(
        replay.frames().len() > turns_with_a_claim_step,
        "every frame cannot be a claim decision"
    );
}

fn rules_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

fn independent_rules_sources() -> Vec<(PathBuf, Vec<u8>)> {
    let root = rules_root();
    let mut paths = Vec::new();
    collect_rust_paths(&root, &root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|relative| {
            let bytes = fs::read(root.join(&relative)).expect("rules source must be readable");
            (relative, bytes)
        })
        .collect()
}

/// The preimage `build.rs` documents:
/// `domain || rules_version || sorted(path || len || bytes)`.
fn independent_rules_hash(sources: &[(PathBuf, Vec<u8>)]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.source.v2");
    hasher.update(&TilesRules::RULES_VERSION.0.to_le_bytes());
    for (relative, bytes) in sources {
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn collect_rust_paths(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("rules directory must be readable") {
        let path = entry
            .expect("rules directory entry must be readable")
            .path();
        if path.is_dir() {
            collect_rust_paths(root, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(
                path.strip_prefix(root)
                    .expect("rules source must remain under rules directory")
                    .to_owned(),
            );
        }
    }
}
