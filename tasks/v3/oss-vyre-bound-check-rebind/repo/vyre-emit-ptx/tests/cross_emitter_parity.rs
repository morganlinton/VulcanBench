//! Cross-emitter parity between `vyre-emit-ptx` and `vyre-emit-naga`.
//!
//! Every descriptor in the matrix must lower through both emitters.
//! Failure in either means the substrate-neutral promise of
//! `KernelDescriptor` is broken for that shape.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn rw_slot(id: u32, name: &str) -> BindingSlot {
    rw_slot_typed(id, name, DataType::U32)
}

fn rw_slot_typed(id: u32, name: &str, element_type: DataType) -> BindingSlot {
    BindingSlot {
        slot: id,
        element_type,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: name.into(),
    }
}

fn add_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "add_store".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot(0, "out")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // Use LocalInvocationId so the op survives constant folding.
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                }, // 7
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                }, // 0 (idx)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7), LiteralValue::U32(0)],
        },
    }
}

fn mul_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "mul_store".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot(0, "out")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                }, // 0 (idx)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    }
}

fn fma_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "fma_store".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot_typed(0, "out", DataType::F32)],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::F32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                }, // 7
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(3),
                }, // 0 (idx)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(5),
                }, // 11
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![1, 2, 5],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 3, 4],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::F32(7.0),
                LiteralValue::U32(0),
                LiteralValue::F32(11.0),
            ],
        },
    }
}

fn op_corpus() -> Vec<KernelDescriptor> {
    vec![add_descriptor(), mul_descriptor(), fma_descriptor()]
}

#[test]
fn every_op_lowers_through_ptx_and_naga() {
    for desc in op_corpus() {
        let ptx = vyre_emit_ptx::emit_optimized(&desc)
            .unwrap_or_else(|e| panic!("ptx emit_optimized failed for `{}`: {e:?}", desc.id));
        assert!(
            ptx.contains(".version"),
            "ptx for `{}` must include a version directive",
            desc.id
        );

        let naga = vyre_emit_naga::emit_optimized(&desc)
            .unwrap_or_else(|e| panic!("naga emit_optimized failed for `{}`: {e:?}", desc.id));
        assert!(
            !naga.entry_points.is_empty(),
            "naga module for `{}` must expose an entry point",
            desc.id
        );
    }
}

#[test]
fn ptx_contains_expected_instruction_for_each_op() {
    let cases = vec![
        ("add_store", "add"),
        ("mul_store", "mul.lo"),
        ("fma_store", "fma.rn"),
    ];
    for (id, instr) in cases {
        let desc = op_corpus().into_iter().find(|d| d.id == id).unwrap();
        let ptx = vyre_emit_ptx::emit_optimized(&desc).unwrap();
        assert!(
            ptx.contains(instr),
            "PTX for `{}` missing expected instruction `{}`",
            id,
            instr
        );
    }
}

#[test]
fn naga_and_ptx_entry_points_share_workgroup_size() {
    for desc in op_corpus() {
        let naga_module = vyre_emit_naga::emit_optimized(&desc).unwrap();
        let ptx = vyre_emit_ptx::emit_optimized(&desc).unwrap();

        let entry = &naga_module.entry_points[0];
        assert_eq!(entry.workgroup_size, desc.dispatch.workgroup_size);

        assert!(
            ptx.contains(".entry"),
            "PTX for `{}` missing .entry",
            desc.id
        );
    }
}

#[test]
fn ptx_audit_carries_kernel_id() {
    use vyre_emit_ptx::ComputeCapability;
    for desc in op_corpus() {
        let report = vyre_emit_ptx::patterns::audit(&desc, ComputeCapability::SM_80);
        assert_eq!(report.kernel_id, desc.id);
    }
}

#[test]
fn optimized_and_raw_ptx_succeed_together() {
    for desc in op_corpus() {
        let raw = vyre_emit_ptx::emit(&desc);
        let opt = vyre_emit_ptx::emit_optimized(&desc);
        assert_eq!(
            raw.is_ok(),
            opt.is_ok(),
            "ptx divergence on `{}`: raw={:?}, opt={:?}",
            desc.id,
            raw.err(),
            opt.err(),
        );
    }
}

#[test]
fn ptx_output_contains_required_directives() {
    for desc in op_corpus() {
        let ptx = vyre_emit_ptx::emit_optimized(&desc).unwrap();
        assert!(
            ptx.contains(".version"),
            "PTX for `{}` missing .version",
            desc.id
        );
        assert!(
            ptx.contains(".target"),
            "PTX for `{}` missing .target",
            desc.id
        );
        assert!(
            ptx.contains(".address_size"),
            "PTX for `{}` missing .address_size",
            desc.id
        );
    }
}
