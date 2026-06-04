import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

export const REPO_ROOT = resolve(import.meta.dirname, "../../..");
export const EXAMPLES_DIR = join(REPO_ROOT, "skill/references/examples");

export interface Example {
  name: string;
  path: string;
  source: string;
  unsupported: boolean;
}

export function listExamples(): Example[] {
  const entries = readdirSync(EXAMPLES_DIR)
    .filter((n) => n.endsWith(".huff"))
    .sort();
  return entries.map((name) => {
    const path = join(EXAMPLES_DIR, name);
    return {
      name,
      path,
      source: readFileSync(path, "utf8"),
      unsupported: name.includes("unsupported"),
    };
  });
}

export interface HuffcResult {
  ok: boolean;
  ts?: string;
  stderr: string;
}

/**
 * Invoke the Rust `huffc` binary on a file, returning emitted TS to stdout.
 * Uses `--stdout` so we never touch the filesystem alongside the source.
 */
export function runHuffc(huffPath: string): HuffcResult {
  const r = spawnSync(
    "cargo",
    ["run", "-q", "-p", "huff-cli", "--", "--stdout", huffPath],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  if (r.status === 0) {
    return { ok: true, ts: r.stdout, stderr: r.stderr };
  }
  return { ok: false, stderr: r.stderr };
}
