use super::*;
use naga::{Binding, Block, BuiltIn, Statement, TypeInner};
use std::sync::Mutex;
use vyre_foundation::ir::{BinOp, DataType, UnOp};
use vyre_foundation::memory_model::MemoryOrdering;
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

static MODULE_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn empty_desc() -> KernelDescriptor {
    KernelDescriptor {
        id: "empty".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

fn empty_desc_with_workgroup(id: &str, x: u32) -> KernelDescriptor {
    KernelDescriptor {
        id: id.into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(x, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

#[test]
fn module_cache_key_is_128_bit_and_descriptor_sensitive() {
    let a = descriptor_cache_key(&empty_desc_with_workgroup("a", 1));
    let b = descriptor_cache_key(&empty_desc_with_workgroup("a", 2));
    assert_eq!(a.0.len(), 16);
    assert_ne!(a, b);
}

/// Two descriptors whose only difference is the NaN bit-payload of an F32
/// literal must produce DIFFERENT cache keys. With `format!("{desc:?}")` both
/// would format as `"F32(NaN)"` regardless of bit pattern → same hash →
/// spurious hit or spurious miss (depending on which direction the collision
/// fires). The fix: derive the key from the `Hash` impl, which uses
/// `v.to_bits()` for F32 and therefore distinguishes NaN payloads.
#[test]
fn cache_key_distinguishes_nan_bit_patterns() {
    fn desc_with_nan(nan_bits: u32) -> KernelDescriptor {
        KernelDescriptor {
            id: "nan_test".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(f32::from_bits(nan_bits))],
            },
        }
    }
    // Two quiet NaN bit patterns with distinct payloads.
    let quiet_nan_a = 0x7FC0_0001u32; // quiet NaN, payload 1
    let quiet_nan_b = 0x7FC0_0002u32; // quiet NaN, payload 2
    assert!(f32::from_bits(quiet_nan_a).is_nan());
    assert!(f32::from_bits(quiet_nan_b).is_nan());

    let key_a = descriptor_cache_key(&desc_with_nan(quiet_nan_a));
    let key_b = descriptor_cache_key(&desc_with_nan(quiet_nan_b));
    assert_ne!(
        key_a, key_b,
        "descriptors differing only in NaN bit payload must have different cache keys; \
         Debug-based keys would incorrectly collapse both to 'F32(NaN)'"
    );
}

#[test]
fn op_dispatch_route_cache_hits_preserve_uncached_classification() {
    let kinds = [
        KernelOpKind::Literal,
        KernelOpKind::Literal,
        KernelOpKind::BinOpKind(BinOp::Add),
        KernelOpKind::BinOpKind(BinOp::Mul),
        KernelOpKind::UnOpKind(UnOp::BitNot),
        KernelOpKind::UnOpKind(UnOp::Abs),
        KernelOpKind::Cast {
            target: DataType::U32,
        },
        KernelOpKind::Cast {
            target: DataType::I32,
        },
        KernelOpKind::Barrier {
            ordering: MemoryOrdering::SeqCst,
        },
        KernelOpKind::Barrier {
            ordering: MemoryOrdering::Acquire,
        },
        KernelOpKind::LoadGlobal,
        KernelOpKind::LoadGlobal,
    ];
    let (parity, hits) = emitter::op_dispatch_route_cache_probe(&kinds);
    assert!(
        parity,
        "Fix: cached Naga op-dispatch route classification must match uncached classification."
    );
    assert!(
        hits >= 6,
        "Fix: repeated Naga op kinds must hit the dispatch-route cache; observed {hits}."
    );
}

fn u32_output_slot(slot: u32) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: Some(8),
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: format!("out{slot}"),
    }
}

fn trap_sidecar_slot(slot: u32) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: Some(vyre_lower::TRAP_SIDECAR_WORDS),
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: vyre_lower::TRAP_SIDECAR_NAME.to_owned(),
    }
}

fn async_copy_desc(kind: KernelOpKind) -> KernelDescriptor {
    KernelDescriptor {
        id: "async-copy".into(),
        bindings: BindingLayout {
            slots: vec![u32_output_slot(0), u32_output_slot(1)],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(16)],
            child_bodies: vec![],
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
                    kind,
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::AsyncWait { tag: "copy".into() },
                    operands: vec![],
                    result: None,
                },
            ],
        },
    }
}

fn block_has_loop(block: &Block) -> bool {
    block.iter().any(|statement| match statement {
        Statement::Loop { .. } => true,
        Statement::Block(child) => block_has_loop(child),
        Statement::If { accept, reject, .. } => block_has_loop(accept) || block_has_loop(reject),
        _ => false,
    })
}

fn block_has_atomic(block: &Block) -> bool {
    block.iter().any(|statement| match statement {
        Statement::Atomic { .. } => true,
        Statement::Block(child) => block_has_atomic(child),
        Statement::If { accept, reject, .. } => {
            block_has_atomic(accept) || block_has_atomic(reject)
        }
        Statement::Loop {
            body, continuing, ..
        } => block_has_atomic(body) || block_has_atomic(continuing),
        _ => false,
    })
}

mod atomics;
mod binop;
mod byte_element_load;
mod cache_entry;
mod descriptor_control;
mod optimized_errors;
mod subgroup;
