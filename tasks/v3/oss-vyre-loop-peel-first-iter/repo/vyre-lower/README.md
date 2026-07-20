# vyre-lower

Substrate-neutral lowering, optimization, analysis, and verification
for vyre's `KernelDescriptor` IR.

This crate sits between vyre's frontend (which produces a high-level
`vyre::Program`) and the substrate-specific emitters
(`vyre-emit-naga`, `vyre-emit-ptx`, `vyre-emit-spirv`). It owns:

- The `KernelDescriptor` IR: a flat, SSA-shaped, structured-control-
  flow program that every emitter consumes verbatim.
- 12 rewrite passes that simplify the IR before lowering.
- 11 analyses that report on the IR (coalescing, bank conflicts,
  shared-mem promotion candidates, def-use chains, etc.).
- A structural verifier (`verify`) that catches dangling refs,
  duplicate result-ids, and out-of-range pool/child-body indices.
- Performance instrumentation (`OptimizationStats`).

If you're emitting GPU code, you want `vyre_emit_*::emit_optimized`:
those wrappers call `vyre_lower::rewrites::run_all` for you.

## Quick start

```rust
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    rewrites::{run_all, run_all_with_stats},
    verify,
};
use vyre_foundation::ir::DataType;

let desc = KernelDescriptor {
    id: "store_seven".into(),
    bindings: BindingLayout {
        slots: vec![BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "out".into(),
        }],
    },
    dispatch: Dispatch::new(64, 1, 1),
    body: KernelBody {
        ops: vec![
            KernelOp { kind: KernelOpKind::Literal, operands: vec![0], result: Some(0) },
            KernelOp { kind: KernelOpKind::Literal, operands: vec![1], result: Some(1) },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![0, 0, 1],
                result: None,
            },
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
    },
};

// Run the optimization pipeline.
let (optimized, stats) = run_all_with_stats(&desc);
println!(
    "{} ops -> {} ops in {} iterations",
    stats.ops_before, stats.ops_after, stats.iterations,
);

// Sanity: the optimized form is structurally valid.
assert!(verify(&optimized).is_ok());
```

## The IR

A `KernelDescriptor` is:

- `id: String`: diagnostic.
- `bindings: BindingLayout { slots: Vec<BindingSlot> }`: buffers
  bound at the kernel boundary, looked up by `BindingSlot.slot` field.
- `dispatch: Dispatch { workgroup_size }`: thread-group geometry.
- `body: KernelBody`: the program.

A `KernelBody` is:

- `ops: Vec<KernelOp>`: flat op stream, walked linearly.
- `child_bodies: Vec<KernelBody>`: referenced by `If`/`ForLoop`/
  `Block`/`Region` ops via a child-body index in their operands.
- `literals: Vec<LiteralValue>`: pool, referenced by `Literal` ops.

A `KernelOp` is `{ kind: KernelOpKind, operands: Vec<u32>, result: Option<u32> }`.
Operands are typed by position per `KernelOpKind`: some positions
are SSA result-id refs, some are literal-pool indices, some are
binding slot ids, some are child-body indices.

**Per-body id space.** Each `KernelBody` has its own SSA id space.
Result-ids in a child body do NOT exist in the parent body's id
space. Rewrites that move ops across bodies must respect this: see
the LICM module for the consequence of getting it wrong.

## Rewrite pipeline

`run_all` applies these passes in order, then iterates to fixed
point (up to 4 iterations):

1. **strength_reduce**: `Mul/Div/Mod` by power-of-2 → shift/and.
2. **const_fold**: folds compile-time-constant arithmetic. Coverage:
   - `BinOp(Lit, Lit)` → single `Lit` (full BinOp×Type matrix:
     U32/I32/F32/Bool × all 22 BinOp variants including comparisons,
     bitwise, wrapping arithmetic).
   - `UnOp(Lit)` → single `Lit` (10 unary ops: BitNot, Negate,
     LogicalNot, Popcount, Clz, Ctz, ReverseBits, Abs, Floor, Ceil,
     Round, Trunc, Sqrt, Cos, Sin).
   - `Cast(Lit)` → typed `Lit` (int↔int, int↔float, bool→int,
     same-type; float→int only when finite + in range).
   - `Fma(Lit, Lit, Lit)` → single `Lit` (F32 only, finite-result).
