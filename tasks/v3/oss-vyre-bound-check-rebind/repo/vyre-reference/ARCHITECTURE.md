# vyre-reference  -  architecture

The CPU oracle. Every IR construct lowered onto a GPU backend has
a corresponding CPU-evaluator branch here. The conform suite uses
these branches as the truth oracle; any GPU output that diverges
is a backend bug.

## Modules

### `eval_node.rs`
Per-`Node` evaluation arm. Walks the entry vector, dispatches per
variant, threads the per-buffer state through.

### `eval_expr.rs`
Per-`Expr` evaluation arm. Pure expression eval against the
running `EvalCtx`.

### `eval_call.rs`
Operation-call evaluator. Looks up the op in the registry and
executes the CPU reference.

### `eval_expr_cast.rs`
`Expr::Cast` arm  -  type-cast semantics for the supported
DataTypes.

### `cpu_op.rs`
The per-op registry surface that downstream crates plug into.
Mirrors `vyre-foundation::cpu_op` but evaluates against an in-
memory state vector.

### `dialect_dispatch.rs`
Routes an op call to the right dialect's CPU reference. Used
when extension dialects ship their own CPU oracles.

### `atomics.rs`
`Expr::Atomic` arm  -  implements all `AtomicOp` variants with
single-thread sequential consistency (the multi-thread path is
the GPU's responsibility, the CPU oracle just defines the answer
for sequenced execution).

### `dual.rs`
Dual-execution helper: runs the same Program through GPU and CPU
in parallel, byte-compares the outputs, surfaces the first
divergence.

### `workgroup.rs` (also under workgroup module)
CPU emulation of workgroup-shared scratch. Bounded execution
with rayon-scheduled per-workgroup fan-out.

### `float_ops.rs`
IEEE-754 float arms. Required for ops like softmax/attention
whose output is float-valued.

## Public types

- **`Reference`**  -  the CPU evaluator entry point.
- **`EvalCtx`**  -  running per-buffer state.
- **`DualRunner`**  -  GPU+CPU parallel runner that emits
  divergence reports.

## Integration points

- The conform runner calls into this for every probe.
- `vyre-aot` uses this as the test oracle when verifying an
  artifact bundle reproduces.
