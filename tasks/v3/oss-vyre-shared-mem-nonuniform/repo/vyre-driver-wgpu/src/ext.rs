//! WGSL-specific dispatch helpers for the wgpu backend.
//!
//! Raw WGSL is a property of the wgpu implementation, not the substrate-neutral
//! [`vyre_driver::VyreBackend`] contract.

use std::sync::Arc;

use dashmap::mapref::entry::Entry;

use crate::engine::record_and_readback::{record_and_readback, DispatchLabels, RecordAndReadback};
use crate::pipeline::{BufferBindingInfo, OutputBindingLayout, OutputLayout};
use crate::WgpuBackend;
use vyre_emit_naga::program::bind_group_for;
use vyre_foundation::ir::{BufferAccess, DataType};

impl WgpuBackend {
    /// Dispatch a raw WGSL compute shader.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when shader compilation, staging-buffer
    /// creation, command submission, or readback fails.
    pub fn dispatch_wgsl(
        &self,
        wgsl: &str,
        input: &[u8],
        output_size: usize,
        workgroup_size: u32,
    ) -> Result<Vec<u8>, String> {
        if workgroup_size == 0 {
            return Err("Fix: dispatch_wgsl workgroup_size must be greater than zero.".to_string());
        }
        let device_queue = self.current_device_queue();
        let (device, _queue) = &*device_queue;

        let cache_key = dispatch_wgsl_pipeline_cache_key(wgsl, "main")?;
        let pipeline = if let Some(hit) = self.wgsl_dispatch_pipeline_cache.get(&cache_key) {
            Arc::clone(hit.value())
        } else {
            let compiled = Arc::new(
                crate::runtime::compile_compute_pipeline(
                    device,
                    "vyre backend dispatch_wgsl",
                    wgsl,
                    "main",
                )
                .map_err(|error| error.to_string())?,
            );
            match self.wgsl_dispatch_pipeline_cache.entry(cache_key) {
                Entry::Occupied(hit) => Arc::clone(hit.get()),
                Entry::Vacant(slot) => {
                    slot.insert(Arc::clone(&compiled));
                    compiled
                }
            }
        };

        let output_word_count = output_size
            .checked_add(3)
            .and_then(|n| n.checked_div(4))
            .ok_or_else(|| {
                format!(
                    "Fix: output_size {output_size} overflows WGSL dispatch word-count calculation; split the dispatch into smaller chunks."
                )
            })?
            .max(1);
        let output_bytes = output_word_count.checked_mul(4).ok_or_else(|| {
            format!(
                "Fix: output_word_count {output_word_count} overflows usize bytes; reduce output_size"
            )
        })?;
        let input_len_u32 = u32::try_from(input.len()).map_err(|_| {
            format!(
                "Fix: input length {} exceeds u32 capacity; split the dispatch into u32-sized chunks",
                input.len()
            )
        })?;
        let output_len_u32 = u32::try_from(output_word_count).map_err(|_| {
            format!(
                "Fix: output_word_count {output_word_count} exceeds u32 capacity; reduce output_size"
            )
        })?;
        let params = [input_len_u32, output_len_u32, 0u32, 0u32];
        let params_bytes = bytemuck::try_cast_slice(&params).map_err(|error| {
            vyre_driver::BackendError::new(format!(
                "WGSL dispatch params could not be viewed as bytes: {error}. Fix: keep dispatch parameter buffers aligned to u32."
            ))
            .into_message()
        })?;

        let workgroup_count = u32::try_from(
            output_word_count
            .div_ceil(usize::try_from(workgroup_size).map_err(|error| {
                format!(
                    "Fix: WGSL workgroup_size {workgroup_size} cannot fit usize: {error}; reduce workgroup size."
                )
            })?)
            .max(1),
        )
        .map_err(|_| {
            format!(
                "Fix: WGSL dispatch requires more than u32::MAX workgroups for {output_word_count} output words and workgroup size {workgroup_size}; split the dispatch."
            )
        })?;
        let input_word_count = input.len().div_ceil(4).max(1);
        let input_word_count_u32 = u32::try_from(input_word_count).map_err(|_| {
            format!(
                "Fix: input word count {input_word_count} exceeds u32 capacity; split the dispatch into u32-sized chunks."
            )
        })?;
        let buffer_bindings = [
            BufferBindingInfo {
                internal_trap: false,
                group: bind_group_for(vyre_foundation::ir::MemoryKind::Readonly),
                binding: 0,
                name: Arc::from("input"),
                access: BufferAccess::ReadOnly,
                kind: vyre_foundation::ir::MemoryKind::Readonly,
                hints: vyre_foundation::ir::MemoryHints::default(),
                element: DataType::U32,
                count: input_word_count_u32,
                is_output: false,
                preserve_input_contents: false,
            },
            BufferBindingInfo {
                internal_trap: false,
                group: bind_group_for(vyre_foundation::ir::MemoryKind::Global),
                binding: 1,
                name: Arc::from("output"),
                access: BufferAccess::ReadWrite,
                kind: vyre_foundation::ir::MemoryKind::Global,
                hints: vyre_foundation::ir::MemoryHints::default(),
                element: DataType::U32,
                count: output_len_u32,
                is_output: true,
                preserve_input_contents: false,
            },
            BufferBindingInfo {
                internal_trap: false,
                group: bind_group_for(vyre_foundation::ir::MemoryKind::Uniform),
                binding: 2,
                name: Arc::from("params"),
                access: BufferAccess::Uniform,
                kind: vyre_foundation::ir::MemoryKind::Uniform,
                hints: vyre_foundation::ir::MemoryHints::default(),
                element: DataType::U32,
                count: 4,
                is_output: false,
                preserve_input_contents: false,
            },
        ];
        let max_group: u32 = buffer_bindings.iter().map(|b| b.group).max().unwrap_or(0);
        let bind_group_count = max_group.checked_add(1).ok_or_else(|| {
            "raw WGSL bind-group count overflowed u32. Fix: lower buffer group indices before dispatch."
                .to_string()
        })?;
        let bind_group_capacity = usize::try_from(bind_group_count).map_err(|error| {
            format!(
                "raw WGSL bind-group count {bind_group_count} cannot fit usize: {error}. Fix: lower buffer group indices before dispatch."
            )
        })?;
        let mut bind_group_layouts = Vec::with_capacity(bind_group_capacity);
        bind_group_layouts
            .extend((0..=max_group).map(|g| Arc::new(pipeline.get_bind_group_layout(g))));
        let inputs = [input, params_bytes];
        let output_bindings: Arc<[OutputBindingLayout]> = Arc::from([OutputBindingLayout {
            binding: 1,
            name: Arc::from("output"),
            layout: OutputLayout {
                full_size: output_bytes,
                read_size: output_size,
                copy_offset: 0,
                copy_size: output_bytes,
                trim_start: 0,
            },
            word_count: output_word_count,
        }]);
        let dispatch_arena = self.dispatch_arena_snapshot();
        let outputs = record_and_readback(RecordAndReadback {
            device_queue: &device_queue,
            pool: dispatch_arena.pool(),
            readback_rings: None,
            pipeline: pipeline.as_ref(),
            bind_group_layouts: &bind_group_layouts,
            bind_group_cache: None,
            buffer_bindings: &buffer_bindings,
            inputs: &inputs,
            output_bindings: &output_bindings,
            trap_tags: &[],
            workgroup_count: [workgroup_count, 1, 1],
            indirect: None,
            labels: DispatchLabels {
                bind_group: "vyre backend dispatch_wgsl bind group",
                encoder: "vyre backend dispatch_wgsl",
                compute: "vyre backend dispatch_wgsl compute",
            },
            iterations: 1,
            timestamp_profile: false,
        })
        .map_err(|error| error.into_message())?;

        outputs
            .into_iter()
            .next()
            .ok_or_else(|| "WGSL dispatch produced no output. Fix: declare binding(1) as the output storage buffer.".to_string())
    }
}

