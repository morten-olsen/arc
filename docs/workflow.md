# Arc — Developer Workflow

This document walks through the real scenarios Arc is designed for — from the simplest case (one developer, no agents) to the full picture (mixed team, parallel agents, PR reviews, hotfixes).

The key concepts are:

- **Task** — A unit of work with its own worktree and branch. Optional.
- **Change** — A declaration of intent. Becomes a clean commit in the final history.
- **Checkpoint** — A frequent save. Belongs to the current change. Squashed on finalize.
- **Fix** — A checkpoint linked to a specific earlier change (for review feedback). Squashed into that change on finalize.
- **Finalize** — Collapse checkpoints and fixes into their parent changes, producing clean atomic commits.

---

## The Lifecycle of a Task

```
arc task new "Add rate limiting" --ref JIRA-1234
  │
  ├── arc change "Add sliding window counter"
  │     ├── arc checkpoint          (save work)
  │     ├── arc checkpoint          (save more work)
  │     └── arc checkpoint          (done with this change)
  │
  ├── arc change "Wire limiter into middleware"
  │     ├── arc checkpoint
  │     └── arc checkpoint
  │
  ├── arc push                      (push for PR review)
  │
  ├── [reviewer gives feedback]
  │     ├── arc fix <change-id>     (fix linked to specific change)
  │     ├── arc checkpoint
  │     └── arc push                (reviewer sees incremental progress)
  │
  ├── arc task finalize             (squash into clean commits)
  ├── arc push --force              (force push clean history)
  └── [merge via GitHub PR]
```

After finalize, the branch has exactly two commits:

```
Add sliding window counter
  Token bucket was too bursty for our traffic pattern.

Wire limiter into HTTP middleware
  Hook rate limiting into the request pipeline.
```

Clean, atomic, each one a working unit. No checkpoint noise, no "fix review comments" commits.

---

## Scenario 1: Solo Feature Development

Alice is working on a new feature. No agents, no team — just her.

### Create a task

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

She's now in an isolated worktree with its own branch. Her main worktree is untouched.

### Declare a change and work

```
$ arc change "Add sliding window counter" \
    --intent "Token bucket was too bursty for our traffic pattern"

Change [7d64aa2f]: Add sliding window counter  (empty)
```

This declares what she's about to build. The commit is empty — a statement of intent. Now she writes code:

```
# write some code...
$ arc checkpoint

Checkpoint [7d64aa2f]: saved

# write more code...
$ arc checkpoint "core logic done"

Checkpoint [7d64aa2f]: core logic done

# add tests...
$ arc checkpoint "tests passing"

Checkpoint [7d64aa2f]: tests passing
```

Each checkpoint creates a new commit that belongs to the current change. She can checkpoint as often as she wants — this is her working history, not the final history.

### Next change

```
$ arc change "Wire limiter into HTTP middleware" \
    --intent "Hook rate limiting into the request pipeline"
```

This starts a new change. Everything from here until the next `arc change` belongs to this change.

### Review the work

```
$ arc log

  [994c0a76]  Wire limiter into HTTP middleware
    Hook rate limiting into the request pipeline
  [7d64aa2f]  Add sliding window counter
    Token bucket was too brusty for our traffic pattern
```

`arc log` shows changes only — no checkpoint noise. Use `arc log --all` to see everything.

### Finalize and push

```
$ arc task finalize

Finalizing task: Add rate limiting
  Squashing checkpoints into parent changes...
  [7d64aa2f] Add sliding window counter (4 checkpoints → 1 commit)
  [994c0a76] Wire limiter into HTTP middleware (3 checkpoints → 1 commit)

Branch now has 2 clean commits. Ready to push.

$ arc push

Pushed task/a1b2c3d4-add-rate-limiting to origin.
```

She opens a PR on GitHub. The reviewer sees two clean, atomic commits with clear messages and intent.

### After merge

