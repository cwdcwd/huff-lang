# Huff TS Transpiler — Walking Skeleton Plan

## Goal

Stand up a Rust-implemented transpiler that consumes a *subset* of Huff v0.1 and emits idiomatic TypeScript. Prove the lex → parse → AST → emit pipeline end-to-end on at least the two Hello World examples and a stripped-down FileService. Defer async, generics, auth, shared ownership, sum types, and match expressions.

## Repo layout

```
huff-lang/
├── README.md
├── skill/                          (existing — untouched)
└── packages/
    ├── Cargo.toml                  (workspace manifest)
    ├── huff-ast/                   (AST node types, shared by parser + all emitters)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── huff-parser/                (lexer + parser: source → AST)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── lexer.rs            (raw tokens + indentation pass)
    │       ├── token.rs            (token kinds)
    │       └── parser.rs           (recursive descent → AST)
    ├── huff-emit-ts/               (AST → TypeScript source)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── huff-cli/                   (binary: huffc input.huff → input.ts)
        ├── Cargo.toml
        └── src/main.rs
```

`packages/` (not `crates/`) matches the monorepo framing and leaves room for non-Rust packages later (e.g. a JS wrapper, a vscode extension).

## Subset for v0 (the walking skeleton)

**In:**
- `prog Name` and `mod Name` top-level forms (no `svc`)
- `use ModuleName`
- `err Name` and `err Name(field: Type, …)`
- `type Name` (product types) and `type Name = OtherType` (aliases)
- `state x: T = v` (single) and `state` block
- `op Name(p: T, …) ReturnType` — sync only
- Primitives: `str`, `bool`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bytes`
- Composites: `[]T`, `T?` (no `map`, no tuples, no generics)
- Error return `T!E`, `T!(E1 | E2)`, propagation `call()!`
- `pre cond : Err`
- `let x = expr` bindings
- Effects: `!io.writeln`, `!io.write`, `!io.err`, `!stateField = expr`, `!stateField += expr`
- Expressions: int/str/bool literals, names, calls, member access `x.field`, `.len`, binary arith/compare/logical, string `+`
- Pipelines: `->map`, `->filter`, `->each` with single-arg closures `name => expr`
- Implicit return (last expression)

**Out (deferred to v1+):**
- `svc` and `auth`
- `op~` and `~call`, `par`, `race`, `timeout`
- Generics (`<T: Constraint>`)
- `shared<T>`, `shared<mut T>`
- Ownership intent tokens (`&T`, `+T`)
- Match expressions (`expr?` with arms)
- Sum types (`type X = A | B`)
- `map<K, V>`, tuples
- Multi-line closures
- Optional chaining `?->` and `??`

## Phases

### Phase 1 — AST crate (no dependencies)

Define every node type the v0 subset needs, in `huff-ast`. Shape is unsurprising recursive enums:

- `File { kind: ProgKind, name, items: Vec<Item> }`
- `Item::Use | Err | Type | State | Op`
- `Op { name, params, return_type: Option<Type>, error_type: Option<ErrorType>, body: Vec<Stmt> }`
- `Stmt::Let | Effect | Pre | Expr`
- `Expr::Lit | Name | Call | Member | Binary | Unary | Pipeline | Closure | Propagate`
- `Type::Prim | Named | List | Optional | Alias`

Spans on every node (`Span { start: usize, end: usize }`) — the emitter doesn't need them but error reporting does, and adding them later is painful.

Deliverable: `cargo build -p huff-ast` succeeds; no logic to test yet.

### Phase 2 — Lexer (the indentation problem)

Two-pass design, because indentation-as-syntax is the single hardest mechanical thing in this language:

1. **Raw lexer** — produces a flat stream including `Newline` and `Whitespace` tokens. Hand-rolled (`logos` crate is fine for the raw pass; it doesn't help with indentation).
2. **Layout pass** — walks the raw stream, tracks an indent stack, emits synthetic `Indent` / `Dedent` / `Newline` tokens, drops blank-line whitespace. Same algorithm Python uses.

Token kinds: keywords (`prog`, `mod`, `svc`, `use`, `err`, `type`, `state`, `auth`, `op`, `let`, `pre`), punctuation (`:`, `=`, `(`, `)`, `[`, `]`, `,`, `.`, `?`, `!`, `~`, `&`, `+`, `->`, `=>`, `|`), operators (`==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `+=`, `-=`), literals (int, float, str, bool), `Ident`, `Indent`, `Dedent`, `Newline`, `Eof`.

Test harness: golden token streams for ~10 hand-written snippets covering all indentation edge cases (nested ops, blank lines inside indented blocks, trailing newline, no trailing newline).

Deliverable: `cargo test -p huff-parser lexer` passes.

### Phase 3 — Parser

Recursive descent. No parser generator — the grammar is small enough, and the overloaded `!`/`~`/`->`/`?` tokens are easier to handle by hand with explicit context.

Disambiguation rules to encode:
- `!` at statement start → effect; after a type → error return marker; after a call expression → propagation
- `~` after `op` keyword → async marker (rejected in v0 with "async not yet supported"); before a call → await (rejected in v0)
- `->` at expr position → pipeline; in type position → function type (not in v0); after pattern → match arm (not in v0)
- `?` after type → optional type; after expr followed by Indent → match expression (not in v0); after expr followed by anything else → error in v0

Pratt parser for binary expressions with the precedence table from §5 of the spec.

Test harness: parse every code block in `examples.md` that fits the v0 subset (Hello World minimal, Hello World full minus async/match, FileService minus auth/async). Snapshot the AST with `insta`.

Deliverable: every v0-subset example parses to a stable AST snapshot.

### Phase 4 — TS emitter

