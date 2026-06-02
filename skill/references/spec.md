# Huff Language Specification

Version: 0.1 (working draft)

---

## Table of Contents

1. Design Goals
2. Program Structure
3. Type System
4. Operations
5. Expressions
6. Effects
7. State
8. Error Handling
9. Ownership Model
10. Async and Concurrency
11. Modules and Services
12. Grammar Summary

---

## 1. Design Goals

Huff is not a systems language, a scripting language, or a general-purpose language.
It is an **LLM-native intermediate representation** with the following design goals
in priority order:

1. Token efficiency for LLM generation and consumption
2. Sound semantics sufficient for transpiler target correctness
3. Structural correctness — invalid programs are inexpressible
4. Human legibility as a secondary concern only

Huff does not target a runtime directly. It targets a transpiler, which resolves
implementation mechanics and emits a target language (TypeScript, Rust, etc.) or
LLVM IR. The transpiler is the complexity boundary. Huff stays above it.

---

## 2. Program Structure

### Top-level forms

A Huff file contains exactly one of:
- `prog Name` — executable program with an entry point
- `mod Name` — reusable module without entry point
- `svc Name` — service with request/response operations and optional auth

### Indentation

Huff uses indentation for block structure (2 spaces). No braces, no semicolons.
Block members are indented one level from their parent declaration.

### Order within a top-level form

Declarations appear in this order (all optional except at least one `op`):
1. `use` statements
2. `err` declarations
3. `type` declarations
4. `state` declarations
5. `auth` declaration (svc only)
6. `op` declarations

### Example structure

```
prog MyApp
  use MyModule

  err NotFound
  err Unauthorized

  type User
    id: UserId
    name: str

  state count: u32 = 0

  op Main(args: []str)
    // ...

```

---

## 3. Type System

### Primitive types

```
str       — UTF-8 string
bool      — true / false
i32       — signed 32-bit integer
u32       — unsigned 32-bit integer
i64       — signed 64-bit integer
u64       — unsigned 64-bit integer
f32       — 32-bit IEEE float
f64       — 64-bit IEEE float
bytes     — raw byte sequence
```

### Composite types

```
[]T           — ordered list of T
map<K, V>    — key-value map; K must satisfy Eq + Ord
T?            — optional value; absent or T
(T, U)        — tuple; up to 6 elements
```

### Named types

```
type Name
  field: Type
  field2: Type
```

Named types are product types (structs). No inheritance. No methods on types —
behavior lives in operations. Field names are IR identifiers, kept short.

### Type aliases

```
type UserId = u64
type Token = str
```

Aliases create distinct types — `UserId` and `u64` are not interchangeable without
explicit conversion.

### Generic types

```
type Pair<A, B>
  a: A
  b: B
```

### Constraints

Constraints express required capabilities. Applied to generic type parameters.

```
<T: Fmt>      — T can be represented as str
<T: Eq>       — T supports equality comparison
<T: Ord>      — T supports ordering (implies Eq)
<T: Clone>    — T can be duplicated
<T: Send>     — T is safe to move across async boundaries
```

Multiple constraints: `<T: Eq + Ord>`

### Shared ownership

```
shared<T>       — reference-counted shared ownership, immutable
shared<mut T>   — reference-counted shared ownership, mutable (requires sync)
```

`shared<T>` is an explicit opt-in. It is never inferred. Use only when multiple
owners genuinely exist (caches, pools, cross-op state).

---

## 4. Operations

### Basic form

```
op Name(param: Type, param2: Type) ReturnType
  body
```

- Parameters are positional, named, typed
- Return type follows the parameter list; omit for unit return
- Body is indented block
- Last expression is the implicit return value
- No `return` keyword

### Async operations

```
op~ Name(param: Type) ReturnType
  body
```

`~` suffix marks the operation as async. All `~` call sites must be inside an `op~`.

### Calling operations

```
let x = Name(arg1, arg2)       // sync call
let x = ~Name(arg1, arg2)      // async call — must be inside op~
```

### Generic operations

```
op Make<T: Fmt>(val: T) str
  val->fmt()
```

