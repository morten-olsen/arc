#!/usr/bin/env bash
#
# End-to-end integration test for Arc CLI.
#
# Usage:
#   cargo build && bash tests/e2e.sh
#
# Or with a custom binary path:
#   ARC_BIN=./target/release/arc bash tests/e2e.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ARC_BIN="${ARC_BIN:-$PROJECT_DIR/target/debug/arc}"

PASS=0
FAIL=0
TESTS=()

pass() { PASS=$((PASS + 1)); TESTS+=("  ✓ $1"); }
fail() { FAIL=$((FAIL + 1)); TESTS+=("  ✗ $1: $2"); }

assert_contains() {
    local label="$1" output="$2" expected="$3"
    if echo "$output" | grep -qF "$expected"; then
        pass "$label"
    else
        fail "$label" "expected output to contain '$expected'"
    fi
}

assert_not_contains() {
    local label="$1" output="$2" unexpected="$3"
    if echo "$output" | grep -qF "$unexpected"; then
        fail "$label" "output should not contain '$unexpected'"
    else
        pass "$label"
    fi
}

assert_exit_code() {
    local label="$1" expected="$2" actual="$3"
    if [ "$actual" -eq "$expected" ]; then
        pass "$label"
    else
        fail "$label" "expected exit code $expected, got $actual"
    fi
}

assert_line_count() {
    local label="$1" output="$2" expected="$3"
    local actual
    actual=$(echo "$output" | grep -c '.' || true)
    if [ "$actual" -eq "$expected" ]; then
        pass "$label"
    else
        fail "$label" "expected $expected lines, got $actual"
    fi
}

# ─── Setup ────────────────────────────────────────────────────────────

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Arc CLI End-to-End Tests"
echo "========================"
echo "Binary: $ARC_BIN"
echo "Tmpdir: $TMPDIR"
echo ""

# Check binary exists
if [ ! -x "$ARC_BIN" ]; then
    echo "ERROR: Binary not found at $ARC_BIN"
    echo "Run 'cargo build' first."
    exit 1
fi

# ─── Test: Save command removed ───────────────────────────────────────

OUT=$($ARC_BIN save 2>&1 || true)
assert_contains "save command removed" "$OUT" "unrecognized subcommand"

# ─── Test: Help output ────────────────────────────────────────────────

OUT=$($ARC_BIN --help 2>&1)
assert_contains "help shows fix command" "$OUT" "fix"
assert_contains "help shows checkpoint" "$OUT" "checkpoint"
assert_not_contains "help omits save" "$OUT" "save"

OUT=$($ARC_BIN task --help 2>&1)
assert_contains "task help shows sync" "$OUT" "sync"
assert_contains "task help shows finalize" "$OUT" "finalize"
assert_contains "task help shows abandon" "$OUT" "abandon"

# ─── Setup: test repository ──────────────────────────────────────────

cd "$TMPDIR"
git init --bare remote.git --quiet
git clone remote.git repo --quiet 2>&1
cd repo
git config user.email "test@test.com"
git config user.name "Test User"
git config commit.gpgsign false
git config tag.gpgsign false
git config gpg.format openpgp
git config --unset gpg.program 2>/dev/null || true
git config --unset gpg.ssh.program 2>/dev/null || true
echo "hello" > README.md
git add -A && git commit -m "initial commit" --quiet
git push origin main --quiet 2>&1

# ─── Test: arc init ───────────────────────────────────────────────────

OUT=$($ARC_BIN init 2>&1)
assert_contains "arc init" "$OUT" "Initialized Arc"

# ─── Test: task new with --ref ────────────────────────────────────────

OUT=$($ARC_BIN task new "Test feature" --ref TEST-1 2>&1)
assert_contains "task new creates task" "$OUT" "Created task: Test feature"
assert_contains "task new shows ref" "$OUT" "Ref:      TEST-1"
assert_contains "task new shows branch" "$OUT" "Branch:"

