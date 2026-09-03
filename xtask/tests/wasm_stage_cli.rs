//! Process-level integration tests for `cargo xtask stage-wasm-game`.

use std::process::Command;
#[test]
fn stage_wasm_game_rejects_removed_output_override() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["stage-wasm-game", "--out-dir", "unused"])
        .output()
        .expect("xtask binary should be runnable");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is valid utf8");
    assert!(stderr.contains("unexpected argument for stage-wasm-game: --out-dir"));
}
