use anyhow::{bail, Context, Result};
use git2::{Repository, Signature};
use std::process::{Command, Stdio};

use crate::format::commit_message::{self, CommitMetadata};

/// Stage all changes and create a commit.
pub fn create_commit(
    repo: &Repository,
    summary: &str,
    intent: Option<&str>,
    metadata: &CommitMetadata,
) -> Result<git2::Oid> {
    let workdir = repo.workdir().context("bare repo not supported")?;
    stage_all(workdir)?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let sig = repo.signature().or_else(|_| Signature::now("Arc User", "arc@localhost"))?;
    let message = commit_message::format(summary, intent, metadata);

    let parent_commit = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None,
    };

    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)?;
    Ok(oid)
}

/// Create an empty commit (for declaring a change before writing code).
pub fn create_empty_commit(
    repo: &Repository,
    summary: &str,
    intent: Option<&str>,
    metadata: &CommitMetadata,
) -> Result<git2::Oid> {
    let head = repo.head().context("No HEAD — create an initial commit first")?;
    let parent = head.peel_to_commit()?;
    let tree = parent.tree()?;

    let sig = repo.signature().or_else(|_| Signature::now("Arc User", "arc@localhost"))?;
    let message = commit_message::format(summary, intent, metadata);

    let oid = repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent])?;
    Ok(oid)
}

/// Create a revert commit for the given commit SHA, and update the working tree.
pub fn revert_commit(repo: &Repository, target: git2::Oid) -> Result<git2::Oid> {
    let commit = repo.find_commit(target)?;
    let head = repo.head()?.peel_to_commit()?;

    let mut revert_index = repo.revert_commit(&commit, &head, 0, None)?;
    let tree_oid = revert_index.write_tree_to(repo)?;
    let tree = repo.find_tree(tree_oid)?;

    let sig = repo.signature().or_else(|_| Signature::now("Arc User", "arc@localhost"))?;
    let msg = format!("Revert: {}", commit.summary().unwrap_or("unknown"));

    let oid = repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[&head])?;

    // Update working tree to match the new commit
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::new().force(),
    ))?;

    Ok(oid)
}

/// Stage all changes via git CLI (avoids libgit2 terminal output quirks).
fn stage_all(workdir: &std::path::Path) -> Result<()> {
    let status = Command::new("git")
        .args(["add", "-A"])
        .current_dir(workdir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git add")?;

    if !status.success() {
        bail!("git add -A failed");
    }
    Ok(())
}
