use anyhow::{bail, Result};

use crate::context::ArcContext;
use crate::git;

pub fn run(to: Option<String>) -> Result<()> {
    let ctx = ArcContext::open()?;

    if let Some(target_change_id) = to {
        // Undo back to a specific change
        let changes_to_undo: Vec<(String, String)> = {
            let mut stmt = ctx.db.prepare(
                "SELECT id, git_sha FROM changes
                 WHERE status = 'active' AND created_at > (
                     SELECT created_at FROM changes WHERE id LIKE ?1 || '%'
                 )
                 ORDER BY created_at DESC"
            )?;
            stmt.query_map([&target_change_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect()
        };

        if changes_to_undo.is_empty() {
            bail!("No changes found after '{target_change_id}'");
        }

        for (change_id, sha) in &changes_to_undo {
            let oid = git2::Oid::from_str(sha)?;
            git::commit::revert_commit(&ctx.repo, oid)?;
            ctx.db.execute(
                "UPDATE changes SET status = 'undone' WHERE id = ?1",
                [change_id],
            )?;
            let short = &change_id[..8.min(change_id.len())];
            println!("Undone [{short}]");
        }
    } else {
        // Undo the last active change
        let (change_id, sha): (String, String) = ctx.db.query_row(
            "SELECT id, git_sha FROM changes WHERE status = 'active' ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|_| anyhow::anyhow!("No active changes to undo."))?;

        let oid = git2::Oid::from_str(&sha)?;
        git::commit::revert_commit(&ctx.repo, oid)?;
        ctx.db.execute(
            "UPDATE changes SET status = 'undone' WHERE id = ?1",
            [&change_id],
        )?;

        let short = &change_id[..8.min(change_id.len())];
        println!("Undone [{short}]");
    }

    Ok(())
}
