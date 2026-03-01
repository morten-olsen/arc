# Arc — Getting Started

Install Arc, set up a project, and learn the workflow in 5 minutes.

---

## Install

Build from source (requires [Rust](https://rustup.rs/)):

```
git clone <repo-url> && cd arc
cargo install --path .
```

This puts the `arc` binary in `~/.cargo/bin/`. Make sure that's in your `PATH`.

### Shell integration

Arc needs a small shell wrapper so that `arc task switch` can change your working directory. Add this to your `~/.bashrc`, `~/.zshrc`, or equivalent:

```
eval "$(arc shell-init)"
```

Restart your shell or `source` the file. Without this, everything works except `arc task switch` — you'd have to `cd` into worktrees manually.

This is the same pattern used by `nvm`, `conda`, and other tools. The `arc` binary can't change your shell's working directory, so the wrapper handles it.

---

## Initialize a project

In any Git repository (or an empty directory):

```
$ arc init

Initialized Git repository.
Initialized Arc.

Add this to your shell profile for task switching:
  eval "$(arc shell-init)"
```

This creates:

- A `.arc/` directory (local only, gitignored automatically) with a SQLite index and a `worktrees/` directory
- Git hooks for future integration
- A `refs/arc/config.json` ref for shared team configuration

If the directory isn't a Git repo yet, Arc runs `git init` for you. No remote required — you can add one later.

Your existing Git history is untouched. Nothing is rewritten.

---

## The workflow

There are three concepts to learn:

- **Change** — a declaration of intent. "I'm about to build X because Y." Becomes a clean commit in the final history.
- **Checkpoint** — a save. Do it as often as you want. Gets squashed into its parent change when you finalize.
- **Task** — a unit of work with its own isolated directory. Optional, but recommended.

### 1. Create a task

```
$ arc task new "Add rate limiting" --ref JIRA-1234

Created task: Add rate limiting
  Branch:   task/a1b2c3d4-add-rate-limiting
  Worktree: .arc/worktrees/add-rate-limiting

Switch to it: arc task switch add-rate-limiting
```

```
$ arc task switch add-rate-limiting
```

You're now in `.arc/worktrees/add-rate-limiting/` on a new branch. Your main worktree is untouched — you can have a dozen tasks open and they never interfere.

### 2. Declare a change

Before writing code (or after — it works either way), declare what you're about to do:

```
$ arc change "Add sliding window counter" \
    --intent "Token bucket was too bursty for our traffic pattern"

Change [7d64aa2f]: Add sliding window counter  (empty)
```

This creates a commit that captures your intent. If you had uncommitted work already, it gets included. If not, the commit is empty — a stake in the ground for what you're about to build.

This is the key idea: **declare intent first, then iterate.** The change becomes a clean commit in your final history. Everything you checkpoint after this belongs to it.

### 3. Write code, checkpoint often

Write your code as normal. When you want to save your progress:

```
$ arc checkpoint

Checkpoint: saved

$ arc checkpoint "core logic done"

Checkpoint: core logic done

$ arc checkpoint "tests passing"

Checkpoint: tests passing
```

Each checkpoint is a real Git commit, but it's working history — not final history. Save as often as you want. After every test run. After every paragraph of code. After every "wait, let me try something." There's no such thing as checkpointing too often.

### 4. Declare the next change

When you're done with one logical unit and ready for the next:

```
$ arc change "Wire limiter into HTTP middleware" \
    --intent "Hook rate limiting into the request pipeline"

Change [994c0a76]: Wire limiter into middleware  (empty)
```

This starts a new change. Everything you checkpoint from here belongs to this one. You've now declared two units of work, each with clear intent. Keep going.

### 5. Experiment safely

Want to try something you might throw away?

```
$ arc checkpoint "trying redis approach"

# ... it doesn't work out ...

$ arc undo

Undone: trying redis approach
```

This reverts the checkpoint and updates your working directory. The undone checkpoint is still in history (`arc log --all`) if you ever need to see what you tried.

### 6. Review your work

```
$ arc log

  [994c0a76]  Wire limiter into HTTP middleware
    Hook rate limiting into the request pipeline
  [7d64aa2f]  Add sliding window counter
    Token bucket was too brusty for our traffic pattern
```

`arc log` shows changes only — no checkpoint noise. Use `--all` to see everything.

```
$ arc task status

Task: Add rate limiting
  ID:      a1b2c3d4
  Status:  in_progress
  Branch:  task/a1b2c3d4-add-rate-limiting
  Changes: 2
```

### 7. See why any line was written

`arc intent` is like `git blame`, but shows the *why* instead of the *who*:

```
$ arc intent src/limiter.rs

  1 │ fn limit(req: &Request) {    Add sliding window counter
    │                              Token bucket was too bursty for our traffic pattern
  2 │     let window = 60;
  3 │     let max = 100;
  4 │     check(req, window, max)  Wire limiter into HTTP middleware
    │                              Hook rate limiting into the request pipeline
```

Filter to specific lines with `--line`:

```
$ arc intent src/limiter.rs --line 1
$ arc intent src/limiter.rs --line 2,4
```

Lines from non-Arc commits (e.g. before `arc init`) show `(no arc intent)`.

### 8. Push for PR review

When you're ready for review, push the branch:

```
$ arc push

Pushed task/a1b2c3d4-add-rate-limiting to origin.
```

Open a PR on GitHub. The reviewer sees the full working history — changes and checkpoints. The commit messages are structured so anyone can follow them, even without Arc:

```
Add sliding window counter
  Token bucket was too brusty for our traffic pattern

[checkpoint → Add sliding window counter] core logic done
[checkpoint → Add sliding window counter] tests passing
Wire limiter into HTTP middleware
  Hook rate limiting into the request pipeline

[checkpoint → Wire limiter into middleware] add middleware hook
[checkpoint → Wire limiter into middleware] integration tests
```

### 9. Handle review feedback

The reviewer finds an issue in the rate limiter. Fix it properly — linked to the change it belongs to:

```
$ arc fix 7d64aa2f "handle empty window edge case"

# make the fix...
$ arc checkpoint
$ arc push
```

The reviewer sees:

```
[fix → Add sliding window counter] handle empty window edge case
[checkpoint] saved
```

No ambiguous "address review comments" commit. The fix is clearly linked to the change it addresses.

### 10. Finalize

PR approved. Collapse everything into clean atomic commits:

```
$ arc task finalize

Finalizing task: Add rate limiting
  [7d64aa2f] Add sliding window counter (4 checkpoints + 1 fix → 1 commit)
  [994c0a76] Wire limiter into middleware (3 checkpoints → 1 commit)

Branch now has 2 clean commits.
```

Force push the clean branch:

```
$ arc push --force
```

Merge via GitHub. Two clean, atomic commits with clear intent. No interactive rebase. No pain.

### 11. Clean up

After the PR is merged on GitHub:

```
$ arc task complete

Task completed: Add rate limiting
  Worktree removed.
  Branch cleaned up.
```

---

## Keeping your branch up to date

Main moved forward while you were working? Sync:

```
$ arc task sync

Syncing add-rate-limiting with main...
  Auto-checkpointing uncommitted work...
  Rebasing onto origin/main...
Done. 0 conflicts.
```

Arc auto-checkpoints any dirty work before rebasing, so you never lose uncommitted changes. If there are conflicts, Arc shows your intent alongside them so you remember *why* you made the change that's conflicting.

---

## Working on multiple tasks

Each task is its own worktree. Have as many as you want:

```
$ arc task new "Add rate limiting"
$ arc task new "Fix login bug"
$ arc task new "Update docs"

$ arc task list
  [a1b2c3d4] Add rate limiting   (in_progress)  .arc/worktrees/add-rate-limiting
  [e5f6g7h8] Fix login bug       (in_progress)  .arc/worktrees/fix-login-bug
  [i9j0k1l2] Update docs         (in_progress)  .arc/worktrees/update-docs
```

Switch between them instantly:

```
$ arc task switch fix-login-bug
```

Each worktree has its own files, its own branch, its own uncommitted state. Switching is just `cd` — nothing is stashed, nothing is lost. You can have multiple terminals open in different tasks simultaneously.

---

## Working without a task

Not everything needs a task. For a quick fix on any branch:

```
$ arc change "Fix typo in config" --intent "API URL had trailing slash"
# edit file...
$ arc checkpoint
```

Tasks add isolation via worktrees. Without a task, `arc change` and `arc checkpoint` still work — you get structured commits on whatever branch you're on.

---

## Agent-authored code

When an agent writes code, pass the `--agent` flag:

```
$ arc change "Implement retry logic" \
    --intent "Exponential backoff for failed payment attempts" \
    --agent --model claude-sonnet-4-5

$ arc checkpoint --agent --model claude-sonnet-4-5
$ arc checkpoint "implementation complete" --agent --model claude-sonnet-4-5
```

The commit messages reflect this:

```
[checkpoint → Implement retry logic] implementation complete

---
arc:change:id: a5b6c7d8-...
arc:author:type: agent
arc:author:model: claude-sonnet-4-5
```

A reviewer on GitHub — even without Arc — can see this was agent-written.

---

## What your teammates see

If you push to GitHub, your teammates see completely normal Git:

```
$ git log --oneline
7d64aa2 Add sliding window counter
994c0a7 Wire limiter into HTTP middleware
152cff5 Initial commit
```

The full commit message has some extra metadata at the bottom:

```
Add sliding window counter

Token bucket was too brusty for our traffic pattern

---
arc:change:id: 7d64aa2f-...
arc:author:type: human
arc:task:ref: JIRA-1234
```

A readable commit message with some trailing data anyone can ignore. The `refs/arc/*` metadata is pushed alongside but GitHub doesn't render it — it's invisible unless someone installs Arc.

---

## Removing Arc

If you decide Arc isn't for you:

```
$ arc eject

  Removed worktrees
  Deleted refs/arc/*
  Removed hooks
  Deleted .arc/

Done. This is now a plain Git repository.
Your code, branches, and commit history are unchanged.
```

Or just stop using `arc` commands and use `git` directly. The repo is a normal Git repo at all times. There is no lock-in and no penalty for leaving.

---

## Command reference

| Command | What it does |
|---|---|
| `arc init` | Initialize Arc (creates `.arc/`, hooks, gitignore, config ref) |
| `arc shell-init` | Print the shell wrapper for your profile |
| `arc task new "goal" [--ref TICKET]` | Create a task with its own worktree and branch |
| `arc task list` | List all tasks with status and worktree path |
| `arc task switch <name>` | `cd` into a task's worktree (fuzzy matched) |
| `arc task status` | Show current task details and change count |
| `arc task sync` | Rebase task onto latest base branch |
| `arc task finalize` | Squash checkpoints into clean atomic commits |
| `arc task complete` | Clean up worktree and branch after merge |
| `arc task abandon [--reason "..."]` | Drop a task, record why in metadata |
| `arc change "summary" [--intent "why"]` | Declare a new change (squash boundary) |
| `arc checkpoint ["message"]` | Save work (belongs to current change) |
| `arc fix <change-id> ["message"]` | Save work linked to a specific earlier change |
| `arc undo` | Revert the last change or checkpoint |
| `arc log [--all]` | Show change history (checkpoints hidden by default) |
| `arc intent <file> [--line <range>]` | Show the Arc intent behind each line of a file |
| `arc push` | Push code + metadata to remote |
| `arc pull` | Pull code + metadata from remote |
| `arc eject` | Remove all Arc artifacts, leave a clean Git repo |

### Agent flags (available on `change`, `checkpoint`, `fix`)

| Flag | Purpose |
|---|---|
| `--agent` | Mark as agent-authored |
| `--model <name>` | Which model (e.g., `claude-sonnet-4-5`) |
