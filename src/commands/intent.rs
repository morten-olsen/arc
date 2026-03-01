use std::collections::HashMap;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::context::ArcContext;

/// A single line from git blame --porcelain output.
struct BlameLine {
    sha: String,
    final_line: usize,
    content: String,
}

pub fn run(file: String, line_range: Option<String>) -> Result<()> {
    let ctx = ArcContext::open()?;
    let workdir = ctx
        .repo
        .workdir()
        .context("bare repositories are not supported")?;

    // Build git blame command
    let mut cmd = Command::new("git");
    cmd.args(["blame", "--porcelain"]);
    if let Some(ref range) = line_range {
        // Accept "10" or "10,20"
        let range_arg = if range.contains(',') {
            let parts: Vec<&str> = range.splitn(2, ',').collect();
            format!("{},{}", parts[0], parts[1])
        } else {
            format!("{},{}", range, range)
        };
        cmd.args(["-L", &range_arg]);
    }
    cmd.arg(&file);
    cmd.current_dir(workdir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().context("failed to run git blame")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git blame failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines = parse_porcelain(&stdout)?;

    if lines.is_empty() {
        println!("No lines to annotate.");
        return Ok(());
    }

    // Collect unique SHAs and query DB once per SHA
    let unique_shas: Vec<String> = {
        let mut seen = HashMap::new();
        for bl in &lines {
            seen.entry(bl.sha.clone()).or_insert(());
        }
        seen.into_keys().collect()
    };

    // sha -> Option<(summary, Option<intent>)>
    let mut sha_info: HashMap<String, Option<(String, Option<String>)>> = HashMap::new();
    let mut stmt = ctx
        .db
        .prepare("SELECT summary, intent FROM changes WHERE git_sha = ?1")?;

    for sha in &unique_shas {
        let result: Option<(String, Option<String>)> = stmt
            .query_row([sha], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok();
        sha_info.insert(sha.clone(), result);
    }

    // Find max line number width for alignment
    let max_line = lines.iter().map(|l| l.final_line).max().unwrap_or(0);
    let line_width = max_line.to_string().len();

    // Find max content width (capped for readability)
    let max_content_width = lines
        .iter()
        .map(|l| l.content.len())
        .max()
        .unwrap_or(0)
        .min(60);

    // Walk lines, grouping consecutive lines from the same SHA
    let mut prev_sha: Option<&str> = None;

    for bl in &lines {
        let is_new_group = prev_sha.map_or(true, |p| p != bl.sha);
        let content_display = &bl.content;

        if is_new_group {
            // First line of a new SHA group — show summary
            let annotation = match sha_info.get(&bl.sha) {
                Some(Some((summary, _))) => summary.clone(),
                _ => "(no arc intent)".to_string(),
            };
            println!(
                " {:>width$} \u{2502} {:<cwidth$} {}",
                bl.final_line,
                content_display,
                annotation,
                width = line_width,
                cwidth = max_content_width,
            );

            // Second line: intent (if present)
            if let Some(Some((_, Some(intent)))) = sha_info.get(&bl.sha) {
                println!(
                    " {:>width$} \u{2502} {:<cwidth$} {}",
                    "",
                    "",
                    intent,
                    width = line_width,
                    cwidth = max_content_width,
                );
            }
        } else {
            // Continuation line — no annotation
            println!(
                " {:>width$} \u{2502} {}",
                bl.final_line,
                content_display,
                width = line_width,
            );
        }

        prev_sha = Some(&bl.sha);
    }

    Ok(())
}

/// Parse `git blame --porcelain` output into a list of BlameLine entries.
///
/// Porcelain format: each group starts with
///   `<40-char-sha> <orig_line> <final_line> [<num_lines>]`
/// followed by header key-value pairs, then a `\t<content>` line.
fn parse_porcelain(output: &str) -> Result<Vec<BlameLine>> {
    let mut result = Vec::new();
    let mut current_sha = String::new();
    let mut current_final_line: usize = 0;

    for line in output.lines() {
        if line.starts_with('\t') {
            // Content line — everything after the leading tab
            result.push(BlameLine {
                sha: current_sha.clone(),
                final_line: current_final_line,
                content: line[1..].to_string(),
            });
        } else {
            // Could be a header line (key value) or a commit line (sha orig final [count])
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0].len() == 40 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
                current_sha = parts[0].to_string();
                current_final_line = parts[2].parse().unwrap_or(0);
            }
            // Otherwise it's a header line like "author John" — skip
        }
    }

    Ok(result)
}
