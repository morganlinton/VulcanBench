use super::*;

#[test]
fn emit_fuses_four_adjacent_u32_loads_to_ptx_vector_load() {
    let s = emit(&two_slot_u32_kernel(
        "vec_load",
        vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(1),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 0],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 3],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![3, 1],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 5],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 1],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 7],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 8],
                result: None,
            },
        ],
        vec![LiteralValue::U32(0), LiteralValue::U32(1)],
    ))
    .unwrap();
    assert!(s.contains("ld.global.nc.v4.u32"));
    assert_eq!(s.matches("ld.global.u32").count(), 0);
    assert!(
        !s.contains("add.u32"),
        "fused vector load must not leave dead scalar index-increment adds:\n{s}"
    );
}

#[test]
fn emit_fuses_four_adjacent_load_constant_ops_to_ptx_vector_load() {
    // Reproduces the post-`const_buffer_promote` shape: that rewrite turns a
    // read-only-global buffer's `LoadGlobal` ops into `LoadConstant` and flips
    // the binding to `MemoryClass::Constant` BEFORE PTX emission. This backend
    // has no `.const` state-space path — `load_space_for` maps Constant to the
    // plain `"global"` space — so the four consecutive `LoadConstant` ops MUST
    // still fuse to one `ld.global.v4.u32`. Before the fix, the emit-side
    // `is_vector_load_op` excluded `LoadConstant`, silently emitting 4× scalar
    // `ld.global.u32` (a 4× memory-transaction Law-7 pessimization on exactly
    // the buffers the promote pass targets).
    let desc = KernelDescriptor {
        id: "vec_load_constant".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(16),
                    memory_class: MemoryClass::Constant,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(16),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "output".into(),
                },
            ],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::LoadConstant,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::LoadConstant,
                    operands: vec![0, 3],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::LoadConstant,
                    operands: vec![0, 5],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![5, 1],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::LoadConstant,
                    operands: vec![0, 7],
                    result: Some(8),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 4],
                    result: Some(9),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![9, 6],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![10, 8],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![1, 0, 11],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    };
    let s = emit(&desc).unwrap();
    assert!(
        s.contains("ld.global.v4.u32"),
        "four consecutive LoadConstant ops must fuse to one ld.global.v4.u32\n{s}"
    );
    assert_eq!(
        s.matches("ld.global.u32").count(),
        0,
        "fused vector load must not leave scalar ld.global.u32 behind\n{s}"
    );
    assert!(
        s.contains("st.global.u32"),
        "result store must remain\n{s}"
    );
}

#[test]
fn generated_dynamic_reassociated_load_indices_fuse_to_v4() {
    for seed in 0..1024 {
        let s = emit(&dynamic_reassociated_vector_load_kernel(seed))
            .unwrap_or_else(|error| panic!("seed {seed} failed to emit: {error}"));
        assert!(
            s.contains("ld.global.nc.v4.u32"),
            "seed {seed} must recover v4 load fusion after affine reassociation:\n{s}"
        );
        assert_eq!(
            s.matches("ld.global.u32").count() + s.matches("ld.global.nc.u32").count(),
            0,
            "seed {seed} must not leave scalar data loads after v4 load fusion:\n{s}"
        );
    }
}

#[test]
fn misaligned_scalar_gather_values_do_not_fuse_to_vector_store() {
    let s = emit(&dynamic_misaligned_gather_to_vector_store_kernel()).unwrap();
    assert!(
        !s.contains("ld.global.nc.v4.u32") && !s.contains("ld.global.v4.u32"),
        "Fix: misaligned dynamic gather must stay scalar on the load side.\n{s}"
    );
    assert!(
        !s.contains("st.global.v2.u32") && !s.contains("st.global.v4.u32"),
        "Fix: values produced by a scalarized misaligned gather must not be repacked into a vector store on live CUDA.\n{s}"
    );
    assert!(
        s.matches("ld.global.u32").count() + s.matches("ld.global.nc.u32").count() >= 4,
        "Fix: misaligned gather fixture must emit scalar loads.\n{s}"
    );
    assert!(
        s.matches("st.global.u32").count() >= 4,
        "Fix: misaligned gather fixture must emit scalar stores.\n{s}"
    );
}

