# Huff Language Examples

A progression of complete, working Huff programs demonstrating all language constructs.

---

## Table of Contents

1. Hello World — minimal program
2. Hello World — full construct progression
3. File Service — service with auth
4. Task Queue — async, concurrency, shared state
5. Auth Service — composition, error propagation
6. Data Pipeline — generics, pipelines, modules

---

## 1. Hello World — Minimal

**Prose:** Print "Hello World" to standard output.

```huff
prog HelloWorld

  op Main()
    !io.writeln("Hello World")
```

Token count: 9 meaningful tokens. Equivalent TypeScript would be ~4x longer with
imports, exports, and type scaffolding.

---

## 2. Hello World — Full Construct Progression

**Prose:** Greet a list of named people. If no names provided, fail. Track count
of greetings printed. Support fetching names from a URL. Extract greeting logic
into a reusable module.

### Module

```huff
mod Greetings

  type Greeting
    to: str
    msg: str

  op Make(name: str) Greeting
    Greeting(name, "Hello " + name)

  op MakeCustom(name: str, prefix: str) Greeting
    Greeting(name, prefix + " " + name)
```

### Program

```huff
prog HelloWorld
  use Greetings

  err EmptyList
  err FetchFailed(msg: str)

  state count: u32 = 0

  op~ FetchNames(url: str) []str!FetchFailed
    let data = ~io.fetch(url)!
    data->json()

  op PrintGreetings(names: []str)
    pre names.len > 0 : EmptyList
    names
      ->map(Greetings.Make)
      ->each(g =>
          !io.writeln(g.msg)
          !count += 1
        )

  op~ Main(args: []str)
    let names = args?
      ["--url", url] -> ~FetchNames(url)!
      _ -> args->filter(a => !a.starts("--"))
    PrintGreetings(names)!
    !io.writeln("Printed " + count->fmt() + " greetings")
```

**Constructs demonstrated:**
- `mod` / `use` — modules
- `type` — named types
- `err` with data — errors carrying payloads
- `state` — mutable program state
- `op~` / `~call` — async operations
- `!` propagation — error forwarding
- `pre` — preconditions
- `->map` / `->filter` / `->each` — pipelines
- `?` match — pattern matching
- `!io.writeln` — effects
- `!count += 1` — state mutation in effect block
- `->fmt()` — constraint-based formatting

---

## 3. File Service

**Prose:** A file storage service. Users authenticate with tokens. Authenticated
users can upload, download, delete, and list their files. Files have names, sizes,
and owners. Download and delete require ownership.

```huff
svc FileService

  err NotFound
  err Unauthorized
  err ValidationFailed(msg: str)

  type FileId = u64
  type UserId = u64
  type Token = str

  type File
    id: FileId
    name: str
    size: u32
    owner: UserId
    data: bytes

  state
    files: map<FileId, File> = {}
    sessions: map<Token, UserId> = {}
    nextId: FileId = 1

  auth sessions[token] -> UserId

  op Upload(token: Token, name: str, data: bytes) FileId!ValidationFailed
    pre name.len > 0 : ValidationFailed("name required")
    pre name.len <= 255 : ValidationFailed("name too long")
    let id = nextId
    !files[id] = File(id, name, data.len, auth.userId, data)
    !nextId += 1
    id

  op Download(token: Token, id: FileId) bytes!(NotFound | Unauthorized)
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    files[id].data

  op Delete(token: Token, id: FileId)!(NotFound | Unauthorized)
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    !files.del(id)

  op List(token: Token) []File
    files->where(f => f.owner == auth.userId)

  op Rename(token: Token, id: FileId, name: str)!(NotFound | Unauthorized | ValidationFailed)
    pre files->contains(id) : NotFound
    pre files[id].owner == auth.userId : Unauthorized
    pre name.len > 0 : ValidationFailed("name required")
    !files[id].name = name
```

**Constructs demonstrated:**
- `svc` — service declaration
- `type X = Y` — type aliases
- `state` block — multiple state fields
- `auth` — service-level authentication
- Multiple `pre` conditions per op
- `!(A | B)` — union error return types
- `files->contains` / `files->where` — collection operations
- `files.del` — map mutation effect

---

## 4. Task Queue