```
$ arc task complete

Task completed: Add rate limiting
  Worktree removed.
  Branch cleaned up.
```

---

## Scenario 2: Agent-Assisted Development

Marcus is building a payment retry feature. He writes the interface by hand and has an agent implement it.

### Human-written change

```
$ arc task new "Add payment retry" --ref PAY-567
$ arc task switch add-payment-retry

$ arc change "Define retry interface and configuration type" \
    --intent "Establish the contract before implementing. \
              Follows our team pattern of interfaces first."
```

He writes the code, checkpoints as he goes.

### Agent-written change

Marcus asks his AI agent to implement the interface. The agent (or Marcus on behalf of the agent) uses:

```
$ arc change "Implement retry logic" \
    --intent "Implement the PaymentRetrier interface with exponential backoff" \
    --agent --model claude-sonnet-4-5

$ arc checkpoint --agent --model claude-sonnet-4-5
$ arc checkpoint --agent --model claude-sonnet-4-5
$ arc checkpoint "implementation complete" --agent --model claude-sonnet-4-5
```

The `--agent` flag marks these as agent-authored. The commit messages and metadata reflect this:

```
[checkpoint → Implement retry logic] implementation complete

---
arc:change:id: a5b6c7d8-...
arc:author:type: agent
arc:author:model: claude-sonnet-4-5
arc:task:ref: PAY-567
```

A reviewer on GitHub — even without Arc — can see this was agent-written.

### Human reviews and continues

Marcus reviews the agent's work. It's mostly good, but he wants to tweak the error handling. He doesn't create a new change (that would leave a broken commit in history). Instead, he keeps working within the agent's change:

```
# make manual edits...
$ arc checkpoint "fix error handling to use domain types"
```

This checkpoint is human-authored (no `--agent` flag) but belongs to the agent's change. The metadata records both contributions. When finalized, the commit is a collaborative unit — agent-implemented, human-refined.

### Agent misstep

Marcus asks the agent for a circuit breaker. It produces something with global mutable state — wrong pattern for this team.

```
$ arc undo

Reverted: Add circuit breaker with provider tracking
  The undone change is preserved in history for reference.
```

He rephrases and tries again. The undo is recorded — future archaeology shows what was tried and rejected.

---

## Scenario 3: Parallel Agents

Marcus has two features to build. He creates two tasks and has two agents work on them simultaneously in separate terminal sessions:

```
Terminal 1:                              Terminal 2:
$ arc task new "Rate limiting"           $ arc task new "Fix auth bug"
$ arc task switch rate-limiting          $ arc task switch fix-auth-bug
  → .arc/worktrees/rate-limiting/          → .arc/worktrees/fix-auth-bug/
  (agent works here)                       (agent works here)
```

Each agent has its own worktree, branch, and staging area. They can't interfere with each other. Marcus can check on either by opening the worktree in his editor.

When both are done, he reviews each independently and merges them through separate PRs.

---

## Scenario 4: Emergency Hotfix

Marcus is mid-feature when production breaks. He needs to fix it immediately.

```
$ arc task new "hotfix: fix payment crash"
$ arc task switch fix-payment-crash
```

The hotfix task branches from `main`, not from his feature branch. His feature work is untouched in its worktree. He makes the fix:

```
$ arc change "Fix null pointer in payment callback" \
    --intent "Production crash when payment provider returns empty response body"

# fix the bug...
$ arc checkpoint
$ arc task finalize
$ arc push
```

He opens a PR for the hotfix. The team reviews and merges it via GitHub. Meanwhile, his feature worktree hasn't been touched. He switches back:

```
$ arc task switch rate-limiting
```

Later, he syncs his feature branch with the updated main:

```
$ arc task sync

Syncing rate-limiting with main...
  Auto-checkpointing uncommitted work...
  Rebasing onto main...
Done. 0 conflicts.
```

---

## Scenario 5: PR Review Cycle