WORKTREE="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d | head -1)"
cd "$WORKTREE"

# ─── Test: task status shows ticket ref ───────────────────────────────

OUT=$($ARC_BIN task status 2>&1)
assert_contains "task status shows name" "$OUT" "Task: Test feature"
assert_contains "task status shows ref" "$OUT" "Ref:     TEST-1"

# ─── Test: change command ─────────────────────────────────────────────

echo "feature code" > feature.rs
OUT=$($ARC_BIN change "First change" --intent "Testing the workflow" 2>&1)
assert_contains "change creates commit" "$OUT" "Change ["
assert_contains "change shows summary" "$OUT" "First change"

# ─── Test: checkpoint command ─────────────────────────────────────────

echo "more code" >> feature.rs
OUT=$($ARC_BIN checkpoint "some work" 2>&1)
assert_contains "checkpoint 1" "$OUT" "Checkpoint ["

echo "even more" >> feature.rs
OUT=$($ARC_BIN checkpoint "more work" 2>&1)
assert_contains "checkpoint 2" "$OUT" "Checkpoint ["

# ─── Test: second change ─────────────────────────────────────────────

echo "second feature" > second.rs
OUT=$($ARC_BIN change "Second change" --intent "More testing" 2>&1)
assert_contains "second change" "$OUT" "Change ["

echo "done" >> second.rs
OUT=$($ARC_BIN checkpoint "done" 2>&1)
assert_contains "checkpoint 3" "$OUT" "Checkpoint ["

# ─── Test: arc intent ─────────────────────────────────────────────────

OUT=$($ARC_BIN intent feature.rs 2>&1)
assert_contains "intent shows summary" "$OUT" "First change"
assert_contains "intent shows intent text" "$OUT" "Testing the workflow"

OUT=$($ARC_BIN intent feature.rs --line 1 2>&1)
assert_contains "intent --line shows content" "$OUT" "feature code"

OUT=$($ARC_BIN intent README.md 2>&1)
assert_contains "intent non-arc shows fallback" "$OUT" "(no arc intent)"

# ─── Test: fix command ────────────────────────────────────────────────

FIRST_ID=$($ARC_BIN log --all 2>&1 | grep "First change" | head -1 | sed 's/.*\[\(.*\)\].*/\1/')
echo "fix" >> feature.rs
OUT=$($ARC_BIN fix "$FIRST_ID" "fix edge case" 2>&1)
assert_contains "fix creates commit" "$OUT" "Fix ["
assert_contains "fix shows message" "$OUT" "fix edge case"

# ─── Test: log (default hides checkpoints/fixes) ─────────────────────

OUT=$($ARC_BIN log 2>&1)
assert_contains "log shows changes" "$OUT" "First change"
assert_contains "log shows second" "$OUT" "Second change"
assert_not_contains "log hides checkpoints" "$OUT" "checkpoint"
assert_not_contains "log hides fixes" "$OUT" "fix  "

# ─── Test: log --all shows everything ────────────────────────────────

OUT=$($ARC_BIN log --all 2>&1)
assert_contains "log --all shows checkpoints" "$OUT" "checkpoint"
assert_contains "log --all shows fixes" "$OUT" "fix  "

# ─── Test: git log has proper summary prefixes ────────────────────────

OUT=$(git log --oneline 2>&1)
assert_contains "git log has checkpoint prefix" "$OUT" "[checkpoint"
assert_contains "git log has fix prefix" "$OUT" "[fix"

# ─── Test: finalize squashes correctly ────────────────────────────────

OUT=$($ARC_BIN task finalize 2>&1)
assert_contains "finalize runs" "$OUT" "Finalize complete"
assert_contains "finalize leaves 2 commits" "$OUT" "2 clean commits remain"

