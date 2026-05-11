use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn cleanup_ephemeral_workspace(path: &Path) -> Result<bool> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }

    Ok(!path.exists())
}
