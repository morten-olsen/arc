use anyhow::{bail, Result};

use crate::context::ArcContext;

pub fn run(force: bool) -> Result<()> {
    let ctx = ArcContext::open()?;

    // Push current branch
    let mut args = vec!["push"];
    if force {
        args.push("--force-with-lease");
    }
    let status = std::process::Command::new("git")
        .args(&args)
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