/// When adjacent `LoadGlobal` ops exist at the same slot but the base index is
/// not provably aligned (runtime value with unknown modulo), `align_vector_chain`
/// returns `None` and the fallback to scalar loads must be announced via a PTX
/// comment. Before this fix, the fallback was silent: the operator had no way to
/// detect that a hot scan kernel was running in scalar mode after vector-fusion
/// analysis completed.
#[test]
fn vector_fusion_alignment_fallback_emits_diagnostic_comment() {
    // Base index: `LocalInvocationId * 3`, which is NOT divisible by 4 for
    // most values (tid % 4 != 0 in general), so alignment cannot be proved
    // for a v4 or v2 load. The emitter must fall back to scalar loads AND emit
    // `// vyre: vector-fusion-skipped` in the PTX text.
    let desc = KernelDescriptor {
        id: "unaligned_fusion_fallback".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(1),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "output".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // tid = LocalInvocationId[0]
                KernelOp { kind: KernelOpKind::LocalInvocationId, operands: vec![0], result: Some(0) },
                // stride = 3  (not a power of 2 aligned multiplier)
                KernelOp { kind: KernelOpKind::Literal, operands: vec![0], result: Some(1) },
                // base = tid * 3
                KernelOp { kind: KernelOpKind::BinOpKind(BinOp::Mul), operands: vec![0, 1], result: Some(2) },
                // inc1 = 1
                KernelOp { kind: KernelOpKind::Literal, operands: vec![1], result: Some(3) },
                // base+1 = base + 1
                KernelOp { kind: KernelOpKind::BinOpKind(BinOp::Add), operands: vec![2, 3], result: Some(4) },
                // Four adjacent loads at base, base+1, base+2, base+3.
                // Alignment of `tid * 3` mod 4 is unknown → fusion must be skipped.
                KernelOp { kind: KernelOpKind::LoadGlobal, operands: vec![0, 2], result: Some(5) },
                KernelOp { kind: KernelOpKind::LoadGlobal, operands: vec![0, 4], result: Some(6) },
                // Store just one result to keep the kernel non-trivial.
                KernelOp { kind: KernelOpKind::Literal, operands: vec![2], result: Some(7) },
                KernelOp { kind: KernelOpKind::StoreGlobal, operands: vec![1, 7, 5], result: None },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3), LiteralValue::U32(1), LiteralValue::U32(0)],
        },
    };
    let s = emit(&desc).expect("Fix: unaligned adjacent load kernel must emit PTX without error.");
    // Must NOT fuse into a vector load (alignment unknown for tid*3).
    assert!(
        !s.contains("ld.global.nc.v2.u32") && !s.contains("ld.global.nc.v4.u32")
            && !s.contains("ld.global.v2.u32") && !s.contains("ld.global.v4.u32"),
        "Fix: loads at a non-provably-aligned base must NOT be fused into a vector load; \
         got:\n{}", &s[..s.len().min(600)]
    );
    // Must emit the Law-10 diagnostic comment announcing the skipped fusion.
    assert!(
        s.contains("vyre: vector-fusion-skipped"),
        "Fix: when vector fusion is skipped due to unknown alignment, the emitter must \
         write a `// vyre: vector-fusion-skipped` comment in the PTX text so the operator \
         can detect scalar fallback. Got PTX:\n{}", &s[..s.len().min(600)]
    );
}

