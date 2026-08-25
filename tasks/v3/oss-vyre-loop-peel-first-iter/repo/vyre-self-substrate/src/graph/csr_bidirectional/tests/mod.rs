use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::Mutex;
use vyre_foundation::ir::Program;

mod reference_closure_tests;
mod release_path_tests;

struct BidirDispatcher {
    outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for BidirDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        // 4 nodes dispatched at CSR_FRONTIER_STEP_WORKGROUP_SIZE (256 threads/group):
        // ceil(4/256) == 1 workgroup, NOT 4. The grid was corrected from a 256x
        // over-dispatch in plan_csr_bidirectional_step (vyre-primitives
        // csr-bidir-grid-miscompile); the dispatcher now sees the right block count.
        assert_eq!(grid_override, Some([1, 1, 1]));
        if inputs.len() != 7 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: bidirectional test dispatcher expected 7 inputs, got {}.",
                inputs.len()
            )));
        }
        Ok(self.outputs.clone())
    }
}

struct StaticBidirInputRecordingDispatcher {
    outputs: Vec<Vec<u8>>,
    edge_targets: Mutex<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for StaticBidirInputRecordingDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.edge_targets
            .lock()
            .expect("Fix: bidirectional static-input recorder mutex should not be poisoned")
            .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[2]));
        Ok(self.outputs.clone())
    }
}

fn linear_graph() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    // 0 -> 1 -> 2 -> 3
    (vec![0, 1, 2, 3, 3], vec![1, 2, 3], vec![1, 1, 1])
}

#[test]
fn step_includes_forward_and_backward_neighbors() {
    let (off, tgt, msk) = linear_graph();
    // Seed = {1}. Forward = {2}, backward = {0}. Union ⊇ {0, 2}.
    let out = reference_bidirectional_step(4, &off, &tgt, &msk, &[0b0010], 0xFFFF_FFFF);
    assert!(out[0] & 0b0001 != 0, "0 should be in backward step from 1");
    assert!(out[0] & 0b0100 != 0, "2 should be in forward step from 1");
}

#[test]
fn empty_seed_yields_empty_step() {
    let (off, tgt, msk) = linear_graph();
    let out = reference_bidirectional_step(4, &off, &tgt, &msk, &[0u32], 0xFFFF_FFFF);
    assert_eq!(out, vec![0u32]);
}

/// Closure-bar: substrate call equals direct primitive call.
#[test]
fn matches_primitive_directly() {
    let (off, tgt, msk) = linear_graph();
    let seed = vec![0b0010];
    let via_substrate = reference_bidirectional_step(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF);
    let via_primitive = reference_csr_bidir(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF);
    assert_eq!(via_substrate, via_primitive);
}

/// Adversarial: kind-mask filter must reject edges whose kinds
/// don't intersect `allow_mask`. The bidirectional step is a
/// pure successor/predecessor union; with no matching edges,
/// no neighbors are flagged (the primitive does not retain
/// the seed in its output).
#[test]
fn allow_mask_filters_out_wrong_edge_kinds() {
    let off = vec![0, 1, 1];
    let tgt = vec![1];
    let msk = vec![0b0010]; // edge kind bit 1
    let out = reference_bidirectional_step(2, &off, &tgt, &msk, &[0b01], 0b0001);
    let direct = reference_csr_bidir(2, &off, &tgt, &msk, &[0b01], 0b0001);
    // Substrate output must match primitive directly.
    assert_eq!(out, direct);
    // And bit 1 (would-be neighbor via a kind-0 edge that doesn't
    // exist) must NOT be set in the result.
    assert_eq!(out[0] & 0b10, 0);
}

/// bidirectional_closure on a linear chain {0 -> 1 -> 2 -> 3} with
/// seed {0} must reach every node within 3 iterations.
#[test]
fn closure_reaches_full_chain() {
    let (off, tgt, msk) = linear_graph();
    let out = reference_bidirectional_closure(4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 5);
    assert_eq!(out, vec![0b1111]);
}

#[test]
fn closure_into_matches_owned_closure() {
    let (off, tgt, msk) = linear_graph();
    let owned = reference_bidirectional_closure(4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 5);
    let mut current = Vec::new();
    let mut next = Vec::new();
    reference_bidirectional_closure_into(
        4,
        &off,
        &tgt,
        &msk,
        &[0b0001],
        0xFFFF_FFFF,
        5,
        &mut current,
        &mut next,
    );
    assert_eq!(current, owned);
}

