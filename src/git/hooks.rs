use anyhow::{Context, Result};
use std::path::Path;

const HOOKS: &[&str] = &[
    "post-commit",
    "post-merge",
    "post-rebase",
    "post-checkout",
];

const HOOK_MARKER: &str = "# arc-managed-hook";

fn hook_script(event: &str) -> String {
    format!(
        "#!/bin/sh\n{HOOK_MARKER}\nexec arc hook {event}\n"
    )
}

/// Install Arc's Git hooks into the repository's hooks directory.
pub fn install(git_dir: &Path) -> Result<()> {
    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    for hook in HOOKS {
        let path = hooks_dir.join(hook);

        // Don't overwrite existing non-Arc hooks
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if !content.contains(HOOK_MARKER) {
                eprintln!("  Skipping {hook}: existing hook found (not managed by Arc)");
                continue;
            }
        }

        std::fs::write(&path, hook_script(hook))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

/// Remove Arc's Git hooks from the repository's hooks directory.
pub fn uninstall(git_dir: &Path) -> Result<()> {
    let hooks_dir = git_dir.join("hooks");
    for hook in HOOKS {
        let path = hooks_dir.join(hook);
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .context("reading hook")?;
            if content.contains(HOOK_MARKER) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}
