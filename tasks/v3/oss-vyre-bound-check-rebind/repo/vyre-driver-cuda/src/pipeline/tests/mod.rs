mod cache_core_contracts;
mod cache_pressure_contracts;
mod input_key_contracts;
mod pipeline_contracts;

use std::sync::Arc;

use smallvec::smallvec;
use vyre_driver::binding::{Binding, BindingPlan, BindingRole};
use vyre_driver::replace_output_buffers_preserving_slots;
use vyre_driver::LaunchPlan;

use crate::backend::CudaDispatchPlan;
use crate::synthetic_device_caps::blackwell_sm120_caps;

use super::{
    add_shape_bytes, cuda_compiled_pipeline_identity_key, cuda_graph_lane_count_for_batch,
    materialized_input_key, MaterializedPipelineOutputCache, MaterializedPipelineOutputCacheEntry,
    MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE, MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE,
};

fn single_input_output_plan(byte_len: usize) -> CudaDispatchPlan {
    CudaDispatchPlan {
        bindings: BindingPlan {
            bindings: vec![Binding {
                name: Arc::from("state"),
                binding: 0,
                buffer_index: 0,
                role: BindingRole::InputOutput,
                element_size: 1,
                preferred_alignment: 1,
                element_count: byte_len as u32,
                static_byte_len: Some(byte_len),
                input_index: Some(0),
                output_index: Some(0),
            }],
            input_indices: vec![0],
            output_indices: vec![0],
            shared_indices: vec![],
        },
        output_binding_indices: smallvec![0],
        launch: LaunchPlan {
            grid: [1, 1, 1],
            workgroup: [128, 1, 1],
            element_count: byte_len as u32,
            param_words: vec![1, 2, 3, 4],
            max_binding_alignment: 1,
        },
        cooperative: false,
        fixpoint_iterations: 1,
    }
}

fn generated_pipeline_identity_key(seed: u32, salt: u32) -> [u8; 32] {
    let mut out = [0_u8; 32];
    let mut state = seed ^ salt ^ 0xC0DA_CAFE;
    for (index, byte) in out.iter_mut().enumerate() {
        state = state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .rotate_left((index as u32) & 15);
        *byte = (state >> ((index & 3) * 8)) as u8;
    }
    out
}

fn generated_pipeline_identity_launch(seed: u32) -> LaunchPlan {
    LaunchPlan {
        element_count: 1 + (seed % 4096),
        workgroup: [
            32 + (seed % 8) * 32,
            1 + (seed.rotate_left(3) % 4),
            1 + (seed.rotate_left(5) % 2),
        ],
        grid: [
            1 + (seed % 1024),
            1 + (seed.rotate_left(7) % 16),
            1 + (seed.rotate_left(11) % 8),
        ],
        param_words: Vec::new(),
        max_binding_alignment: std::mem::size_of::<u64>(),
    }
}

