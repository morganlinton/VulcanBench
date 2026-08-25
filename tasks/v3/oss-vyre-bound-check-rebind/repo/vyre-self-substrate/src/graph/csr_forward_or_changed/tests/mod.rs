use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::Mutex;
use vyre_foundation::ir::Program;

mod reference_contracts;
mod release_path_contracts;

struct CsrChangedDispatcher {
    outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for CsrChangedDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        if inputs.len() != 7 && inputs.len() != 8 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: csr_forward_or_changed test dispatcher expected 7 legacy inputs or 8 changed-history inputs, got {}.",
                inputs.len()
            )));
        }
        Ok(self.outputs.clone())
    }
}

struct RecordingCsrChangedDispatcher {
    outputs: Vec<Vec<u8>>,
    frontier_inputs: Mutex<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for RecordingCsrChangedDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.frontier_inputs
            .lock()
            .expect("Fix: frontier recording mutex should not be poisoned")
            .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[5]));
        Ok(self.outputs.clone())
    }
}

struct StaticCsrInputRecordingDispatcher {
    outputs: Vec<Vec<u8>>,
    edge_targets: Mutex<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for StaticCsrInputRecordingDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.edge_targets
            .lock()
            .expect("Fix: static input recording mutex should not be poisoned")
            .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[2]));
        Ok(self.outputs.clone())
    }
}

fn linear_graph() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    // 0 -> 1 -> 2 -> 3
    (vec![0, 1, 2, 3, 3], vec![1, 2, 3], vec![1, 1, 1])
}

#[test]
fn gpu_into_decodes_exact_outputs_into_reused_frontier() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut frontier = Vec::with_capacity(4);
    let ptr = frontier.as_ptr();
    forward_closure_via_change_flag_gpu_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut frontier,
    )
    .expect("Fix: dispatch succeeds");
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(frontier.as_ptr(), ptr);
}

#[test]
fn gpu_rejects_extra_outputs() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
            u32_slice_to_le_bytes(&[99]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let err = forward_closure_via_change_flag_gpu(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        4,
    )
    .expect_err("extra outputs must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn gpu_rejects_trailing_changed_bytes() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1111]), vec![0, 0, 0, 0, 1]],
    };
    let (off, tgt, msk) = linear_graph();
    let err = forward_closure_via_change_flag_gpu(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        4,
    )
    .expect_err("trailing changed bytes must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn gpu_rejects_non_boolean_changed_flag() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[2]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let err = forward_closure_via_change_flag_gpu(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        1,
    )
    .expect_err("non-boolean changed flag must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn gpu_rejects_bad_seed_width_without_clobbering_frontier() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("bad seed width must be rejected before dispatch");
        }
    }

    let (off, tgt, msk) = linear_graph();
    let mut scratch = ForwardChangedGpuScratch::default();
    let mut frontier = vec![0xCAFE_BABEu32];
    let capacity = frontier.capacity();

    let err = forward_closure_via_change_flag_gpu_with_scratch_into(
        &NoDispatch,
        4,
        &off,
        &tgt,
        &msk,
        &[],
        0xFFFF_FFFF,
        5,
        &mut scratch,
        &mut frontier,
    )
    .expect_err("bad seed width must be rejected before mutating reusable frontier storage");

    assert!(matches!(err, DispatchError::BadInputs(_)));
    assert_eq!(frontier, vec![0xCAFE_BABEu32]);
    assert_eq!(frontier.capacity(), capacity);
    assert!(scratch.inputs.is_empty());
    assert_eq!(scratch.program_builds(), 0);
}

#[test]
fn gpu_reuses_dispatch_input_buffers() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut scratch =
        ForwardChangedGpuScratch::with_input_capacities(&[32, 32, 32, 32, 32, 32, 32, 8], 1);
    let mut frontier = Vec::with_capacity(4);
    let input_caps = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    let frontier_ptr = frontier.as_ptr();
    forward_closure_via_change_flag_gpu_with_scratch_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .unwrap();
    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_caps
    );
    assert_eq!(frontier.as_ptr(), frontier_ptr);
    assert_eq!(frontier, vec![0b1111]);
}

