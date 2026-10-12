//! KernelDescriptor-to-WGPU binding metadata.
//!
//! This module owns compile-time binding reflection: converting lowered
//! `KernelDescriptor` slots into stable `BufferBindingInfo`, bind-group layout
//! fingerprints, live WGPU bind-group layouts, and trap sidecar tags. The
//! parent `pipeline` module orchestrates compilation and dispatch only.

use std::sync::Arc;

use rustc_hash::FxHashSet;
use vyre_driver::{BackendError, BackendLayoutClass, BackendLayoutFingerprint, BackendLayoutSlot};
use vyre_emit_naga::program::TrapTag;
use vyre_lower::TRAP_SIDECAR_NAME;

use crate::descriptor_mapping::{
    descriptor_bind_group, descriptor_buffer_access, descriptor_memory_kind,
};
use crate::pipeline::element_size_bytes;

/// Metadata for one buffer binding derived from a `Program` at compile time.
#[derive(Clone, Debug)]
pub(crate) struct BufferBindingInfo {
    /// `group N` slot.
    pub group: u32,
    /// `binding slot N` slot.
    pub binding: u32,
    /// Buffer name referenced by IR loads/stores.
    pub name: Arc<str>,
    /// Access mode.
    pub access: vyre_foundation::ir::BufferAccess,
    /// Memory tier.
    pub kind: vyre_foundation::ir::MemoryKind,
    /// Non-binding optimization hints.
    pub hints: vyre_foundation::ir::MemoryHints,
    /// Element type.
    pub element: vyre_foundation::ir::DataType,
    /// Static element count (`0` means runtime-sized).
    pub count: u32,
    /// Whether this binding is returned to the caller after dispatch.
    pub is_output: bool,
    /// Whether this writable binding must preserve caller-supplied initial bytes.
    pub preserve_input_contents: bool,
    /// Backend-owned trap sidecar; not supplied by callers and not returned as
    /// a public output.
    pub internal_trap: bool,
}

pub(crate) fn descriptor_buffer_bindings(
    descriptor: &vyre_lower::KernelDescriptor,
    public_output_bindings: &FxHashSet<u32>,
    explicit_output_bindings: &FxHashSet<u32>,
    pipeline_live_out_bindings: &FxHashSet<u32>,
) -> Result<Vec<BufferBindingInfo>, BackendError> {
    let mut bindings = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(
        &mut bindings,
        descriptor.bindings.slots.len(),
    )
    .map_err(|source| {
            BackendError::new(format!(
                "descriptor buffer binding allocation failed for {} slots: {source}. Fix: split the lowered kernel before WGPU pipeline metadata extraction.",
                descriptor.bindings.slots.len()
            ))
        })?;
    for slot in &descriptor.bindings.slots {
        let Some(group) = descriptor_bind_group(slot.memory_class) else {
            continue;
        };
        let access = descriptor_buffer_access(slot.visibility);
        let internal_trap = slot.name == TRAP_SIDECAR_NAME;
        let is_output = public_output_bindings.contains(&slot.slot) && !internal_trap;
        let explicit_output = explicit_output_bindings.contains(&slot.slot);
        let pipeline_live_out = pipeline_live_out_bindings.contains(&slot.slot);
        let preserve_input_contents = access == vyre_foundation::ir::BufferAccess::ReadWrite
            && !explicit_output
            && !(is_output && pipeline_live_out)
            && !internal_trap;
        bindings.push(BufferBindingInfo {
            group,
            binding: slot.slot,
            name: Arc::from(slot.name.as_str()),
            access,
            kind: descriptor_memory_kind(slot.memory_class),
            hints: vyre_foundation::ir::MemoryHints::default(),
            element: slot.element_type.clone(),
            count: descriptor_element_count(slot.element_count),
            is_output,
            preserve_input_contents,
            internal_trap,
        });
    }
    Ok(bindings)
}

fn descriptor_element_count(element_count: Option<u32>) -> u32 {
    match element_count {
        Some(count) => count,
        None => 0,
    }
}

pub(crate) fn bind_group_layout_fingerprint(
    bindings: &[BufferBindingInfo],
) -> Result<BackendLayoutFingerprint, BackendError> {
    let mut slots = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut slots, bindings.len()).map_err(|source| {
        BackendError::new(format!(
            "bind-group layout fingerprint allocation failed for {} bindings: {source}. Fix: split the lowered kernel before WGPU pipeline metadata extraction.",
            bindings.len()
        ))
    })?;
    for binding in bindings {
        let class = match binding.kind {
            vyre_foundation::ir::MemoryKind::Uniform | vyre_foundation::ir::MemoryKind::Push => {
                BackendLayoutClass::Uniform
            }
            _ => BackendLayoutClass::Storage,
        };
        let read_only = matches!(binding.kind, vyre_foundation::ir::MemoryKind::Readonly)
            || matches!(
                binding.access,
                vyre_foundation::ir::BufferAccess::ReadOnly
                    | vyre_foundation::ir::BufferAccess::Uniform
            );
        slots.push(BackendLayoutSlot {
            group: binding.group,
            binding: binding.binding,
            class,
            read_only,
            element_size: element_size_bytes(&binding.element)?,
        });
    }
    Ok(BackendLayoutFingerprint::new(slots))
}

