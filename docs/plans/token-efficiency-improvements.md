# Plan: Improve Huff IR Token Efficiency

## TL;DR

Current Claude-tokenizer ratios are 1.23–1.58× (Huff vs emitted TS). The shortfall has two root causes: (1) BPE tokenizer penalty — Claude's tokenizer encodes familiar TS patterns more efficiently per-character than novel Huff syntax, and (2) measurement distortion from tiny examples + emitter verbosity inflating the denominator poorly. The plan attacks both via tokenizer-aware syntax adjustments, semantic density improvements, TS emitter diet, and expanded measurement corpus.

## Diagnosis

### The BPE Penalty (the core problem)
| Example | Huff chars/Claude-tok | TS chars/Claude-tok |
|---------|----------------------|---------------------|
| hello   | 1.40                 | 2.04                |
| counter | 1.89                 | 2.66                |
| greetings | 2.13               | 2.72                |
| async   | 1.65                 | 2.51                |

Claude's BPE vocabulary was trained on massive TS/JS corpora. Tokens like `function`, `export`, `console.log`, `const` are likely single BPE entries. Huff's novel syntax (`!io.writeln`, `op~`, `prog`) gets shredded into individual characters/small subwords. **Character compression ≠ token compression** when the tokenizer doesn't know your language.

### Measurement Distortion
- The `countTokens` API likely adds fixed framing overhead (~25-30 tokens), which crushes the ratio for the tiny 60-char `hello.huff` (explains why cl100k shows 1.60× but Claude shows only 1.23× for the same file).
- Emitter adds non-semantic TS lines: `// prog X`, `// entry point`, blank lines, redundant parens in expressions.
- The example corpus (4 supported files, 60-213 chars) is too small to draw conclusions.

---

## Steps

### Phase 1: Fix the Measurement (unblocks everything else)

1. **Write 3-5 larger examples** (200-500 lines each) that exercise the full v0 feature set: multi-op programs with state, errors, preconditions, pipelines, closures, types, modules with `use`. These amortize fixed overhead and reveal the true ratio.
   - `todo.huff` — CRUD-style stateful program (state, errors, preconditions, pipelines)
   - `transform.huff` — data transformation pipeline (pipelines, closures, types, modules)
   - `calculator.huff` — expression evaluator (binary ops, match-when-ready, error propagation)
   - Each paired with hand-written "idiomatic TS equivalent" as a second comparison point

2. **Add a "min-token TS" emit mode** to `huff-emit-ts` — strip all comments, minimize blank lines, drop redundant parens. This measures the *semantic* compression ratio without emitter noise.

3. **Measure raw text (no API overhead)** — add a `--raw` flag to `token-counts.ts` that subtracts the fixed overhead by measuring a known-length calibration string and subtracting. Or simply compare cl100k numbers which don't have this problem.

### Phase 2: Tokenizer-Aware Syntax Changes (breaking, high-impact)

4. **`!io.writeln()` → `!log()` / `!print()` / `!err()`** — Drop the `io.` namespace prefix. The `!` already marks the effect; `io.` is 3 extra tokens for zero semantic content. `log`, `print` are single BPE tokens in every major tokenizer.
   - Files affected: `packages/huff-parser/src/parser.rs`, `packages/huff-emit-ts/src/lib.rs` (map_effect_call), `skill/SKILL.md`, `skill/references/spec.md`, all examples

5. **`op` → `fn`** — `fn` is a guaranteed single BPE token (Rust training data). `op` *might* be one, but `fn` is safer and still conveys "function/operation" clearly. The `!`/`~` modifiers still distinguish effect and async semantics.
   - Alternative: keep `op` but measure whether it's actually multi-token with Claude. Only change if measured penalty exists.

6. **String interpolation: `"hi {name}"` instead of `"hi " + name`** — Concatenation via `+` requires 3+ tokens per join point (space, `+`, space). Interpolation with `{}` is 2 tokens (the braces) but eliminates the surrounding quote closings and openings. Net savings: 2-3 tokens per interpolation site. Most BPE tokenizers handle `{name}` efficiently because of template literal training data.
   - Parser change: recognize `{expr}` inside string literals
   - Emitter change: emit JS template literals `` `hi ${name}` ``