GIT_LOG=$(git log --oneline 2>&1)
COMMIT_COUNT=$(echo "$GIT_LOG" | grep -v "initial commit" | grep -c '.' || true)
if [ "$COMMIT_COUNT" -eq 2 ]; then
    pass "finalize: exactly 2 task commits remain"
else
    fail "finalize: exactly 2 task commits remain" "got $COMMIT_COUNT commits"
fi

# Verify checkpoints marked as squashed
OUT=$($ARC_BIN log --all 2>&1)
SQUASHED_COUNT=$(echo "$OUT" | grep -c '\[squashed\]' || true)
if [ "$SQUASHED_COUNT" -ge 3 ]; then
    pass "finalize: checkpoints/fixes marked squashed"
else
    fail "finalize: checkpoints/fixes marked squashed" "expected >=3 squashed, got $SQUASHED_COUNT"
fi

# ─── Test: agent flags ───────────────────────────────────────────────

cd "$TMPDIR/repo"
$ARC_BIN task new "Agent test" 2>&1 >/dev/null
WORKTREE2="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d | grep -v test-feature | head -1)"
cd "$WORKTREE2"

echo "agent work" > agent.rs
$ARC_BIN change "Agent change" --agent --model "claude-sonnet-4-5-20250929" 2>&1 >/dev/null

echo "more" >> agent.rs
$ARC_BIN checkpoint "Agent checkpoint" --model "claude-sonnet-4-5-20250929" 2>&1 >/dev/null

OUT=$($ARC_BIN log --all 2>&1)
assert_contains "agent model in log" "$OUT" "(agent: claude-sonnet-4-5-20250929)"

# ─── Test: task complete ──────────────────────────────────────────────

OUT=$($ARC_BIN task complete 2>&1)
assert_contains "complete cleans up" "$OUT" "Task completed"

cd "$TMPDIR/repo"
OUT=$($ARC_BIN task list 2>&1)
assert_contains "completed task in list" "$OUT" "(completed)"

# ─── Test: task abandon ──────────────────────────────────────────────

$ARC_BIN task new "Abandon test" 2>&1 >/dev/null
WORKTREE3="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d | grep -v test-feature | head -1)"
cd "$WORKTREE3"

echo "tmp" > tmp.rs
$ARC_BIN change "Will abandon" 2>&1 >/dev/null

OUT=$($ARC_BIN task abandon --reason "just testing" 2>&1)
assert_contains "abandon runs" "$OUT" "Task abandoned"

cd "$TMPDIR/repo"
OUT=$($ARC_BIN task list 2>&1)
assert_contains "abandoned task in list" "$OUT" "(abandoned)"

# ─── Test: task sync ─────────────────────────────────────────────────

$ARC_BIN task new "Sync test" 2>&1 >/dev/null
WORKTREE4="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d | grep -v test-feature | head -1)"
cd "$WORKTREE4"

echo "sync work" > sync.rs
$ARC_BIN change "Sync work" 2>&1 >/dev/null

# Simulate upstream change
cd "$TMPDIR/repo"
echo "upstream" >> README.md
git add -A && git commit -m "upstream work" --quiet
git push origin main --quiet 2>&1

cd "$WORKTREE4"
OUT=$($ARC_BIN task sync 2>&1)
assert_contains "sync fetches" "$OUT" "Fetching from origin"
assert_contains "sync rebases" "$OUT" "Sync complete"

# Verify upstream commit is in history
OUT=$(git log --oneline 2>&1)
assert_contains "sync includes upstream" "$OUT" "upstream work"

# ─── Test: arc change --amend ────────────────────────────────────────

cd "$TMPDIR/repo"
TASK_OUT=$($ARC_BIN task new "Amend test" 2>&1)
WORKTREE5="$(echo "$TASK_OUT" | grep "Worktree:" | sed 's/.*Worktree: *//' | sed 's|/$||')"
cd "$WORKTREE5"

echo "original" > amend.rs
OUT=$($ARC_BIN change "Original summary" --intent "Original intent" 2>&1)
assert_contains "amend: initial change created" "$OUT" "Change ["

