//! Process-level checks for the activated `pack-assets` command dispatch.

use std::process::Command;

#[test]
fn pack_assets_without_a_game_returns_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("pack-assets")
        .output()
        .expect("xtask binary should be runnable");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("xtask stderr should be UTF-8");
    assert!(stderr.contains("usage: cargo xtask pack-assets <game>"));
}
