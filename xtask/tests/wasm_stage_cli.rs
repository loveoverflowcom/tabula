//! Process-level integration tests for `cargo xtask stage-wasm-game`.

use std::process::Command;
use tempfile::tempdir;

#[test]
fn stage_wasm_game_stages_into_custom_output_directory() {
    let out_dir = tempdir().expect("temp dir created");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args([
            "stage-wasm-game",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("xtask binary should be runnable");

    assert!(
        output.status.success(),
        "stage-wasm-game failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout is valid utf8");
    assert!(stdout.contains("stage-wasm-game: staged browser host into"));
    assert!(stdout.contains("index.html"));
    assert!(stdout.contains("mq_js_bundle.js"));
    assert!(stdout.contains("tabula-game-client.wasm"));

    assert!(out_dir.path().join("index.html").is_file());
    assert!(out_dir.path().join("mq_js_bundle.js").is_file());
    assert!(out_dir.path().join("tabula-game-client.wasm").is_file());
}
