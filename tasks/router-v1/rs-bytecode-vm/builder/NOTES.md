# rs-bytecode-vm: builder notes

Rust counterpart of `tasks/v1/py-bytecode-vm`, authored 2026-11-05 for the
Router v1 suite's Rust small-hard cell (no public task fills it).

- Starting repo: `src/isa.rs` is complete (values, AST, instruction set,
  errors); `src/compiler.rs` and `src/vm.rs` are `todo!()` stubs that
  compile. No dependencies, so `cargo test --offline` works in the Rust
  sandbox image.
- Gold: 161-line diff implementing the lowering with backpatched absolute
  forward jumps (`and`/`or` keep the selected operand on the stack with
  `Dup`/`Pop`) and a stack VM with typed errors and a one-value final-stack
  check.
- Hidden tests: `tests/tests/test_vmlang.rs` (11 fail-to-pass, 1
  pass-to-pass `construct`). The nested short-circuit-in-condition and
  flat-ISA tests catch off-by-one jump targets that the simple cases miss.
- Verified locally with cargo 1.87 (brew rustup): gold passes 12/12, the
  stub compiles and every fail-to-pass test panics on `todo!()`.
