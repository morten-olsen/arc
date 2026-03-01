# Arc

**Version control for how you actually work.**

---

## The Problem With Git (Yes, All of It)

Be honest. Run `git log --oneline -20` on your main project. Count how many commit messages tell you *why* the change was made.

We'll wait.

Now look at the ones that say `fix`, `wip`, `stuff`, `address review comments`, and the one someone typed during a panicked rebase at 2am. That's your project's institutional memory. That's what the new hire will read when they're debugging a production incident at 3am and need to understand *why* the payment retry logic uses a sliding window instead of a token bucket.

Good luck to them.

And that's just the human problem. Now add AI agents that write half your code, and Git — a tool designed in 2005 for humans emailing patches — has no idea. `git blame` says *you* wrote that clever bitwise optimization. You didn't. A robot did, based on a prompt you typed at 11pm and immediately forgot. The reasoning is gone. The prompt is gone. The alternatives it considered are gone. The commit message says:

```
implement auth improvements
```

Helpful.

## What Is Arc?

Arc is Git, but for how people actually (should) work in 2026.

Your code is still in Git. You still push to GitHub. Your CI still works. Your coworkers who refuse to install anything new don't have to. It was Git the whole time.

But Arc fixes the things Git gets wrong:

**Git makes clean history hard. Arc makes it automatic.** Everyone agrees commits should be atomic, self-contained units of working code. Nobody achieves it because `git rebase -i` is a nightmare. Arc flips it: you declare what you're building *before* you build it, save as often as you want, and the tool produces clean commits when you're done. The clean history is a byproduct of the workflow, not a weekend of interactive rebase.

**Git loses the "why." Arc captures it.** Every change has structured intent — one sentence explaining *why*, not just *what*. Six months from now, when someone asks "why is there an idempotency key here?", the answer is in the repo. Not in a Jira ticket that got moved. Not in a Slack thread that got archived. Not in the head of someone who left.

**Git makes context switching painful. Arc makes it instant.** Every task gets its own directory. Switching is `cd`, not `git stash && git checkout && git stash pop && oh no where did my changes go`. You can have five features in flight, two agents working in parallel, and a hotfix branch — all at once, no stashing, no conflicts, no drama.

**Git doesn't know about agents. Arc does.** When a robot writes your code, that fact is recorded. Which model, what prompt, how confident it was. Optional for now, but the plumbing is there from day one.

## "But I Don't Use AI Agents"

Cool. Arc is still better than what you have.

The workflow improvements alone are worth it:

```bash
# Declare what you're about to build (not just "wip")
$ arc change "Add sliding window rate limiter" \
    --intent "Token bucket was too bursty for our traffic pattern"

# Save as often as you want. Nobody sees this mess.
$ arc checkpoint
$ arc checkpoint "tests passing"
$ arc checkpoint

# When you're done, one command → clean atomic commits
$ arc task finalize
```

Three declared changes, forty-seven checkpoints, two dead-end approaches — all collapsed into three clean commits with clear intent. No interactive rebase. No `fixup`. No `squash`. No tears.

## How It Works (60-Second Version)

### The workflow

```bash
# Start a task — gets its own directory, its own branch
$ arc task new "Add rate limiting" --ref JIRA-1234

# Switch into it (your other work is untouched)
$ arc task switch add-rate-limiting

# Declare what you're building
$ arc change "Add sliding window counter" \
    --intent "Token bucket was too bursty for our traffic pattern"

# Write code. Save whenever you want. Go wild.
$ arc checkpoint
$ arc checkpoint "core logic done"
$ arc checkpoint

# Next logical unit of work
$ arc change "Wire limiter into HTTP middleware" \
    --intent "Hook rate limiting into the request pipeline"

# More code, more saves
$ arc checkpoint
$ arc checkpoint "integration tests passing"

# Push for PR review — reviewers see the full working history
$ arc push

# Reviewer finds an issue in the rate limiter? Fix it properly.
$ arc fix 7d64aa2f "handle empty window edge case"
$ arc checkpoint
$ arc push

# PR approved. Collapse into clean atomic commits.
$ arc task finalize
$ arc push --force

# Merged on GitHub. Clean up.
$ arc task complete
```

Your branch goes from this:

```
Add sliding window counter
[checkpoint] implement core logic
[checkpoint] core logic done
[checkpoint] add tests
Wire limiter into HTTP middleware
[checkpoint] add middleware hook
[checkpoint] integration tests passing
[fix → Add sliding window counter] handle empty window edge case
```

To this:

```
Add sliding window counter
  Token bucket was too brusty for our traffic pattern.

Wire limiter into HTTP middleware
  Hook rate limiting into the request pipeline.
```

Two clean commits. Each one compiles, passes tests, and explains why it exists. The checkpoints, the dead ends, the review fixes — all folded in. The reviewer saw the full history. The final history is clean.

### Under the hood

