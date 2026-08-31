use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_game_api::GameRules;
use tabula_testkit::{ReplayIdentity, ReplayRunner, ValidatedReplay};

use tabula_game_tictactoe::{TicTacToeModule, TicTacToeRules};

#[test]
fn rules_hash_matches_independent_rules_subtree_oracle() {
    let sources = independent_rules_sources();
    assert!(!sources.is_empty());
    assert_eq!(TicTacToeRules::RULES_HASH, independent_rules_hash(&sources));
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
fn bot_source_is_outside_rules_hash() {
    let sources = independent_rules_sources();
    assert!(!sources
        .iter()
        .any(|(path, _)| path == Path::new("../bot.rs")));
}

#[test]
fn presentation_source_is_outside_rules_hash() {
    let sources = independent_rules_sources();
    assert!(!sources
        .iter()
        .any(|(path, _)| path == Path::new("../ui.rs") || path.starts_with("../presentation")));
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
fn synthetic_non_rules_source_does_not_participate_in_compiled_hash() {
    let sources = independent_rules_sources();
    let compiled = TicTacToeRules::RULES_HASH;
    let mut with_non_rules_source = sources.clone();
    with_non_rules_source.push((PathBuf::from("../bot.rs"), b"synthetic bot".to_vec()));

    assert_eq!(compiled, independent_rules_hash(&sources));
    assert_ne!(compiled, independent_rules_hash(&with_non_rules_source));
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

fn independent_rules_hash(sources: &[(PathBuf, Vec<u8>)]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.source.v2");
    hasher.update(&TicTacToeRules::RULES_VERSION.0.to_le_bytes());
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

#[test]
fn committed_tictactoe_replay_reproduces_its_independent_final_hash() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/replays/tictactoe-golden.tbr");
    let replay = ValidatedReplay::read(&path).expect("committed replay must decode");
    assert_eq!(replay.frames().len(), 5);
    assert_eq!(replay.frames()[0].logical_time.0, 1_000);
    assert_eq!(replay.frames()[4].logical_time.0, 5_000);

    let mut runner = ReplayRunner::<TicTacToeRules>::open(
        &path,
        ReplayIdentity::from_module::<TicTacToeModule>(),
    )
    .expect("committed replay must match the typed runner");
    let report = runner.verify().expect("replay execution must succeed");
    assert!(report.is_verified(), "{report:?}");
    assert_eq!(report.expected_outcome(), report.actual_outcome());
    assert!(report.expected_outcome().is_some());
    assert_eq!(
        report.actual_final_state_hash().0,
        [
            0xee, 0xd9, 0x71, 0x6c, 0x09, 0xa0, 0xb5, 0x11, 0x81, 0x86, 0xf2, 0xd2, 0x29, 0x17,
            0xa2, 0x0f, 0xed, 0xdf, 0xe0, 0x77, 0x6d, 0x7d, 0x0e, 0x19, 0x32, 0x32, 0x32, 0xd5,
            0xd1, 0x6b, 0x9f, 0x5a,
        ]
    );
}