# Capture the change id from the commit message trailers
AMEND_SHA_BEFORE=$(git rev-parse HEAD)

echo "updated" >> amend.rs
OUT=$($ARC_BIN change --amend "Amended summary" --intent "Updated intent" 2>&1)
assert_contains "amend: reports amended" "$OUT" "Amended ["
assert_contains "amend: shows old → new" "$OUT" "Original summary"
assert_contains "amend: shows new summary" "$OUT" "Amended summary"

# SHA should have changed (amend rewrites the commit)
AMEND_SHA_AFTER=$(git rev-parse HEAD)
if [ "$AMEND_SHA_BEFORE" != "$AMEND_SHA_AFTER" ]; then
    pass "amend: commit SHA changed"
else
    fail "amend: commit SHA changed" "SHA is still $AMEND_SHA_BEFORE"
fi

# The commit message should have the new summary
OUT=$(git log -1 --format=%s 2>&1)
assert_contains "amend: git log shows new summary" "$OUT" "Amended summary"

# The arc trailer should still be present
OUT=$(git log -1 --format=%B 2>&1)
assert_contains "amend: trailer preserved" "$OUT" "arc:change:id:"

# Only one task commit on this branch (above whatever is on main)
MERGE_BASE=$(git merge-base HEAD origin/main)
COMMIT_COUNT=$(git rev-list --count "$MERGE_BASE"..HEAD)
if [ "$COMMIT_COUNT" -eq 1 ]; then
    pass "amend: still 1 task commit"
else
    fail "amend: still 1 task commit" "got $COMMIT_COUNT"
fi

# arc log should show the updated summary
OUT=$($ARC_BIN log 2>&1)
assert_contains "amend: log shows new summary" "$OUT" "Amended summary"
assert_not_contains "amend: log hides old summary" "$OUT" "Original summary"

# ─── Test: arc push --force ─────────────────────────────────────────

# Push the task branch first (normal push to set up tracking)
git push -u origin HEAD --quiet 2>&1

# Amend again to create divergence from remote
echo "force update" >> amend.rs
$ARC_BIN change --amend "Force-amended summary" 2>&1 >/dev/null

# Normal push should fail (diverged)
set +e
OUT=$(git push 2>&1)
PUSH_EXIT=$?
set -e
if [ "$PUSH_EXIT" -ne 0 ]; then
    pass "push: normal push rejected after amend"
else
    fail "push: normal push rejected after amend" "push succeeded unexpectedly"
fi

# arc push --force should succeed
OUT=$($ARC_BIN push --force 2>&1)
assert_contains "push --force: succeeds" "$OUT" "Pushed."

# ─── Test: derived-from trailer (renamed from squashed-from) ────────

# Verify the format/parse roundtrip via commit message
cd "$TMPDIR/repo"
TASK_OUT=$($ARC_BIN task new "Derived-from test" 2>&1)
WORKTREE6="$(echo "$TASK_OUT" | grep "Worktree:" | sed 's/.*Worktree: *//' | sed 's|/$||')"
cd "$WORKTREE6"

echo "a" > a.rs
$ARC_BIN change "Change A" 2>&1 >/dev/null
echo "b" > b.rs
$ARC_BIN change "Change B" 2>&1 >/dev/null

# Manually write a commit with derived-from trailer to verify parsing
CHANGE_A_ID=$(git log --format=%B 2>&1 | grep "arc:change:id:" | tail -1 | sed 's/arc:change:id: //')
CHANGE_B_ID=$(git log --format=%B 2>&1 | grep "arc:change:id:" | head -1 | sed 's/arc:change:id: //')

# Create a commit message with arc:derived-from trailer and verify it parses
DERIVED_MSG="Squash merge of test

---
arc:change:id: test-derived-uuid
arc:derived-from: $CHANGE_A_ID, $CHANGE_B_ID
"
echo "merged" > merged.rs
git add -A
git commit -m "$DERIVED_MSG" --quiet 2>&1

