use super::super::*;
use super::linear_graph;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::cell::RefCell;
use vyre_foundation::ir::Program;

struct PersistentBfsDispatcher {
    outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for PersistentBfsDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        if inputs.len() != 8 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: persistent BFS test dispatcher expected 8 inputs, got {}.",
                inputs.len()
            )));
        }
        Ok(self.outputs.clone())
    }
}

struct RecordingPersistentBfsDispatcher {
    outputs: Vec<Vec<u8>>,
    edge_targets: RefCell<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for RecordingPersistentBfsDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        if inputs.len() != 8 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: persistent BFS recording dispatcher expected 8 inputs, got {}.",
                inputs.len()
            )));
        }
        self.edge_targets
            .borrow_mut()
            .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[2]));
        Ok(self.outputs.clone())
    }
}

struct LargeScratchPersistentBfsDispatcher {
    outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for LargeScratchPersistentBfsDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([3, 1, 1]));
        if inputs.len() != 8 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: large persistent BFS test dispatcher expected 8 inputs, got {}.",
                inputs.len()
            )));
        }
        if inputs[7].len() != 12 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: large persistent BFS changed scratch must allocate 12 bytes, got {}.",
                inputs[7].len()
            )));
        }
        Ok(self.outputs.clone())
    }
}

#[test]
fn via_into_decodes_exact_outputs_into_reused_frontier() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[1]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut frontier = Vec::with_capacity(4);
    let ptr = frontier.as_ptr();
    let changed = bfs_expand_via_into(
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
    assert_eq!(changed, 1);
    assert_eq!(frontier.as_ptr(), ptr);
}

#[test]
fn via_into_rejects_non_boolean_changed_flag_readback() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[7]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut frontier = vec![0xDEAD_BEEF];
    let capacity = frontier.capacity();

    let err = bfs_expand_via_into(
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
    .expect_err("Fix: persistent BFS wrapper must reject malformed changed-flag readback");

    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error variant: {err:?}"
    );
    assert_eq!(
        frontier,
        vec![0b1111],
        "frontier readback remains available for diagnostics even when the scalar flag is malformed"
    );
    assert_eq!(frontier.capacity(), capacity);
}

#[test]
fn via_large_graph_allocates_changed_active_scratch_without_extra_outputs() {
    let node_count = 513u32;
    let words = ((node_count + 31) / 32) as usize;
    let dispatcher = LargeScratchPersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&vec![0u32; words]),
            u32_slice_to_le_bytes(&[0, 0, 0]),
        ],
    };
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let frontier_in = vec![0u32; words];
    let mut frontier = Vec::new();

    let changed = bfs_expand_via_into(
        &dispatcher,
        node_count,
        &edge_offsets,
        &[],
        &[],
        &frontier_in,
        0xFFFF_FFFF,
        64,
        &mut frontier,
    )
    .expect("Fix: large persistent BFS dispatch should allocate internal active scratch.");

    assert_eq!(changed, 0);
    assert_eq!(frontier, vec![0u32; words]);
}

#[test]
fn via_with_scratch_reuses_dispatch_storage() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[1]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let mut scratch = PersistentBfsGpuScratch::default();
    let mut frontier = Vec::with_capacity(1);

    let changed = bfs_expand_via_with_scratch_into(
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
    .expect("Fix: dispatch succeeds");
    assert_eq!(changed, 1);
    assert_eq!(frontier, vec![0b1111]);
    let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    let frontier_capacity = frontier.capacity();

    let changed = bfs_expand_via_with_scratch_into(
        &dispatcher,
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
    .expect("Fix: dispatch succeeds");
    assert_eq!(changed, 1);
    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(frontier.capacity(), frontier_capacity);
}

#[test]
fn via_refreshes_static_graph_inputs_for_same_shape_content_change() {
    let dispatcher = RecordingPersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[1]),
        ],
        edge_targets: RefCell::new(Vec::new()),
    };
    let edge_offsets = vec![0, 1, 2, 3, 3];
    let first_targets = vec![1, 2, 3];
    let second_targets = vec![2, 3, 0];
    let edge_kind_mask = vec![1, 1, 1];
    let mut scratch = PersistentBfsGpuScratch::default();
    let mut frontier = Vec::new();

    bfs_expand_via_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &first_targets,
        &edge_kind_mask,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first same-shape persistent BFS dispatch should succeed");
    bfs_expand_via_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &second_targets,
        &edge_kind_mask,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: second same-shape persistent BFS dispatch should refresh graph inputs");

    assert_eq!(
        dispatcher.edge_targets.borrow().as_slice(),
        &[first_targets, second_targets]
    );
    let snapshot = scratch.plan_cache.snapshot();
    assert_eq!(snapshot.entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
}

#[test]
fn via_zero_iters_validates_and_returns_seed_without_dispatch_or_cache() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("zero-iteration persistent BFS must not dispatch");
        }
    }

    let (off, tgt, msk) = linear_graph();
    let mut scratch = PersistentBfsGpuScratch::default();
    let mut frontier = Vec::with_capacity(8);
    let ptr = frontier.as_ptr();
    let changed = bfs_expand_via_with_scratch_into(
        &NoDispatch,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0011],
        0xFFFF_FFFF,
        0,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: zero-iteration persistent BFS should validate and return seed frontier");

    assert_eq!(changed, 0);
    assert_eq!(frontier, vec![0b0011]);
    assert_eq!(frontier.as_ptr(), ptr);
    assert!(scratch.inputs.is_empty());
    assert_eq!(scratch.static_input_key, None);
}

#[test]
fn via_rejects_extra_outputs() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[1]),
            u32_slice_to_le_bytes(&[99]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let err = bfs_expand_via(&dispatcher, 4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 4)
        .expect_err("extra outputs must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_rejects_trailing_changed_bytes() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1111]), vec![1, 0, 0, 0, 2]],
    };
    let (off, tgt, msk) = linear_graph();
    let err = bfs_expand_via(&dispatcher, 4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 4)
        .expect_err("trailing changed bytes must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_rejects_mismatched_edge_arrays() {
    let dispatcher = PersistentBfsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1111]),
            u32_slice_to_le_bytes(&[1]),
        ],
    };
    let err = bfs_expand_via(
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
fn release_via_path_does_not_call_cpu_or_local_saturating_helpers() {
    let source = include_str!("../dispatch.rs");
    let start = source
        .find("pub fn bfs_expand_via")
        .expect("Fix: via path marker must exist");
    let release_path = &source[start..];
    assert!(!release_path.contains("reference_persistent_bfs"));
    assert!(!release_path.contains("reference_"));
    assert!(!release_path.contains("cpu_ref"));
    assert!(!release_path.contains("saturating_mul"));
    assert!(!release_path.contains("fill_"));
}
