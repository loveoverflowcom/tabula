//! Process-level checks for the activated `pack-assets` command dispatch.

use std::{path::Path, process::Command};

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

#[test]
fn pack_assets_resolves_workspace_from_a_nested_directory() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should be inside the workspace");
    let nested_directory = repository_root.join("games/chess");
    let expected_manifest = repository_root.join("games/does-not-exist/game.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(nested_directory)
        .args(["pack-assets", "does-not-exist"])
        .output()
        .expect("xtask binary should be runnable");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("xtask stderr should be UTF-8");
    assert!(stderr.contains(&expected_manifest.display().to_string()));
    assert!(!stderr.contains("games/chess/games/does-not-exist"));
}
