use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::Mutex;
use vyre_primitives::graph::path_reconstruct::try_cpu_ref_batched;

struct PathDispatcher;

impl OptimizerDispatcher for PathDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 4);
        let parent = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let targets = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let out_words = inputs[2].len() / std::mem::size_of::<u32>();
        let max_depth = out_words / targets.len().max(1);
        if targets.len() == 1 {
            assert_eq!(grid_override, Some([1, 1, 1]));
        } else {
            assert_eq!(grid_override, Some([1, 1, 1]));
        }
        let mut paths = Vec::with_capacity(out_words);
        let mut lens = Vec::with_capacity(targets.len());
        try_cpu_ref_batched(&parent, &targets, max_depth as u32, &mut paths, &mut lens)
            .map_err(DispatchError::BackendError)?;
        Ok(vec![
            u32_slice_to_le_bytes(&paths),
            u32_slice_to_le_bytes(&lens),
        ])
    }
}

struct RecordingPathDispatcher {
    outputs: Vec<Vec<u8>>,
    parents: Mutex<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for RecordingPathDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.parents
            .lock()
            .expect("Fix: path parent recorder mutex should not be poisoned")
            .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[0]));
        Ok(self.outputs.clone())
    }
}

#[test]
fn reconstructs_chain_to_root() {
    // 0 is root (parent[0] = 0); 1 -> 0; 2 -> 1; 3 -> 2.
    let parent = vec![0, 0, 1, 2];
    let path = reference_path_to_root(&parent, 3, 4);
    assert_eq!(path, vec![3, 2, 1, 0]);
}

#[test]
fn reconstructs_root_yields_singleton() {
    let parent = vec![0, 0, 1];
    let path = reference_path_to_root(&parent, 0, 4);
    assert_eq!(path, vec![0]);
}

/// Closure-bar: substrate call equals primitive call exactly.
#[test]
fn matches_primitive_directly() {
    let parent = vec![0, 0, 1, 2];
    let mut a = Vec::new();
    let mut b = Vec::new();
    let len_a = reference_reconstruct_path(&parent, 3, 4, &mut a);
    let len_b = path_reconstruct_cpu(&parent, 3, 4, &mut b);
    assert_eq!((len_a, &a), (len_b, &b));
}

/// Adversarial: max_depth bound must terminate even on a cycle
/// (parent forms a non-trivial loop). The primitive's contract:
/// stop when length reaches `max_depth`.
#[test]
fn max_depth_terminates_on_cycle() {
    // 0 -> 1 -> 2 -> 0 (cycle, no real root).
    let parent = vec![1, 2, 0];
    let path = reference_path_to_root(&parent, 0, 5);
    assert_eq!(path.len(), 5);
}

/// Adversarial: scratch buffer is zero-filled to `max_depth`
/// past the actual path length. A common bug is to leave stale
/// values in scratch slots beyond `len`  -  assert all unused
/// slots are zero.
#[test]
fn scratch_zero_filled_past_len() {
    let parent = vec![0, 0, 1];
    let mut scratch = Vec::new();
    let len = reference_reconstruct_path(&parent, 2, 8, &mut scratch);
    assert_eq!(len, 3);
    assert_eq!(scratch.len(), 8);
    for &v in &scratch[len as usize..] {
        assert_eq!(v, 0, "trailing slots must be zero-filled");
    }
}

/// Adversarial: scratch is cleared before each call, so reuse
/// across reconstructions doesn't leak old paths.
#[test]
fn scratch_cleared_between_calls() {
    let parent = vec![0, 0, 1, 2];
    let mut scratch = Vec::new();
    // First call: deep path.
    assert_eq!(reference_reconstruct_path(&parent, 3, 4, &mut scratch), 4);
    // Second call: target is root, expect path length 1.
    let len = reference_reconstruct_path(&parent, 0, 4, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 0);
}

#[test]
fn reconstruct_path_via_dispatches_single_target() {
    let parent = vec![0, 0, 1, 2];
    let mut scratch = Vec::new();

    let len = reconstruct_path_via(&PathDispatcher, &parent, 3, 4, &mut scratch).unwrap();

    assert_eq!(len, 4);
    assert_eq!(scratch, vec![3, 2, 1, 0]);
}

