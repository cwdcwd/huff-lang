# Huff

> An LLM-native intermediate representation language grounded in Shannon's information theory.

Named after David Huffman, whose entropy coding insight — assign shorter codes to higher-frequency, higher-signal symbols — is the governing design principle of this language.

---

## What is Huff?

Huff is an intermediate representation (IR) language with one primary design constraint: **every token must carry maximum semantic weight**.

It is not designed to be written by humans. It is designed to be generated and consumed by large language models, with sound enough semantics to feed a transpiler that targets TypeScript, Rust, or LLVM IR.

The workflow is:

```
prose ──► LLM ──► Huff IR ──► transpiler ──► compiled output
                    ▲               │
                    └───────────────┘
                   (fed back for further generation)
```

The LLM both generates Huff and consumes Huff as context for further generation. Token efficiency at both stages is the governing concern.

---

## The Problem

Current programming languages carry enormous legacy overhead — verbose keywords, redundant type annotations, syntactic ceremony — that made sense when humans were the primary authors. An LLM generating TypeScript must emit and re-consume all of that overhead:

```typescript
async upload(token: string, name: string, data: Uint8Array): Promise<FileId> {
  const userId = this.authenticate(token)
  if (!userId) throw new UnauthorizedError()
  if (name.length === 0) throw new ValidationError("name required")
  if (name.length > 255) throw new ValidationError("name too long")
  const id = this.nextId
  this.files.set(id, { id, name, size: data.length, owner: userId, data })
  this.nextId++
  return id
}
```

The equivalent Huff:

```huff
op Upload(token: Token, name: str, data: bytes) FileId!ValidationFailed
  pre name.len > 0 : ValidationFailed("name required")
  pre name.len <= 255 : ValidationFailed("name too long")
  let id = nextId
  !files[id] = File(id, name, data.len, auth.userId, data)
  !nextId += 1
  id
```

Same semantics. Fewer tokens, less ceremony — the transpiler handles everything below the meaning boundary.

### Measured token efficiency

The walking-skeleton TS transpiler now emits TypeScript for the v0 subset (incl. async), so the token-efficiency claim is measured rather than asserted. Two tokenizers are reported in [docs/token-counts.csv](docs/token-counts.csv):

- **cl100k** — the GPT-4-era BPE (`tiktoken` npm package). Free, offline, deterministic. A reasonable stand-in for "what an LLM sees".
- **Claude** — Anthropic's [`messages/count_tokens`](https://docs.anthropic.com/en/build-with-claude/token-counting) endpoint, which reports the *actual* token count Claude uses. Reachable via `ANTHROPIC_API_KEY` (Anthropic API) or, by default, `bedrock-runtime:CountTokens` (AWS Bedrock — works with SSO credentials).

Ratios across the v0-subset examples ([hello, counter, greetings, async](skill/references/examples)):

| tokenizer | ts/huff ratio |
|---|---|
| cl100k | **1.50–1.74×** |
| Claude (Sonnet 4.6) | **1.23–1.58×** |

So the emitted TypeScript is consistently more verbose than the Huff source — meaningfully so by Claude's own tokenizer — but well short of the 4× the manifesto used to assert. The 4× number was an *unmeasured* claim made before any transpiler existed; reality on the v0 subset is closer to **1.2–1.7×**.

A few caveats worth keeping in mind:

- The walking-skeleton emitter is verbose on purpose (header comments, blank lines, explicit entry-point call). A "minimum-token TS" mode would shrink the denominator further.
- `count_tokens` is documented as an *estimate* — tiny system-added overhead may be folded in.
- The high-end ratio (where Huff really shines) lives in the `svc`/auth/effect-typed examples that v0 doesn't transpile yet. We'll re-measure once those phases land instead of guessing.

To regenerate the CSV: `npm --prefix packages/huff-tools run token-counts`. The script auto-selects the Claude path (Anthropic API if `ANTHROPIC_API_KEY` is set, else Bedrock if AWS creds resolve, else cl100k-only).

---

## Design Principles

1. **Every token carries semantic weight** — no ceremony that doesn't contribute meaning
2. **Correctness is structural** — invalid states are inexpressible, not caught at runtime
3. **Effects are explicit and scoped** — anything touching the outside world is marked with `!`
4. **State mutation is controlled** — always visible, always intentional
5. **Errors are values** — not exceptions, not side channels
6. **Concurrency is first-class** — not bolted on
7. **Composition over inheritance** — behavior is assembled, not inherited
8. **The compiler infers everything it can** — the LLM emits only what is ambiguous or load-bearing
9. **The transpiler owns mechanics** — Huff expresses intent, the transpiler resolves implementation

