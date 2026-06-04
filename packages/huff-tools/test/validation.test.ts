import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it, expect } from "vitest";

import { listExamples, runHuffc, EXAMPLES_DIR } from "../src/huffc.ts";

const HERE = import.meta.dirname;

const examples = listExamples();
const supported = examples.filter((e) => !e.unsupported);
const unsupported = examples.filter((e) => e.unsupported);

describe("supported v0-subset examples", () => {
  it.each(supported)("$name transpiles", async ({ name, path }) => {
    const r = runHuffc(path);
    expect(r.ok, r.stderr).toBe(true);
    expect(r.ts).toBeTruthy();
    await expect(r.ts).toMatchFileSnapshot(
      join(HERE, "snapshots", `${name}.ts.snap`),
    );
  });
});

describe("out-of-subset examples error cleanly", () => {
  it.each(unsupported)("$name fails with not-yet-supported", ({ path }) => {
    const r = runHuffc(path);
    expect(r.ok).toBe(false);
    expect(r.stderr).toMatch(/not yet supported:/);
  });
});

describe("Definition of Done — hello.huff runs end-to-end", () => {
  it("emits TS that node executes and prints 'Hello World'", () => {
    const path = join(EXAMPLES_DIR, "hello.huff");
    const r = runHuffc(path);
    expect(r.ok, r.stderr).toBe(true);

    const dir = mkdtempSync(join(tmpdir(), "huff-hello-"));
    try {
      const tsPath = join(dir, "hello.ts");
      writeFileSync(tsPath, r.ts!);
      const out = execFileSync("node", [tsPath], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      });
      expect(out.trim()).toBe("Hello World");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("async.huff runs end-to-end and prints 'hello world'", () => {
    const path = join(EXAMPLES_DIR, "async.huff");
    const r = runHuffc(path);
    expect(r.ok, r.stderr).toBe(true);

    const dir = mkdtempSync(join(tmpdir(), "huff-async-"));
    try {
      const tsPath = join(dir, "async.ts");
      writeFileSync(tsPath, r.ts!);
      const out = execFileSync("node", [tsPath], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      });
      expect(out.trim()).toBe("hello world");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("AST-level invariants surfaced via huffc", () => {
  it("svc_unsupported.huff names svc in the error", () => {
    const r = runHuffc(join(EXAMPLES_DIR, "svc_unsupported.huff"));
    expect(r.ok).toBe(false);
    expect(r.stderr).toMatch(/not yet supported: svc/);
  });

  it("match_unsupported.huff names match in the error", () => {
    const r = runHuffc(join(EXAMPLES_DIR, "match_unsupported.huff"));
    expect(r.ok).toBe(false);
    expect(r.stderr).toMatch(/not yet supported: match/);
  });
});