#[test]
fn reconstruct_path_via_with_scratch_reuses_program_by_depth() {
    let parent = vec![0, 0, 1, 2];
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    let mut path = Vec::new();

    let len = reconstruct_path_via_with_scratch(
        &PathDispatcher,
        &parent,
        3,
        4,
        &mut dispatch_scratch,
        &mut path,
    )
    .unwrap();
    assert_eq!(len, 4);
    assert_eq!(dispatch_scratch.single_program_builds(), 1);

    let len = reconstruct_path_via_with_scratch(
        &PathDispatcher,
        &parent,
        2,
        4,
        &mut dispatch_scratch,
        &mut path,
    )
    .unwrap();
    assert_eq!(len, 3);
    assert_eq!(dispatch_scratch.single_program_builds(), 1);

    reconstruct_path_via_with_scratch(
        &PathDispatcher,
        &parent,
        2,
        8,
        &mut dispatch_scratch,
        &mut path,
    )
    .unwrap();
    assert_eq!(dispatch_scratch.single_program_builds(), 2);
}

#[test]
fn reconstruct_path_via_with_scratch_refreshes_same_shape_parent_content() {
    let dispatcher = RecordingPathDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0, 0, 0, 0]),
            u32_slice_to_le_bytes(&[1]),
        ],
        parents: Mutex::new(Vec::new()),
    };
    let first_parent = vec![0, 0, 1, 2];
    let second_parent = vec![0, 0, 0, 0];
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    let mut path = Vec::new();

    reconstruct_path_via_with_scratch(
        &dispatcher,
        &first_parent,
        3,
        4,
        &mut dispatch_scratch,
        &mut path,
    )
    .expect("Fix: first path dispatch should succeed");
    reconstruct_path_via_with_scratch(
        &dispatcher,
        &second_parent,
        3,
        4,
        &mut dispatch_scratch,
        &mut path,
    )
    .expect("Fix: same-shape parent content change should refresh static parent input");

    let recorded = dispatcher
        .parents
        .lock()
        .expect("Fix: path parent recorder mutex should not be poisoned");
    assert_eq!(recorded.as_slice(), &[first_parent, second_parent]);
    assert_eq!(
        dispatch_scratch.single_program_builds(),
        1,
        "Fix: parent content changes should refresh staged inputs without rebuilding the depth-keyed program."
    );
}

#[test]
fn path_to_root_via_truncates_padding() {
    let parent = vec![0, 0, 1, 2];

    let path = path_to_root_via(&PathDispatcher, &parent, 2, 8).unwrap();

    assert_eq!(path, vec![2, 1, 0]);
}

#[test]
fn reconstruct_path_via_rejects_len_readback_beyond_primitive_depth() {
    let dispatcher = RecordingPathDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[3, 2, 1, 0]),
            u32_slice_to_le_bytes(&[5]),
        ],
        parents: Mutex::new(Vec::new()),
    };
    let parent = vec![0, 0, 1, 2];
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    let mut path = Vec::new();

    let err = reconstruct_path_via_with_scratch(
        &dispatcher,
        &parent,
        3,
        4,
        &mut dispatch_scratch,
        &mut path,
    )
    .expect_err("Fix: path reconstruction must reject impossible GPU length readback");

    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error variant: {err:?}"
    );
}

#[test]
fn reconstruct_paths_via_batches_targets_in_one_dispatch() {
    let parent = vec![0, 0, 1, 2];

    let (paths, lens) = reconstruct_paths_via(&PathDispatcher, &parent, &[3, 0, 2], 4).unwrap();

    assert_eq!(lens, vec![4, 1, 3]);
    assert_eq!(paths, vec![3, 2, 1, 0, 0, 0, 0, 0, 2, 1, 0, 0]);
}

#[test]
fn reconstruct_paths_via_rejects_any_batched_len_beyond_primitive_depth() {
    let dispatcher = RecordingPathDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[3, 2, 1, 0, 2, 1, 0, 0]),
            u32_slice_to_le_bytes(&[4, 5]),
        ],
        parents: Mutex::new(Vec::new()),
    };
    let parent = vec![0, 0, 1, 2];
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    let mut paths = Vec::new();
    let mut lens = Vec::new();

    let err = reconstruct_paths_via_with_scratch_into(
        &dispatcher,
        &parent,
        &[3, 2],
        4,
        &mut dispatch_scratch,
        &mut paths,
        &mut lens,
    )
    .expect_err("Fix: batched path reconstruction must reject impossible GPU length readback");

    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error variant: {err:?}"
    );
}

