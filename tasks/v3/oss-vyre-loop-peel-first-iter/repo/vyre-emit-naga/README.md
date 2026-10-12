# vyre-emit-naga

Naga IR emitter for vyre's `KernelDescriptor`.

Consumes a substrate-neutral `vyre_lower::KernelDescriptor` and
produces a `naga::Module` ready to feed wgpu, WGSL, GLSL-out, or any
other naga back-end. Works in tandem with `vyre-emit-spirv` (which
routes through naga to share the lossless lowering).

## Quick start

```rust
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
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

// Recommended path: run optimization first, then lower.
let module = vyre_emit_naga::emit_optimized(&desc).unwrap();
assert_eq!(module.entry_points[0].name, "main");
```

## API

- `emit(&desc) -> Result<naga::Module, EmitError>`: lower exactly as
  given. Use this when you need bytewise determinism on the input
  descriptor (e.g. testing).
- `emit_optimized(&desc) -> Result<naga::Module, EmitError>`:
  recommended. Calls `vyre_lower::rewrites::run_all` first, then
  `emit`. In debug builds, also runs `vyre_lower::verify` on the
  optimized form (`debug_assert!`).
- `emit_optimized_with_stats(&desc) -> Result<(naga::Module, OptimizationStats), EmitError>`:
  same as `emit_optimized` but also returns the optimization stats
  (op count delta, bindings dropped, fixed-point iterations).

## Substrate-specific patterns

`patterns/` houses Naga-specific analyses:

- `vec_pack`: adjacent scalar load/store fusion candidates for
  vec2/vec3/vec4 lowering.
- `push_constant_inline`: small uniform candidates for push-constant
  promotion (avoids a uniform binding).
- `bind_group_reuse`: multi-kernel bind-group sharing.
- `pipeline_prewarm`: pipeline cache hints.

Run `patterns::audit(&desc)` for a unified `NagaAuditReport` covering
all per-kernel patterns. Run `patterns::audit_optimized(&desc)` to
audit the post-`run_all` form: answers "what naga-specific
optimizations remain after the standard rewrite pipeline?".

These complement the substrate-neutral analyses in
`vyre_lower::analyses` (coalesce, bank conflict, etc.): each layer
operates on the layer below.

## Errors

- `EmitError::UnsupportedOp(op)`: the descriptor uses a
  `KernelOpKind` variant this emitter doesn't lower yet (e.g.,
  `SubgroupAdd` outside Vulkan-targeting contexts).
- `EmitError::NagaConstructionFailed(s)`: naga rejected the IR.
- `EmitError::InvalidBinding { slot, reason }`: binding can't be
  represented in Naga (e.g., write-only storage in some address spaces).
- `EmitError::InvalidDescriptor(s)`: the descriptor itself is
  malformed in a way `vyre_lower::verify` would catch: call verify
  first if you want a more granular report.

## See also

- `vyre-lower`  -  IR + rewrite stack + verify.
- `vyre-emit-spirv`  -  routes through this crate to produce SPIR-V
  binaries.
- `vyre-emit-ptx`  -  independent PTX emitter for CUDA.

## License

MIT OR Apache-2.0.