3. **identity_elim**: `Add(x, 0)`, `Mul(x, 1)`, etc. → `x`;
   `Mul(x, 0)`, `BitAnd(x, 0)` → `0`; `Select(Lit_bool, then, else)` →
   then or else (bool-cond folding).
4. **branch_collapse**: `If(Lit_bool, ...)` → selected arm inlined.
5. **loop_unroll**: small constant-bound loops (≤ 4 iterations).
6. **licm**: currently a no-op (see module docs).
7. **load_forwarding**: store-to-load and load-to-load forwarding
   with per-slot aliasing rules.
8. **dce**: drops result-producing ops with no users.
9. **dead_store**: drops stores whose value is overwritten before
   any observation.
10. **dce** (again): cleans up what dead_store orphaned.
11. **canonicalize**: sorts commutative-op operands so CSE catches
    `Add(a, b) == Add(b, a)`.
12. **cse**: merges structurally-equivalent ops.
13. **drop_unused_bindings**: strips binding slots no surviving op
    references.
14. **drop_unused_literals**: strips pool entries no Literal op
    references (const_fold + identity_elim leave plenty of orphans).
15. **drop_unused_child_bodies**: strips child bodies orphaned by
    branch_collapse / loop_unroll inlining.

Each pass is total (no `Result`, returns input on no-op), preserves
semantic equivalence, and is individually idempotent. The wrapping
fixed-point loop catches inter-pass dependencies (e.g., CSE merging
two index ops exposes a dead_store opportunity that dead_store
missed in the first pass).

## Analyses

11 substrate-neutral analyses in `vyre_lower::analyses`:

- `coalesce`: memory-access coalescence per warp/workgroup.
- `shared_mem_promote`: global → shared-memory tile candidates.
- `bank_conflict`: shared-memory bank conflict detection.
- `vec_pack`: adjacent-load vectorization candidates (companion
  to `vyre_emit_naga::patterns::vec_pack`).
- `workgroup_uniform`: values uniform across a workgroup.
- `texture_promote`: read-mostly buffer → texture candidates.
- `layout_aos_to_soa`: AoS-to-SoA layout transform candidates.
- `const_buffer_promote`: uniform-buffer promotion candidates.
- `dead_op`: result-producing ops with no users (a less efficient
  cousin of `def_use::dead_by_no_use`).
- `common_subexpr`: equivalence groups for CSE.
- `def_use`: full def-use chains with per-body `UseSite`s.

Each returns a serializable report. Run `audit::audit(desc)` for a
unified `PerfAuditReport` with prioritized recommendations, or
`audit::audit_optimized(desc)` to audit the post-`run_all` form
(answers "what perf issues remain after the standard pipeline?").
The same `audit` + `audit_optimized` pair is mirrored in
`vyre_emit_naga::patterns`, `vyre_emit_ptx::patterns`, and
`vyre_emit_spirv::patterns` for substrate-specific concerns.

## Verifier

`verify(desc) -> Result<(), Vec<VerifyError>>` checks:

- Result-id uniqueness within each body.
- No dangling result-id refs.
- Literal-pool indices in range.
- Child-body indices in range.
- `Literal` ops have ≥1 operand.
- Per-kind minimum operand counts.

Errors are collected (not short-circuited) so a single call surfaces
every violation. Both `vyre_emit_*::emit_optimized` functions
`debug_assert!(verify(optimized).is_ok())`: production builds skip
the check; debug/test builds catch any rewrite bug at the boundary
with a clean panic message.

The 1000-descriptor fuzz harness at `tests/rewrite_soundness_fuzz.rs`
is the regression gate: every shape in the corpus must produce a
descriptor that verifies after `run_all`.

## See also

- `vyre-emit-naga` / `vyre-emit-ptx` / `vyre-emit-spirv`: substrate
  emitters that consume `KernelDescriptor`. Each exposes `emit` and
  `emit_optimized`.
- `vyre-foundation`: IR primitives (`BinOp`, `UnOp`, `DataType`,
  `MemoryOrdering`) that `KernelOpKind` embeds.

## License

MIT OR Apache-2.0.