OUT=$(git log -1 --format=%B 2>&1)
assert_contains "derived-from: trailer in commit" "$OUT" "arc:derived-from:"
assert_contains "derived-from: has change A" "$OUT" "$CHANGE_A_ID"
assert_contains "derived-from: has change B" "$OUT" "$CHANGE_B_ID"

# Also verify backward compat: a commit with old squashed-from trailer
OLD_MSG="Old squash merge

---
arc:change:id: test-old-uuid
arc:squashed-from: old-1, old-2
"
echo "old" > old.rs
git add -A
git commit -m "$OLD_MSG" --quiet 2>&1

OUT=$(git log -1 --format=%B 2>&1)
assert_contains "squashed-from compat: old trailer in commit" "$OUT" "arc:squashed-from:"

# ─── Test: task adopt --last N ────────────────────────────────────────

cd "$TMPDIR/repo"

# Make commits on main (not in a task)
echo "adopt1" > adopt1.rs
$ARC_BIN change "Adopt first" 2>&1 >/dev/null
echo "adopt2" > adopt2.rs
$ARC_BIN change "Adopt second" 2>&1 >/dev/null

MAIN_HEAD_BEFORE=$(git rev-parse HEAD)

OUT=$($ARC_BIN task adopt "Adopted feature" --last 2 2>&1)
assert_contains "adopt creates task" "$OUT" "Created task: Adopted feature"
assert_contains "adopt shows count" "$OUT" "Adopted:  2 commit(s)"
assert_contains "adopt shows branch" "$OUT" "Branch:"

# Main should have been reset (no longer at the same HEAD)
MAIN_HEAD_AFTER=$(git rev-parse HEAD)
if [ "$MAIN_HEAD_BEFORE" != "$MAIN_HEAD_AFTER" ]; then
    pass "adopt: main branch was reset"
else
    fail "adopt: main branch was reset" "HEAD unchanged"
fi

# The adopted files should NOT be on main anymore
if [ ! -f adopt1.rs ]; then
    pass "adopt: adopt1.rs removed from main"
else
    fail "adopt: adopt1.rs removed from main" "file still exists"
fi

# Find the worktree and verify commits are there with arc:task: trailer
ADOPT_WT="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d -name "adopted*" | head -1)"
if [ -n "$ADOPT_WT" ]; then
    pass "adopt: worktree created"
else
    fail "adopt: worktree created" "no worktree found"
fi

if [ -n "$ADOPT_WT" ]; then
    # Check files exist in worktree
    if [ -f "$ADOPT_WT/adopt1.rs" ] && [ -f "$ADOPT_WT/adopt2.rs" ]; then
        pass "adopt: files present in worktree"
    else
        fail "adopt: files present in worktree" "missing files"
    fi

    # Check commit messages have arc:task: trailer
    cd "$ADOPT_WT"
    OUT=$(git log --format=%B -2 2>&1)
    assert_contains "adopt: commits have arc:task trailer" "$OUT" "arc:task:"
    assert_contains "adopt: commits have change id" "$OUT" "arc:change:id:"

    cd "$TMPDIR/repo"
fi

# ─── Test: task adopt (default — all ahead of upstream) ──────────────

# Make more commits on main
echo "default1" > default1.rs
$ARC_BIN change "Default adopt first" 2>&1 >/dev/null
echo "default2" > default2.rs
$ARC_BIN change "Default adopt second" 2>&1 >/dev/null

# Push to set up upstream tracking
git push origin main --quiet 2>&1

# Make commits that are ahead of upstream
echo "ahead1" > ahead1.rs
$ARC_BIN change "Ahead first" 2>&1 >/dev/null
echo "ahead2" > ahead2.rs
$ARC_BIN change "Ahead second" 2>&1 >/dev/null
echo "ahead3" > ahead3.rs
$ARC_BIN change "Ahead third" 2>&1 >/dev/null

