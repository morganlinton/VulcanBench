//! Hostile `KernelDescriptor` corpus shared by `vyre-emit-*` adversarial
//! matrix tests.
//!
//! Each case is a real program shape (not empty-kernel smoke) with a stable
//! id, family tag, and expected emit outcome. Backend tests assert on
//! lowered artifact structure rather than `is_ok()` alone.

use std::sync::Arc;

use vyre_foundation::ir::{AtomicOp, BinOp, DataType, UnOp};
use vyre_foundation::runtime::memory_model::MemoryOrdering;

use crate::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

/// Stable family tag for matrix assertions in each emit crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmitAdversarialFamily {
    DeepIfElse,
    HostileWorkgroup,
    MultiBinding,
    SharedGlobalTile,
    LoopWithBarrier,
    AtomicCounter,
    DeadIdentityChain,
    VecLoadFusion,
    SignedBufferArithmetic,
    RejectCall,
    RejectGridSyncBarrier,
}

/// Whether emitters should accept or reject the descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitOutcome {
    Success,
    Reject,
}

/// Backends that must consume this corpus in their own emit/descriptor tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmitAdversarialBackend {
    Naga,
    Metal,
    Ptx,
    Spirv,
    Wgpu,
    Cuda,
}

pub const REQUIRED_BACKENDS: [EmitAdversarialBackend; 6] = [
    EmitAdversarialBackend::Naga,
    EmitAdversarialBackend::Metal,
    EmitAdversarialBackend::Ptx,
    EmitAdversarialBackend::Spirv,
    EmitAdversarialBackend::Wgpu,
    EmitAdversarialBackend::Cuda,
];

#[must_use]
pub fn required_backends() -> &'static [EmitAdversarialBackend] {
    &REQUIRED_BACKENDS
}

/// One adversarial emit program case.
#[derive(Debug, Clone)]
pub struct EmitAdversarialCase {
    pub id: &'static str,
    pub family: EmitAdversarialFamily,
    pub descriptor: KernelDescriptor,
    pub outcome: EmitOutcome,
}

fn slot(
    slot: u32,
    name: &str,
    element_type: DataType,
    memory_class: MemoryClass,
    visibility: BindingVisibility,
    count: Option<u32>,
) -> BindingSlot {
    BindingSlot {
        slot,
        element_type,
        element_count: count,
        memory_class,
        visibility,
        name: name.into(),
    }
}

fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
    KernelOp {
        kind,
        operands,
        result,
    }
}

fn lit(pool: u32, result: u32) -> KernelOp {
    op(KernelOpKind::Literal, vec![pool], Some(result))
}

fn local_x(result: u32) -> KernelOp {
    op(KernelOpKind::LocalInvocationId, vec![0], Some(result))
}

fn deep_if_else() -> EmitAdversarialCase {
    let inner_then = KernelBody {
        ops: vec![
            lit(0, 10),
            lit(1, 11),
            op(KernelOpKind::StoreGlobal, vec![0, 11, 10], None),
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(7), LiteralValue::U32(0)],
    };
    let inner_else = KernelBody {
        ops: vec![
            lit(0, 12),
            lit(1, 13),
            op(KernelOpKind::StoreGlobal, vec![0, 13, 12], None),
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(13), LiteralValue::U32(0)],
    };
    let outer_then = KernelBody {
        ops: vec![
            lit(0, 1),
            op(KernelOpKind::StructuredIfThenElse, vec![1, 0, 1], None),
        ],
        child_bodies: vec![inner_then, inner_else],
        literals: vec![LiteralValue::Bool(true)],
    };
    let outer_else = KernelBody {
        ops: vec![
            lit(0, 20),
            lit(1, 21),
            op(KernelOpKind::StoreGlobal, vec![0, 21, 20], None),
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(42), LiteralValue::U32(1)],
    };
    EmitAdversarialCase {
        id: "adv_deep_if_else",
        family: EmitAdversarialFamily::DeepIfElse,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_deep_if_else".into(),
            bindings: BindingLayout {
                slots: vec![slot(
                    0,
                    "out",
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadWrite,
                    Some(64),
                )],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    op(KernelOpKind::StructuredIfThenElse, vec![0, 0, 1], None),
                ],
                child_bodies: vec![outer_then, outer_else],
                literals: vec![LiteralValue::Bool(false)],
            },
        },
    }
}

