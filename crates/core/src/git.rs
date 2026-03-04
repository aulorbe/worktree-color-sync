use crate::paths::canonical_or_original;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorktreeKey {
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
}

impl WorktreeKey {
    pub fn as_string(&self) -> String {
        format!(
            "{}\0{}",
            self.repo_root.display(),
            self.worktree_path.display()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub key: WorktreeKey,
    pub branch_ref: Option<String>,
}

#[derive(Debug, Clone)]
struct PorcelainEntry {
    worktree: PathBuf,
    branch: Option<String>,
}

pub fn resolve_worktree(cwd: &Path, timeout: Duration) -> Result<Option<WorktreeInfo>> {
    let toplevel = run_git(cwd, &["rev-parse", "--show-toplevel"], timeout)?;
    let toplevel = match toplevel {
        Some(v) => PathBuf::from(v.trim()),
        None => return Ok(None),
    };

    let common_dir = run_git(cwd, &["rev-parse", "--git-common-dir"], timeout)?
        .ok_or_else(|| anyhow!("failed to resolve git common dir"))?;

    let repo_root = repo_root_from_git_common_dir(&toplevel, common_dir.trim());

    let porcelain = run_git(cwd, &["worktree", "list", "--porcelain"], timeout)?
        .ok_or_else(|| anyhow!("failed to list worktrees"))?;
    let entries = parse_worktree_porcelain(&porcelain);

    let top_canon = canonical_or_original(&toplevel);
    let maybe_entry = entries
        .into_iter()
        .find(|entry| canonical_or_original(&entry.worktree) == top_canon);

    let Some(entry) = maybe_entry else {
        return Ok(Some(WorktreeInfo {
            key: WorktreeKey {
                repo_root,
                worktree_path: top_canon,
            },
            branch_ref: None,
        }));
    };

    Ok(Some(WorktreeInfo {
        key: WorktreeKey {
            repo_root,
            worktree_path: canonical_or_original(&entry.worktree),
        },
        branch_ref: entry.branch,
    }))
}

fn repo_root_from_git_common_dir(toplevel: &Path, common_dir: &str) -> PathBuf {
    let common_path = Path::new(common_dir);
    let common_abs = if common_path.is_absolute() {
        common_path.to_path_buf()
    } else {
        toplevel.join(common_path)
    };

    let common_abs = canonical_or_original(&common_abs);
    if common_abs.ends_with(".git") {
        common_abs
            .parent()
            .map(canonical_or_original)
            .unwrap_or(common_abs)
    } else {
        canonical_or_original(toplevel)
    }
}

fn run_git(cwd: &Path, args: &[&str], timeout: Duration) -> Result<Option<String>> {
    let start = Instant::now();
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(cwd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let output = cmd
        .output()
        .with_context(|| format!("failed to execute git with args {args:?}"))?;

    if start.elapsed() > timeout {
        return Err(anyhow!("git command timed out: {args:?}"));
    }

    if !output.status.success() {
        return Ok(None);
    }

    let text = String::from_utf8(output.stdout).context("invalid utf-8 from git output")?;
    Ok(Some(text))
}

fn parse_worktree_porcelain(raw: &str) -> Vec<PorcelainEntry> {
    let mut entries = Vec::new();
    let mut current: Option<PorcelainEntry> = None;

    for line in raw.lines() {
        if line.trim().is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(PorcelainEntry {
                worktree: PathBuf::from(path),
                branch: None,
            });
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(entry) = current.as_mut() {
                entry.branch = Some(branch.to_string());
            }
        }
    }

    if let Some(entry) = current.take() {
        entries.push(entry);
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_simple() {
        let raw = "worktree /tmp/repo\nHEAD abc\nbranch refs/heads/main\n\nworktree /tmp/feature\nHEAD def\nbranch refs/heads/feature\n";
        let parsed = parse_worktree_porcelain(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].branch.as_deref(), Some("refs/heads/feature"));
    }

    #[test]
    fn parse_porcelain_detached_head() {
        let raw = "worktree /tmp/repo\nHEAD abc\ndetached\n";
        let parsed = parse_worktree_porcelain(raw);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].branch, None);
    }

    #[test]
    fn parse_porcelain_with_spaces() {
        let raw = "worktree /tmp/my repo/wt one\nHEAD abc\nbranch refs/heads/wt-one\n";
        let parsed = parse_worktree_porcelain(raw);
        assert_eq!(parsed[0].worktree, PathBuf::from("/tmp/my repo/wt one"));
    }

    #[test]
    fn repo_root_uses_parent_when_common_dir_points_to_dot_git() {
        let toplevel = PathBuf::from("/tmp/repo/worktrees/feature");
        let root = repo_root_from_git_common_dir(&toplevel, "/tmp/repo/.git");
        assert_eq!(root, PathBuf::from("/tmp/repo"));
    }

    #[test]
    fn repo_root_falls_back_to_toplevel_when_common_dir_is_not_dot_git() {
        let toplevel = PathBuf::from("/tmp/repo/worktrees/feature");
        let root = repo_root_from_git_common_dir(&toplevel, "/tmp/repo/.git/worktrees/feature");
        assert_eq!(root, PathBuf::from("/tmp/repo/worktrees/feature"));
    }

    #[test]
    fn repo_root_handles_relative_common_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let toplevel = repo_root.join("worktrees/feature");

        std::fs::create_dir_all(repo_root.join(".git")).unwrap();
        std::fs::create_dir_all(&toplevel).unwrap();

        let root = repo_root_from_git_common_dir(&toplevel, "../../.git");
        assert_eq!(root, canonical_or_original(&repo_root));
    }
}