---

## Quick Look

```huff
svc FileService

  err NotFound
  err Unauthorized
  err ValidationFailed(msg: str)

  type File
    id: FileId
    name: str
    size: u32
    owner: UserId

  state
    files: map<FileId, File> = {}
    sessions: map<Token, UserId> = {}

  auth sessions[token] -> UserId

  op Upload(token: Token, name: str, data: bytes) FileId!ValidationFailed
    pre name.len > 0 : ValidationFailed("name required")
    !files[newId] = File(newId, name, data.len, auth.userId)
    newId

  op Download(token: Token, id: FileId) bytes!(NotFound | Unauthorized)
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    files[id].data

  op List(token: Token) []File
    files->where(f => f.owner == auth.userId)
```

Key syntax tokens:

| Token | Meaning |
|---|---|
| `!` prefix | effectful statement (IO, state mutation) |
| `~` on `op` | async operation |
| `~` prefix on call | await |
| `->` | pipeline operator |
| `?` suffix | match expression |
| `pre` | precondition — structural error on violation |
| `shared<T>` | explicitly shared ownership (opt-in) |

---

## Repository Structure

```
huff-lang/
├── README.md
├── Cargo.toml                — Rust workspace (transpiler crates only)
├── packages/
│   ├── huff-ast/             — AST node types
│   ├── huff-parser/          — lexer + recursive-descent parser
│   ├── huff-emit-ts/         — AST → TypeScript emitter
│   ├── huff-cli/             — `huffc` binary
│   └── huff-tools/           — TS analysis & validation harness (vitest, tiktoken, Anthropic SDK, AWS SDK)
├── docs/
│   ├── plans/                — design plans (e.g. ts-transpiler-walking-skeleton.md)
│   └── token-counts.csv      — measured token efficiency, regenerated by huff-tools
└── skill/
    ├── SKILL.md              — LLM skill: generation rules, quick reference, transpiler contract
    └── references/
        ├── spec.md           — complete language specification and formal grammar
        ├── examples.md       — six worked examples with token efficiency analysis
        └── examples/         — runnable .huff files exercised by the validation harness
```

The split is deliberate: **Rust owns the transpiler binaries**; **TypeScript owns the analysis and end-to-end validation** (so the same language we target is the one we measure with).

### Using the skill

The `skill/` directory is structured for use with Claude's skill system. Drop `skill/SKILL.md` and its `references/` directory into your Claude skills folder. Claude will then generate and reason about Huff IR when prompted to produce structured program representations.

### Building & testing

```
cargo test --workspace                   # transpiler unit tests
npm --prefix packages/huff-tools install # one-time
npm --prefix packages/huff-tools test    # validation harness (snapshots + end-to-end run)
npm --prefix packages/huff-tools run token-counts  # regenerate docs/token-counts.csv
```

---

## Status

Huff is a working concept and language design. The following are defined:

- [x] Core syntax and token vocabulary
- [x] Type system (primitives, composites, generics, aliases)
- [x] Operation model (sync, async, generic)
- [x] Effect system (`!` / `~`)
- [x] Error handling (values, preconditions, propagation)
- [x] Ownership intent model
- [x] Pipeline operators
- [x] Module and service model
- [x] Auth declaration pattern
- [x] Formal grammar
- [x] LLM generation skill

The following are scoped for future versions:

- [ ] Sum types / discriminated unions (syntax sketched in examples)
- [ ] Interface / protocol formal definitions
- [ ] Effect typing in operation signatures
- [x] Reference transpiler implementation (TypeScript target) — v0 walking skeleton in [packages/](packages); covers `prog`/`mod` + sync & async ops; defers `svc`/auth, generics, sum types, match expressions, and shared ownership
- [ ] Formal verification of ownership model

---

## Theoretical Foundation

Huff takes its name and its core insight from David Huffman's 1952 paper *A Method for the Construction of Minimum-Redundancy Codes*. Huffman's algorithm assigns codewords such that the most frequent, highest-information symbols receive the shortest representations.

Huff the language applies this principle to programming language design: the constructs an LLM emits most frequently — effects, type declarations, operation signatures, pipelines — receive the shortest syntactic representations (`!`, `type`, `op`, `->`). Constructs that represent deliberate, less-frequent decisions — shared ownership, explicit borrow intent — are longer and more visible.

The result is a language whose token distribution is shaped by semantic frequency rather than historical accident.

---

## License

MIT
