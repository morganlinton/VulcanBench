# tests/SKILL.md  -  vyre-macros

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-macros` provides **proc macros**: `#[vyre_pass]`,
`#[vyre_ast_registry]`. These macros generate IR enum variants and
optimizer-pass registrations at compile time. The whole contract is
compile-time, so most tests are compile-time checks.

## Critical invariants

- **Macro output is deterministic.** Same input → same generated
  tokens, byte-identical. Verified via `trybuild` snapshot tests.
- **Macro rejects invalid input with an actionable error.** Every
  malformed invocation surfaces a `compile_error!` with a `Fix:`
  hint pointing at the exact token that caused the problem.
- **Macro supports forward-compatible extension.** Adding a new
  field to the input DSL is additive  -  old call sites still
  compile unchanged.

## Adversarial surface

- Empty input (zero variants declared)  -  produces an empty enum,
  not an error
- Duplicate variant names  -  caught at macro time with `Fix:`
- Reserved keywords as variant names  -  caught
- Malformed braces / commas / colons  -  caught

## Current gaps

- `trybuild` coverage for every `compile_error!` path  -  gap test
  per malformed-input class
- Snapshot tests for macro output so any change to emitted tokens
  is a reviewable diff

## Cross-crate contracts

- Consumed by `vyre-foundation` for `Node` / `Expr` registration
- Consumed by `vyre-foundation::optimizer` for pass registration

## Bench targets

Macros run at compile time; per-call runtime benches don't apply.
Consider `./cargo_full +nightly build-std` measurements for the
vyre-foundation crate with and without the macros to quantify
expansion cost.

## Fuzz targets

Proc-macro fuzzing via `cargo-fuzz` is nontrivial but valuable  - 
feeding random token streams through the macro should never panic.

## What NOT to test here

- Generated-IR runtime semantics  -  `vyre-foundation/tests`
- Pass scheduling  -  `vyre-foundation/tests`

## Running

```bash
./cargo_full test -p vyre-macros
./cargo_full test -p vyre-macros --test adversarial   # trybuild compile_error cases
./cargo_full test -p vyre-macros --test integration
```
