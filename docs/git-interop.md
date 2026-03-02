# Arc — Git Interoperability

Arc uses Git as its storage backend. This means Arc repositories are subject to all the operations that Git (and Git hosting platforms like GitHub) can perform — including operations that rewrite, squash, or discard commits. This document describes how Arc preserves metadata lineage through each of these operations.

## The Core Challenge

Arc stores rich metadata (intent, reasoning, agent attribution, sessions, reviews) and links it to code via Change UUIDs and content hashes. Git operations can destroy the link between metadata and code in two ways:

1. **Commit SHAs change.** Rebase, amend, and squash create new commits with new SHAs. Any metadata keyed by SHA becomes orphaned.
2. **Commit messages are rewritten.** Squash-merge on GitHub generates a new message, discarding the original messages (and their `arc:` trailers).

Arc's architecture is designed so that neither of these is fatal. The metadata lives in `refs/arc/*`, indexed by Change UUID and content hash — not by commit SHA. The commit message trailers are a convenience, not the source of truth.

## Squash Merge (GitHub PR)

This is the most common destructive operation. A developer opens a PR for a task branch, and the reviewer clicks "Squash and merge" on GitHub.

### What Happens

```
Task branch (3 changes):
  commit a1b2c3  arc:change:id: uuid-1  "Define retry interface"
  commit d4e5f6  arc:change:id: uuid-2  "Implement retry logic"
  commit g7h8i9  arc:change:id: uuid-3  "Add circuit breaker"

GitHub squash-merges into main:
  commit x9y8z7  "Add payment retry (#42)"
                  ← new SHA, new message, original trailers gone
```

Three Changes with rich metadata become one Git commit with a generated message. The `arc:` trailers are gone from the commit.

### How Arc Recovers

On the next `arc pull`, Arc detects the squash-merge and reconciles:

```
$ arc pull

Pulling from origin...
  Detected squash-merge: task/f7a1b2c3-payment-retry → main
  Squash commit: x9y8z7
  Original changes: uuid-1, uuid-2, uuid-3

  Reconciling...
    Created Change uuid-sq for squash commit x9y8z7
    Linked: uuid-sq ← squashed-from [uuid-1, uuid-2, uuid-3]
    Re-indexed content hashes against squash commit
    Task PAYMENT-RETRY: marked completed (squash-merged)
```

**Detection** uses three signals:

1. **Branch deletion + main advance.** Arc tracks which task branches exist. When one disappears and main has a new commit, it's a merge candidate.
2. **GitHub API.** `gh pr list --state merged` confirms which PR merged which branch into which commit.
3. **Diff matching.** Arc compares the cumulative diff of the task's changes against the squash commit's diff. If they match (or nearly match), it's confirmed.

**Reconciliation** creates a new Change in the metadata store:

```json
{
  "id": "uuid-sq",
  "parent": null,
  "git_commit": "x9y8z7",
  "author": {
    "type": "human",
    "identity": "marcus"
  },
  "summary": "Squash merge of PAYMENT-RETRY",
  "intent": "Combined: Define retry interface + Implement retry logic + Add circuit breaker",
  "derived_from": ["uuid-1", "uuid-2", "uuid-3"],
  "tags": ["squash-merge"],
  "created_at": "2026-03-01T14:30:00Z"
}
```

> **Note:** design.md defines `derived_from` as `Change id | null` (singular). For squash reconciliation, Arc extends this to an array of Change ids to capture all source Changes. The original per-change authors are preserved in the linked Changes.

The original Changes (uuid-1, uuid-2, uuid-3) remain in the metadata store with all their detail — prompts, reasoning, session transcripts. They're linked to the squash Change via `derived_from`.

### Blame After Squash

`git blame` after a squash attributes every line to the single squash commit — useless for understanding who actually wrote what.

`arc blame` uses its content-hash index, which is independent of the commit graph:

```
$ git blame src/payments/retry.go
  x9y8z7  (marcus 2026-03-01)  func (r *retrier) AttemptWithRetry(
  x9y8z7  (marcus 2026-03-01)      ctx context.Context,
  x9y8z7  (marcus 2026-03-01)      payment *Payment,
  x9y8z7  (marcus 2026-03-01)  ) (*Result, error) {
  x9y8z7  (marcus 2026-03-01)      var lastErr error
  ← every single line: x9y8z7, marcus

$ arc blame src/payments/retry.go
  32│ func (r *retrier) AttemptWithRetry(    marcus (human)        uuid-1
  33│     ctx context.Context,
  34│     payment *Payment,
  35│ ) (*Result, error) {
  36│     var lastErr error                  claude-sonnet-4-5     uuid-2
  37│     for attempt := 0; attempt <          prompt: "Implement the
  38│         r.config.MaxAttempts;              PaymentRetrier..."
  39│         attempt++ {                       confidence: 0.92
  45│         lastErr = &PaymentError{       marcus (human)        uuid-3
  46│             Code: ErrRetryFailed,        intent: "Use domain error
  47│             Cause: err,                   types for consistency"
  ← line-level attribution preserved through squash
```

