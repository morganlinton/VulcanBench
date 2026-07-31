use super::*;
use crate::reg::{PtxType, Reg};
use vyre_foundation::ir::{BinOp, DataType, UnOp};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MatrixMmaElement, MatrixMmaLayout, MatrixMmaShape,
    MemoryClass,
};

fn one_store_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "store_one".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
            }],
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        },
    }
}

fn two_slot_u32_kernel(
    id: &str,
    ops: Vec<KernelOp>,
    literals: Vec<LiteralValue>,
) -> KernelDescriptor {
    KernelDescriptor {
        id: id.into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(16),
                    memory_class: MemoryClass::Global,
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
            ops,
            child_bodies: vec![],
            literals,
        },
    }
}

fn empty_child_body() -> KernelBody {
    KernelBody {
        ops: vec![],
        child_bodies: vec![],
        literals: vec![],
    }
}

mod async_ops;
mod atomics;
mod barrier;
mod control_flow;
mod data_tensor;
mod memory_vector;
mod optimized;
mod preamble;
mod scalar_ops;
mod subgroup;
mod types_registers;
