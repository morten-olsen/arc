use crate::metadata::change::{Change, ChangeStatus, ChangeType};

/// Format a change for terminal display in `arc log`.
pub fn format_change(change: &Change) -> String {
    let status_marker = match change.status {
        ChangeStatus::Active => "",
        ChangeStatus::Undone => " [undone]",
        ChangeStatus::Squashed => " [squashed]",
    };

    let type_prefix = match change.change_type {
        ChangeType::Checkpoint => "checkpoint  ",
        ChangeType::Fix => "fix  ",
        ChangeType::Undo => "undo  ",
        ChangeType::Change => "",
    };

    let short_id = &change.id[..8.min(change.id.len())];
    let sha_short = change
        .git_sha
        .as_deref()
        .map(|s| &s[..7.min(s.len())])
        .unwrap_or("-------");

    let mut out = format!("  {sha_short}  [{short_id}]  {type_prefix}{}{status_marker}\n", change.summary);

    if let Some(ref intent) = change.intent {
        out.push_str(&format!("    {intent}\n"));
    }

    if let Some(ref model) = change.author_model {
        out.push_str(&format!("    (agent: {model})\n"));
    }

    out
}
