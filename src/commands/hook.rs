use anyhow::Result;

pub fn run(event: &str) -> Result<()> {
    match event {
        "post-commit" => {
            // In v1, hooks are installed but mostly no-ops.
            // Future: index git commits not made via `arc change`.
            Ok(())
        }
        "post-merge" | "post-rebase" | "post-checkout" => {
            // Future: rebuild change-map, update task status
            Ok(())
        }
        _ => {
            eprintln!("Unknown hook event: {event}");
            Ok(())
        }
    }
}
