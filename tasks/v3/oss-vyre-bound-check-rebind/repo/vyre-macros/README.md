# vyre-macros

Proc-macros for the [`vyre`](../core) GPU compute IR compiler.

This crate is consumed at compile time only: you do not depend on it directly
in most cases. The `vyre` crate re-exports everything you need:

```rust
use vyre::optimizer::{vyre_pass, Pass, PassAnalysis, PassResult, PassMetadata};
use vyre::ir::Program;
```

## Macros

### `#[vyre_pass(name = "...", requires = [...], invalidates = [...])]`

Register a unit struct as an optimizer pass in the global
`inventory::collect!(PassRegistration)` registry. The scheduler picks it up
automatically and applies it to every `Program` during `vyre::optimize()`.

The annotated type must expose three inherent methods with exactly the
signatures below: the macro wires them into the `Pass` trait impl:

```rust
use vyre::ir::Program;
use vyre::optimizer::{vyre_pass, PassAnalysis, PassResult};

#[vyre_pass(name = "fold_zero_add", requires = [], invalidates = [])]
pub struct FoldZeroAdd;

impl FoldZeroAdd {
    fn analyze(&self, program: &Program) -> PassAnalysis {
        // Cheap check: does the program contain an `Add` with a zero literal?
        // Return PassAnalysis::RUN to proceed or PassAnalysis::SKIP to bypass.
        let _ = program;
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        // Rewrite the program. If `before == after`, set `changed: false`
        // so the scheduler can prove the fixpoint.
        let before = program.clone();
        let after = program; // (real rewrite would go here)
        PassResult::from_programs(&before, after)
    }

    fn fingerprint(&self, program: &Program) -> u64 {
        // Fast hash used to detect whether a second pass would do anything.
        // Use `vyre::optimizer::fingerprint_program(program)` if you don't
        // need custom semantics.
        vyre::optimizer::fingerprint_program(program)
    }
}
```

#### Arguments

| Argument        | Type           | Meaning                                                                                             |
|-----------------|----------------|-----------------------------------------------------------------------------------------------------|
| `name`          | `&'static str` | Stable pass name used in diagnostics and scheduling.                                                |
| `requires`      | `&[&str]`      | Pass names that must have already run (or analyses that must be available) before this one fires.   |
| `invalidates`   | `&[&str]`      | Analyses invalidated when this pass changes the program.                                            |

Every pass contributes to the shared fixpoint loop: the scheduler keeps
iterating until either every pass reports `changed: false` or the safety cap
is hit.

## See also

- [`vyre::optimizer`](https://docs.rs/vyre/latest/vyre/optimizer/)  -  the trait
  definition, registry, and scheduler.
- [`vyre-core/src/optimizer/passes/`](../vyre-core/src/optimizer/passes/)  -  reference
  passes (`const_fold`, `strength_reduce`, etc.).

## License

Dual-licensed under MIT or Apache-2.0 at your option, matching the rest of
the `vyre` workspace.