fn dispatch_wgsl_pipeline_cache_key(wgsl: &str, entry_point: &str) -> Result<[u8; 32], String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-wgpu.dispatch_wgsl.pipeline.v1");
    hasher.update(
        &crate::numeric::usize_to_u64(entry_point.len(), "dispatch_wgsl entry point length")
            .map_err(|error| error.into_message())?
            .to_le_bytes(),
    );
    hasher.update(entry_point.as_bytes());
    hasher.update(
        &crate::numeric::usize_to_u64(wgsl.len(), "dispatch_wgsl WGSL length")
            .map_err(|error| error.into_message())?
            .to_le_bytes(),
    );
    hasher.update(wgsl.as_bytes());
    Ok(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_wgsl_reuses_backend_pipeline_cache() {
        let Ok(backend) = WgpuBackend::acquire() else {
            panic!("Fix: WGPU dispatch_wgsl cache test requires a live GPU adapter");
        };
        let wgsl = r#"
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(1) @binding(2) var<uniform> params: vec4<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.y) {
        return;
    }
    output[gid.x] = input[gid.x] + 1u;
}
"#;
        let input: Vec<u8> = [41_u32, 99_u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();

        let first = backend
            .dispatch_wgsl(wgsl, &input, 8, 64)
            .expect("Fix: first raw WGSL dispatch must compile and run");
        let second = backend
            .dispatch_wgsl(wgsl, &input, 8, 64)
            .expect("Fix: second raw WGSL dispatch must reuse the cached pipeline and run");

        assert_eq!(first, second);
        assert_eq!(first, [42_u32, 100_u32].as_slice().as_bytes());
        assert_eq!(
            backend.wgsl_dispatch_pipeline_cache.len(),
            1,
            "Fix: identical dispatch_wgsl source must compile once per backend instance"
        );
    }

    trait U32SliceBytes {
        fn as_bytes(&self) -> &[u8];
    }

    impl U32SliceBytes for [u32] {
        fn as_bytes(&self) -> &[u8] {
            bytemuck::cast_slice(self)
        }
    }
}