**Prose:** An async task queue service. Tasks are submitted with a payload and
priority. A worker pool processes tasks concurrently. Track total processed count.
Support graceful drain — wait for in-progress tasks before shutdown.

```huff
svc TaskQueue

  err QueueFull
  err TaskNotFound
  err WorkerFailed(taskId: TaskId, reason: str)

  type TaskId = u64
  type Priority = u32

  type Task
    id: TaskId
    payload: bytes
    priority: Priority
    status: TaskStatus

  type TaskStatus = Pending | Running | Done | Failed(reason: str)

  type WorkResult
    taskId: TaskId
    output: bytes

  state
    queue: []Task = []
    processed: shared<mut u64> = 0
    maxSize: u32 = 1000

  op~ Submit(payload: bytes, priority: Priority) TaskId!QueueFull
    pre queue.len < maxSize : QueueFull
    let id = queue.len->into(TaskId) + 1
    let task = Task(id, payload, priority, Pending)
    !queue = queue->sort(t => t.priority)->append(task)
    id

  op~ Process(task: Task) WorkResult!WorkerFailed
    !task.status = Running
    let result = ~io.post("/worker", task.payload)
    let output = result?
      Ok(data) -> data
      Err(msg) -> WorkerFailed(task.id, msg)!
    !task.status = Done
    !processed += 1
    WorkResult(task.id, output)

  op~ RunWorkers(concurrency: u32)
    let pending = queue->where(t => t.status == Pending)
    let batches = pending->chunk(concurrency)
    batches->each(batch =>
      let results = ~par(batch->map(Process))
      results->each(r => !io.writeln("done: " + r.taskId->fmt()))
    )

  op~ Drain()
    let running = queue->where(t => t.status == Running)
    running->each(t => ~io.poll(t.id))
    !io.writeln("drained " + processed->fmt() + " tasks")

  op Stats() (u32, u64)
    (queue->where(t => t.status == Pending)->count(), processed)
```

**Constructs demonstrated:**
- `shared<mut T>` — concurrently mutable state
- `type X = A | B` — sum types (v0.1 preview syntax)
- `~par(...)` — parallel async execution
- Multi-line closures
- Tuple return types
- `->chunk` — batch pipeline op
- `->append` — list mutation
- `->into(Type)` — explicit type conversion

---

## 5. Auth Service

**Prose:** A JWT-based authentication service. Users register with email and
password. Login returns a session token. Tokens expire after 24 hours.
Password reset sends an email. Registration requires unique email.

```huff
svc AuthService
  use Crypto
  use Email

  err AlreadyExists
  err NotFound
  err InvalidCredentials
  err TokenExpired
  err WeakPassword(reason: str)

  type UserId = u64
  type Token = str
  type Hash = bytes

  type User
    id: UserId
    email: str
    passHash: Hash
    createdAt: u64

  type Session
    token: Token
    userId: UserId
    expiresAt: u64

  state
    users: map<str, User> = {}
    sessions: map<Token, Session> = {}
    nextId: UserId = 1

  op~ Register(email: str, pass: str) UserId!(AlreadyExists | WeakPassword)
    pre !users->contains(email) : AlreadyExists
    pre pass.len >= 8 : WeakPassword("minimum 8 characters")
    pre pass->hasUpper() : WeakPassword("requires uppercase letter")
    let hash = ~Crypto.hash(pass)
    let id = nextId
    !users[email] = User(id, email, hash, io.now())
    !nextId += 1
    id

  op~ Login(email: str, pass: str) Token!(NotFound | InvalidCredentials)
    pre users->contains(email) : NotFound
    let user = users[email]
    let valid = ~Crypto.verify(pass, user.passHash)
    pre valid : InvalidCredentials
    let token = ~Crypto.token()
    let session = Session(token, user.id, io.now() + 86400000)
    !sessions[token] = session
    token

  op ValidateToken(token: Token) UserId!(TokenExpired | NotFound)
    pre sessions->contains(token) : NotFound
    let session = sessions[token]
    pre session.expiresAt > io.now() : TokenExpired
    session.userId

  op~ ResetPassword(email: str)!NotFound
    pre users->contains(email) : NotFound
    let token = ~Crypto.token()
    ~Email.send(email, "Reset your password", "Token: " + token)

  op Logout(token: Token)
    !sessions.del(token)
```

