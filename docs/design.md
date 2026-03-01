# Arc — System Design

Arc is a version control system built for the age of AI-assisted software development. It uses Git as its storage backend and transport layer while introducing new primitives, workflows, and metadata capabilities designed for codebases where humans and AI agents collaborate.

## Design Principles

1. **Git as plumbing, Arc as porcelain.** Git's content-addressed object store and distributed sync protocol are battle-tested. We don't replace them — we build on top of them. Every Arc repository is a valid Git repository. You can push to GitHub, clone with `git`, run CI on standard infrastructure.

2. **Zero lock-in.** An Arc repository is a Git repository. Full stop. Anyone can `git clone` it, `git log` it, `git blame` it. The code is there, the commit messages are readable, the branches are normal branches. Arc metadata lives in separate refs — if you stop using Arc, you lose the rich metadata but keep a perfectly normal Git repo. There is no migration, no export step, no penalty.

3. **Incremental adoption.** Arc does not require a team to go all-in. A single developer can `arc init` on an existing Git repository and start using `arc change` instead of `git commit`. Their teammates continue using plain Git — they see normal commits with slightly more structured messages. Arc metadata refs sit quietly in the background until someone installs Arc and starts reading them. There's no flag day.

4. **Intent is a first-class citizen.** Every code change should capture *why* it was made, not just *what* changed. Arc encourages structured intent, and optionally captures reasoning, prompts, and alternative approaches considered. For pure human workflows without agents, this alone is a meaningful improvement over free-text commit messages.

5. **Agent identity is native.** The system distinguishes between human-authored and agent-authored code at the primitive level. Which model, which tool, which session, what confidence level — all tracked natively, not inferred from conventions. When no agent is involved, changes are simply attributed to the human author — the same way Git works today, just with richer structure.

6. **Designed for undo.** Agents produce code quickly and often need to backtrack. But humans make mistakes too. Checkpointing and reverting are cheap, expected operations — not exceptional recovery procedures.

7. **Reviews are built in.** Code review is a core primitive, not a feature of the hosting platform. Reviews can be performed by humans or agents, and the tool can enforce review policies before work is finalized.

8. **Worktree-first parallelism.** Every task gets its own Git worktree — a fully independent working directory with its own branch and staging area. There is no branch switching, no stashing, no "save my state" dance. You can have multiple tasks open simultaneously, and multiple agents working on different tasks in parallel with zero interference. Context switching is `cd`, not `git checkout`.

## Primitives

Arc introduces four core primitives that map to — but extend — Git's data model.

### Change

A Change is Arc's equivalent of a commit. It records a code modification along with structured metadata about who made it, why, and how.

```
Change {
  id:            uuid (stable across rebases)
  parent:        Change id | null
  snapshot:      tree hash (Git tree object)

  author: {
    type:        human | agent
    identity:    string (user id or agent name)
    model:       string | null (e.g., "claude-opus-4-6")
    tool:        string | null (e.g., "opencode")
    session:     Session id | null
  }

  summary:       string ("Add token refresh interceptor")

  intent:        string ("Users are getting logged out after 1 hour
                  because access tokens expire with no refresh
                  mechanism. This adds transparent token refresh.")

  reasoning:     string | null (chain-of-thought, approach rationale)
  prompt:        string | null (the prompt that produced this change)
  confidence:    float | null (0.0–1.0, agent's self-assessed confidence)

  review:        Review id | null
  derived_from:  Change id | null (tracks cherry-picks, suggestions)
  tags:          string[] (e.g., ["security", "bugfix", "refactor"])

  created_at:    timestamp
}
```

**Key difference from a Git commit:** A Change has a UUID that persists across rebases. Git commit SHAs change when history is rewritten; Change UUIDs do not. This gives every unit of work a stable identity regardless of what Git operations are performed on it.

**Mapping to Git:** A Change is stored as a Git commit. The Change UUID and structured metadata are embedded in the commit message as a parseable header block. The code snapshot is a normal Git tree.

### Task

A Task is a unit of work with a defined goal. It replaces the concept of a feature branch, adding structure around what the branch is for and when it's done.

```
Task {
  id:            uuid
  name:          string ("token-refresh")
  goal:          string ("Implement automatic token refresh so
                  users are not logged out after token expiry")
  status:        draft | in_progress | review | completed | abandoned
  criteria:      string[] (acceptance criteria)

  assignee: {
    type:        human | agent
    identity:    string
  }

  changes:       Change id[] (ordered)
  base:          Change id (where this task branched from)
  parent_task:   Task id | null (for subtasks)
  worktree_path: string | null (path to the task's worktree)

  created_at:    timestamp
  completed_at:  timestamp | null
}
```

