use anyhow::{bail, Context, Result};
use git2::Repository;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::index::sqlite;

/// Everything a command needs to interact with an Arc repository.
pub struct ArcContext {
    pub repo: Repository,
    pub repo_root: PathBuf,
    pub arc_dir: PathBuf,
    pub db: Connection,
}

impl ArcContext {
    /// Open an Arc context from the current directory.
    /// Works from the main worktree or any task worktree.
    pub fn open() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let repo = Repository::discover(&cwd)
            .context("Not in a Git repository. Run `arc init` first.")?;

        // For worktrees, the commondir points to the main repo's .git
        let repo_root = find_repo_root(&repo)?;
        let arc_dir = repo_root.join(".arc");

        if !arc_dir.exists() {
            bail!("Not an Arc repository. Run `arc init` first.");
        }

        let db = sqlite::open(&arc_dir)?;

        Ok(Self { repo, repo_root, arc_dir, db })
    }

    /// Get the current task ID by checking which worktree we're in.
    pub fn current_task_id(&self) -> Result<Option<String>> {
        let cwd = std::fs::canonicalize(std::env::current_dir()?)?;
        let wt_dir = std::fs::canonicalize(self.arc_dir.join("worktrees"))
            .unwrap_or_else(|_| self.arc_dir.join("worktrees"));

        // Check if cwd is inside a task worktree
        if let Ok(relative) = cwd.strip_prefix(&wt_dir) {
            let slug = relative
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .unwrap_or("");

            if !slug.is_empty() {
                // Look up task by worktree slug
                let mut stmt = self.db.prepare(
                    "SELECT id FROM tasks WHERE worktree_path LIKE ? AND status = 'in_progress'"
                )?;
                let task_id: Option<String> = stmt
                    .query_row([format!("%{slug}")], |row| row.get(0))
                    .ok();
                return Ok(task_id);
            }
        }

        Ok(None)
    }
}

/// Find the root of the main worktree (where .arc lives).
fn find_repo_root(repo: &Repository) -> Result<PathBuf> {
    // repo.commondir() points to the shared .git dir for all worktrees.
    // For the main worktree it equals repo.path(). For linked worktrees
    // it follows the commondir file back to the main .git directory.
    let common_git_dir = repo.commondir();

    // The repo root is the parent of the .git directory
    let root = common_git_dir
        .parent()
        .context("Cannot determine repo root")?;
    Ok(std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()))
}

/// Find the repo root for init (before .arc exists).
pub fn find_repo_root_from(path: &Path) -> Result<PathBuf> {
    let repo = Repository::discover(path)
        .context("Not in a Git repository")?;
    find_repo_root(&repo)
}