### Entry point

In a `prog`, `op Main` is the entry point. It receives `[]str` args or no params.
It returns unit (no return type annotation).

```
op Main(args: []str)
  // program body
```

---

## 5. Expressions

### Bindings

```
let name = expr          // immutable binding
let name: Type = expr    // with explicit type (rarely needed)
```

Bindings are immutable. To model mutable local state, prefer pipelines or
restructured ops. Mutable program state lives in `state` declarations.

### Arithmetic and comparison

```
x + y    x - y    x * y    x / y    x % y
x == y   x != y   x < y    x > y    x <= y    x >= y
x && y   x || y   !x
```

### String operations

```
"Hello " + name          // concatenation
name.len                 // length -> u32
name.trim()              // whitespace stripped -> str
name.upper()             // uppercase -> str
name.lower()             // lowercase -> str
name.contains(sub)       // bool
name.starts(prefix)      // bool
name.split(sep)          // []str
```

### Optional expressions

```
val ?? default           // unwrap or default
val?->op()               // optional chaining — short-circuits to () if absent
```

### Tuple construction and access

```
let t = (a, b)
let first = t.0
let second = t.1
```

### Type construction

```
MyType(field1, field2)          // positional
MyType(fieldName: val, ...)     // named (when order ambiguous)
```

### Match expression

```
expr?
  Pattern -> result
  Pattern -> result
  _ -> default
```

Patterns can be:
- Literal values: `"admin"`, `42`, `true`
- Type variants (when sum types added — see future work)
- Wildcard `_` (required if non-exhaustive)

Match is an expression — it returns a value. Assign with `let`.

### Pipeline expressions

```
xs->map(f)           // []A -> []B where f: A -> B
xs->filter(pred)     // []A -> []A where pred: A -> bool
xs->where(pred)      // alias for filter (preferred in query contexts)
xs->each(f)          // []A -> () side-effecting; f must use !
xs->reduce(f, init)  // []A -> B where f: (B, A) -> B
xs->first()          // []A -> A?
xs->last()           // []A -> A?
xs->count()          // []A -> u32
xs->flat()           // [][]A -> []A
xs->sort()           // []A -> []A (A must satisfy Ord)
xs->sort(key)        // []A -> []A sorted by key: A -> Ord
xs->zip(ys)          // []A, []B -> [](A, B)
```

Pipeline stages chain with `->`. The pipeline is lazy — no computation until
consumed by a terminal op (`->each`, `->reduce`, `->first`, `->count`).

```
names
  ->filter(n => n.len > 0)
  ->map(n => Greeting(n, "Hello " + n))
  ->each(g => !io.write(g.msg))
```

### Closures

```
x => expr                    // single param, expression body
(x, y) => expr               // multi-param
(x, y) =>                    // multi-line body
  let z = x + y
  !io.write(z->fmt())
  z
```

---

## 6. Effects

Effects are operations that interact with the world outside the program.
Every effectful statement is prefixed with `!`. This is enforced — the compiler
rejects effects without `!` and rejects `!` on pure expressions.

### IO effects

```
!io.write(str)              // write string to stdout
!io.writeln(str)            // write string + newline to stdout
!io.err(str)                // write to stderr
!io.write(path, bytes)      // write bytes to file
```

### IO reads (pure — no `!` required)

```
io.read(path)               // read file -> bytes
io.readstr(path)            // read file -> str
io.exists(path)             // bool
io.ls(path)                 // []str (directory listing)
```

### Async IO

```
~io.fetch(url)              // HTTP GET -> bytes
~io.fetch(url)->json()      // HTTP GET -> parsed map
~io.post(url, body)         // HTTP POST -> bytes
~io.post(url, body)->json() // HTTP POST -> parsed map
```

### State mutation effects

Inside any `!` block, bare state field names refer to declared state:

```
state count: u32 = 0

op Inc()
  !count += 1              // mutates state.count
```

Outside `!`, state fields are readable without `!`:

```
op GetCount() u32
  count                   // pure read of state.count
```

---

## 7. State

### Single state field

```
state name: Type = initialValue
```