#[test]
fn reconstruct_paths_via_with_scratch_reuses_dispatch_and_outputs() {
    let parent = vec![0, 0, 1, 2];
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    let mut paths = Vec::with_capacity(12);
    let mut lens = Vec::with_capacity(3);

    reconstruct_paths_via_with_scratch_into(
        &PathDispatcher,
        &parent,
        &[3, 0, 2],
        4,
        &mut dispatch_scratch,
        &mut paths,
        &mut lens,
    )
    .unwrap();

    let input_capacities = dispatch_scratch
        .inputs
        .iter()
        .map(Vec::capacity)
        .collect::<Vec<_>>();
    let paths_capacity = paths.capacity();
    let lens_capacity = lens.capacity();
    assert_eq!(dispatch_scratch.batched_program_builds(), 1);

    reconstruct_paths_via_with_scratch_into(
        &PathDispatcher,
        &parent,
        &[2, 1, 0],
        4,
        &mut dispatch_scratch,
        &mut paths,
        &mut lens,
    )
    .unwrap();

    assert_eq!(
        dispatch_scratch
            .inputs
            .iter()
            .map(Vec::capacity)
            .collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(paths.capacity(), paths_capacity);
    assert_eq!(lens.capacity(), lens_capacity);
    assert_eq!(dispatch_scratch.batched_program_builds(), 1);
    assert_eq!(lens, vec![3, 2, 1]);
    assert_eq!(paths, vec![2, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);

    reconstruct_paths_via_with_scratch_into(
        &PathDispatcher,
        &parent,
        &[3, 2],
        4,
        &mut dispatch_scratch,
        &mut paths,
        &mut lens,
    )
    .unwrap();
    assert_eq!(dispatch_scratch.batched_program_builds(), 2);
}

#[test]
fn reconstruct_paths_via_rejects_zero_depth() {
    let err = reconstruct_paths_via(&PathDispatcher, &[0], &[0], 0).unwrap_err();

    assert!(matches!(err, DispatchError::BadInputs(_)));
}

#[test]
fn test_dispatcher_delegates_parent_walk_to_primitive_oracle() {
    let source = include_str!("mod.rs");
    let dispatcher_section = source
        .split("impl OptimizerDispatcher for PathDispatcher")
        .nth(1)
        .expect("Fix: path-reconstruct test dispatcher implementation must exist")
        .split("fn read_u32s")
        .next()
        .expect("Fix: test dispatcher should precede read_u32s helper");

    assert!(dispatcher_section.contains("try_cpu_ref_batched"));
    assert!(!dispatcher_section.contains(concat!("let mut current = ", "target")));
    assert!(!dispatcher_section.contains(concat!("parent", ".get(current as usize)")));
}

#[test]
fn production_source_keeps_cpu_path_helpers_out_of_via_path() {
    let source = include_str!("../dispatch.rs");
    let via_section = source
        .split("pub fn reconstruct_path_via(")
        .nth(1)
        .expect("Fix: via section should exist")
        .split("pub fn reconstruct_paths_via(")
        .next()
        .expect("Fix: batched via wrapper should follow single-target path wrapper");

    assert!(!via_section.contains("path_reconstruct_cpu"));
    assert!(!via_section.contains("reference_reconstruct_path"));
    assert!(!via_section.contains("Vec::with_capacity(max_depth as usize)"));
    assert!(!source.contains("fingerprint_u32_slice"));
    assert!(!source.contains("struct PathReconstructStaticInputKey"));
    assert!(via_section.contains("refresh_keyed_dispatch_inputs"));
    assert!(via_section.contains("dispatch_two_u32_outputs_from_prepared_into"));
    assert!(source.contains("BATCHED_PATHS_BUFFER"));
    assert!(source.contains("BATCHED_LENS_BUFFER"));
}