/// Constant-memory `LoadConstant` ops must NEVER emit the `.nc` (non-coherent
/// read-only cache) suffix — that bypass is semantically distinct from the
/// constant path. But the `.nc` decision is made by `load_space_for`, NOT by
/// vector fusion: `analyze_texture_promote` only flags `MemoryClass::Global` +
/// `ReadOnly` slots, so a `Constant` binding is never a read-only-cache slot and
/// `load_space_for` maps it to the plain `"global"` space (this backend has no
/// `.const` state-space path; constant pointers go through `cvta.to.global`).
///
/// Vectorization is orthogonal to `.nc`: a provably-aligned unit-stride chain of
/// `LoadConstant` ops MUST fuse to a single plain `ld.global.v4.u32`, exactly as
/// the equivalent `LoadGlobal` chain would (same global address space, same
/// coherence — just one 16-byte transaction instead of four 4-byte ones). The
/// old contract over-broadly forbade ALL constant-load fusion to dodge `.nc`,
/// which threw away the 4× memory-transaction win on precisely the read-only
/// buffers `const_buffer_promote` produces (it rewrites read-only-global
/// `LoadGlobal` → `LoadConstant` before emission).
#[test]
fn constant_binding_loads_fuse_to_plain_global_vector_load_never_nc() {
    let desc = KernelDescriptor {
        id: "const_no_vec".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(16),
                    memory_class: MemoryClass::Constant,
                    visibility: BindingVisibility::ReadOnly,
                    name: "const_input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(4),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "output".into(),
                },
            ],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            // Four consecutive LoadConstant ops at indices 0, 1, 2, 3.
            ops: vec![
                KernelOp { kind: KernelOpKind::Literal, operands: vec![0], result: Some(0) },
                KernelOp { kind: KernelOpKind::Literal, operands: vec![1], result: Some(1) },
                KernelOp { kind: KernelOpKind::Literal, operands: vec![2], result: Some(2) },
                KernelOp { kind: KernelOpKind::Literal, operands: vec![3], result: Some(3) },
                KernelOp { kind: KernelOpKind::LoadConstant, operands: vec![0, 0], result: Some(4) },
                KernelOp { kind: KernelOpKind::LoadConstant, operands: vec![0, 1], result: Some(5) },
                KernelOp { kind: KernelOpKind::LoadConstant, operands: vec![0, 2], result: Some(6) },
                KernelOp { kind: KernelOpKind::LoadConstant, operands: vec![0, 3], result: Some(7) },
                KernelOp { kind: KernelOpKind::StoreGlobal, operands: vec![1, 0, 4], result: None },
                KernelOp { kind: KernelOpKind::StoreGlobal, operands: vec![1, 1, 5], result: None },
                KernelOp { kind: KernelOpKind::StoreGlobal, operands: vec![1, 2, 6], result: None },
                KernelOp { kind: KernelOpKind::StoreGlobal, operands: vec![1, 3, 7], result: None },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
            ],
        },
    };
    let s = emit(&desc).expect("Fix: constant-memory kernel must emit PTX without error.");
    // The real semantic guard: constant loads must NEVER use the `.nc`
    // non-coherent read-only bypass, in scalar OR vector form.
    assert_eq!(
        s.matches("ld.global.nc.").count(),
        0,
        "Fix: LoadConstant ops must NOT emit the .nc non-coherent cache suffix \
         (constant bindings are not read-only-cache slots). PTX:\n{}",
        &s[..s.len().min(600)]
    );
    // The perf contract: a provably-aligned unit-stride constant-load chain MUST
    // fuse to one plain ld.global.v4.u32, leaving zero scalar data loads.
    assert!(
        s.contains("ld.global.v4.u32"),
        "Fix: four aligned unit-stride LoadConstant ops must fuse to one \
         ld.global.v4.u32. PTX:\n{}",
        &s[..s.len().min(600)]
    );
    assert_eq!(
        s.matches("ld.global.u32").count(),
        0,
        "Fix: fused constant vector load must not leave scalar ld.global.u32 \
         behind. PTX:\n{}",
        &s[..s.len().min(600)]
    );
}

