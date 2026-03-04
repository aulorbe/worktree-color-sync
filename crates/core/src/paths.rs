use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

pub fn expand_tilde(input: &str) -> Result<PathBuf> {
    if let Some(stripped) = input.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("home directory not found"))?;
        return Ok(home.join(stripped));
    }

    if input == "~" {
        return dirs::home_dir().ok_or_else(|| anyhow!("home directory not found"));
    }

    Ok(PathBuf::from(input))
}

pub fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create parent directory {}", parent.display()))?;
    Ok(())
}