**Mapping to Git:** A Task corresponds to a Git branch (`task/<task-id>`) plus a Git worktree (a full working directory at `.arc/worktrees/<task-slug>/`) plus a metadata blob stored under `refs/arc/tasks/<task-id>.json`. Creating a task creates the worktree; completing or abandoning it removes it.

**Parallel agents:** Because each task is an isolated worktree, multiple agents can work on different tasks simultaneously in separate terminal sessions. They edit different directories, commit to different branches, and share only the Git object store (which is designed for concurrent access). Conflicts surface at merge time — when you want them — not while agents are working.

### Session

A Session captures a continuous working period — typically one invocation of an AI coding agent, but also applicable to human work sessions.

```
Session {
  id:            uuid
  task:          Task id
  agent: {
    model:       string
    tool:        string
    version:     string
  }

  changes:       Change id[] (changes made in this session)
  checkpoints:   Change id[] (intermediate saves)

  transcript: [{
    role:        user | assistant
    content:     string
    timestamp:   timestamp
  }]

  context: {
    files_read:       string[]
    files_modified:   string[]
    errors_encountered: string[]
  }

  started_at:    timestamp
  ended_at:      timestamp
}
```

**Mapping to Git:** Session data is stored as blob(s) under `refs/arc/sessions/<session-id>/`. Large transcripts are stored as separate blobs to keep the index lightweight.

### Review

A Review is a first-class assessment of a Task or Change, performed by a human or an agent.

```
Review {
  id:            uuid
  target:        Task id | Change id
  reviewer: {
    type:        human | agent
    identity:    string
    model:       string | null
  }

  status:        pending | approved | needs_changes | rejected
  comments: [{
    file:        string
    line_range:  [start, end]
    content:     string
    severity:    info | suggestion | warning | blocker
  }]

  analysis:      string | null (full review reasoning for agent reviews)
  created_at:    timestamp
}
```

**Mapping to Git:** Reviews are stored as blobs under `refs/arc/reviews/<review-id>.json`.

## Git Ref Layout

All Arc metadata lives under `refs/arc/` — a namespace that coexists with standard Git refs without interference.

```
refs/
  heads/
    main                              # production branch (standard Git)
    task/<task-id>                    # task branches (standard Git branches)

  arc/
    tasks/
      <task-id>.json                  # task metadata
    sessions/
      <session-id>/
        meta.json                     # session metadata
        transcript.json               # full conversation log
    reviews/
      <review-id>.json                # review data
    index/
      change-map.json                 # Change UUID → Git commit SHA
      content-index.json              # content hash → Change UUID
    config.json                       # repository-level Arc configuration
```

**What lives where:**

| Location | Shared? | Visible on GitHub? | Contents |
|---|---|---|---|
| `refs/arc/*` | Yes (pushed/pulled) | No (GitHub only renders `refs/heads/` and `refs/tags/`) | All shared metadata: tasks, sessions, reviews, indexes |
| `.arc/` | No (local only, gitignored) | No | SQLite index, worktrees, auto-save timeline |
| `.arc/worktrees/<slug>/` | No (local only) | No | One full working directory per active task |
| `.arc/sessions/<worktree>/` | No (local only) | No | Per-worktree session state (supports parallel agents) |
| Commit messages | Yes (part of Git history) | Yes (as trailing metadata below the main message) | `arc:` trailers: Change UUID, author type, model, confidence |

Nothing in the working tree is added by Arc. There are no sidecar files, no `.arc-metadata/` directories, no JSON files alongside your source code. The `.arc/` directory is local-only (like `.git/`) and should be added to `.gitignore`. Worktrees are also local — they're working directories for active tasks, not artifacts. The shared metadata lives exclusively in Git refs that GitHub and other platforms do not render in PRs, file browsers, or diffs.

These refs are pushed and pulled alongside code using `arc push` and `arc pull`, which internally run:

```
git push origin HEAD 'refs/arc/*'
git fetch origin 'refs/arc/*:refs/arc/*'
```

## Content-Addressed Identity

The central challenge of metadata-over-Git is that Git commit SHAs change during rebase, amend, cherry-pick, and squash-merge. Arc solves this with two complementary identity mechanisms. For a detailed walkthrough of how each Git operation affects Arc metadata and how Arc recovers, see [Git Interoperability](git-interop.md).

### Change UUIDs

Every Change has a UUID stored in the Git commit message. When a rebase rewrites commits, the UUIDs are preserved because they're part of the message content. After a rebase, Arc rebuilds the `change-map.json` index by scanning commits for their UUIDs.

```
Post-rebase reconciliation:

1. Walk all commits on the current branch
2. Parse Change UUID from each commit message
3. Rebuild change-map: { uuid → new_sha }
4. Update all refs/arc/ references that pointed to old SHAs
```