OUT=$($ARC_BIN task adopt "Default adopt test" 2>&1)
assert_contains "default adopt creates task" "$OUT" "Created task: Default adopt test"
assert_contains "default adopt count" "$OUT" "Adopted:  3 commit(s)"

# Main should be back at origin/main
MAIN_SHA=$(git rev-parse HEAD)
ORIGIN_SHA=$(git rev-parse origin/main)
if [ "$MAIN_SHA" = "$ORIGIN_SHA" ]; then
    pass "default adopt: main reset to origin/main"
else
    fail "default adopt: main reset to origin/main" "main=$MAIN_SHA origin=$ORIGIN_SHA"
fi

# ─── Test: task adopt moves dirty working state ──────────────────────

# Create a commit, then add untracked + staged + unstaged changes
echo "tracked" > tracked_adopt.rs
$ARC_BIN change "Tracked for dirty test" 2>&1 >/dev/null

# Untracked file
echo "untracked1" > untracked1.txt
mkdir -p subdir
echo "untracked2" > subdir/untracked2.txt

# Unstaged modification to a committed file
echo "modified content" >> tracked_adopt.rs

# Staged change (new file)
echo "staged" > staged_file.txt
git add staged_file.txt

OUT=$($ARC_BIN task adopt "Dirty state test" --last 1 2>&1)
assert_contains "adopt with dirty state: creates task" "$OUT" "Created task: Dirty state test"

# All dirty files should be gone from main
if [ ! -f untracked1.txt ] && [ ! -f subdir/untracked2.txt ] && [ ! -f staged_file.txt ]; then
    pass "adopt: dirty files removed from main"
else
    fail "adopt: dirty files removed from main" "files still exist"
fi

# All dirty files should be in the worktree
DIRTY_WT="$(find "$TMPDIR/repo/.arc/worktrees" -maxdepth 1 -mindepth 1 -type d -name "dirty*" | head -1)"
if [ -n "$DIRTY_WT" ]; then
    if [ -f "$DIRTY_WT/untracked1.txt" ] && [ -f "$DIRTY_WT/subdir/untracked2.txt" ]; then
        pass "adopt: untracked files present in worktree"
    else
        fail "adopt: untracked files present in worktree" "files missing"
    fi
    if [ -f "$DIRTY_WT/staged_file.txt" ]; then
        pass "adopt: staged file present in worktree"
    else
        fail "adopt: staged file present in worktree" "missing"
    fi
    # Unstaged modification: tracked_adopt.rs should have "modified content"
    if grep -q "modified content" "$DIRTY_WT/tracked_adopt.rs" 2>/dev/null; then
        pass "adopt: unstaged modification present in worktree"
    else
        fail "adopt: unstaged modification present in worktree" "modification missing"
    fi
else
    fail "adopt: dirty state worktree" "no worktree found"
    fail "adopt: staged file present in worktree" "no worktree found"
    fail "adopt: unstaged modification present in worktree" "no worktree found"
fi

# ─── Test: task adopt fails if no commits ────────────────────────────

set +e
OUT=$($ARC_BIN task adopt "No commits" --since HEAD 2>&1)
ADOPT_EXIT=$?
set -e
assert_exit_code "adopt: fails if no commits" 1 "$ADOPT_EXIT"
assert_contains "adopt: no commits message" "$OUT" "No commits to adopt"

# ─── Test: task help shows adopt ─────────────────────────────────────

OUT=$($ARC_BIN task --help 2>&1)
assert_contains "task help shows adopt" "$OUT" "adopt"

# ─── Results ──────────────────────────────────────────────────────────

echo ""
echo "Results"
echo "-------"
for t in "${TESTS[@]}"; do
    echo "$t"
done
echo ""
echo "Total: $((PASS + FAIL)) tests, $PASS passed, $FAIL failed"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
