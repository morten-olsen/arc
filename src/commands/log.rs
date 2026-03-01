use anyhow::Result;

use crate::context::ArcContext;
use crate::format::display;
use crate::metadata::change::Change;

pub fn run(all: bool, task: Option<String>) -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_filter = match task {
        Some(t) => Some(t),
        None => ctx.current_task_id()?,
    };

    let query = if task_filter.is_some() {
        "SELECT id, git_sha, summary, intent, author_type, author_name, task_id, change_type, status, created_at, parent_change_id, author_model
         FROM changes WHERE task_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, git_sha, summary, intent, author_type, author_name, task_id, change_type, status, created_at, parent_change_id, author_model
         FROM changes ORDER BY created_at DESC"
    };

    let mut stmt = ctx.db.prepare(query)?;

    let rows = if let Some(ref tid) = task_filter {
        stmt.query_map([tid], row_to_change)?
    } else {
        stmt.query_map([], row_to_change)?
    };

    let changes: Vec<Change> = rows.filter_map(|r| r.ok()).collect();

    if changes.is_empty() {
        println!("No changes recorded yet.");
        return Ok(());
    }

    for change in &changes {
        if !all && matches!(change.change_type,
            crate::metadata::change::ChangeType::Checkpoint |
            crate::metadata::change::ChangeType::Fix
        ) {
            continue;
        }
        if !all && change.status == crate::metadata::change::ChangeStatus::Undone {
            continue;
        }
        print!("{}", display::format_change(change));
    }

    Ok(())
}

fn row_to_change(row: &rusqlite::Row) -> rusqlite::Result<Change> {
    let author_type_str: String = row.get(4)?;
    let change_type_str: String = row.get(7)?;
    let status_str: String = row.get(8)?;

    Ok(Change {
        id: row.get(0)?,
        git_sha: row.get(1)?,
        summary: row.get(2)?,
        intent: row.get(3)?,
        author_type: match author_type_str.as_str() {
            "agent" => crate::metadata::change::AuthorType::Agent,
            _ => crate::metadata::change::AuthorType::Human,
        },
        author_name: row.get(5)?,
        task_id: row.get(6)?,
        change_type: match change_type_str.as_str() {
            "checkpoint" => crate::metadata::change::ChangeType::Checkpoint,
            "fix" => crate::metadata::change::ChangeType::Fix,
            "undo" => crate::metadata::change::ChangeType::Undo,
            _ => crate::metadata::change::ChangeType::Change,
        },
        status: match status_str.as_str() {
            "undone" => crate::metadata::change::ChangeStatus::Undone,
            "squashed" => crate::metadata::change::ChangeStatus::Squashed,
            _ => crate::metadata::change::ChangeStatus::Active,
        },
        created_at: chrono::Utc::now(),
        parent_change_id: row.get(10)?,
        author_model: row.get(11)?,
    })
}