This works because `arc blame` doesn't ask Git "which commit last touched this line?" It asks its own index "which Change produced this block of code?" — answered by content hash matching.

### Arc-Controlled Squash

When Arc controls the merge (via `arc task complete --squash` instead of the GitHub UI), it writes a squash commit with full metadata:

```
$ arc task complete --squash

Add payment retry with exponential backoff

Implement automatic retry with exponential backoff for failed
payment attempts, with a stateless circuit breaker via Redis.

---
arc:change:id: uuid-sq
arc:squashed-from: uuid-1, uuid-2, uuid-3
arc:task:id: f7a1b2c3
arc:author:type: mixed
arc:authors: marcus (human), claude-sonnet-4-5 (agent)
```

This is the ideal path — the squash commit itself carries the lineage. But Arc doesn't depend on it. The reconciliation path handles the GitHub UI case where Arc can't control the message.

## Rebase

Rebase rewrites commit SHAs but preserves commit messages (including `arc:` trailers).

### What Happens

```
Before rebase:
  commit a1b2c3 (main)
  commit d4e5f6  arc:change:id: uuid-1  "Define retry interface"
  commit g7h8i9  arc:change:id: uuid-2  "Implement retry logic"

After rebase onto updated main:
  commit a1b2c3 (main, with new commits)
  commit NEW-S1  arc:change:id: uuid-1  "Define retry interface"
  commit NEW-S2  arc:change:id: uuid-2  "Implement retry logic"
  ← new SHAs, but same messages (UUIDs preserved)
```

### How Arc Handles It

```
$ arc task sync
  # or: git rebase main (Arc detects on next operation)

  1. Record pre-rebase state: { uuid-1: d4e5f6, uuid-2: g7h8i9 }
  2. Perform: git rebase main
  3. Walk new commits, extract Change UUIDs from messages
  4. Rebuild change-map: { uuid-1: NEW-S1, uuid-2: NEW-S2 }
  5. Update refs/arc/index/change-map.json
```

Because the UUIDs are in the commit messages and messages survive rebase, this is straightforward. No content-hash matching needed — just re-scan and re-index.

## Amend

Amending the most recent commit changes its SHA and potentially its message.

### What Happens

```
Before amend:
  commit a1b2c3  arc:change:id: uuid-1  "Define retry interface"

After amend (added a file, updated message):
  commit NEW-SHA  arc:change:id: uuid-1  "Define retry interface and config"
```

### How Arc Handles It

If the `arc:change:id` trailer is preserved in the amended message (which it is by default — amending usually keeps existing message content), Arc re-indexes on the next operation:

```
change-map: { uuid-1: a1b2c3 } → { uuid-1: NEW-SHA }
```

If the user rewrites the message and removes the trailer, Arc falls back to content-hash matching to re-link the metadata.

Arc can also wrap this via `arc change --amend`, which stages changes, amends the Git commit, and updates the change-map in one step. When the user runs plain `git commit --amend` instead, Arc detects the amended commit on the next operation and re-indexes automatically.

## Cherry-Pick

Cherry-pick copies a commit to a different branch, creating a new commit with a new SHA.

### What Happens

```
On feature-branch:
  commit a1b2c3  arc:change:id: uuid-1  "Fix auth token validation"

Cherry-picked to hotfix-branch:
  commit NEW-SHA  arc:change:id: uuid-1  "Fix auth token validation"
  ← new SHA, same message (UUID preserved)
```

### How Arc Handles It

The same UUID now appears in two commits on different branches. Arc's change-map supports this:

```json
{
  "uuid-1": {
    "commits": {
      "feature-branch": "a1b2c3",
      "hotfix-branch": "NEW-SHA"
    },
    "canonical": "a1b2c3"
  }
}
```

The `canonical` field points to the original. The Change metadata (intent, reasoning, agent info) is shared — it describes the same logical change regardless of which branch it's on.

> **Note:** This multi-branch mapping is a change-map extension beyond design.md's `derived_from` field. The cherry-picked commit retains the same UUID in its message, creating a one-UUID-to-many-commits situation that the change-map must track. The `derived_from` field on a new Change can also represent cherry-pick lineage when the cherry-pick produces a distinct Change.

`arc log` on either branch shows the full metadata. `arc blame` on either branch attributes the code correctly.

## Force Push