fn hostile_workgroup_1024() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_hostile_wg_1024",
        family: EmitAdversarialFamily::HostileWorkgroup,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_hostile_wg_1024".into(),
            bindings: BindingLayout {
                slots: vec![slot(
                    0,
                    "out",
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadWrite,
                    Some(1024),
                )],
            },
            dispatch: Dispatch::new(1024, 1, 1),
            body: KernelBody {
                ops: vec![
                    local_x(0),
                    lit(0, 1),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 1], Some(2)),
                    op(KernelOpKind::StoreGlobal, vec![0, 0, 2], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1)],
            },
        },
    }
}

fn multi_binding_mixed() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_multi_binding",
        family: EmitAdversarialFamily::MultiBinding,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_multi_binding".into(),
            bindings: BindingLayout {
                slots: vec![
                    slot(
                        0,
                        "u32_buf",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadWrite,
                        Some(128),
                    ),
                    slot(
                        1,
                        "f32_buf",
                        DataType::F32,
                        MemoryClass::Global,
                        BindingVisibility::ReadWrite,
                        Some(128),
                    ),
                    slot(
                        2,
                        "const_u32",
                        DataType::U32,
                        MemoryClass::Constant,
                        BindingVisibility::ReadOnly,
                        Some(16),
                    ),
                ],
            },
            dispatch: Dispatch::new(128, 1, 1),
            body: KernelBody {
                ops: vec![
                    local_x(0),
                    lit(0, 1),
                    op(KernelOpKind::LoadGlobal, vec![2, 1], Some(2)),
                    op(KernelOpKind::LoadGlobal, vec![0, 1], Some(3)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![2, 3], Some(4)),
                    op(KernelOpKind::StoreGlobal, vec![0, 1, 4], None),
                    op(KernelOpKind::LoadGlobal, vec![1, 1], Some(5)),
                    op(
                        KernelOpKind::Cast {
                            target: DataType::F32,
                        },
                        vec![4],
                        Some(6),
                    ),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![5, 6], Some(7)),
                    op(KernelOpKind::StoreGlobal, vec![1, 1, 7], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        },
    }
}

fn shared_global_tile() -> EmitAdversarialCase {
    let shared_slot = crate::lower::WORKGROUP_SLOT_BASE;
    EmitAdversarialCase {
        id: "adv_shared_global_tile",
        family: EmitAdversarialFamily::SharedGlobalTile,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_shared_global_tile".into(),
            bindings: BindingLayout {
                slots: vec![
                    slot(
                        0,
                        "global_in",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadOnly,
                        Some(256),
                    ),
                    slot(
                        1,
                        "global_out",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadWrite,
                        Some(256),
                    ),
                    slot(
                        shared_slot,
                        "tile",
                        DataType::U32,
                        MemoryClass::Shared,
                        BindingVisibility::ReadWrite,
                        Some(256),
                    ),
                ],
            },
            dispatch: Dispatch::new(256, 1, 1),
            body: KernelBody {
                ops: vec![
                    local_x(0),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                    op(KernelOpKind::StoreShared, vec![shared_slot, 0, 1], None),
                    op(
                        KernelOpKind::Barrier {
                            ordering: MemoryOrdering::SeqCst,
                        },
                        vec![],
                        None,
                    ),
                    op(KernelOpKind::LoadShared, vec![shared_slot, 0], Some(2)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![2, 1], Some(3)),
                    op(KernelOpKind::StoreGlobal, vec![1, 0, 3], None),
                ],
                child_bodies: vec![],
                literals: vec![],
            },
        },
    }
}

fn loop_with_barrier() -> EmitAdversarialCase {
    let loop_body = KernelBody {
        ops: vec![
            op(
                KernelOpKind::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                },
                vec![],
                None,
            ),
            local_x(10),
            lit(0, 11),
            op(KernelOpKind::StoreGlobal, vec![0, 10, 11], None),
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(7)],
    };
    EmitAdversarialCase {
        id: "adv_loop_barrier",
        family: EmitAdversarialFamily::LoopWithBarrier,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_loop_barrier".into(),
            bindings: BindingLayout {
                slots: vec![slot(
                    0,
                    "out",
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadWrite,
                    Some(8),
                )],
            },
            dispatch: Dispatch::new(8, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    lit(1, 1),
                    op(
                        KernelOpKind::StructuredForLoop {
                            loop_var: Arc::from("i"),
                        },
                        vec![0, 1, 0],
                        None,
                    ),
                ],
                child_bodies: vec![loop_body],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
            },
        },
    }
}