### Content Hashes

For line-level attribution (blame), Arc computes content hashes of code blocks. These survive any Git operation because they're derived from the code itself, not from Git's commit graph.

```
Layer 1: Exact content hash
  → SHA-256 of normalized code lines
  → Matches identical code after rebase, cherry-pick, copy

Layer 2: Structural hash (optional)
  → Hash of the AST, ignoring formatting and variable names
  → Matches semantically equivalent code

Layer 3: Anchor-based
  → Metadata attached to stable identifiers (function names, class names)
  → Survives minor edits within a function
```

## Sync Protocol

Arc uses Git as its transport layer. Metadata refs are pushed and fetched alongside code.

### Push

```
arc push

  1. Validate: all changes have required metadata
  2. Update refs/arc/index/* with current mappings
  3. git push origin HEAD
  4. git push origin 'refs/arc/*'
```

### Pull

```
arc pull

  1. git fetch origin 'refs/arc/*:refs/arc/*'
  2. git pull origin <branch>
  3. Reconcile: rebuild change-map if commits changed
  4. Merge metadata: append-only for most objects
```

### Reconciliation After History Rewrites

When Git operations rewrite history (rebase, squash-merge, amend), Arc reconciles its metadata store on the next `pull`. The reconciliation process re-indexes Change UUIDs, rebuilds content-hash mappings, and links squashed commits back to their original Changes. This is described in detail in [Git Interoperability](git-interop.md).

### Conflict Resolution

