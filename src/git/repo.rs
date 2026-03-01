use anyhow::{Context, Result};
use git2::Repository;
use std::path::Path;

/// Open the Git repository at or above the given path.
pub fn open(path: &Path) -> Result<Repository> {
    Repository::discover(path).context("Not in a Git repository. Run `git init` first.")
}

/// Initialize a new Git repository at the given path.
pub fn init(path: &Path) -> Result<Repository> {
    Repository::init(path).context("Failed to initialize Git repository")
}

/// Get the current branch name, if on a branch.
pub fn current_branch(repo: &Repository) -> Result<Option<String>> {
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(None),
    };

    if head.is_branch() {
        Ok(head.shorthand().map(String::from))
    } else {
        Ok(None)
    }
}