### Multiple state fields

```
state
  count: u32 = 0
  cache: map<str, bytes> = {}
  active: bool = true
```

### State in services

State in a `svc` is the service's persistent store. The transpiler maps this to
the appropriate backing mechanism (in-memory, database, etc.) based on type and
configuration.

```
svc Counter
  state total: u64 = 0

  op Inc()
    !total += 1

  op Get() u64
    total
```

### Shared state

When state must be accessed across async boundaries or from concurrent operations:

```
state pool: shared<ConnectionPool>
state cache: shared<mut map<str, bytes>>
```

`shared<mut T>` implies synchronization — the transpiler emits appropriate
locking or atomic operations.

---

## 8. Error Handling

### Declaring errors

```
err NotFound
err Unauthorized
err ValidationFailed(msg: str)    // errors can carry data
```

### Preconditions

```
op GetFile(id: FileId) File
  pre files->contains(id) : NotFound
  files[id]
```

`pre condition : Error` — if condition is false, operation returns the named error.
Multiple preconditions are allowed, one per line.

```
op Transfer(from: AccountId, to: AccountId, amt: f64)
  pre amt > 0 : ValidationFailed("amount must be positive")
  pre accounts->contains(from) : NotFound
  pre accounts[from].balance >= amt : ValidationFailed("insufficient funds")
  // ...
```

### Error return types

When an operation can fail, the return type uses `!`:

```
op GetFile(id: FileId) File!NotFound
op Transfer(from: AccountId, to: AccountId, amt: f64)!ValidationFailed
```

For multiple error variants:

```
op Login(creds: Credentials) Session!(Unauthorized | NotFound)
```

The transpiler maps these to Result/Either types in the target language.

### Error propagation

```
let file = GetFile(id)!     // propagate error upward (like Rust's ?)
```

`!` suffix on a call that returns `T!E` either unwraps `T` or propagates `E`
to the caller. Caller must declare compatible error type.

---

## 9. Ownership Model

Huff expresses ownership **intent**. The transpiler resolves mechanics.

### Intent declarations

The LLM emits ownership intent only when the transpiler cannot infer it:

```
op Print(file: &File)      // borrow intent — don't take ownership
op Store(file: File)        // move intent — take ownership
op Clone(file: +File)       // copy intent — duplicate the value
```

In most cases, intent is inferred from usage:
- Op reads a value and doesn't store it or return it → borrow inferred
- Op stores a value in state or returns it → move inferred
- Emit explicit intent only when the pattern is ambiguous

### Explicit intent tokens

| Token | Intent |
|---|---|
| `&T` | borrow — read access, no ownership transfer |
| `T` (default) | move — ownership transfers to this op |
| `+T` | clone — duplicate; original and copy both valid |
| `shared<T>` | shared ref-counted ownership |

### Invariants (enforced by transpiler)

- No use after move
- No move while borrowed
- Exclusive mutation requires exclusive access
- Borrows do not outlive owners
- Types may not contain borrow fields (only owned or shared fields)
- Values crossing `~` await points must be owned or `shared<T>`, not borrowed

---

## 10. Async and Concurrency

### Async operations

```
op~ FetchUser(id: UserId) User
  let data = ~io.fetch("/users/" + id->fmt())
  data->json()->into(User)
```

`op~` declares async. `~expr` awaits a value. All `~` calls must be inside `op~`.

### Parallel execution

```
let (a, b) = ~par(FetchA(), FetchB())    // run concurrently, await both
```

`par(...)` takes N async calls and returns a tuple of results when all complete.

### Racing

```
let result = ~race(FetchPrimary(), FetchFallback())   // first to complete wins
```

### Timeouts

```
let result = ~timeout(FetchData(), 5000)   // ms; returns T? (absent on timeout)
```

### Shared mutable state across async

Requires `shared<mut T>`. The transpiler emits appropriate synchronization.

```
state hits: shared<mut u64> = 0

op~ HandleRequest(req: Request) Response
  !hits += 1
  // ...
```

---

## 11. Modules and Services

### Modules

