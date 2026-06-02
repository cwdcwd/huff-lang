#!/usr/bin/env bash
#
# ralph.sh — a "Ralph Wiggum" loop.
#
# Named after the meme of brute-forcing an agent: feed Claude the same prompt
# over and over in a loop until it declares the work finished. No clever
# orchestration, just persistence. "Me fail English? That's unpossible."
#
# Each iteration runs `claude -p` (headless / print mode) against PROMPT_FILE.
# The loop stops when Claude's output contains the sentinel string, hits the
# safety cap on iterations, or you Ctrl-C it.
#
# Usage:
#   ./scripts/ralph.sh [PROMPT_FILE]
#
# Environment overrides:
#   PROMPT_FILE   prompt fed each iteration   (default: PROMPT.md)
#   SENTINEL      stop when output contains it (default: RALPH_DONE)
#   MAX_ITERS     safety cap on iterations     (default: 50)
#   SLEEP_SECS    pause between iterations      (default: 2)
#   LOG_DIR       per-iteration logs land here (default: .ralph)
#   CLAUDE_ARGS   extra flags passed to claude (default: --dangerously-skip-permissions)
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

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "ralph: prompt file '$PROMPT_FILE' not found." >&2
  echo "       Create it with the task you want Claude to grind on, then re-run." >&2
  exit 1
fi

mkdir -p "$LOG_DIR"

# Stop the loop cleanly on Ctrl-C instead of dumping a stack of errors.
trap 'echo; echo "ralph: interrupted at iteration ${i:-0}. Bye."; exit 130' INT

echo "ralph: looping on '$PROMPT_FILE'"
echo "ralph: stop sentinel='$SENTINEL'  max_iters=$MAX_ITERS  logs=$LOG_DIR/"
echo

for ((i = 1; i <= MAX_ITERS; i++)); do
  ts="$(date +%Y%m%d-%H%M%S)"
  log="$LOG_DIR/iter-$(printf '%03d' "$i")-$ts.log"

  echo "=== ralph iteration $i/$MAX_ITERS ($ts) ==="

  # Feed the prompt on stdin; tee output to both the console and a log file.
  # `|| true` keeps the loop alive even if a single iteration exits nonzero —
  # Ralph doesn't give up just because one try failed.
  claude -p $CLAUDE_ARGS < "$PROMPT_FILE" 2>&1 | tee "$log" || true

  if grep -q -- "$SENTINEL" "$log"; then
    echo
    echo "ralph: sentinel '$SENTINEL' found in iteration $i — work reported done."
    echo "ralph: logs in $LOG_DIR/"
    exit 0
  fi

  echo "ralph: no sentinel yet, going around again in ${SLEEP_SECS}s..."
  sleep "$SLEEP_SECS"
done

echo
echo "ralph: hit MAX_ITERS=$MAX_ITERS without seeing '$SENTINEL'. Stopping."
echo "ralph: logs in $LOG_DIR/"
exit 1
