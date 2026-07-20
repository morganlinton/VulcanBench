use super::*;
use vyre_foundation::ir::MemoryOrdering;

fn barrier_kernel(ordering: MemoryOrdering) -> KernelDescriptor {
    KernelDescriptor {
        id: "barrier_scope".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Barrier { ordering },
                operands: vec![],
                result: None,
            }],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

#[test]
fn workgroup_barrier_emits_cta_barrier() {
    let ptx = emit(&barrier_kernel(MemoryOrdering::SeqCst))
        .expect("Fix: workgroup-scope barriers must remain PTX-emittable.");

    assert!(
        ptx.contains("bar.sync 0;"),
        "Fix: workgroup-scope barrier lowering must emit a CTA barrier."
    );
}

#[test]
fn grid_sync_barrier_is_not_silently_downgraded_to_cta_barrier() {
    match emit(&barrier_kernel(MemoryOrdering::GridSync)) {
        Err(EmitError::InvalidDescriptor(message)) => {
            assert!(
                message.contains("GridSync") && message.contains("bar.sync 0"),
                "Fix: GridSync rejection must name the semantic scope loss; got: {message}"
            );
        }
        Ok(ptx) => panic!(
            "Fix: PTX emitter silently accepted GridSync; this would downgrade cross-grid synchronization to CTA scope. PTX:\n{ptx}"
        ),
        Err(other) => panic!(
            "Fix: GridSync PTX rejection must be an actionable InvalidDescriptor, not {other:?}."
        ),
    }
}

#[test]
fn nested_barrier_kernel_keeps_lanes_live_and_predicates_global_store() {
    let kernel = KernelDescriptor {
        id: "nested_barrier_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(256, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::StructuredBlock,
                    operands: vec![0],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Barrier {
                        ordering: MemoryOrdering::SeqCst,
                    },
                    operands: vec![],
                    result: None,
                }],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(7)],
        },
    };

    let ptx = emit(&kernel)
        .expect("Fix: nested workgroup barriers with global stores must remain PTX-emittable.");

    assert!(
        ptx.contains("Full-workgroup entry")
            && !ptx.contains("setp.ge.u32     %p0, %r3, %r26;")
            && !ptx.contains("@%p0 bra $L_exit;"),
        "barrier kernels must not exit lanes before all lanes reach shared/barrier code:\n{ptx}"
    );
    assert!(
        ptx.lines()
            .any(|line| line.contains("st.global.u32") && line.trim_start().starts_with("@%p")),
        "global stores in full-workgroup-entry kernels must be bounds-predicated:\n{ptx}"
    );
    assert!(
        ptx.contains("bar.sync 0;"),
        "nested barrier body must still lower to a CTA barrier:\n{ptx}"
    );
}
