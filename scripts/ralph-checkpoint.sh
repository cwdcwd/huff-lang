#!/usr/bin/env bash
#
# ralph-checkpoint.sh — a Ralph Wiggum loop that commits after every iteration.
#
# Same brute-force loop as ralph.sh, but each iteration's changes are committed
# to a dedicated branch as a checkpoint. If an iteration makes things worse, you
# can `git revert <sha>` or `git reset --hard <sha>` back to any prior pass —
# nothing is lost, every attempt is on the timeline.
#
# Usage:
#   ./scripts/ralph-checkpoint.sh [PROMPT_FILE]
#
# Environment overrides (in addition to ralph.sh's):
#   PROMPT_FILE   prompt fed each iteration    (default: PROMPT.md)
#   SENTINEL      stop when output contains it  (default: RALPH_DONE)
#   MAX_ITERS     safety cap on iterations      (default: 50)
#   SLEEP_SECS    pause between iterations       (default: 2)
#   LOG_DIR       per-iteration logs land here  (default: .ralph)
#   CLAUDE_ARGS   extra flags passed to claude  (default: --dangerously-skip-permissions)
#   BRANCH        checkpoint branch to commit to (default: ralph/<timestamp>)
#
set -euo pipefail

PROMPT_FILE="${1:-${PROMPT_FILE:-PROMPT.md}}"
SENTINEL="${SENTINEL:-RALPH_DONE}"
MAX_ITERS="${MAX_ITERS:-50}"
SLEEP_SECS="${SLEEP_SECS:-2}"
LOG_DIR="${LOG_DIR:-.ralph}"
CLAUDE_ARGS="${CLAUDE_ARGS:---dangerously-skip-permissions}"

if ! command -v claude >/dev/null 2>&1; then
  echo "ralph: 'claude' CLI not found on PATH." >&2
  exit 127
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ralph: not inside a git repo — checkpoint variant needs one." >&2
  exit 1
fi

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "ralph: prompt file '$PROMPT_FILE' not found." >&2
  exit 1
fi

# Refuse to run on a dirty tree: checkpoints are only meaningful if every commit
# on the branch is the loop's own work, not a mix with your uncommitted changes.
if [[ -n "$(git status --porcelain)" ]]; then
  echo "ralph: working tree is dirty. Commit or stash first so checkpoints stay clean." >&2
  git status --short >&2
  exit 1
fi

# Branch off wherever we are now, so the original branch is never touched.
START_REF="$(git rev-parse --abbrev-ref HEAD)"
BRANCH="${BRANCH:-ralph/$(date +%Y%m%d-%H%M%S)}"
git checkout -b "$BRANCH"

mkdir -p "$LOG_DIR"

trap 'echo; echo "ralph: interrupted at iteration ${i:-0}. Checkpoints are on branch '\''$BRANCH'\''."; exit 130' INT

echo "ralph: looping on '$PROMPT_FILE'"
echo "ralph: checkpoint branch='$BRANCH' (forked from '$START_REF')"
echo "ralph: stop sentinel='$SENTINEL'  max_iters=$MAX_ITERS  logs=$LOG_DIR/"
echo

# Commit whatever the iteration produced. Returns 0 if a commit was made,
# 1 if there was nothing to commit (so we can note a no-op iteration).
checkpoint() {
  local n="$1"
  # Don't sweep logs/snapshots-from-the-loop into the checkpoint noise.
  git add -A -- ':!'"$LOG_DIR"
  if git diff --cached --quiet; then
    return 1
  fi
  git commit -q -m "ralph: checkpoint iteration $n" \
    -m "Automated checkpoint from ralph-checkpoint.sh. Prompt: $PROMPT_FILE"
  return 0
}

for ((i = 1; i <= MAX_ITERS; i++)); do
  ts="$(date +%Y%m%d-%H%M%S)"
  log="$LOG_DIR/iter-$(printf '%03d' "$i")-$ts.log"

  echo "=== ralph iteration $i/$MAX_ITERS ($ts) ==="

  claude -p $CLAUDE_ARGS < "$PROMPT_FILE" 2>&1 | tee "$log" || true

  if checkpoint "$i"; then
    echo "ralph: committed checkpoint $(git rev-parse --short HEAD)"
  else
    echo "ralph: iteration $i made no file changes — nothing to checkpoint."
  fi

  if grep -q -- "$SENTINEL" "$log"; then
    echo
    echo "ralph: sentinel '$SENTINEL' found in iteration $i — work reported done."
    echo "ralph: checkpoints on branch '$BRANCH'. Review with: git log --oneline $START_REF..$BRANCH"
    exit 0
  fi

  echo "ralph: no sentinel yet, going around again in ${SLEEP_SECS}s..."
  sleep "$SLEEP_SECS"
done

echo
echo "ralph: hit MAX_ITERS=$MAX_ITERS without seeing '$SENTINEL'. Stopping."
echo "ralph: checkpoints on branch '$BRANCH'. Review with: git log --oneline $START_REF..$BRANCH"
exit 1
