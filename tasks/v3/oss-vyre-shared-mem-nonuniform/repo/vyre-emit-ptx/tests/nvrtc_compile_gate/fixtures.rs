//! PTX fixture builders for the CUDA NVRTC compile/execute gate.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn rw_slot(id: u32, name: &str) -> BindingSlot {
    rw_slot_typed(id, name, DataType::U32)
}

fn rw_slot_typed(id: u32, name: &str, element_type: DataType) -> BindingSlot {
    slot_typed(id, name, element_type, BindingVisibility::ReadWrite)
}

fn slot_typed(
    id: u32,
    name: &str,
    element_type: DataType,
    visibility: BindingVisibility,
) -> BindingSlot {
    BindingSlot {
        slot: id,
        element_type,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility,
        name: name.into(),
    }
}

mod scalar_op_fixtures;
mod vector_load_fixtures;
mod vector_store_fixtures;

pub(crate) use scalar_op_fixtures::*;
pub(crate) use vector_load_fixtures::*;
pub(crate) use vector_store_fixtures::*;