fn atomic_counter() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_atomic_counter",
        family: EmitAdversarialFamily::AtomicCounter,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_atomic_counter".into(),
            bindings: BindingLayout {
                slots: vec![slot(
                    0,
                    "counter",
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadWrite,
                    Some(1),
                )],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    lit(1, 1),
                    op(
                        KernelOpKind::Atomic {
                            op: AtomicOp::Add,
                            ordering: MemoryOrdering::SeqCst,
                        },
                        vec![0, 0, 1],
                        None,
                    ),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        },
    }
}

fn dead_identity_chain() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_dead_identity",
        family: EmitAdversarialFamily::DeadIdentityChain,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_dead_identity".into(),
            bindings: BindingLayout {
                slots: vec![slot(
                    0,
                    "out",
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadWrite,
                    Some(1),
                )],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    lit(1, 1),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![1, 0], Some(2)),
                    op(KernelOpKind::BinOpKind(BinOp::Mul), vec![1, 0], Some(3)),
                    op(KernelOpKind::UnOpKind(UnOp::BitNot), vec![2], Some(4)),
                    op(KernelOpKind::UnOpKind(UnOp::BitNot), vec![4], Some(5)),
                    op(KernelOpKind::StoreGlobal, vec![0, 0, 1], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
            },
        },
    }
}

fn vec_load_fusion() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_vec_load_fusion",
        family: EmitAdversarialFamily::VecLoadFusion,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_vec_load_fusion".into(),
            bindings: BindingLayout {
                slots: vec![
                    slot(
                        0,
                        "input",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadOnly,
                        Some(16),
                    ),
                    slot(
                        1,
                        "output",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadWrite,
                        Some(16),
                    ),
                ],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    lit(1, 1),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 1], Some(3)),
                    op(KernelOpKind::LoadGlobal, vec![0, 3], Some(4)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![3, 1], Some(5)),
                    op(KernelOpKind::LoadGlobal, vec![0, 5], Some(6)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![5, 1], Some(7)),
                    op(KernelOpKind::LoadGlobal, vec![0, 7], Some(8)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![2, 4], Some(9)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![9, 6], Some(10)),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![10, 8], Some(11)),
                    op(KernelOpKind::StoreGlobal, vec![1, 0, 11], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        },
    }
}

fn reject_call() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_reject_call",
        family: EmitAdversarialFamily::RejectCall,
        outcome: EmitOutcome::Reject,
        descriptor: KernelDescriptor {
            id: "adv_reject_call".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![op(
                    KernelOpKind::Call {
                        op_id: Arc::from("vyre.primitives.unknown"),
                    },
                    vec![],
                    None,
                )],
                child_bodies: vec![],
                literals: vec![],
            },
        },
    }
}

fn reject_grid_sync_barrier() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_reject_grid_sync",
        family: EmitAdversarialFamily::RejectGridSyncBarrier,
        outcome: EmitOutcome::Reject,
        descriptor: KernelDescriptor {
            id: "adv_reject_grid_sync".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![op(
                    KernelOpKind::Barrier {
                        ordering: MemoryOrdering::GridSync,
                    },
                    vec![],
                    None,
                )],
                child_bodies: vec![],
                literals: vec![],
            },
        },
    }
}