Metadata is overwhelmingly append-only. Two developers annotating the same code produces two annotations — not a conflict. For the rare cases where metadata does conflict (e.g., two people editing a Task's status simultaneously), Arc uses last-writer-wins with timestamp ordering.

## Configuration

Repository-level configuration is stored in `refs/arc/config.json`:

```json
{
  "version": 1,
  "policies": {
    "require_intent": true,
    "require_review_before_complete": true,
    "auto_checkpoint_interval": "5m",
    "allowed_reviewers": ["human", "agent"],
    "min_review_count": 1
  },
  "agents": {
    "trusted_models": ["claude-opus-4-6", "claude-sonnet-4-5"],
    "require_session_transcript": false,
    "max_confidence_without_review": 0.9
  }
}
```

## CLI Commands

| Command | Purpose |
|---|---|
| `arc init` | Initialize an Arc repository (wraps `git init`, sets up shell wrapper) |
| `arc task new <goal>` | Create a new task — creates worktree, branch, and metadata |
| `arc task list` | List all tasks, their status, and worktree paths |
| `arc task status` | Show detailed status of the current task |
| `arc task complete` | Merge the task into base branch, clean up worktree |
| `arc task switch <name>` | `cd` into a task's worktree (via shell wrapper) |
| `arc change <summary>` | Record a change with structured metadata |
| `arc checkpoint [message]` | Save a lightweight intermediate state |
| `arc undo [--to <change-id>]` | Revert one or more changes |
| `arc explore <question>` | Start an exploration — creates a temporary worktree |
| `arc review request` | Mark the current task as ready for review |
| `arc review run` | Run an automated agent review |
| `arc review show` | Display review comments |
| `arc review approve` | Approve the current task or change |
| `arc log [--reasoning] [--prompts]` | Show change history with optional metadata |
| `arc intent <file> [--line <range>]` | Show the Arc intent behind each line (like blame, but shows *why*) |
| `arc blame <file>` | Show line-level attribution with agent metadata |
| `arc query <question>` | Natural language query against the metadata store |
| `arc stats` | Show repository statistics (human vs. agent, models, etc.) |
| `arc push` | Push code and metadata to remote |
| `arc pull` | Pull code and metadata from remote |
| `arc sync` | Bidirectional sync |

## Adoption and Exit

Arc is designed so that adoption is incremental and exit is free.

### Adopting Arc on an Existing Git Repository

```
$ cd my-existing-project
$ arc init

Initialized Arc in existing Git repository.
No files modified. No history rewritten.
Created refs/arc/ namespace for metadata.
Created .arc/ directory (local only, gitignored).

Shell integration: add this to your shell profile:
  eval "$(arc shell-init)"
```

That's it. The existing Git history is untouched. From this point forward, the developer can use `arc change` instead of `git commit` to start capturing richer metadata. Or they can keep using `git commit` for some changes and `arc change` for others — Arc reads Git commits it didn't create and treats them as human-authored changes with minimal metadata.

The shell integration adds a thin wrapper so that `arc task switch` can `cd` you into a task's worktree. This is the same pattern used by `nvm`, `conda`, and other tools that need to change the shell's working directory — the `arc` binary can't change the parent shell's directory, so a shell function handles it.

### Mixed Teams (Some Use Arc, Some Use Git)

Arc is designed for mixed adoption:

| Developer uses | What they see | What they produce |
|---|---|---|
| `arc change` | Rich metadata, agent info, reasoning | Arc-enhanced Git commits + metadata refs |
| `git commit` | Normal Git experience, unchanged | Standard Git commits (Arc indexes them with minimal metadata) |
| `arc blame` | Full attribution (human, agent, prompts) | — |
| `git blame` | Normal blame (author, date, message) | — |

The Arc metadata refs (`refs/arc/*`) are invisible to plain Git users. They don't appear in `git log`, don't affect `git diff`, and don't cause merge conflicts. They're fetched only by tools that know to ask for them.

### Leaving Arc

If a team decides to stop using Arc:

```
$ arc eject

Removing worktrees...
  Removed .arc/worktrees/rate-limiting/ (branch preserved: task/a1b2-rate-limiting)
  Removed .arc/worktrees/fix-auth/ (branch preserved: task/e5f6-fix-auth)
Removing Arc metadata refs...
  Deleted refs/arc/tasks/*
  Deleted refs/arc/sessions/*
  Deleted refs/arc/reviews/*
  Deleted refs/arc/index/*
  Deleted refs/arc/config.json
Removing local Arc data...
  Deleted .arc/

Done. This is now a plain Git repository.
Your code, branches, and commit history are completely unchanged.
Task branches are preserved as regular Git branches.
```

Or simply stop using Arc commands and switch back to Git. The metadata refs remain but are inert — no different from any other unused ref. The commit history is standard Git. There is no migration, no data loss, and no degraded state.

### The Commit Message Contract

Arc stores a structured header in Git commit messages, but it's designed to be human-readable for people not using Arc:

```
Add token refresh interceptor

Intercept 401 responses and automatically retry with a fresh
token, preventing user-visible session expiration.

---
arc:change:id: c1d2e3f4-5678-9abc-def0-1234567890ab
arc:author:type: agent
arc:author:model: claude-sonnet-4-5
arc:session: sess-m1n2o3
arc:confidence: 0.92
```

A plain Git user sees a clear commit message with some trailing metadata they can ignore. An Arc user's tooling parses the structured block. If Arc is abandoned, the structured block stays in history as harmless extra context — arguably still useful, since it tells future readers which commits were agent-authored.

## Parallel Agent Execution

The worktree-first architecture makes it trivial to run multiple agents simultaneously on different tasks. Each agent operates in its own worktree with its own branch and staging area — there are no shared mutable resources at the working-tree level.

```
Terminal 1 (Agent A):                    Terminal 2 (Agent B):
$ arc task new "Rate limiting"           $ arc task new "Fix auth bug"
  → .arc/worktrees/rate-limiting/          → .arc/worktrees/fix-auth/
$ cd .arc/worktrees/rate-limiting/       $ cd .arc/worktrees/fix-auth/
$ arc session start --agent opencode     $ arc session start --agent opencode
  ... agent works ...                      ... agent works ...
$ arc change "Add limiter"               $ arc change "Fix token expiry"
  ... no conflicts ...                     ... no conflicts ...
```

**Shared resources and how they're handled:**

| Resource | Concurrency model |
|---|---|
| Git object store | Designed for concurrent access — no coordination needed |
| SQLite index (`.arc/db.sqlite`) | WAL mode — multiple concurrent readers and writers |
| Session state | Per-worktree files — no sharing, no conflicts |
| `refs/arc/*` | Each task writes to its own ref path; Git ref updates are atomic |
| Remote push | Different branches — `git push` doesn't conflict |

The only point where parallel work converges is the deliberate merge step (`arc task complete`), which is exactly when you want to discover and resolve conflicts.

## Implementation Notes

- **Language:** Rust for performance and single-binary distribution. Use `gitoxide` or `libgit2` bindings for Git operations.
- **Local index:** SQLite database in `.arc/` for fast queries against metadata. Rebuilt from `refs/arc/*` on clone/fetch.
- **Commit message format:** Structured trailer block (parseable) following a human-readable summary. Tools that don't understand Arc see a clear commit message with some trailing metadata. Tools that do understand Arc parse the structured block.
- **Backward compatibility:** `git log`, `git blame`, `git diff` all work on an Arc repository. They just don't show the rich metadata. `arc log`, `arc blame`, `arc diff` are the enhanced versions.
- **Forward compatibility:** If Arc is removed, the repository remains a fully functional Git repository with no artifacts, broken references, or degraded state.

For the full phased implementation plan, technology choices, project structure, and schema details, see [Implementation Plan](plan.md).