**Constructs demonstrated:**
- Multi-module composition (`use Crypto`, `use Email`)
- Error propagation chain across ops
- `io.now()` — pure system call (no `!`)
- `~Crypto.verify` — async external module call
- Complex precondition chains
- Derived state (`nextId`) as a simple counter pattern

---

## 6. Data Pipeline

**Prose:** A generic data pipeline module. Accepts a source of records, applies
a transformation chain, filters invalid results, and writes to a sink.
Support both sync and async transformations. Report metrics on completion.

```huff
mod Pipeline

  type Metrics
    input: u32
    output: u32
    dropped: u32
    elapsed: u64

  op~ Run<A: Send, B: Send>(
    source: []A,
    transform: A -> B?,
    sink: B -> ()
  ) Metrics
    let start = io.now()
    let results = source->map(transform)->filter(r => r != ())
    let dropped = source.len - results.len
    results->each(sink)
    Metrics(source.len, results.len, dropped, io.now() - start)

  op~ RunAsync<A: Send, B: Send>(
    source: []A,
    transform: A -> ~B?,
    sink: B -> ~(),
    concurrency: u32
  ) Metrics
    let start = io.now()
    let batches = source->chunk(concurrency)
    let results = batches->flat()->filter(r => r != ())
    let dropped = source.len - results.len
    results->each(r => ~sink(r))
    Metrics(source.len, results.len, dropped, io.now() - start)

  op LogMetrics(m: Metrics)
    !io.writeln(
      "in=" + m.input->fmt()
      + " out=" + m.output->fmt()
      + " dropped=" + m.dropped->fmt()
      + " ms=" + m.elapsed->fmt()
    )
```

**Usage:**

```huff
prog ETL
  use Pipeline

  type Record
    id: u64
    val: f64

  op Parse(raw: str) Record?
    let parts = raw.split(",")
    parts.len == 2?
      true -> Record(parts[0]->parse(), parts[1]->parse())
      false -> ()

  op~ Main(args: []str)
    let lines = io.readstr(args[0]).split("\n")
    let metrics = ~Pipeline.Run(lines, Parse, r =>
      !io.writeln(r.id->fmt() + ": " + r.val->fmt())
    )
    Pipeline.LogMetrics(metrics)
```

**Constructs demonstrated:**
- Generic operations with multiple type parameters and constraints
- Higher-order operations (functions as parameters)
- Async function type parameters (`A -> ~B?`)
- `->chunk` and `->flat` for batch processing
- `->parse()` — type-inferred string parsing
- Composing modules in a program
- `io.readstr` pure read, `->split` pipeline

---

## Token Efficiency Comparison

To illustrate the efficiency goal, here is the File Service `Upload` operation
in Huff vs equivalent TypeScript:

**Huff (15 tokens of meaningful content):**
```huff
op Upload(token: Token, name: str, data: bytes) FileId!ValidationFailed
  pre name.len > 0 : ValidationFailed("name required")
  pre name.len <= 255 : ValidationFailed("name too long")
  let id = nextId
  !files[id] = File(id, name, data.len, auth.userId, data)
  !nextId += 1
  id
```

**Equivalent TypeScript (~60 tokens, 4× inflation):**
```typescript
async upload(
  token: string,
  name: string,
  data: Uint8Array
): Promise<FileId> {
  const userId = this.authenticate(token)
  if (!userId) throw new UnauthorizedError()
  if (name.length === 0) throw new ValidationError("name required")
  if (name.length > 255) throw new ValidationError("name too long")
  const id = this.nextId
  this.files.set(id, {
    id,
    name,
    size: data.length,
    owner: userId,
    data
  })
  this.nextId++
  return id
}
```

The TypeScript carries: `async`, `token: string`, `name: string`,
`data: Uint8Array`, `Promise<FileId>`, `const userId = this.authenticate(token)`,
`if (!userId) throw new UnauthorizedError()`, `this.files.set(...)`, explicit
object construction with repeated field names, `this.nextId++`, `return id`.

All of that is either inferable from the Huff spec or transpiler responsibility.
The LLM generating TypeScript must emit and then re-consume all of it. The LLM
generating Huff emits only what carries semantic content.