/// Signed (`i32`) buffer load feeding a mixed-kind arithmetic chain against
/// unsigned literals, result narrowed into a `u32` output buffer.
///
/// This is the shape that hid a whole class of emit-validity bugs: the loaded
/// word is `Sint`, but bit/shift arithmetic against `u32` literals (and the
/// final store into a `u32` buffer) requires operand-kind reconciliation. WGSL
/// rejects `BitAnd(i32, u32)` and a shift whose amount is not `u32`; PTX must
/// pick the signed vs unsigned form. Every backend must reconcile the kinds —
/// validating emitters (naga) catch any regression at the matrix gate.
fn signed_buffer_arithmetic() -> EmitAdversarialCase {
    EmitAdversarialCase {
        id: "adv_signed_buffer_arith",
        family: EmitAdversarialFamily::SignedBufferArithmetic,
        outcome: EmitOutcome::Success,
        descriptor: KernelDescriptor {
            id: "adv_signed_buffer_arith".into(),
            bindings: BindingLayout {
                slots: vec![
                    slot(
                        0,
                        "src",
                        DataType::I32,
                        MemoryClass::Global,
                        BindingVisibility::ReadOnly,
                        Some(4),
                    ),
                    slot(
                        1,
                        "out",
                        DataType::U32,
                        MemoryClass::Global,
                        BindingVisibility::ReadWrite,
                        Some(4),
                    ),
                ],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    lit(0, 0),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                    lit(1, 2),
                    op(KernelOpKind::BinOpKind(BinOp::BitAnd), vec![1, 2], Some(3)),
                    lit(2, 4),
                    op(KernelOpKind::BinOpKind(BinOp::Shr), vec![3, 4], Some(5)),
                    lit(3, 6),
                    op(KernelOpKind::BinOpKind(BinOp::Add), vec![5, 6], Some(7)),
                    op(KernelOpKind::StoreGlobal, vec![1, 0, 7], None),
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(0xff),
                    LiteralValue::U32(3),
                    LiteralValue::U32(1),
                ],
            },
        },
    }
}

/// Full adversarial emit corpus (success + rejection cases).
#[must_use]
pub fn corpus() -> Vec<EmitAdversarialCase> {
    vec![
        deep_if_else(),
        hostile_workgroup_1024(),
        multi_binding_mixed(),
        shared_global_tile(),
        loop_with_barrier(),
        atomic_counter(),
        dead_identity_chain(),
        vec_load_fusion(),
        signed_buffer_arithmetic(),
        reject_call(),
        reject_grid_sync_barrier(),
    ]
}

/// Cases that must lower successfully through every emit backend.
#[must_use]
pub fn success_cases() -> Vec<EmitAdversarialCase> {
    corpus()
        .into_iter()
        .filter(|case| case.outcome == EmitOutcome::Success)
        .collect()
}

/// Cases that must be rejected without panic.
#[must_use]
pub fn rejection_cases() -> Vec<EmitAdversarialCase> {
    corpus()
        .into_iter()
        .filter(|case| case.outcome == EmitOutcome::Reject)
        .collect()
}

/// Lookup a case by stable id (for targeted matrix assertions).
#[must_use]
pub fn case_by_id(id: &str) -> Option<EmitAdversarialCase> {
    corpus().into_iter().find(|case| case.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adversarial_corpus_has_at_least_eight_success_programs() {
        assert!(
            success_cases().len() >= 8,
            "Fix: emit adversarial corpus must include ≥8 hostile success programs."
        );
    }

    #[test]
    fn success_corpus_descriptors_verify() {
        for case in success_cases() {
            let errors = crate::verify(&case.descriptor);
            assert!(
                matches!(errors, Ok(_)),
                "Fix: adversarial case `{}` must verify before emit testing: {:?}",
                case.id,
                errors
            );
        }
    }

    #[test]
    fn required_backend_matrix_names_all_emit_consumers() {
        assert_eq!(
            required_backends(),
            [
                EmitAdversarialBackend::Naga,
                EmitAdversarialBackend::Metal,
                EmitAdversarialBackend::Ptx,
                EmitAdversarialBackend::Spirv,
                EmitAdversarialBackend::Wgpu,
                EmitAdversarialBackend::Cuda,
            ],
            "Fix: adversarial emit corpus must stay wired to every descriptor-emitting backend."
        );
    }

    #[test]
    fn case_ids_are_unique_and_match_descriptor_ids() {
        let mut ids = std::collections::BTreeSet::new();
        for case in corpus() {
            assert!(
                ids.insert(case.id),
                "Fix: duplicate adversarial emit case id `{}`",
                case.id
            );
            assert_eq!(
                case.id, case.descriptor.id,
                "Fix: corpus case id and KernelDescriptor id must match for backend matrix diagnostics."
            );
        }
    }
}