#[test]
fn gpu_refreshes_static_inputs_when_same_shape_graph_content_changes() {
    let dispatcher = StaticCsrInputRecordingDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b0001]),
            u32_slice_to_le_bytes(&[0]),
        ],
        edge_targets: Mutex::new(Vec::new()),
    };
    let edge_offsets = vec![0, 1, 2, 3, 3];
    let first_targets = vec![1, 2, 3];
    let second_targets = vec![2, 3, 0];
    let edge_kind_mask = vec![1, 1, 1];
    let mut scratch = ForwardChangedGpuScratch::default();
    let mut frontier = Vec::new();

    forward_closure_via_change_flag_gpu_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &first_targets,
        &edge_kind_mask,
        &[0b0001],
        0xFFFF_FFFF,
        1,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first same-shape dispatch should succeed");
    forward_closure_via_change_flag_gpu_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &second_targets,
        &edge_kind_mask,
        &[0b0001],
        0xFFFF_FFFF,
        1,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: second same-shape dispatch should refresh static CSR inputs");

    let recorded_targets = dispatcher
        .edge_targets
        .lock()
        .expect("Fix: static input recording mutex should not be poisoned");
    assert_eq!(
        recorded_targets.as_slice(),
        &[first_targets, second_targets]
    );
    assert_eq!(
        scratch.program_builds(),
        1,
        "Fix: same-shape graph content changes should refresh staged static inputs without rebuilding the primitive program."
    );
}

#[test]
fn gpu_reuses_cached_program_by_primitive_key() {
    let history_dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
        ],
    };
    let legacy_dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut scratch = ForwardChangedGpuScratch::default();
    let mut frontier = Vec::new();

    forward_closure_via_change_flag_gpu_with_scratch_into(
        &history_dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first changed-history dispatch should build one program");
    assert_eq!(scratch.program_builds(), 1);

    forward_closure_via_change_flag_gpu_with_scratch_into(
        &history_dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0011],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: identical primitive key should reuse the cached program");
    assert_eq!(scratch.program_builds(), 1);

    forward_closure_via_change_flag_gpu_with_scratch_into(
        &history_dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0b0001,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: changed allow mask should rebuild the primitive program");
    assert_eq!(scratch.program_builds(), 2);

    forward_closure_via_change_flag_gpu_with_scratch_into(
        &legacy_dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0b0001,
        65,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: switching changed-history policy should rebuild the program");
    assert_eq!(scratch.program_builds(), 3);
}

#[test]

fn gpu_rejects_mismatched_edge_arrays() {
    let dispatcher = CsrChangedDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
        ],
    };
    let err = forward_closure_via_change_flag_gpu(
        &dispatcher,
        2,
        &[0, 1, 1],
        &[1],
        &[],
        &[0b01],
        0xFFFF_FFFF,
        1,
    )
    .expect_err("mismatched edge arrays must be rejected");
    assert!(matches!(err, DispatchError::BadInputs(_)));
}

#[test]
fn generated_gpu_seed_copy_bounds_to_primitive_frontier_words() {
    for node_count in 1u32..=512 {
        let frontier_words = node_count.div_ceil(32) as usize;
        let edge_offsets = vec![0; node_count as usize + 1];
        for extra_words in 0..8usize {
            let seed_len = frontier_words + extra_words;
            let seed = (0..seed_len)
                .map(|idx| 0xA5A5_0000u32 ^ idx as u32 ^ node_count)
                .collect::<Vec<_>>();
            let dispatcher = RecordingCsrChangedDispatcher {
                outputs: vec![
                    u32_slice_to_le_bytes(&vec![0; frontier_words]),
                    u32_slice_to_le_bytes(&[0]),
                ],
                frontier_inputs: Mutex::new(Vec::new()),
            };
            let mut frontier = Vec::new();

            let result = forward_closure_via_change_flag_gpu_into(
                &dispatcher,
                node_count,
                &edge_offsets,
                &[],
                &[],
                &seed,
                0xFFFF_FFFF,
                1,
                &mut frontier,
            );

            if extra_words == 0 {
                result.expect("Fix: exact-width empty-edge generated CSR closure should dispatch");
                let observed = dispatcher
                    .frontier_inputs
                    .lock()
                    .expect("Fix: frontier recording mutex should not be poisoned");
                assert_eq!(
                    observed.len(),
                    1,
                    "node_count={node_count} extra_words={extra_words}"
                );
                assert_eq!(
                    observed[0],
                    seed[..frontier_words],
                    "node_count={node_count} extra_words={extra_words}"
                );
            } else {
                let err = result.expect_err(
                    "Fix: oversized generated seed must be rejected instead of silently truncated",
                );
                assert!(
                    matches!(err, DispatchError::BadInputs(_)),
                    "node_count={node_count} extra_words={extra_words} err={err:?}"
                );
                let observed = dispatcher
                    .frontier_inputs
                    .lock()
                    .expect("Fix: frontier recording mutex should not be poisoned");
                assert!(
                    observed.is_empty(),
                    "node_count={node_count} extra_words={extra_words}"
                );
            }
        }
    }
}
