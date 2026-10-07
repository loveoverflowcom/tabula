//! Shared workspace-root discovery for xtask commands.

use std::path::PathBuf;

/// Resolves the Cargo workspace containing the current working directory.
pub(crate) fn root() -> Result<PathBuf, cargo_metadata::Error> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    Ok(metadata.workspace_root.as_std_path().to_path_buf())
}

/// Cargo's actual target directory, including an explicit `CARGO_TARGET_DIR`.
pub(crate) fn target_dir() -> Result<PathBuf, cargo_metadata::Error> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    Ok(metadata.target_directory.as_std_path().to_path_buf())
}
