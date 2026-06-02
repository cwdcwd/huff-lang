# Task: build the Huff TS transpiler walking skeleton

You are working in the `huff-lang` repo. Implement the walking-skeleton transpiler
described in `docs/plans/ts-transpiler-walking-skeleton.md`. Read that plan and
`CLAUDE.md` first; they are the source of truth.

This prompt is run repeatedly in a loop. Each iteration you make **incremental,
committed progress** — you do NOT have to finish everything in one pass. Pick up
where the last iteration left off.

## Each iteration, do this

1. **Orient.** Read the plan and run `cargo test --workspace 2>&1 | tail -40`
   (if `packages/` doesn't exist yet, start at Phase 1). Look at recent git log
   to see what prior iterations did.
2. **Pick the next smallest unit of work** that moves toward the Definition of
   Done. Follow the phase order: AST → lexer → parser → emitter → CLI → validation.
   Do not skip ahead; later phases depend on earlier ones compiling and testing green.
3. **Implement it.** Match the crate layout, dependency choices, and v0 subset in
   the plan. Honor the deferred list — emit a clear "not yet supported: <feature>"
   error for out-of-subset constructs rather than half-implementing them.
4. **Verify.** Build and test what you changed (`cargo build`, `cargo test -p <crate>`).
   Do not declare progress on something that doesn't compile.
5. **Stop** when you've completed one coherent, green unit of work. The loop will
   re-invoke you for the next.

## Rules

- Stay within the **v0 subset** (plan §"Subset for v0"). Resist scope creep.
- Honor the **open questions** already settled in the plan: errors-as-exceptions
  for v0, `namespace` module model, module-level `state`, all numerics → `number`.
- Keep the existing `skill/`, `README.md`, and `docs/` untouched except to read them.
- If a phase's deliverable is satisfied and tests pass, move to the next phase.
- Never run destructive commands (`git reset --hard`, `rm -rf`, force-push, etc.).

## Done condition — read carefully

The work is complete ONLY when every item in the plan's "Definition of done" holds:

- `cargo test --workspace` passes,
- `cargo run -p huff-cli -- skill/references/examples/hello.huff` produces TS that
  runs and prints "Hello World",
- a snapshot exists for each v0-subset example,
- out-of-subset examples produce a clear "not yet supported: <feature>" error,
- the token-count CSV is checked in.

When and ONLY when you have verified all of the above in this iteration — by
actually running the commands and seeing them pass — print the exact token

    RALPH_DONE

on its own line as the last line of your output. If anything is incomplete,
unverified, or failing, do NOT print it; describe what's left and stop so the
next iteration can continue.
