use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Get the .arc/worktrees directory, creating it if needed.
pub fn worktrees_dir(repo_root: &Path) -> Result<PathBuf> {
    let dir = repo_root.join(".arc").join("worktrees");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Create a worktree for a task.
/// If `start_point` is provided, the worktree starts at that commit instead of HEAD.
/// Returns the path to the new worktree.
pub fn create(repo_root: &Path, slug: &str, branch: &str, start_point: Option<&str>) -> Result<PathBuf> {
    let wt_dir = worktrees_dir(repo_root)?;
    let wt_path = wt_dir.join(slug);

    if wt_path.exists() {
        bail!("Worktree already exists at {}", wt_path.display());
    }

    let mut cmd = std::process::Command::new("git");
    cmd.args(["worktree", "add", "-b", branch]);
    cmd.arg(&wt_path);
    if let Some(sp) = start_point {
        cmd.arg(sp);
    }
    let status = cmd
        .current_dir(repo_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to run git worktree add")?;

    if !status.success() {
        bail!("git worktree add failed");
    }

    Ok(wt_path)
}

/// Remove a worktree for a task.
pub fn remove(repo_root: &Path, slug: &str) -> Result<()> {
    let wt_path = worktrees_dir(repo_root)?.join(slug);
    if !wt_path.exists() {
        return Ok(());
    }

    let status = std::process::Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(&wt_path)
        .current_dir(repo_root)
        .status()
        .context("Failed to run git worktree remove")?;

    if !status.success() {
        bail!("git worktree remove failed");
    }
    Ok(())
}

/// List all Arc-managed worktrees (slugs).
pub fn list(repo_root: &Path) -> Result<Vec<String>> {
    let wt_dir = worktrees_dir(repo_root)?;
    let mut slugs = Vec::new();
    if wt_dir.exists() {
        for entry in std::fs::read_dir(&wt_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    slugs.push(name.to_string());
                }
            }
        }
    }
    slugs.sort();
    Ok(slugs)
}

/// Get the path to a worktree by slug.
pub fn path_for(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".arc").join("worktrees").join(slug)
}
