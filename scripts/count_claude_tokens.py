#!/usr/bin/env -S uv run --with boto3 python3
"""Count Claude tokens for the v0-subset Huff examples and their emitted TS,
then merge those columns into docs/token-counts.csv.

Two ways to get Claude's tokenizer count:

1. Anthropic API: POST /v1/messages/count_tokens with x-api-key.
2. AWS Bedrock: bedrock-runtime.count_tokens — same numbers, IAM auth.

This script uses Bedrock so it works with SSO credentials (no console.anthropic.com
key needed). The Rust `huff-token-csv` binary writes the cl100k columns and leaves
claude_* blank if no API key; this script fills the gaps from a different auth path.

Run after `cargo run -p huff-tests --bin huff-token-csv` so the cl100k columns are
already populated.
"""

from __future__ import annotations

import csv
import json
import subprocess
import sys
from pathlib import Path

import boto3

REPO_ROOT = Path(__file__).resolve().parent.parent
EXAMPLES_DIR = REPO_ROOT / "skill" / "references" / "examples"
CSV_PATH = REPO_ROOT / "docs" / "token-counts.csv"
MODEL_ID = "anthropic.claude-sonnet-4-6"
REGION = "us-east-1"


def emit_ts(huff_path: Path) -> str:
    """Run the huff CLI to emit TS to stdout-ish, then read the file it wrote."""
    out_path = huff_path.with_suffix(".ts")
    subprocess.run(
        ["cargo", "run", "-q", "-p", "huff-cli", "--", str(huff_path)],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
    )
    ts = out_path.read_text()
    out_path.unlink()
    return ts


def claude_count(client, text: str) -> int:
    body = {
        "anthropic_version": "bedrock-2023-05-31",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": text}],
    }
    resp = client.count_tokens(
        modelId=MODEL_ID,
        input={"invokeModel": {"body": json.dumps(body)}},
    )
    return resp["inputTokens"]


def main() -> int:
    if not CSV_PATH.exists():
        print(
            f"error: {CSV_PATH} not found. Run `cargo run -p huff-tests --bin huff-token-csv` first.",
            file=sys.stderr,
        )
        return 1

    client = boto3.client("bedrock-runtime", region_name=REGION)

    rows = list(csv.DictReader(CSV_PATH.open()))
    for row in rows:
        name = row["example"]
        huff_path = EXAMPLES_DIR / name
        if not huff_path.exists():
            continue
        huff_src = huff_path.read_text()
        ts_src = emit_ts(huff_path)
        h = claude_count(client, huff_src)
        t = claude_count(client, ts_src)
        ratio = t / h if h else 0.0
        row["claude_huff_tokens"] = str(h)
        row["claude_ts_tokens"] = str(t)
        row["claude_ratio"] = f"{ratio:.2f}"
        print(f"{name}: huff={h} ts={t} ratio={ratio:.2f}")

    fieldnames = list(rows[0].keys())
    with CSV_PATH.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {CSV_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
