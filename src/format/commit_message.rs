/// Structured metadata extracted from or written to commit messages.
#[derive(Debug, Default)]
pub struct CommitMetadata {
    pub change_id: Option<String>,
    pub author_type: Option<String>,
    pub author_model: Option<String>,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub confidence: Option<f64>,
    pub prompt_hash: Option<String>,
    pub derived_from: Vec<String>,
    pub change_type: Option<String>,
    pub parent_change_summary: Option<String>,
    pub task_ref: Option<String>,
}

/// Format a commit message with Arc trailers.
///
/// When `change_type` is "checkpoint" or "fix" and `parent_change_summary` is set,
/// the summary line becomes `[checkpoint → Parent Summary] message` or
/// `[fix → Parent Summary] message`.
pub fn format(summary: &str, intent: Option<&str>, metadata: &CommitMetadata) -> String {
    let formatted_summary = format_summary(summary, metadata);

    let mut msg = formatted_summary;

    if let Some(intent) = intent {
        msg.push_str("\n\n");
        msg.push_str(intent);
    }

    msg.push_str("\n\n---");

    if let Some(ref id) = metadata.change_id {
        msg.push_str(&format!("\narc:change:id: {id}"));
    }
    if let Some(ref author_type) = metadata.author_type {
        msg.push_str(&format!("\narc:author:type: {author_type}"));
    }
    if let Some(ref model) = metadata.author_model {
        msg.push_str(&format!("\narc:author:model: {model}"));
    }
    if let Some(ref task_id) = metadata.task_id {
        msg.push_str(&format!("\narc:task: {task_id}"));
    }
    if let Some(ref change_type) = metadata.change_type {
        msg.push_str(&format!("\narc:type: {change_type}"));
    }
    if let Some(ref task_ref) = metadata.task_ref {
        msg.push_str(&format!("\narc:task:ref: {task_ref}"));
    }
    if let Some(ref session_id) = metadata.session_id {
        msg.push_str(&format!("\narc:session: {session_id}"));
    }
    if let Some(confidence) = metadata.confidence {
        msg.push_str(&format!("\narc:confidence: {confidence}"));
    }
    if let Some(ref hash) = metadata.prompt_hash {
        msg.push_str(&format!("\narc:prompt:hash: {hash}"));
    }
    if !metadata.derived_from.is_empty() {
        msg.push_str(&format!(
            "\narc:derived-from: {}",
            metadata.derived_from.join(", ")
        ));
    }

    msg.push('\n');
    msg
}

/// Format the summary line with type prefix for checkpoints and fixes.
fn format_summary(summary: &str, metadata: &CommitMetadata) -> String {
    match (metadata.change_type.as_deref(), metadata.parent_change_summary.as_deref()) {
        (Some("checkpoint"), Some(parent)) => format!("[checkpoint \u{2192} {parent}] {summary}"),
        (Some("fix"), Some(parent)) => format!("[fix \u{2192} {parent}] {summary}"),
        _ => summary.to_string(),
    }
}

/// Parse Arc metadata from a commit message.
/// Returns None if the message has no Arc trailers.
pub fn parse(message: &str) -> Option<CommitMetadata> {
    let separator = message.find("\n---\n")?;
    let trailer_section = &message[separator + 5..];

    let mut meta = CommitMetadata::default();
    let mut found_any = false;

    for line in trailer_section.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("arc:change:id: ") {
            meta.change_id = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:author:type: ") {
            meta.author_type = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:author:model: ") {
            meta.author_model = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:task: ") {
            meta.task_id = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:type: ") {
            meta.change_type = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:task:ref: ") {
            meta.task_ref = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:session: ") {
            meta.session_id = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:confidence: ") {
            meta.confidence = value.parse().ok();
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:prompt:hash: ") {
            meta.prompt_hash = Some(value.to_string());
            found_any = true;
        } else if let Some(value) = line.strip_prefix("arc:derived-from: ") {
            meta.derived_from = value.split(", ").map(String::from).collect();
        } else if let Some(value) = line.strip_prefix("arc:squashed-from: ") {
            // Backward compat: read old trailer format
            meta.derived_from = value.split(", ").map(String::from).collect();
            found_any = true;
        }
    }

    if found_any { Some(meta) } else { None }
}

/// Extract the summary (first line) and intent (body before trailers) from a commit message.
pub fn extract_summary_and_intent(message: &str) -> (String, Option<String>) {
    let content = if let Some(sep) = message.find("\n---\n") {
        &message[..sep]
    } else {
        message
    };

    let mut lines = content.lines();
    let summary = lines.next().unwrap_or("").to_string();

    let body: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();

    let intent = if body.is_empty() { None } else { Some(body) };

    (summary, intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_format_parse() {
        let meta = CommitMetadata {
            change_id: Some("abc-123".into()),
            author_type: Some("human".into()),
            author_model: None,
            task_id: Some("task-456".into()),
            change_type: Some("change".into()),
            task_ref: Some("PROJ-42".into()),
            ..Default::default()
        };

        let msg = format("Add feature", Some("Because we need it"), &meta);
        let parsed = parse(&msg).expect("should parse");

        assert_eq!(parsed.change_id.as_deref(), Some("abc-123"));
        assert_eq!(parsed.author_type.as_deref(), Some("human"));
        assert_eq!(parsed.task_id.as_deref(), Some("task-456"));
        assert_eq!(parsed.change_type.as_deref(), Some("change"));
        assert_eq!(parsed.task_ref.as_deref(), Some("PROJ-42"));
    }

    #[test]
    fn test_parse_no_trailers() {
        let msg = "Just a regular commit message\n\nWith a body.";
        assert!(parse(msg).is_none());
    }

    #[test]
    fn test_extract_summary_and_intent() {
        let msg = "Add feature\n\nBecause we need it\n\n---\narc:change:id: abc\n";
        let (summary, intent) = extract_summary_and_intent(msg);
        assert_eq!(summary, "Add feature");
        assert_eq!(intent.as_deref(), Some("Because we need it"));
    }

    #[test]
    fn test_checkpoint_summary_format() {
        let meta = CommitMetadata {
            change_id: Some("chk-1".into()),
            change_type: Some("checkpoint".into()),
            parent_change_summary: Some("Add auth".into()),
            ..Default::default()
        };

        let msg = format("wip styles", None, &meta);
        assert!(msg.starts_with("[checkpoint \u{2192} Add auth] wip styles"));

        let parsed = parse(&msg).expect("should parse");
        assert_eq!(parsed.change_type.as_deref(), Some("checkpoint"));
    }

    #[test]
    fn test_fix_summary_format() {
        let meta = CommitMetadata {
            change_id: Some("fix-1".into()),
            change_type: Some("fix".into()),
            parent_change_summary: Some("Add auth".into()),
            ..Default::default()
        };

        let msg = format("handle edge case", None, &meta);
        assert!(msg.starts_with("[fix \u{2192} Add auth] handle edge case"));

        let parsed = parse(&msg).expect("should parse");
        assert_eq!(parsed.change_type.as_deref(), Some("fix"));
    }

    #[test]
    fn test_roundtrip_with_model() {
        let meta = CommitMetadata {
            change_id: Some("abc-123".into()),
            author_type: Some("agent".into()),
            author_model: Some("claude-sonnet-4-5-20250929".into()),
            ..Default::default()
        };

        let msg = format("AI-generated change", None, &meta);
        let parsed = parse(&msg).expect("should parse");

        assert_eq!(parsed.author_type.as_deref(), Some("agent"));
        assert_eq!(parsed.author_model.as_deref(), Some("claude-sonnet-4-5-20250929"));
    }
}