pub(crate) fn create_bind_group_layouts(
    device: &wgpu::Device,
    buffer_bindings: &[BufferBindingInfo],
    max_group: u32,
) -> Result<Arc<[Arc<wgpu::BindGroupLayout>]>, BackendError> {
    let group_count = max_group.checked_add(1).ok_or_else(|| {
        BackendError::new(
            "bind-group layout count overflowed u32. Fix: lower the maximum bind-group index before WGPU pipeline creation.",
        )
    })?;
    let group_count = usize::try_from(group_count).map_err(|source| {
        BackendError::new(format!(
            "bind-group layout count cannot fit host usize: {source}. Fix: reduce the maximum bind-group index before WGPU pipeline creation."
        ))
    })?;
    let mut layouts: Vec<Arc<wgpu::BindGroupLayout>> = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut layouts, group_count).map_err(|source| {
        BackendError::new(format!(
            "bind-group layout vector allocation failed for {group_count} groups: {source}. Fix: split the lowered kernel before WGPU pipeline creation."
        ))
    })?;
    for group_index in 0..=max_group {
        let group_binding_count = buffer_bindings
            .iter()
            .filter(|binding| binding.group == group_index)
            .count();
        let mut entries = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(&mut entries, group_binding_count).map_err(|source| {
            BackendError::new(format!(
                "bind-group layout entry allocation failed for group {group_index} with {group_binding_count} bindings: {source}. Fix: split the lowered kernel before WGPU pipeline creation."
            ))
        })?;
        for binding in buffer_bindings
            .iter()
            .filter(|binding| binding.group == group_index)
        {
            let ty = match binding.kind {
                vyre_foundation::ir::MemoryKind::Uniform
                | vyre_foundation::ir::MemoryKind::Push => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                _ => {
                    let read_only =
                        matches!(binding.kind, vyre_foundation::ir::MemoryKind::Readonly)
                            || matches!(
                                binding.access,
                                vyre_foundation::ir::BufferAccess::ReadOnly
                                    | vyre_foundation::ir::BufferAccess::Uniform
                            );
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    }
                }
            };
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: binding.binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty,
                count: None,
            });
        }
        layouts.push(Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("vyre P-6 bind group layout"),
                entries: &entries,
            },
        )));
    }
    Ok(layouts.into())
}

pub(crate) fn descriptor_trap_tags(
    descriptor: &vyre_lower::KernelDescriptor,
) -> Result<Vec<TrapTag>, BackendError> {
    fn recursive_op_count(body: &vyre_lower::KernelBody) -> Result<usize, BackendError> {
        let mut count = body.ops.len();
        for child in &body.child_bodies {
            count = count.checked_add(recursive_op_count(child)?).ok_or_else(|| {
                BackendError::new(
                    "kernel descriptor recursive op count overflowed usize. Fix: split nested kernel bodies before descriptor metadata extraction.",
                )
            })?;
        }
        Ok(count)
    }

    fn walk(
        body: &vyre_lower::KernelBody,
        seen: &mut FxHashSet<vyre_lower::descriptor::Name>,
        out: &mut Vec<TrapTag>,
    ) -> Result<(), BackendError> {
        for op in &body.ops {
            if let vyre_lower::KernelOpKind::Trap { tag } = &op.kind {
                if seen.insert(tag.clone()) {
                    let code = out
                        .len()
                        .checked_add(1)
                        .and_then(|value| u32::try_from(value).ok())
                        .ok_or_else(|| {
                            BackendError::new(
                                "kernel descriptor trap tag code overflowed u32. Fix: split trap-tag metadata before pipeline creation.",
                            )
                        })?;
                    out.push(TrapTag {
                        code,
                        tag: Arc::from(tag.as_ref()),
                    });
                }
            }
        }
        for child in &body.child_bodies {
            walk(child, seen, out)?;
        }
        Ok(())
    }

    let op_count = recursive_op_count(&descriptor.body)?;
    let mut seen = FxHashSet::default();
    vyre_foundation::allocation::try_reserve_hash_set_to_capacity(&mut seen, op_count).map_err(|source| {
        BackendError::new(format!(
            "trap-tag dedup allocation failed for {op_count} descriptor ops: {source}. Fix: split nested kernel bodies before descriptor metadata extraction."
        ))
    })?;
    let mut out = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut out, op_count).map_err(|source| {
        BackendError::new(format!(
            "trap-tag output allocation failed for {op_count} descriptor ops: {source}. Fix: split nested kernel bodies before descriptor metadata extraction."
        ))
    })?;
    walk(&descriptor.body, &mut seen, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn descriptor_metadata_source_has_no_release_path_panic_or_infallible_capacity() {
        let source = include_str!("descriptor_metadata.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: descriptor metadata production source must precede tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".expect(")
                && !production.contains("Vec::with_capacity")
                && !production.contains("SmallVec::with_capacity")
                && !production.contains("with_capacity_and_hasher"),
            "Fix: WGPU descriptor metadata extraction must reject oversized lowered kernels with BackendError instead of aborting."
        );
        assert!(
            production.contains("try_reserve_vec_to_capacity")
                && production.contains("try_reserve_hash_set_to_capacity")
                && production.contains("Result<Vec<BufferBindingInfo>, BackendError>")
                && production.contains("Result<Arc<[Arc<wgpu::BindGroupLayout>]>, BackendError>")
                && production.contains("Result<Vec<TrapTag>, BackendError>"),
            "Fix: WGPU descriptor metadata allocation and overflow paths must stay fallible at the pipeline boundary."
        );
    }
}
