//! Shared descriptor-slot mapping for WGPU emission and pipeline metadata.
//!
//! `KernelDescriptor` lowering feeds both the offline emitter and the live WGPU
//! pipeline builder. Keeping bind-group, memory-kind, and access mapping here
//! prevents the two paths from drifting under new memory classes.

/// Return the WGPU bind group used for a lowered memory class.
pub(crate) fn descriptor_bind_group(memory_class: vyre_lower::MemoryClass) -> Option<u32> {
    match memory_class {
        vyre_lower::MemoryClass::Shared | vyre_lower::MemoryClass::Scratch => None,
        vyre_lower::MemoryClass::Uniform => Some(1),
        vyre_lower::MemoryClass::Global | vyre_lower::MemoryClass::Constant => Some(0),
    }
}

/// Convert lowered binding visibility into core IR buffer access.
pub(crate) fn descriptor_buffer_access(
    visibility: vyre_lower::BindingVisibility,
) -> vyre_foundation::ir::BufferAccess {
    match visibility {
        vyre_lower::BindingVisibility::ReadOnly => vyre_foundation::ir::BufferAccess::ReadOnly,
        vyre_lower::BindingVisibility::WriteOnly => vyre_foundation::ir::BufferAccess::WriteOnly,
        vyre_lower::BindingVisibility::ReadWrite => vyre_foundation::ir::BufferAccess::ReadWrite,
    }
}

/// Convert lowered memory classes into core IR memory tiers.
pub(crate) fn descriptor_memory_kind(
    memory_class: vyre_lower::MemoryClass,
) -> vyre_foundation::ir::MemoryKind {
    match memory_class {
        vyre_lower::MemoryClass::Shared => vyre_foundation::ir::MemoryKind::Shared,
        vyre_lower::MemoryClass::Constant => vyre_foundation::ir::MemoryKind::Readonly,
        vyre_lower::MemoryClass::Uniform => vyre_foundation::ir::MemoryKind::Uniform,
        vyre_lower::MemoryClass::Global => vyre_foundation::ir::MemoryKind::Global,
        vyre_lower::MemoryClass::Scratch => vyre_foundation::ir::MemoryKind::Local,
    }
}