```
mod Greetings

  type Greeting
    to: str
    msg: str

  op Make(name: str) Greeting
    Greeting(name, "Hello " + name)
```

Modules export all top-level declarations. No explicit export keyword.

### Using modules

```
prog HelloApp
  use Greetings

  op Main(names: []str)
    names->map(Greetings.Make)->each(g => !io.writeln(g.msg))
```

### Services

Services are modules with:
- Request/response operation semantics
- Optional `auth` declaration
- State as service-level persistence

```
svc FileService
  state files: map<FileId, File>
  state sessions: map<Token, UserId>

  type File
    id: FileId
    name: str
    size: u32
    owner: UserId

  auth sessions[token] -> UserId

  op Upload(token: Token, name: str, data: bytes) FileId!ValidationFailed
    pre name.len > 0 : ValidationFailed("name required")
    !files[newId] = File(newId, name, data.len, auth.userId)
    newId

  op Download(token: Token, id: FileId) bytes!NotFound
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    files[id].data

  op Delete(token: Token, id: FileId)!(NotFound | Unauthorized)
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    !files.del(id)

  op List(token: Token) []File
    files->where(f => f.owner == auth.userId)
```

### Auth in services

```
auth sessions[token] -> UserId
```

This declares that a `token` parameter on any op is authenticated against `sessions`,
yielding `auth.userId`. Any op referencing `auth.userId` implicitly requires
a valid `token` parameter. The transpiler emits the auth enforcement.

---

## 12. Grammar Summary

```
huff-file     = prog-decl | mod-decl | svc-decl

prog-decl     = 'prog' Name NEWLINE INDENT prog-body DEDENT
mod-decl      = 'mod' Name NEWLINE INDENT mod-body DEDENT
svc-decl      = 'svc' Name NEWLINE INDENT svc-body DEDENT

prog-body     = use* err* type* state* op+
mod-body      = err* type* op+
svc-body      = use* err* type* state* auth? op+

use-stmt      = 'use' Name
err-decl      = 'err' Name ('(' field-list ')')?
type-decl     = 'type' Name ('<' type-params '>')? NEWLINE INDENT field+ DEDENT
              | 'type' Name '=' Type
state-decl    = 'state' Name ':' Type '=' expr
              | 'state' NEWLINE INDENT (Name ':' Type '=' expr)+ DEDENT
auth-decl     = 'auth' expr '->' Type
op-decl       = 'op' '~'? Name generic? '(' param-list ')' Type? NEWLINE INDENT op-body DEDENT

op-body       = (let-stmt | effect-stmt | pre-stmt | expr)*

let-stmt      = 'let' Name (':' Type)? '=' expr
effect-stmt   = '!' expr
pre-stmt      = 'pre' expr (':' error-ref)?

expr          = literal | name | call | pipeline | match | closure | binary | unary
              | optional-chain | await-expr | par-expr

call          = Name '(' arg-list ')'
await-expr    = '~' call
par-expr      = '~' 'par' '(' call-list ')'
pipeline      = expr '->' pipeline-op ('->' pipeline-op)*
pipeline-op   = Name ('(' arg-list ')')?
match         = expr '?' NEWLINE INDENT (pattern '->' expr)+ DEDENT
closure       = Name '=>' expr
              | '(' name-list ')' '=>' expr
              | '(' name-list ')' '=>' NEWLINE INDENT op-body DEDENT

Type          = PrimType | NamedType | '[]' Type | 'map' '<' Type ',' Type '>'
              | Type '?' | '(' Type ',' Type ')'
              | 'shared' '<' ('mut')? Type '>'
              | '&' Type | '+' Type
```

---

## Future Work (not in v0.1)

- **Sum types / discriminated unions** — `type Shape = Circle(r: f32) | Rect(w: f32, h: f32)`
- **Interfaces / protocols** — formal definition of constraint capabilities (Fmt, Eq, etc.)
- **Effect typing** — tracking which effects an op may produce in its signature
- **Dependent types** — types parameterized on values, for array lengths etc.
- **Macro system** — for common patterns (CRUD generation, auth scaffolding)
