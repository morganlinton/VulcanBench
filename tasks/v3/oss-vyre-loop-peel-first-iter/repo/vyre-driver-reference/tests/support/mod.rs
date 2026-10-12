use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_reference::CpuRefBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};

pub(crate) fn u32_out_buffer(name: &'static str, binding: u32) -> BufferDecl {
    BufferDecl::storage(name, binding, BufferAccess::ReadWrite, DataType::U32).with_count(1)
}

pub(crate) fn dispatch_no_input(program: &Program) -> Vec<Vec<u8>> {
    dispatch_with_inputs(program, &[])
}

pub(crate) fn dispatch_with_inputs(program: &Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let backend = CpuRefBackend;
    backend
        .dispatch(program, inputs, &DispatchConfig::default())
        .expect("Fix: cpu-ref dispatch must succeed for a valid Program.")
}
