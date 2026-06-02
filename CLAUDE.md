# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Huff is a language *design*, not yet an implementation. The repo contains:

- `README.md` — manifesto and quick tour
- `skill/SKILL.md` + `skill/references/` — the Claude skill that teaches an LLM to generate and reason about Huff IR
- `docs/plans/` — design and implementation plans (e.g. the TS transpiler walking-skeleton plan)

There is no compiler, transpiler, or runtime in this repo yet. There are no build, lint, or test commands. If you find yourself wanting to run `cargo` or `npm`, the corresponding package doesn't exist yet — check `docs/plans/` first to see whether it's planned and unbuilt.

## The governing design principle

Huff applies Huffman/Shannon entropy coding to language syntax: **the constructs an LLM emits most frequently get the shortest representations**. Effects (`!`), operations (`op`), pipelines (`->`), async (`~`) are one or two characters. Rare, load-bearing decisions (shared ownership, explicit borrow intent) are longer and more visible.

Two consequences that shape every decision:

1. **The author and consumer are both LLMs.** Token cost is paid twice — once on emission, once on context re-ingestion. Anything inferable by a transpiler is ceremony, and ceremony is cut.
2. **The transpiler is the complexity boundary.** Huff expresses *intent*; the transpiler resolves mechanics (ownership, lifetimes, null checks, auth enforcement, async runtime, sync primitives for `shared<mut T>`). When extending the language, push mechanics down to the transpiler and keep the surface syntax thin.

When evaluating a proposed syntax change, ask: does this token carry semantic content the transpiler couldn't infer? If no, cut it.

## The skill is the spec

`skill/SKILL.md` is the operating manual for *generating* Huff. `skill/references/spec.md` is the formal grammar and language semantics. `skill/references/examples.md` is six worked examples with progressive construct coverage.

These three files are tightly coupled — a change to the language must update all three. The skill's "Common Mistakes to Avoid" section captures patterns that are easy to get wrong (emitting `return`, using `if/else` instead of `?` match, prefixing state fields with `state.` inside `!` blocks).

## Syntax features that need careful handling

These tokens are overloaded and any future parser/emitter must disambiguate by position:

- **`!`** — prefix on a statement = effect; suffix on a type = error return (`T!E`); suffix on a call = error propagation
- **`~`** — suffix on `op` = async declaration; prefix on a call = await
- **`->`** — pipeline operator; function-type arrow; match arm separator
- **`?`** — suffix on a type = optional (`T?`); suffix on an expression followed by indented arms = match expression

Indentation is significant (2 spaces, Python-style). No braces, no semicolons.

## What's defined vs. deferred

The README's status checklist is the source of truth for what's in v0.1 vs. future work. Sum types, formal interface definitions, effect typing in op signatures, the reference transpiler, and ownership-model verification are all explicitly future work. Examples sometimes use future-syntax (e.g. sum types in `examples.md` §4) — these are previews, not v0.1.