#[test]
fn closure_matches_primitive_directly() {
    let (off, tgt, msk) = linear_graph();
    let seed = [0b0001];
    let via_substrate = reference_bidirectional_closure(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF, 5);
    let via_primitive = reference_csr_bidir_closure(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF, 5);
    assert_eq!(via_substrate, via_primitive);
}

#[test]
fn via_step_decodes_exact_output_into_reused_buffer() {
    let dispatcher = BidirDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1010])],
    };
    let (off, tgt, msk) = linear_graph();
    let mut out = Vec::with_capacity(4);
    let ptr = out.as_ptr();
    bidirectional_step_via_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0010],
        0xFFFF_FFFF,
        &mut out,
    )
    .expect("Fix: dispatch succeeds");
    assert_eq!(out, vec![0b1010]);
    assert_eq!(out.as_ptr(), ptr);
}

#[test]
fn via_step_with_scratch_reuses_dispatch_storage() {
    let dispatcher = BidirDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1010])],
    };
    let (off, tgt, msk) = linear_graph();
    let mut scratch = BidirectionalGpuScratch::default();
    let mut out = Vec::with_capacity(1);

    bidirectional_step_via_with_scratch_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0010],
        0xFFFF_FFFF,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: dispatch succeeds");
    assert_eq!(out, vec![0b1010]);
    let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    let out_capacity = out.capacity();
    assert_eq!(scratch.program_builds(), 1);

    bidirectional_step_via_with_scratch_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0100],
        0xFFFF_FFFF,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: dispatch succeeds");
    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(out.capacity(), out_capacity);
    assert_eq!(out, vec![0b1010]);
    assert_eq!(scratch.program_builds(), 1);

    bidirectional_step_via_with_scratch_into(
        &dispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0100],
        0x0000_0001,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: changed allow_mask should dispatch");
    assert_eq!(scratch.program_builds(), 2);
}

#[test]
fn via_step_refreshes_static_inputs_when_same_shape_graph_content_changes() {
    let dispatcher = StaticBidirInputRecordingDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1010])],
        edge_targets: Mutex::new(Vec::new()),
    };
    let edge_offsets = vec![0, 1, 2, 3, 3];
    let first_targets = vec![1, 2, 3];
    let second_targets = vec![2, 3, 0];
    let edge_kind_mask = vec![1, 1, 1];
    let mut scratch = BidirectionalGpuScratch::default();
    let mut out = Vec::new();

    bidirectional_step_via_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &first_targets,
        &edge_kind_mask,
        &[0b0010],
        0xFFFF_FFFF,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: first same-shape bidirectional dispatch should succeed");
    bidirectional_step_via_with_scratch_into(
        &dispatcher,
        4,
        &edge_offsets,
        &second_targets,
        &edge_kind_mask,
        &[0b0010],
        0xFFFF_FFFF,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: second same-shape bidirectional dispatch should refresh static CSR inputs");

    let recorded_targets = dispatcher
        .edge_targets
        .lock()
        .expect("Fix: bidirectional static-input recorder mutex should not be poisoned");
    assert_eq!(
        recorded_targets.as_slice(),
        &[first_targets, second_targets]
    );
    assert_eq!(
        scratch.program_builds(),
        1,
        "Fix: same-shape CSR content changes must refresh static inputs without rebuilding the program."
    );
}

#[test]
fn via_step_uses_bridge_zero_inputs_for_graph_scratch() {
    struct InspectingDispatcher;

    impl OptimizerDispatcher for InspectingDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            // 4 nodes / 256 threads-per-group => ceil(4/256) == 1 workgroup (not 4):
            // the corrected grid from vyre-primitives csr-bidir-grid-miscompile.
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 7);
            assert_eq!(inputs[0], u32_slice_to_le_bytes(&[0, 0, 0, 0]));
            assert_eq!(inputs[4], u32_slice_to_le_bytes(&[0, 0, 0, 0]));
            assert_eq!(inputs[6], u32_slice_to_le_bytes(&[0]));
            Ok(vec![u32_slice_to_le_bytes(&[0b1010])])
        }
    }

    let (off, tgt, msk) = linear_graph();
    let out = bidirectional_step_via(
        &InspectingDispatcher,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0010],
        0xFFFF_FFFF,
    )
    .expect("Fix: dispatch succeeds");

    assert_eq!(out, vec![0b1010]);
}

