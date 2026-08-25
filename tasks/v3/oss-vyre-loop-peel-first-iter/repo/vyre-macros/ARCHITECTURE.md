# vyre-macros  -  architecture

Procedural macros consumed by `vyre-foundation`. Every macro here
is a build-time code generator.

## Modules

### `lib.rs`
Public macro entry points. Currently:

- `vyre_ast_registry!`  -  defines the `Node` and `Expr` enums plus
  the registry consumers (visitor, validator, wire encoder) walk.
- `vyre_define_op!`  -  declares an op + its CPU reference + its
  registration token in one macro invocation. (See
  `define_op.rs`.)

### `ast_registry.rs`
Implementation of `vyre_ast_registry!`. Reads the macro input
syntax, generates the enum, the visitor traits, the wire-format
tags, and the validator skeleton.

### `define_op.rs`
Implementation of `vyre_define_op!`. Reads the per-op declaration
(name, signature, cpu reference, algebraic-law markers) and
generates the OpEntry registration plus the CPU oracle binding.

## Integration points

- `vyre-foundation` only. Other vyre crates use the generated
  surface, never the macro directly.
- The macro contract is frozen  -  changing it requires a
  re-conform pass across every downstream op.
