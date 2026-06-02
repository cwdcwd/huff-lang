---
name: huff-lang
description: >
  Generate, read, extend, and reason about Huff IR — a token-efficient intermediate
  representation language designed for LLM generation and consumption. Use this skill
  whenever the task involves writing Huff code, translating prose requirements into
  Huff IR, extending existing Huff programs, reviewing Huff for correctness, or
  preparing Huff output for transpilation. Trigger on any mention of Huff, LLM IR,
  or requests to generate structured program representations from natural language specs.
---

# Huff Language Skill

Huff is an intermediate representation (IR) language with one primary design constraint:
**every token must carry maximum semantic weight**. It is not optimized for human
readability — it is optimized for LLM generation and consumption, with a sound enough
semantic model to feed a transpiler that targets TypeScript, Rust, or LLVM IR.

The workflow is:

```
prose -> LLM (Huff) -> transpiler -> compiled output
```

The LLM both generates Huff and consumes Huff as context for further generation.
Token efficiency at both stages is the governing concern.

---

## Core Principles

These principles govern every syntax decision. When in doubt, resolve against them.

1. **Every token carries semantic weight** — no ceremony that doesn't contribute meaning
2. **Correctness is structural** — invalid states are inexpressible, not caught at runtime
3. **Effects are explicit and scoped** — anything touching the outside world is marked
4. **State mutation is controlled** — always visible, always intentional
5. **Errors are values** — not exceptions, not side channels
6. **Concurrency is first-class** — not bolted on
7. **Composition over inheritance** — behavior is assembled, not inherited
8. **The compiler infers everything it can** — the LLM emits only what is ambiguous or load-bearing
9. **The transpiler owns mechanics** — Huff expresses intent, the transpiler resolves implementation

---

## Quick Syntax Reference

See `references/spec.md` for the full grammar and all rules.
See `references/examples.md` for complete worked examples.

### Declarations

| Construct | Syntax |
|---|---|
| Program | `prog Name` |
| Module | `mod Name` |
| Service | `svc Name` |
| Use module | `use Name` |
| Type | `type Name` |
| Operation | `op Name(params) ReturnType` |
| Async operation | `op~ Name(params) ReturnType` |
| Error | `err Name` |
| State (single) | `state name: Type = value` |
| State (block) | `state` block with indented fields |

### Operations

```
op Name(param: Type, param2: Type) ReturnType
  let x = expr          // immutable binding
  !effect(x)            // effectful statement
  x                     // implicit return — last expression
```

### Key Tokens

| Token | Meaning |
|---|---|
| `!` prefix | effectful statement (IO, state mutation) |
| `~` on op | async operation |
| `~` prefix on call | await expression |
| `->` | pipeline operator |
| `?` suffix | match expression |
| `&` prefix | borrow (intent only — transpiler resolves) |
| `shared<T>` | explicitly shared ownership |
| `pre` | precondition |
| `:` after pre | error to raise on violation |

### Primitive Types

| Type | Meaning |
|---|---|
| `str` | string |
| `bool` | boolean |
| `i32` | signed 32-bit integer |
| `u32` | unsigned 32-bit integer |
| `f32` | 32-bit float |
| `i64` / `u64` / `f64` | 64-bit variants |
| `bytes` | raw byte sequence |
| `()` | unit / void (also: omit return type) |

### Collection Types

| Type | Meaning |
|---|---|
| `[]T` | list of T |
| `map<K, V>` | key-value map |
| `T?` | optional T |
| `(T, U)` | tuple |

### Effects and IO

```
!io.write(str)          // stdout
!io.err(str)            // stderr
~io.fetch(url)          // async HTTP GET -> bytes
~io.fetch(url)->json()  // async HTTP GET -> parsed
!io.write(file, bytes)  // file write
io.read(file)           // file read (pure — returns bytes)
```

### Pipeline Operations

```
xs->map(f)              // transform each element
xs->filter(pred)        // keep matching elements
xs->each(f)             // side-effect each element (requires ! on f)
xs->reduce(f, init)     // fold
xs->where(pred)         // alias for filter in query contexts
xs->first()             // first element -> T?
xs->count()             // element count -> u32
```

### Match Expression

```
let result = expr?
  PatternA -> valueA
  PatternB -> valueB
  _ -> defaultValue      // exhaustive required
```

### Generics and Constraints

```
op Name<T: Constraint>(x: T) T
```

Built-in constraints: `Fmt` (formattable/stringable), `Eq` (equality), `Ord` (orderable),
`Clone` (copyable), `Send` (safe across async boundaries)

---

## Generation Rules

When generating Huff from prose:

1. **Start with the type model** — identify the nouns in the spec, make them types
2. **Identify state** — what persists between operations?
3. **Identify effects** — what touches the outside world?
4. **Identify errors** — what can go wrong structurally?
5. **Write operations last** — they compose everything above
6. **Omit what can be inferred** — if the transpiler can derive it, don't emit it
7. **Use pipelines for transformations** — prefer `->map`, `->filter` over explicit loops
8. **Keep op bodies short** — if a body exceeds ~6 lines, extract a named op

### What to emit explicitly

- Type definitions
- Operation signatures (name, params, return type)
- Explicit state declarations
- Effect statements (`!`)
- Preconditions (`pre`)
- Error definitions
- Async markers (`~`)
- `shared<T>` when ownership is genuinely shared

### What to omit and let the transpiler infer

- Borrow vs move in unambiguous cases
- Lifetime annotations
- Loop mechanics (use pipeline ops instead)
- Null checks implied by `T?`
- Auth enforcement implied by `auth.userId` reference
- Destructor calls implied by scope end

---

## Validation Rules

A valid Huff program must satisfy:

- Every `op` body's last expression matches the declared return type (or return type omitted for unit)
- Every `err` referenced in a `pre` is declared
- Every type used is declared or is a primitive
- Every `~` call site is inside an `op~`
- Every `!` mutation of state references a declared state field
- `match` expressions (`?`) are exhaustive — wildcard `_` required if not all cases covered
- `shared<mut T>` fields accessed across `op~` boundaries
- No cycles in type definitions (self-referential types not expressible)

---

## Transpiler Contract

Huff makes the following guarantees to the transpiler:

- All types are fully resolved at the top level
- All operations have unambiguous signatures
- Effect boundaries are marked — nothing outside `!` or `~` has side effects
- Error paths are named and exhaustive within declared preconditions
- State mutations only occur inside `!` blocks
- Async boundaries are explicit via `~`

The transpiler is responsible for:

- Resolving ownership mechanics (borrow, move, lifetime)
- Generating null/bounds checks for `T?` and `[]T`
- Implementing auth enforcement patterns
- Expanding pipeline operations to target-language idioms
- Mapping `shared<T>` to appropriate concurrency primitives
- Emitting destructors / cleanup at scope boundaries

---

## Common Mistakes to Avoid

- **Emitting `return` keyword** — last expression is implicit return
- **Writing `if/else`** — use `match` (`?`) instead
- **Writing `for` loops** — use `->each`, `->map`, `->filter`
- **Annotating borrows explicitly** unless the transpiler cannot infer intent
- **Restating types** — if a type is clear from context, omit the annotation
- **Verbose field names** — field names are IR identifiers, not documentation
- **`state.field` prefix in `!` blocks** — inside `!`, bare name refers to state field
- **Empty `effect:` blocks** — use `!` prefix inline, not a block wrapper