Alice pushes her feature for review:

```
$ arc push
```

The raw history is pushed — changes and checkpoints. On GitHub, the reviewer sees:

```
Add sliding window counter
  Token bucket was too bursty for our traffic pattern

[checkpoint → Add sliding window counter] implement core logic
[checkpoint → Add sliding window counter] add tests
Wire limiter into HTTP middleware
  Hook rate limiting into the request pipeline

[checkpoint → Wire limiter into middleware] add middleware hook
[checkpoint → Wire limiter into middleware] integration tests
```

The structure is readable without Arc. Changes are the logical units, checkpoints show the work within each.

### Review feedback

The reviewer says the sliding window logic has an edge case. Alice switches back to the task and fixes it:

```
$ arc task switch rate-limiting

$ arc fix 7d64aa2f "handle empty window edge case"

# make the fix...
$ arc checkpoint

$ arc push
```

`arc fix <change-id>` creates a fix linked to the "Add sliding window counter" change. On GitHub, the reviewer sees the new commits since last review:

```
[fix → Add sliding window counter] handle empty window edge case
[checkpoint] saved
```

The fix is clearly labeled — the reviewer knows exactly what it addresses.

### Finalize and merge

PR approved. Alice finalizes:

```
$ arc task finalize

Finalizing task: Add rate limiting
  [7d64aa2f] Add sliding window counter (4 checkpoints + 1 fix → 1 commit)
  [994c0a76] Wire limiter into middleware (3 checkpoints → 1 commit)

$ arc push --force
```

The fix is folded into the original change. The branch now has two clean commits. The PR is merged via GitHub.

---

## Scenario 6: Reviewing a Coworker's PR

Marcus needs to review a coworker's PR. His rate-limiting work is safely in its worktree — he doesn't need to stash or switch branches.

From his main worktree (or any terminal):

```
$ gh pr checkout 42
```

He reviews, leaves comments, goes back to his work:

```
$ arc task switch rate-limiting
```

No context lost. No stash. No checkout dance. The worktree model makes this trivial.

---

## Scenario 7: Keeping a Task Up to Date

Alice's feature branch has been open for a few days. Main has moved forward with other merges.

```
$ arc task sync

Syncing add-rate-limiting with main...
  Auto-checkpointing uncommitted work...
  Rebasing onto origin/main...
  3 commits replayed cleanly.
Done.
```

If there are conflicts, Arc shows them with context:

```
Syncing add-rate-limiting with main...
  Rebasing onto origin/main...

CONFLICT in src/middleware/limiter.go
  Your change: "Wire limiter into HTTP middleware"
  Intent: Hook rate limiting into the request pipeline
  Conflicting commit on main: a4f7b2c "Refactor middleware chain"

Resolve conflicts and run: arc task sync --continue
Or abort: arc task sync --abort
```

The intent is shown alongside the conflict so you know *why* you made the change that's conflicting — context that plain `git rebase` never gives you.

---

## Scenario 8: Mixed Team

Some developers use Arc. Some use plain Git. Both push to the same GitHub repo.

**Arc user (Alice):**
```
$ arc change "Add rate limiter" --intent "..."
$ arc checkpoint
$ arc task finalize
$ arc push
```

The commits have `arc:` trailers in the message. The metadata is in `refs/arc/*`. On GitHub, it's a normal PR with clean commits.

**Plain Git user (Dan):**
```
$ git commit -m "Fix timezone handling"
$ git push origin main
```

Dan doesn't know or care about Arc. His commits are normal.

**What Alice sees:**
```
$ arc log

  [994c0a76]  Add rate limiter                       alice (human)
    Token bucket was too brusty for our traffic pattern
  [—]         Fix timezone handling                   dan (human)
    (plain Git commit)
  [7d64aa2f]  Add DLQ migration                      alice (human)
    Keep ops simple by using the existing database
```

