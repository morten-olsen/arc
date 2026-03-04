# Arc

Git porcelain for structured commits, intent tracking, and agent attribution. Wraps git — every Arc repo is a valid Git repo.

## Docs

- [docs/design.md](docs/design.md) — Architecture, primitives (Change/Task/Session/Review), ref layout, data model
- [docs/workflow.md](docs/workflow.md) — 10 real-world scenarios, commit message format, command reference
- [docs/git-interop.md](docs/git-interop.md) — How metadata survives squash, rebase, cherry-pick, amend
- [docs/goals.md](docs/goals.md) — Design constraints and non-goals

## Key files

- `src/context.rs` — `ArcContext` (repo + sqlite + paths). Navigates worktrees back to main repo root via `commondir()`
- `src/format/commit_message.rs` — Trailer format/parse. Separator is `\n---\n`, NOT standard git trailers
- `src/index/sqlite.rs` — Schema + manual migrations (bump version in `run_migrations`)
- `src/index/change_map.rs` — UUID↔SHA mapping (currently simpler HashMap, not the multi-branch design from docs)
- `src/git/refs.rs` — Read/write blobs under `refs/arc/*`
- `src/commands/mod.rs` — CLI definition (clap derive)
- `src/global.rs` — Global project registry (`$XDG_DATA_HOME/arc/registry.db`)
- `src/commands/project.rs` — `arc project` subcommands (add/remove/edit/list/status/switch)

## Build & test

```
cargo build
cargo test                          # unit tests
cargo build && bash tests/e2e.sh    # e2e (bash script, not cargo)
```

## Gotchas

- **Rust edition 2024** — requires nightly or recent stable
- **Two storage layers**: `.arc/` is local-only (sqlite index, worktrees); `refs/arc/*` is shared (pushed/pulled as git refs pointing to blobs)
- **Commit trailer separator is `\n---\n`** not git's standard `Signed-off-by` trailer convention — `parse()` in `commit_message.rs` splits on this
- **`arc:squashed-from` is deprecated** — renamed to `arc:derived-from`, but old format is still parsed for backward compat
- **`task switch` needs shell wrapper** — binary can't cd parent shell, so `eval "$(arc shell-init)"` is required in shell profile
- **`Hook` command is hidden** — internal only, used by git hooks installed during `arc init`
- **Uses `git2` (libgit2)**, not gitoxide
- **ChangeMap is simpler than spec** — flat `HashMap<UUID, SHA>`, not the multi-branch/canonical structure described in design.md and git-interop.md
- **`--model` implies `--agent`** — passing a model name auto-sets agent authorship
- **Docs are living documents** — update docs when assumptions change or discrepancies between docs and code are discovered (e.g. ChangeMap divergence from spec)
- **`arc project` commands work from anywhere** — they use `global::open_registry()`, not `ArcContext`
- **`project switch` needs shell wrapper** — same pattern as `task switch`, requires `eval "$(arc shell-init)"`
- **Global registry failure is non-fatal on init** — `arc init` still succeeds if registry can't be written
