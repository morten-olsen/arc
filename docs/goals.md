# Arc — Goals

What Arc is trying to solve, who it's for, and the constraints it operates under.

---

## The Problem

Git tracks *what* changed and *who committed it*. That was enough when humans wrote all the code and commit messages were the only narrative. It's not enough anymore.

**Missing context.** Commit messages are free text with no structure. In practice they're useless — `fix`, `wip`, `stuff`, `address review comments`. The *why* behind a change is lost within days, scattered across Slack threads, Jira tickets, and closed browser tabs.

**No agent awareness.** AI agents now write significant portions of code. Git attributes everything to the human who ran the agent. There's no record of which model produced the code, what prompt was used, or how confident the agent was. When agent-generated code breaks in production, the debugging trail is cold.

**Hostile workflow for modern development.** Real work doesn't happen in neat sequential commits on a single branch. You have multiple features in flight, agents working in parallel, emergency hotfixes, PR review cycles, and context switches every hour. Git's branch/stash/checkout model creates friction at every transition.

**Disposable history.** The development process — what was tried and rejected, what the agent was thinking, why an approach was abandoned — is as valuable as the final code. Git preserves none of it. Interactive rebase actively destroys it.

---

## Who Arc Is For

**Individual developers** who want structured intent on their commits, isolated workspaces for parallel tasks, and a clean atomic history without the pain of interactive rebase.

**Developers using AI agents** who want to track what the agent wrote, review it properly, undo it cleanly, and preserve the reasoning for future archaeology.

**Teams with mixed workflows** where some people use Arc, some use plain Git, and everyone pushes to the same GitHub repo through standard PR review processes.

**Organizations** that need audit trails for AI-generated code — which model, which prompt, which human approved it.

---

## Goals

### 1. Atomic commit history as the easy default

Every commit in the final history should represent one logical, self-contained, working unit of change. This is universally accepted as best practice but rarely achieved because Git makes it hard — you have to think about commit structure retroactively via interactive rebase.

Arc inverts this: you declare intent *before* working (`arc change`), save frequently as you go (`arc checkpoint`), and the tool automatically produces clean atomic commits when you're done (`arc task finalize`). The clean history is a byproduct of the workflow, not an afterthought.

### 2. Structured metadata without ceremony

Every change captures *why* it was made, not just what changed. Ticket references, intent, agent attribution — structured and queryable, not buried in free-text commit messages.

But metadata should flow naturally from the workflow, not feel like paperwork. Declaring a change takes one command. Most metadata fields are optional. The tool adds value at every level of engagement.

### 3. Seamless context switching

Real development involves constant interruption — a production hotfix while you're mid-feature, a PR review request, an agent finishing work on a different task. Switching between contexts should be instant, safe, and lossless.

Arc uses Git worktrees so every task has its own isolated directory. Switching is `cd`, not `git stash && git checkout && ...`. Multiple tasks can be open simultaneously. Multiple agents can work in parallel.

### 4. Works with GitHub flow and existing teams

Arc does not replace GitHub. PRs, merge gates, code review, CI — all unchanged. Arc produces branches that look normal, commits that read naturally, and metadata that's invisible unless you have Arc installed.

A team can adopt Arc incrementally: one developer starts using it, their coworkers see slightly better commit messages. No flag day, no migration, no disruption.

### 5. Agent attribution as a first-class concept

When an agent writes code, that fact should be recorded at the source — not inferred, not guessed, not lost. Which model, which tool, what prompt, what confidence level. All optional for now, but the plumbing exists from day one.

### 6. Clean PR review cycles

The PR review workflow today is broken: you push code, get feedback, add `fix review comments` commits that pollute history, then squash everything and lose the atomic structure. Or you amend commits and force-push, which is error-prone and loses the review trail.

Arc's model: push the full working history (changes + checkpoints) for review. Make fixes explicitly linked to the change they belong to. When approved, finalize into clean atomic commits. The reviewer sees progress, the final history is clean, and nothing is lost.

### 7. Zero lock-in

An Arc repository is a Git repository. `arc eject` removes all Arc artifacts and leaves a perfectly normal repo. Commits have some extra trailers that are harmless to keep. There is no migration, no export, no penalty. This is a hard design constraint, not a nice-to-have.

---

## Non-Goals (For Now)

- **Replacing GitHub's review UI.** Arc captures review metadata but the PR review experience stays on GitHub.
- **Real-time agent session capture.** Recording prompts, reasoning, and transcripts is planned but not part of v1. The schema supports it; the integrations come later.
- **Auto-detecting agents.** For now, agent attribution is explicit (`--agent --model`). IDE/tool plugins come later.
- **Conflict resolution intelligence.** `arc task sync` uses git rebase. Arc doesn't try to resolve conflicts — it surfaces them clearly with context about intent.

---

## Workflow Scenarios

These are the real-world situations Arc must handle well. Each is detailed in [workflow.md](workflow.md).

1. **Solo feature development** — Create a task, declare changes, checkpoint frequently, finalize into clean commits, push.

2. **Agent-assisted development** — Same as above, but the agent creates checkpoints with `--agent` flag. Human reviews, makes manual changes, both contributions tracked.

3. **Parallel agents** — Two agents working on separate tasks in separate worktrees. Zero coordination needed. Conflicts surface at merge time.

4. **Emergency hotfix** — Mid-feature, production breaks. Create a new task (branches from main), fix it, push for PR, switch back to your feature. Your feature work is untouched in its worktree.

5. **PR review cycle** — Push branch for review, get feedback, make fixes linked to specific changes, push again (reviewer sees incremental progress), finalize when approved, merge via GitHub.

6. **Coworker review** — A coworker's PR needs your review. Your work is safe in its worktree. Check out their branch in the main worktree or another terminal. Review. Switch back.

7. **Keeping a task up to date** — `arc task sync` rebases your task onto the latest base branch. Auto-checkpoints dirty work first.

8. **Mixed team** — Some developers use Arc, some use plain Git. Both push to the same repo. Arc enriches what it can, degrades gracefully for plain Git commits.

9. **Abandoning work** — Task didn't work out. `arc task abandon` removes the worktree, records why in metadata.

10. **Working without a task** — Quick one-line fix on main. `arc change` and `arc checkpoint` work anywhere, tasks are optional.

---

## Design Constraints

These are non-negotiable properties of the system:

- **Git underneath.** Every Arc repo is a valid Git repo. `git clone`, `git log`, `git blame` all work.
- **Invisible to non-users.** Arc metadata refs don't appear on GitHub. Commit trailers are readable text anyone can ignore.
- **Additive only.** Arc never rewrites existing Git history during normal operations. It adds metadata alongside.
- **Graceful degradation.** Missing metadata is fine. A commit without intent, a task without a ticket reference, an agent checkpoint without a model — all valid. More metadata is better, but none is required.
- **Local-first.** `.arc/` is local. Works without a remote. Works offline. Syncs when you push.
- **Repo-level config.** Team conventions (required fields, ticket prefixes, finalize strategy) stored in `refs/arc/config.json`, shared via push/pull.