Arc stores metadata in `refs/arc/*` — a Git namespace that's invisible on GitHub. It's pushed alongside your code but doesn't show up in PRs, file browsers, or diffs. Your teammates who use plain Git never see it.

Commit messages have structured trailers:

```
Add sliding window counter

Token bucket was too bursty for our traffic pattern.

---
arc:change:id: 7d64aa2f-...
arc:author:type: human
arc:task:ref: JIRA-1234
```

Anyone reading `git log` sees a well-written commit with some trailing metadata they can ignore. Anyone using Arc sees the full picture.

## The Real-World Scenarios

Arc is designed for how you actually spend your day — not the idealized Git tutorial workflow where you work on one thing at a time and never get interrupted.

**You're mid-feature and production breaks.** `arc task new "hotfix: fix payment crash"` — creates a new task branching from main. Your feature work is untouched in its worktree. Fix the bug, push for PR, switch back. Zero context loss.

**Two agents working simultaneously.** Each gets its own task, its own worktree. They can't interfere with each other. You review each independently.

**PR review feedback.** Your reviewer says the rate limiter has an edge case. `arc fix <change-id>` — the fix is linked to the specific change it addresses. When you finalize, it gets folded into that change. No "address review comments" commit polluting your history.

**Your branch is stale.** `arc task sync` rebases onto main. Auto-checkpoints your dirty work first. Shows your intent alongside any conflicts so you actually know what you were doing when you resolve them.

**Quick one-liner on main.** Not everything needs a task. `arc change` and `arc checkpoint` work anywhere. Use as much or as little structure as you want.

See the [workflow guide](docs/workflow.md) for all ten scenarios in detail.

## "What About My Team That Refuses to Change Anything?"

They don't have to.

- **You** use Arc — structured commits, isolated tasks, agent tracking
- **Your colleague** uses `git commit -m "fix"` — works exactly as before
- **Both** push to the same GitHub repo. Same PRs. Same merge gates. Same CI.

Your colleague's commits show up in `arc log` with whatever Git provides. Your commits show up in `git log` as slightly better than average. Everyone coexists. No flag day.

## "What If I Want to Stop Using Arc?"

```
arc eject
```

That's it. You're left with a perfectly normal Git repository. Your code is unchanged. Your branches are unchanged. Your history is unchanged. The commit messages have some extra trailers that are harmless to keep — they're just text.

There is no migration. No export. No penalty. No "oh god how do I convert this back to Git." It was Git the whole time.

This isn't an accident. It's a [design principle](docs/goals.md). The fastest way to never get adopted is to make people scared of lock-in. So there is none.

## "What Happens When GitHub Squash-Merges My Carefully Annotated PR?"

Ah, the hard problem. GitHub's "Squash and merge" takes your three lovingly crafted atomic commits and smashes them into one commit with a generated message.

Arc handles it. The metadata lives in `refs/arc/*`, independent of the commit graph. On the next `arc pull`, Arc matches the squash commit back to the original changes via content hashes and trailers. Your metadata survives.

Covered in gory detail in [Git Interoperability](docs/git-interop.md).

## Quick Comparison

| | Git | Arc |
|---|---|---|
| Storage | Git objects | Git objects (same) |
| Push to GitHub | Yes | Yes (same remote) |
| Clean atomic history | Aspirational (requires rebase -i) | Automatic (declare → checkpoint → finalize) |
| Commit messages | Free text, often useless | Structured intent |
| Context switching | Branch/stash/checkout dance | `cd` into another worktree |
| Who wrote it | Author field (always human) | Human or agent, with model info |
| Why it was written | Commit message (if you're lucky) | Intent field (inline via `arc intent <file>`) |
| PR review cycle | "address review comments" commits | Fixes linked to changes, squashed on finalize |
| Parallel work | One branch at a time (or pain) | One worktree per task, unlimited |
| Undo | `git reset` (scary) | `arc undo` (not scary) |
| Lock-in | — | None. `arc eject` → plain Git |

## Documentation

- **[Goals](docs/goals.md)** — What Arc is trying to solve, design constraints, and non-goals
- **[Developer Workflow](docs/workflow.md)** — Ten real-world scenarios from solo development to mixed teams
- **[System Design](docs/design.md)** — Architecture, primitives, data model, and implementation notes
- **[Why This Should Exist](docs/reason.md)** — The full argument for Arc
- **[Git Interoperability](docs/git-interop.md)** — How metadata survives squash, rebase, cherry-pick, and amend
- **[Implementation Plan](docs/plan.md)** — Phased build plan and project structure

## Status

Arc is in active development. The core workflow — tasks, changes, checkpoints, undo, log, intent, push/pull, and eject — works end-to-end. Agent attribution, `arc fix`, `arc task finalize`, and `arc task sync` are in progress.

If you've read this far and thought "this is a solution in search of a problem" — go check your `git log`. We already asked you to count. How'd that go?