Dan's commit shows up with whatever Git provides. Alice's commits have full metadata. No gap, no break. Arc enriches what it can and degrades gracefully.

**What Dan sees:**
```
$ git log --oneline

994c0a7 Add rate limiter
b3e4f5a Fix timezone handling
7d64aa2 Add DLQ migration
```

Normal git. The `arc:` trailers are in the full commit message but invisible in `--oneline`. Dan's workflow is unchanged.

---

## Scenario 9: Abandoning a Task

Marcus's rate limiter experiment isn't working out. The approach is wrong.

```
$ arc task abandon --reason "Sliding window approach too memory-intensive, need to rethink"

Abandoning task: Add rate limiting
  Worktree removed.
  Branch deleted.
  Recorded in metadata (reason preserved).
```

The worktree and branch are gone. The metadata records that this task existed, what was tried, and why it was abandoned. Future `arc log --abandoned` shows the history.

---

## Scenario 10: Working Without a Task

Not everything needs a task. For a quick config change on main:

```
$ arc change "Update API timeout to 30s" \
    --intent "Users hitting timeouts on slow connections"

# edit config...
$ arc checkpoint
$ arc push
```

`arc change` and `arc checkpoint` work on any branch, with or without a task. Tasks add isolation and structure. Without a task, you get structured commits on whatever branch you're on.

---

## Commit Message Format

Every Arc-created commit has a structured message:

### Change commit

```
Add sliding window rate limiter

Token bucket was too bursty for our traffic pattern.

---
arc:change:id: 7d64aa2f-...
arc:author:type: human
arc:task:ref: JIRA-1234
```

### Checkpoint commit

```
[checkpoint → Add sliding window rate limiter] core logic done

---
arc:change:id: 7d64aa2f-...
arc:type: checkpoint
arc:author:type: human
```

### Agent checkpoint

```
[checkpoint → Implement retry logic] implementation complete

---
arc:change:id: a5b6c7d8-...
arc:type: checkpoint
arc:author:type: agent
arc:author:model: claude-sonnet-4-5
```

### Fix commit (linked to a specific change)

```
[fix → Add sliding window rate limiter] handle empty window edge case

---
arc:change:id: 7d64aa2f-...
arc:type: fix
arc:author:type: human
```

A GitHub reviewer who has never heard of Arc can read these commit messages and understand the structure. Changes are the logical units. Checkpoints are working saves. Fixes address specific changes.

After `arc task finalize`, only the change commits remain — checkpoints and fixes are squashed into their parent changes.

---

## Command Summary

| Command | What it does |
|---|---|
| `arc init` | Initialize Arc in a Git repo |
| `arc task new "goal" [--ref TICKET]` | Create a task with its own worktree |
| `arc task list` | List all tasks |
| `arc task switch <name>` | Switch to a task's worktree |
| `arc task status` | Show current task details |
| `arc task sync` | Rebase task onto base branch |
| `arc task finalize` | Squash checkpoints into clean commits |
| `arc task complete` | Clean up worktree and branch after merge |
| `arc task abandon [--reason "..."]` | Drop a task, record why |
| `arc change "summary" [--intent "..."]` | Declare a new change (squash boundary) |
| `arc checkpoint ["message"]` | Save work (belongs to current change) |
| `arc fix <change-id> ["message"]` | Save work linked to a specific change |
| `arc undo` | Revert the last change |
| `arc log [--all]` | Show change history |
| `arc intent <file> [--line <range>]` | Show the Arc intent behind each line of a file |
| `arc push` | Push code + metadata to remote |
| `arc pull` | Pull code + metadata from remote |
| `arc eject` | Remove Arc, leave a clean Git repo |

### Agent flags (available on `change`, `checkpoint`, `fix`)

| Flag | Purpose |
|---|---|
| `--agent` | Mark as agent-authored |
| `--model <name>` | Which model (e.g., `claude-sonnet-4-5`) |
