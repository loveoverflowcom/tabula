use std::path::Path;

use tabula_core::{canonical_decode, LogicalTime};
use tabula_game_api::Input;
use tabula_game_chess::{ChessModule, ChessRules, Command, Config};
use tabula_testkit::{ReplayIdentity, ReplayRunner, ValidatedReplay};

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
    assert_eq!(
        report.actual_final_state_hash.0,
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
    assert_eq!(
        report.actual_final_state_hash.0,
        [
            0xbd, 0x61, 0x0f, 0x6d, 0x0a, 0xb8, 0x4f, 0x3b, 0xb0, 0x25, 0xa1, 0x4b, 0xcf, 0x1d,
            0xde, 0xd1, 0x10, 0x7e, 0x13, 0x85, 0x83, 0x9a, 0x8d, 0x59, 0x0b, 0x11, 0xf0, 0xb8,
            0x4e, 0x96, 0x64, 0xed,
        ]
    );
}