Force push replaces remote history. This is destructive from Git's perspective but Arc handles it the same way as rebase — because force push usually follows a rebase or amend.

### How Arc Handles It

```
$ arc push --force
  1. Push code (new SHAs go to remote)
  2. Rebuild change-map from current branch state
  3. Push updated refs/arc/* with new mappings
```

When a developer runs `git push --force` directly (bypassing Arc), the remote history is rewritten but `refs/arc/*` are not updated. Arc reconciles on the next operation by any developer: `arc pull` detects that commit SHAs in the change-map no longer match the branch and triggers a re-scan — walking commits for UUIDs, rebuilding the change-map, and falling back to content-hash matching if needed.

## Interactive Rebase (Reorder, Edit, Drop)

Interactive rebase can reorder commits, edit their content, or drop them entirely.

### Reorder

Commits get new SHAs but messages (and UUIDs) are preserved. Same handling as regular rebase.

### Edit

A commit is modified — its content changes and it gets a new SHA. The UUID in the message stays the same, so Arc re-links it. The content-hash index is updated to reflect the new code content:

```
Before: uuid-1 → content hashes [hash-A, hash-B]
After edit: uuid-1 → content hashes [hash-A, hash-C]
  (hash-B is gone, hash-C is new)
```

Line-level blame is updated accordingly. Lines matching hash-A still attribute to uuid-1. Lines matching hash-C attribute to uuid-1 with a "modified during rebase" annotation.

### Drop

A commit is removed entirely. Its code is gone from the branch. Arc records the orphan status as a change-map annotation — the Change itself in `refs/arc/` is preserved as-is:

```
Change-map entry for a dropped commit:
{
  "uuid-dropped": {
    "status": "orphaned",
    "reason": "commit dropped during interactive rebase",
    "orphaned_at": "2026-03-01T15:00:00Z"
  }
}
```

The Change metadata is preserved in `refs/arc/` (it might be useful for understanding what was tried and discarded). The orphan status is an index-level annotation in the change-map, not a mutation of the Change struct itself. The Change is no longer linked to any commit on the current branch.

## Merge Commit (No Squash)

A standard merge commit preserves all original commits and their messages. This is the easiest case — nothing is rewritten, nothing is lost. Arc needs no reconciliation.

```
$ arc task complete
  # default: git merge --no-ff

  All original commits preserved on main.
  All arc:change:id trailers intact.
  Change-map updated to reflect commits are now on main.
  Task marked completed.
```

## Summary: What Survives What

| Operation | Commit SHAs | Commit messages | arc: trailers | Arc metadata (refs/arc/*) | Content hashes |
|---|---|---|---|---|---|
| **Merge (no squash)** | Preserved | Preserved | Preserved | Intact | Intact |
| **Rebase** | Changed | Preserved | Preserved | Re-indexed via UUID | Intact |
| **Amend** | Changed | Usually preserved | Usually preserved | Re-indexed via UUID | Updated |
| **Cherry-pick** | New SHA | Preserved | Preserved | Multi-branch mapping | Intact |
| **Squash (Arc)** | New SHA | Arc-controlled | New trailer with lineage | Linked via derived-from | Re-indexed |
| **Squash (GitHub)** | New SHA | Rewritten | Lost | Reconciled via detection | Re-indexed |
| **Force push** | Changed | Preserved | Preserved | Re-indexed on pull | Intact |
| **Interactive rebase** | Changed | Preserved (unless edited) | Preserved (unless edited) | Re-indexed, orphaned if dropped | Updated |

The key insight: **Arc's metadata store (`refs/arc/*`) and content-hash index are independent of Git's commit graph.** Git operations that rewrite the commit graph require Arc to re-index, but the metadata itself — the intent, reasoning, prompts, agent attribution, sessions, reviews — is never destroyed by a Git operation. The worst case (GitHub squash-merge) requires reconciliation, but the data is recovered automatically.

## Recommendations for Teams

1. **Prefer `arc task complete` over GitHub's merge button** when possible. Arc produces better squash commits with full lineage metadata.

2. **If using GitHub's merge button, prefer "Create a merge commit"** over "Squash and merge." Merge commits preserve all original commits and require no reconciliation.

3. **If squash-merging via GitHub is your team's convention**, Arc handles it — the reconciliation is automatic. You lose the `arc:` trailers in the squash commit message, but all metadata is preserved in `refs/arc/*`.

4. **Run `arc pull` regularly.** Reconciliation happens during pull. The sooner Arc sees a squash-merge, the sooner it re-indexes.

5. **For CI/CD pipelines that need Arc metadata**, use `arc blame` or `arc log` rather than parsing commit messages. The metadata store is the source of truth, not the commit trailers.
