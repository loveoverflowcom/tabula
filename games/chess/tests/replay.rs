use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_core::{canonical_decode, LogicalTime};
use tabula_game_api::{GameRules, Input};
use tabula_game_chess::{ChessModule, ChessRules, Command, Config};
use tabula_testkit::{ReplayIdentity, ReplayRunner, ValidatedReplay};

#[test]
fn rules_hash_matches_independent_rules_subtree_oracle() {
    let sources = independent_rules_sources();
    assert!(!sources.is_empty());
    assert_eq!(ChessRules::RULES_HASH, independent_rules_hash(&sources));
}

#[test]
fn all_rules_subtree_files_are_discovered_recursively() {
    let root = rules_root();
    let sources = independent_rules_sources();
    let source_paths: Vec<_> = sources.iter().map(|(path, _)| path.clone()).collect();
    let mut filesystem_paths = Vec::new();
    collect_rust_paths(&root, &root, &mut filesystem_paths);

    assert_eq!(filesystem_paths.len(), source_paths.len());
    assert!(filesystem_paths
        .iter()
        .all(|path| source_paths.contains(path)));
    assert!(source_paths.windows(2).all(|paths| paths[0] < paths[1]));
    assert!(source_paths
        .iter()
        .all(|path| !path.is_absolute() && !path.starts_with("..")));
}

#[test]
fn canonical_source_mutation_changes_oracle_hash() {
    let sources = independent_rules_sources();
    let before = independent_rules_hash(&sources);
    let mut mutated = sources.clone();
    mutated[0].1[0] ^= 1;

    assert_ne!(before, independent_rules_hash(&mutated));
}

#[test]
fn canonical_tree_rejects_noncanonical_feature_sources() {
    let canonical_root = rules_root();
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    assert_noncanonical_feature_source(&canonical_root, &manifest_root.join("src/lib.rs"));
    assert_noncanonical_feature_source(&canonical_root, &manifest_root.join("src/bot.rs"));

    let misplaced_bot = canonical_root.join("bot.rs");
    assert!(
        !is_outside_canonical_tree(&canonical_root, &misplaced_bot),
        "a bot source under src/rules must be rejected by the source-boundary policy"
    );
}

#[test]
fn canonical_rules_do_not_depend_on_crate_root_sources() {
    for (relative, bytes) in independent_rules_sources() {
        let source = std::str::from_utf8(&bytes).expect("Rust source must be UTF-8");
        for forbidden in ["crate::", "super::super::", "#[path", "include!"] {
            assert!(
                !source.contains(forbidden),
                "canonical rules source {} must not use {forbidden}",
                relative.display()
            );
        }
    }
}

fn rules_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

fn assert_noncanonical_feature_source(canonical_root: &Path, source: &Path) {
    assert!(
        source.is_file(),
        "noncanonical feature source {} must remain outside src/rules",
        source.display()
    );
    assert!(
        is_outside_canonical_tree(canonical_root, source),
        "noncanonical feature source {} must not be under {}",
        source.display(),
        canonical_root.display()
    );
    assert!(
        !independent_rules_sources()
            .iter()
            .any(|(relative, _)| { relative.file_name() == source.file_name() }),
        "noncanonical feature source {} must not be duplicated in src/rules",
        source.display()
    );
}

fn is_outside_canonical_tree(canonical_root: &Path, source: &Path) -> bool {
    source.strip_prefix(canonical_root).is_err()
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

fn independent_rules_hash(sources: &[(PathBuf, Vec<u8>)]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.source.v2");
    hasher.update(&ChessRules::RULES_VERSION.0.to_le_bytes());
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

fn replay_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/replays/{name}"))
}

#[test]
fn committed_chess_replay_reproduces_its_independent_final_hash() {
    let path = replay_path("chess-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("committed replay must decode");
    assert_eq!(replay.frames().len(), 4);

    let mut runner =
        ReplayRunner::<ChessRules>::open(&path, ReplayIdentity::from_module::<ChessModule>())
            .expect("committed replay must match the typed runner");
    let report = runner.verify().expect("replay execution must succeed");
    assert!(report.is_verified(), "{report:?}");
    assert_eq!(report.expected_outcome(), report.actual_outcome());
    assert!(report.expected_outcome().is_some());
    assert_eq!(
        report.actual_final_state_hash().0,
        [
            0x73, 0x0e, 0xc3, 0xdc, 0x4d, 0xb8, 0xfc, 0x3b, 0xda, 0x3f, 0x2c, 0x8f, 0xc8, 0x7a,
            0xa7, 0x4d, 0x55, 0x2d, 0x88, 0xbc, 0x63, 0xe5, 0x0b, 0x10, 0xf3, 0xf4, 0x7d, 0x4a,
            0x75, 0xf6, 0xb3, 0x8b,
        ]
    );
}

#[test]
fn committed_chess_clock_replay_contains_a_recorded_timer_input() {
    let path = replay_path("chess-clock-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("committed replay must decode");
    assert_eq!(replay.frames().len(), 2);
    assert_eq!(replay.frames()[1].logical_time, LogicalTime(6_000));

    let timer: Input<Command> = canonical_decode(&replay.frames()[1].input)
        .expect("clock replay timer frame must use canonical input encoding");
    assert!(matches!(timer, Input::Timer { timer } if timer.0 == 1));

    let config: Config = canonical_decode(&replay.header().config)
        .expect("clock replay config must use canonical encoding");
    assert!(config.clock.is_some());

    let mut runner =
        ReplayRunner::<ChessRules>::open(&path, ReplayIdentity::from_module::<ChessModule>())
            .expect("committed clock replay must match the typed runner");
    let report = runner
        .verify()
        .expect("clock replay execution must succeed");
    assert!(report.is_verified(), "{report:?}");
    assert_eq!(report.expected_outcome(), report.actual_outcome());
    assert!(report.expected_outcome().is_some());
    assert_eq!(
        report.actual_final_state_hash().0,
        [
            0xbd, 0x61, 0x0f, 0x6d, 0x0a, 0xb8, 0x4f, 0x3b, 0xb0, 0x25, 0xa1, 0x4b, 0xcf, 0x1d,
            0xde, 0xd1, 0x10, 0x7e, 0x13, 0x85, 0x83, 0x9a, 0x8d, 0x59, 0x0b, 0x11, 0xf0, 0xb8,
            0x4e, 0x96, 0x64, 0xed,
        ]
    );
}
