use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_game_api::GameRules;
use tabula_testkit::{ReplayIdentity, ReplayRunner, ValidatedReplay};

use tabula_game_tictactoe::{TicTacToeModule, TicTacToeRules};

#[test]
fn rules_hash_covers_all_rules_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    collect_rust_sources(&root, &root, &mut sources);
    sources.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.v1");
    hasher.update(&TicTacToeRules::RULES_VERSION.0.to_le_bytes());
    for (relative, path) in sources {
        let bytes = fs::read(path).unwrap();
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    assert_eq!(TicTacToeRules::RULES_HASH, *hasher.finalize().as_bytes());
}

fn collect_rust_sources(root: &Path, directory: &Path, files: &mut Vec<(PathBuf, PathBuf)>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_sources(root, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push((path.strip_prefix(root).unwrap().to_owned(), path));
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