#[test]
fn via_step_rejects_extra_outputs() {
    let dispatcher = BidirDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0b1010]),
            u32_slice_to_le_bytes(&[0]),
        ],
    };
    let (off, tgt, msk) = linear_graph();
    let err = bidirectional_step_via(&dispatcher, 4, &off, &tgt, &msk, &[0b0010], 0xFFFF_FFFF)
        .expect_err("extra outputs must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_step_rejects_trailing_output_bytes() {
    let dispatcher = BidirDispatcher {
        outputs: vec![vec![0, 0, 0, 0, 1]],
    };
    let (off, tgt, msk) = linear_graph();
    let err = bidirectional_step_via(&dispatcher, 4, &off, &tgt, &msk, &[0b0010], 0xFFFF_FFFF)
        .expect_err("trailing output bytes must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_step_rejects_mismatched_edge_arrays() {
    let dispatcher = BidirDispatcher {
        outputs: vec![u32_slice_to_le_bytes(&[0b1010])],
    };
    let err = bidirectional_step_via(&dispatcher, 2, &[0, 1, 1], &[1], &[], &[0b01], 0xFFFF_FFFF)
        .expect_err("mismatched edge arrays must be rejected");
    assert!(matches!(err, DispatchError::BadInputs(_)));
}

#[test]
fn via_step_empty_graph_is_validated_by_primitive_and_does_not_dispatch() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("empty bidirectional graph must not dispatch");
        }
    }

    let mut out = vec![u32::MAX];
    bidirectional_step_via_into(&NoDispatch, 0, &[0], &[], &[], &[], u32::MAX, &mut out)
        .expect("Fix: canonical empty graph is valid");
    assert!(out.is_empty());
}

#[test]
fn closure_rejects_bad_seed_without_clobbering_reusable_buffers() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("malformed closure seed must be rejected before dispatch");
        }
    }

    let (off, tgt, msk) = linear_graph();
    let mut scratch = BidirectionalGpuScratch::default();
    let mut current = vec![0xCAFE_BABE];
    let mut next = vec![0xDEAD_BEEF];

    let err = bidirectional_closure_via_with_scratch_into(
        &NoDispatch,
        4,
        &off,
        &tgt,
        &msk,
        &[],
        0xFFFF_FFFF,
        5,
        &mut scratch,
        &mut current,
        &mut next,
    )
    .expect_err("bad seed width must be rejected before mutating reusable buffers");

    assert!(matches!(err, DispatchError::BadInputs(_)));
    assert_eq!(current, vec![0xCAFE_BABE]);
    assert_eq!(next, vec![0xDEAD_BEEF]);
}

#[test]
fn closure_zero_iters_validates_and_returns_seed_without_program_or_dispatch() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("zero-iteration bidirectional closure must not dispatch");
        }
    }

    let (off, tgt, msk) = linear_graph();
    let mut scratch = BidirectionalGpuScratch::default();
    let mut current = Vec::with_capacity(8);
    let mut next = vec![0xDEAD_BEEF];

    bidirectional_closure_via_with_scratch_into(
        &NoDispatch,
        4,
        &off,
        &tgt,
        &msk,
        &[0b0010],
        0xFFFF_FFFF,
        0,
        &mut scratch,
        &mut current,
        &mut next,
    )
    .expect("Fix: zero-iteration closure should still validate inputs");

    assert_eq!(current, vec![0b0010]);
    assert!(next.is_empty());
    assert_eq!(scratch.program_builds(), 0);
    assert!(scratch.inputs.is_empty());
}

#[test]

fn closure_empty_graph_validates_and_returns_empty_without_program_or_dispatch() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("empty bidirectional closure must not dispatch");
        }
    }

    let mut scratch = BidirectionalGpuScratch::default();
    let mut current = vec![0xCAFE_BABE];
    let mut next = vec![0xDEAD_BEEF];

    bidirectional_closure_via_with_scratch_into(
        &NoDispatch,
        0,
        &[0],
        &[],
        &[],
        &[],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut current,
        &mut next,
    )
    .expect("Fix: canonical empty closure should validate and short-circuit");

    assert!(current.is_empty());
    assert!(next.is_empty());
    assert_eq!(scratch.program_builds(), 0);
    assert!(scratch.inputs.is_empty());
}
