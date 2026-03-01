use anyhow::{bail, Result};

use crate::context::ArcContext;

pub fn run() -> Result<()> {
    let ctx = ArcContext::open()?;

    // Push current branch
    let status = std::process::Command::new("git")
        .args(["push"])
        .current_dir(&ctx.repo_root)
        .status()?;

    if !status.success() {
        bail!("git push failed");
    }

    // Push arc metadata refs
    let status = std::process::Command::new("git")
        .args(["push", "origin", "refs/arc/*:refs/arc/*"])
        .current_dir(&ctx.repo_root)
        .status()?;

    if !status.success() {
        // Non-fatal: remote might not exist or might not support it
        eprintln!("Note: could not push Arc metadata refs (this is OK for new repos without a remote).");
    }

    println!("Pushed.");
    Ok(())
}
