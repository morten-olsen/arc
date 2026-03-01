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