7. **Implicit prog/mod from structure** — If a file contains `op Main`, it's a `prog`. Otherwise it's a `mod`. Drop the `prog X` / `mod X` line entirely; derive the name from the filename.
   - This is 2-4 tokens per file — minor but principled (the declaration carries zero semantic content the filename doesn't already provide).
   - Alternative: make it optional (allow explicit but don't require).

8. **Implicit types where inferable** — `state count: u32 = 0` → `state count = 0`. The type is inferable from the literal. Same for `let x: str = "hello"`. Only require annotations when ambiguous (function signatures, uninitialized state).
   - Parser change: make type annotations optional on `state` and `let` when initializer present
   - Emitter: infer TS types from literals

### Phase 3: Semantic Density (higher compression per line)

9. **Collapse common state-mutation patterns** — `!count += 1` is already good, but allow `!count++` as sugar (saves 2 tokens: space + `1`). Common in training data so tokenizes well.

10. **Pipeline-as-expression in more positions** — Allow pipelines as the body of an op without a `let` binding: `items->filter(i => i.active)->map(i => i.name)` as a direct return value. This is likely already supported but should be emphasized in examples.

11. **Multi-precondition sugar** — Instead of repeating `pre ... : Err(...)` N times:
    ```
    pre : ValidationFailed
      name.len > 0 : "name required"
      name.len <= 255 : "name too long"
    ```
    Saves the repeated error type name (1-2 tokens per condition after the first).

### Phase 4: Emitter Diet (improves ratio from denominator side)

12. **Strip comment lines** — Remove `// prog X` and `// entry point` from output. They exist for human readability but inflate token count.

13. **Remove redundant parentheses** — The emitter wraps all binary exprs and await in parens: `("hello " + name)`, `(await Greet("world"))`. Only emit parens when precedence requires them.

14. **Compact error classes** — Current: `export class Empty extends Error { constructor() { super("Empty"); this.name = "Empty"; } }`. Consider emitting a helper pattern or more compact form.

15. **Minimize blank lines** — Currently emits a blank line between every item and after the header. Reduce to single blank line between functions only.

---

## Relevant Files

- `docs/token-counts.csv` — current measurements, will be regenerated
- `packages/huff-tools/src/token-counts.ts` — measurement script (add overhead calibration)
- `packages/huff-parser/src/parser.rs` — syntax changes (string interpolation, optional types, `fn` keyword)
- `packages/huff-parser/src/lexer.rs` — new tokens if needed (interpolation, `fn`)
- `packages/huff-parser/src/token.rs` — token kind additions
- `packages/huff-emit-ts/src/lib.rs` — emitter diet (parens, comments, blank lines, template literals)
- `packages/huff-ast/src/lib.rs` — AST changes for interpolation, optional type annotations
- `skill/references/examples/` — new + updated examples
- `skill/SKILL.md` — update syntax reference (io.writeln → log, op → fn, interpolation)
- `skill/references/spec.md` — formal grammar updates
- `README.md` — update measured ratios section

## Verification

1. `cargo test --workspace` — all existing parser/emitter tests pass after changes
2. `npm --prefix packages/huff-tools test` — validation suite passes (snapshots will need updating)
3. `npm --prefix packages/huff-tools run token-counts` — regenerate CSV, verify improved ratios
4. End-to-end: emitted TS from new larger examples executes correctly under `node`
5. Compare before/after ratios across all examples (old and new)
6. Verify cl100k ratio > 2.0× on the larger examples (amortized overhead)
7. Verify Claude ratio > 1.7× on larger examples

## Decisions

- Breaking changes OK — pre-1.0, optimize aggressively
- `op` → `fn` change contingent on measuring whether `op` is actually multi-token (step 5 note)
- `prog`/`mod` made optional rather than removed (backward compat for files that want to be explicit)
- Keep `!` effect marker even for known-effectful ops (core design principle stays)
- String interpolation uses `{}` not `${}` — shorter, and the parser already knows it's inside a string

## Further Considerations

1. **Custom BPE vocabulary training**: Long-term, if Huff gains adoption, fine-tuned models could have Huff tokens in their vocabulary. But that's a post-adoption problem — for now, design for existing tokenizers.
2. **Tab vs. spaces**: A single tab character is 1 BPE token regardless of visual width. Switching from 2-space indent to tab would save 1 token per indented line. Tradeoff: less familiar to LLMs trained on space-indented code.
3. **Measuring what matters**: The *semantic information density* (unique meaning per token) may be a better metric than raw compression ratio. A token carrying "this is an async function that returns string and can throw NotFound" is more valuable than a token carrying "space space".