Walks the AST and emits TypeScript source. Design choice that needs to be made up front: **how do we represent Huff state?**

Options:
- **`prog` → top-level `let` declarations + an exported `Main()` function.** Simple, matches Huff semantics most directly. Recommended for v0.
- **`prog` → a class with state as fields.** Cleaner for `svc` later but overkill for the walking skeleton.
- **`prog` → a closure that captures state.** Avoids globals but obscures the mapping.

Go with option 1 for `prog`/`mod`. Option 2 will be the right call for `svc` when it lands.

Mappings:
| Huff | TypeScript |
|---|---|
| `prog Name` | `// prog Name` header + module-level decls |
| `mod Name` | `export namespace Name { ... }` |
| `use Other` | `import * as Other from './Other'` |
| `type Foo` (product) | `export type Foo = { ... }` |
| `type X = Y` | `export type X = Y` |
| `err Bar(msg: str)` | `export class Bar extends Error { constructor(public msg: string) { super(msg) } }` |
| `state x: T = v` | `let x: T = v` (module-scoped) |
| `op F(p: T) R` | `export function F(p: T): R { ... }` |
| `op F(...) R!E` | `export function F(...): R { ... }` (errors thrown, not Result-typed in v0; flag in plan as a known shortcut to revisit) |
| `pre cond : Err` | `if (!(cond)) throw new Err(...)` |
| `let x = e` | `const x = e` |
| `!io.writeln(s)` | `console.log(s)` |
| `!stateField = e` | `stateField = e` |
| `!stateField += e` | `stateField += e` |
| `xs->map(f)` | `xs.map(f)` |
| `xs->filter(p)` | `xs.filter(p)` |
| `xs->each(f)` | `xs.forEach(f)` |
| `name.len` | `name.length` |
| `call()!` | `call()` (exceptions propagate naturally; revisit with Result types) |
| primitives | `str→string`, `bool→boolean`, all int/float→`number`, `bytes→Uint8Array` |

**Known shortcut to call out in the plan:** v0 emits errors as thrown exceptions, not `Result<T, E>`. This violates the "errors are values" design principle but keeps the walking skeleton small. Phase 6 introduces a proper `Result` codegen mode behind a flag, then defaults to it.

Deliverable: snapshot tests of generated TS for each v0-subset example. Bonus: pipe the emitted TS through `tsc --noEmit` in CI to verify it actually type-checks.

### Phase 5 — CLI

`huff-cli/src/main.rs`: `huffc input.huff [-o output.ts]`. Reads file, runs lex → parse → emit, writes output. Surfaces parse errors with span-based messages (`error at line 12, col 4: expected ':' after parameter name`).

Deliverable: `cargo run -p huff-cli -- examples/hello.huff` writes a file that runs under `tsx` or `node --loader tsx`.

### Phase 6 — Validation harness

A small test binary (`packages/huff-tests/`) that:
1. Reads each example from `skill/references/examples.md`.
2. Runs it through the transpiler.
3. For v0-subset examples, asserts emission succeeds and the output matches a snapshot.
4. For out-of-subset examples, asserts emission fails with a specific "not yet supported" error so we track coverage growth.

This is also where the **token-efficiency claim** finally gets measured: count tokens (BPE, via `tiktoken-rs`) on the Huff source vs the emitted TS, and emit a CSV. The 4× claim in the README has been an unsupported assertion until now.

## Open questions to settle before Phase 4

These don't block Phases 1–3 but need answers before the emitter is finished. Worth pinning down up front so they don't get re-litigated mid-implementation:

1. **Errors as exceptions or as `Result<T, E>`?** v0 plan above says exceptions for simplicity, but the design principle says values. Pick one before Phase 4 starts.
2. **Module model — namespaces or ES modules?** `export namespace Foo` is a single-file fit; ES modules require multi-file emission and a path resolver. v0 plan uses namespaces; ES modules can come with `svc`.
3. **Where does `state` live for a `prog`?** Module-level `let` works but pollutes the global scope. A wrapping IIFE is cleaner but harder to debug. v0 plan picks module-level; revisit if it bites.
4. **Numeric type fidelity.** Huff has `i32`/`u32`/`i64`/`u64`/`f32`/`f64`. TS has only `number` and `bigint`. v0 plan collapses everything to `number`; this loses range-checking but matches what hand-written TS does. A future strict mode could emit `bigint` for 64-bit and `Brand<number, 'u32'>` for narrow ints.

## Crate dependency choices

Keep these explicit so the plan isn't open-ended:

- `logos` for the raw lexer pass (fast, derive-based)
- `insta` for snapshot tests
- `clap` for the CLI
- `miette` for error reporting with spans (downstream-friendly, prettier than `anyhow`)
- `tiktoken-rs` for the token-counting harness in Phase 6

No parser generator — recursive descent. No serde unless an emitter needs it.

## Order of execution

1. Phase 1 (AST) — half a day. Mostly typing.
2. Phase 2 (lexer + indentation) — one to two days. The hardest mechanical phase.
3. Phase 3 (parser) — two to three days. Most surprises live here.
4. Phase 4 (TS emitter) — one to two days. Mostly mechanical once the AST is right.
5. Phase 5 (CLI) — half a day.
6. Phase 6 (validation harness) — one day.

Total walking-skeleton estimate: ~7 working days for someone fluent in Rust.

## Definition of done

- `cargo test --workspace` passes.
- `cargo run -p huff-cli -- skill/references/examples/hello.huff` produces TypeScript that runs and prints "Hello World".
- A snapshot exists for each v0-subset example.
- Out-of-subset examples produce a clear "not yet supported: <feature>" error, not a parser crash.
- The token-count CSV is checked in, so the README's 4× claim is either confirmed or revised against real numbers.
