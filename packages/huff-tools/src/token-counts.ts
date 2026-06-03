/**
 * Generate `docs/token-counts.csv` measuring token counts of each Huff
 * example against its emitted TypeScript.
 *
 * Two tokenizers are reported:
 *
 * 1. cl100k — GPT-4-era BPE via the `tiktoken` npm package. Always run.
 *    Free, offline, deterministic. A reasonable stand-in for "what an LLM
 *    sees" but not actually Claude's tokenizer.
 *
 * 2. Claude — Anthropic's `messages.countTokens` endpoint, the real Claude
 *    tokenizer. Reachable via:
 *      - ANTHROPIC_API_KEY (direct API), or
 *      - AWS Bedrock CountTokens (works with SSO creds; default).
 *    Skipped silently when neither path resolves; cl100k columns still get
 *    populated, claude_* columns blank.
 *
 * Usage:
 *   npm run token-counts
 *
 * Override the Claude model with HUFF_CLAUDE_MODEL (default: claude-sonnet-4-6).
 */

import { writeFileSync } from "node:fs";
import { resolve, join } from "node:path";

import { get_encoding } from "tiktoken";
import Anthropic from "@anthropic-ai/sdk";
import {
  BedrockRuntimeClient,
  CountTokensCommand,
} from "@aws-sdk/client-bedrock-runtime";

import { listExamples, runHuffc, REPO_ROOT } from "./huffc.ts";

const CSV_PATH = resolve(REPO_ROOT, "docs/token-counts.csv");
const ANTHROPIC_MODEL = process.env.HUFF_CLAUDE_MODEL ?? "claude-sonnet-4-6";
const BEDROCK_MODEL = `anthropic.${ANTHROPIC_MODEL}`;
const REGION = process.env.AWS_REGION ?? "us-east-1";

type ClaudeCounter = (text: string) => Promise<number>;

async function makeClaudeCounter(): Promise<ClaudeCounter | null> {
  if (process.env.ANTHROPIC_API_KEY) {
    console.error(`claude tokenizer: Anthropic API (model=${ANTHROPIC_MODEL})`);
    const client = new Anthropic();
    return async (text) => {
      const r = await client.messages.countTokens({
        model: ANTHROPIC_MODEL,
        messages: [{ role: "user", content: text }],
      });
      return r.input_tokens;
    };
  }

  // Try Bedrock — succeeds silently if AWS creds resolve, otherwise null.
  try {
    const client = new BedrockRuntimeClient({ region: REGION });
    // probe with a tiny call so we fail fast on bad creds rather than mid-loop
    await client.send(
      new CountTokensCommand({
        modelId: BEDROCK_MODEL,
        input: {
          invokeModel: {
            body: new TextEncoder().encode(
              JSON.stringify({
                anthropic_version: "bedrock-2023-05-31",
                max_tokens: 16,
                messages: [{ role: "user", content: "hi" }],
              }),
            ),
          },
        },
      }),
    );
    console.error(`claude tokenizer: AWS Bedrock (model=${BEDROCK_MODEL}, region=${REGION})`);
    return async (text) => {
      const r = await client.send(
        new CountTokensCommand({
          modelId: BEDROCK_MODEL,
          input: {
            invokeModel: {
              body: new TextEncoder().encode(
                JSON.stringify({
                  anthropic_version: "bedrock-2023-05-31",
                  max_tokens: 1024,
                  messages: [{ role: "user", content: text }],
                }),
              ),
            },
          },
        }),
      );
      if (r.inputTokens == null) {
        throw new Error("Bedrock CountTokens returned no inputTokens field");
      }
      return r.inputTokens;
    };
  } catch (e) {
    console.error(
      `claude tokenizer: unavailable (no ANTHROPIC_API_KEY, Bedrock failed: ${(e as Error).message})`,
    );
    return null;
  }
}

async function main() {
  const enc = get_encoding("cl100k_base");
  const claude = await makeClaudeCounter();

  const header = [
    "example",
    "huff_chars",
    "ts_chars",
    "cl100k_huff_tokens",
    "cl100k_ts_tokens",
    "cl100k_ratio",
    "claude_huff_tokens",
    "claude_ts_tokens",
    "claude_ratio",
  ].join(",");

  const lines = [header];

  for (const ex of listExamples()) {
    if (ex.unsupported) continue;
    const r = runHuffc(ex.path);
    if (!r.ok) {
      console.error(`skipping ${ex.name}: ${r.stderr.trim()}`);
      continue;
    }
    const ts = r.ts!;

    const clHuff = enc.encode(ex.source).length;
    const clTs = enc.encode(ts).length;
    const clRatio = clHuff === 0 ? 0 : clTs / clHuff;

    let clHuffTok = "";
    let clTsTok = "";
    let clRatioStr = "";
    if (claude) {
      try {
        const h = await claude(ex.source);
        const t = await claude(ts);
        clHuffTok = String(h);
        clTsTok = String(t);
        clRatioStr = h === 0 ? "0.00" : (t / h).toFixed(2);
        console.error(`${ex.name}: claude huff=${h} ts=${t} ratio=${clRatioStr}`);
      } catch (e) {
        console.error(`${ex.name}: claude count failed: ${(e as Error).message}`);
      }
    }

    lines.push(
      [
        ex.name,
        [...ex.source].length,
        [...ts].length,
        clHuff,
        clTs,
        clRatio.toFixed(2),
        clHuffTok,
        clTsTok,
        clRatioStr,
      ].join(","),
    );
  }

  enc.free();
  const csv = lines.join("\n") + "\n";
  writeFileSync(CSV_PATH, csv);
  console.error(`wrote ${CSV_PATH}`);
  process.stdout.write(csv);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
