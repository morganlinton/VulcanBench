use vyre_lower::{BindingLayout, KernelBody, KernelDescriptor, MemoryClass};

pub(super) fn estimated_module_text_capacity(desc: &KernelDescriptor) -> usize {
    512usize
        .saturating_add(desc.bindings.slots.len().saturating_mul(96))
        .saturating_add(estimate_body_text_capacity(&desc.body, &desc.bindings))
}

pub(super) fn estimate_body_text_capacity(body: &KernelBody, bindings: &BindingLayout) -> usize {
    let op_count = body_op_count_recursive(body);
    let shared_bytes = bindings
        .slots
        .iter()
        .filter(|binding| matches!(binding.memory_class, MemoryClass::Shared))
        .count()
        .saturating_mul(80);
    1536usize
        .saturating_add(shared_bytes)
        .saturating_add(op_count.saturating_mul(144))
}

pub(super) fn body_op_count_recursive(body: &KernelBody) -> usize {
    body.child_bodies
        .iter()
        .fold(body.ops.len(), |count, child| {
            count.saturating_add(body_op_count_recursive(child))
        })
}
