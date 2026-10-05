# Typed query engine

This original OCaml fixture represents an internal expression DSL. Expr uses
typed de Bruijn indices: an environment is a nested pair of values ending in
unit for closed terms. Eval specifies eager, left-to-right evaluation and
observable Read calls. Measure is diagnostic. Optimizer is the missing layer.

The public smoke test exercises the interpreter, sharing, and shadowing.
Run `dune runtest`. Implement the optimizer according to the issue. Preserve
all supplied interfaces and callers. No dependencies beyond the pinned compiler
and Dune are required. The full compiler/toolchain is available offline.
