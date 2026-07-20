use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::Mutex;
use vyre_foundation::ir::Program;
use vyre_primitives::graph::toposort::{
    toposort as toposort_cpu, toposort_csr_into, ToposortError,
};

#[test]
fn toposort_wrappers_use_dedicated_observability_counter() {
    let reference_source = include_str!("../reference.rs");
    let dispatch_source = include_str!("../dispatch.rs");
    assert!(reference_source.contains("toposort_calls"));
    assert!(dispatch_source.contains("toposort_calls"));
    assert!(!reference_source.contains("dataflow_fixpoint_calls"));
    assert!(!dispatch_source.contains("dataflow_fixpoint_calls"));
}

#[test]
fn topo_order_chain_emits_dependency_first() {
    // 0 depends on 1 depends on 2. Order should be [2, 1, 0]
    let order = reference_topo_order(3, &[(0, 1), (1, 2)]).unwrap();
    // Verify the ordering invariant: every (from, to) edge has
    // to before from in the output.
    let pos: std::collections::HashMap<u32, usize> =
        order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    for &(from, to) in &[(0u32, 1u32), (1, 2)] {
        assert!(
            pos[&to] < pos[&from],
            "to ({to}) must precede from ({from}) in toposort"
        );
    }
}

#[test]
fn topo_order_detects_cycle() {
    // 0 -> 1 -> 0 cycle.
    let err = reference_topo_order(2, &[(0, 1), (1, 0)]);
    assert!(matches!(err, Err(ToposortError::Cycle { .. })));
}

#[test]
fn topo_order_rejects_unknown_node() {
    let err = reference_topo_order(2, &[(0, 5)]);
    assert!(matches!(err, Err(ToposortError::UnknownNode { .. })));
}

/// Closure-bar: substrate output equals primitive output.
#[test]
fn matches_primitive_directly() {
    let edges = [(0u32, 1u32), (1, 2), (0, 2)];
    let via_substrate = reference_topo_order(3, &edges).unwrap();
    let via_primitive = toposort_cpu(3, &edges).unwrap();
    assert_eq!(via_substrate, via_primitive);
}

#[test]
fn reachable_walks_directed_chain() {
    // 0 -> 1 -> 2 -> 3. From {0}, every node is reachable.
    let edges = [(0u32, 1u32), (1, 2), (2, 3)];
    let reach = reference_reachable_set(4, &edges, &[0]).unwrap();
    for n in 0..4 {
        assert!(reach.contains(&n), "node {n} must be reachable from 0");
    }
}

#[test]
fn reachable_does_not_walk_reverse_edges() {
    // 0 -> 1. From {1}, only 1 is reachable.
    let reach = reference_reachable_set(2, &[(0, 1)], &[1]).unwrap();
    assert_eq!(reach.len(), 1);
    assert!(reach.contains(&1));
}

/// Adversarial: empty sources yield empty reachable.
#[test]
fn reachable_empty_sources_yields_empty_set() {
    let reach = reference_reachable_set(4, &[(0, 1), (1, 2)], &[]).unwrap();
    assert!(reach.is_empty());
}

/// Adversarial: a self-loop in sources must terminate (visited
/// guard). Naive code that doesn't dedupe would loop forever.
#[test]
fn reachable_self_loop_terminates() {
    // 0 -> 0 (self-loop), 1 isolated.
    let reach = reference_reachable_set(2, &[(0, 0)], &[0]).unwrap();
    assert_eq!(reach.len(), 1);
    assert!(reach.contains(&0));
}

#[test]
fn all_reachable_satisfies_query() {
    let edges = [(0u32, 1u32), (1, 2), (0, 2)];
    // From {0}, can we reach {1, 2}? Yes.
    assert!(reference_all_reachable(3, &edges, &[0], &[1, 2]).unwrap());
    // From {2}, can we reach {0}? No (DAG is one-way).
    assert!(!reference_all_reachable(3, &edges, &[2], &[0]).unwrap());
}

struct ToposortDispatcher;

impl OptimizerDispatcher for ToposortDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        dispatch_with_primitive_csr_oracle(inputs, grid_override)
    }
}

fn dispatch_with_primitive_csr_oracle(
    inputs: &[Vec<u8>],
    grid_override: Option<[u32; 3]>,
) -> Result<Vec<Vec<u8>>, DispatchError> {
    assert_eq!(grid_override, Some([1, 1, 1]));
    assert_eq!(inputs.len(), 5);
    let offsets = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
    let targets = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
    let n = offsets.len() - 1;
    let mut out = Vec::with_capacity(n);
    toposort_csr_into(n as u32, &offsets, &targets, &mut out).map_err(|err| {
        DispatchError::BackendError(format!(
            "Fix: test dispatcher must use the primitive CSR oracle; got {err:?}."
        ))
    })?;
    out.resize(n, 0);
    Ok(vec![u32_slice_to_le_bytes(&out)])
}

struct RecordingToposortDispatcher {
    calls: Mutex<Vec<Vec<Vec<u8>>>>,
}

impl OptimizerDispatcher for RecordingToposortDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        assert_eq!(inputs.len(), 5);
        self.calls
            .lock()
            .expect("Fix: recording toposort dispatcher calls lock should not be poisoned")
            .push(inputs.to_vec());
        dispatch_with_primitive_csr_oracle(inputs, grid_override)
    }
}

#[test]
fn topo_order_csr_via_dispatches_primitive_order() {
    let order = topo_order_csr_via(&ToposortDispatcher, 3, &[0, 2, 3, 3], &[1, 2, 2]).unwrap();
    let pos: std::collections::HashMap<u32, usize> =
        order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    assert!(pos[&0] < pos[&1]);
    assert!(pos[&0] < pos[&2]);
    assert!(pos[&1] < pos[&2]);
}

#[test]
fn topo_order_csr_via_with_scratch_into_reuses_storage() {
    let mut scratch = ToposortGpuScratch::default();
    let mut order = Vec::with_capacity(3);

    topo_order_csr_via_with_scratch_into(
        &ToposortDispatcher,
        3,
        &[0, 2, 3, 3],
        &[1, 2, 2],
        &mut scratch,
        &mut order,
    )
    .unwrap();
    let order_capacity = order.capacity();
    let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    assert_eq!(order.len(), 3);
    assert_eq!(scratch.program_builds(), 1);

    topo_order_csr_via_with_scratch_into(
        &ToposortDispatcher,
        3,
        &[0, 1, 2, 2],
        &[1, 2],
        &mut scratch,
        &mut order,
    )
    .unwrap();
    assert_eq!(order.capacity(), order_capacity);
    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(order.len(), 3);
    assert_eq!(scratch.program_builds(), 1);

    topo_order_csr_via_with_scratch_into(
        &ToposortDispatcher,
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &mut scratch,
        &mut order,
    )
    .unwrap();
    assert_eq!(order.len(), 4);
    assert_eq!(scratch.program_builds(), 2);
}

#[test]
fn topo_order_csr_via_refreshes_static_graph_inputs_for_same_shape_content_change() {
    let dispatcher = RecordingToposortDispatcher {
        calls: Mutex::new(Vec::new()),
    };
    let mut scratch = ToposortGpuScratch::default();
    let mut order = Vec::new();
    let offsets = [0, 2, 3, 3, 3];
    let targets = [1, 2, 3];
    let changed_targets = [2, 3, 3];

    topo_order_csr_via_with_scratch_into(
        &dispatcher,
        4,
        &offsets,
        &targets,
        &mut scratch,
        &mut order,
    )
    .expect("Fix: first topological-sort dispatch should succeed");
    topo_order_csr_via_with_scratch_into(
        &dispatcher,
        4,
        &offsets,
        &changed_targets,
        &mut scratch,
        &mut order,
    )
    .expect("Fix: same-shape topological-sort content change should refresh inputs");

    let calls = dispatcher
        .calls
        .lock()
        .expect("Fix: recording toposort dispatcher calls lock should not be poisoned");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0][1], u32_slice_to_le_bytes(&targets));
    assert_eq!(calls[1][1], u32_slice_to_le_bytes(&changed_targets));
    assert_eq!(scratch.program_builds(), 1);
}

#[test]
fn topo_order_csr_via_reuses_static_graph_inputs_and_rezeros_work_slots() {
    let dispatcher = RecordingToposortDispatcher {
        calls: Mutex::new(Vec::new()),
    };
    let mut scratch = ToposortGpuScratch::default();
    let mut order = Vec::new();
    let offsets = [0, 2, 3, 3];
    let targets = [1, 2, 2];

    topo_order_csr_via_with_scratch_into(
        &dispatcher,
        3,
        &offsets,
        &targets,
        &mut scratch,
        &mut order,
    )
    .expect("Fix: first topological-sort dispatch should succeed");
    let static_capacities = scratch
        .inputs
        .iter()
        .take(2)
        .map(Vec::capacity)
        .collect::<Vec<_>>();
    topo_order_csr_via_with_scratch_into(
        &dispatcher,
        3,
        &offsets,
        &targets,
        &mut scratch,
        &mut order,
    )
    .expect("Fix: repeated topological-sort graph should reuse static inputs");

    let calls = dispatcher
        .calls
        .lock()
        .expect("Fix: recording toposort dispatcher calls lock should not be poisoned");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0][0], calls[1][0]);
    assert_eq!(calls[0][1], calls[1][1]);
    assert_eq!(calls[1][2], vec![0; 12]);
    assert_eq!(calls[1][3], vec![0; 12]);
    assert_eq!(calls[1][4], vec![0; 12]);
    assert_eq!(
        scratch
            .inputs
            .iter()
            .take(2)
            .map(Vec::capacity)
            .collect::<Vec<_>>(),
        static_capacities
    );
    assert_eq!(scratch.program_builds(), 1);
}

#[test]
fn topo_order_csr_static_graph_identity_is_primitive_owned() {
    let root_source = include_str!("../mod.rs");
    let dispatch_source = include_str!("../dispatch.rs");

    assert!(root_source.contains("ToposortCsrStaticInputKey"));
    assert!(!root_source.contains("struct ToposortStaticInputKey"));
    assert!(dispatch_source.contains(".static_input_key(offsets, targets)"));
    assert!(!dispatch_source.contains("fingerprint_u32_slice"));
}

#[test]
fn topo_order_csr_via_rejects_cycle_like_partial_output() {
    let err = topo_order_csr_via(&ToposortDispatcher, 2, &[0, 1, 2], &[1, 0]).unwrap_err();
    assert!(matches!(err, DispatchError::BackendError(_)));
}

#[test]
fn topo_order_csr_via_uses_primitive_order_contract() {
    struct InvertedOrderDispatcher;

    impl OptimizerDispatcher for InvertedOrderDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(vec![u32_slice_to_le_bytes(&[1, 0])])
        }
    }

    let err = topo_order_csr_via(&InvertedOrderDispatcher, 2, &[0, 1, 1], &[1]).unwrap_err();
    assert!(matches!(err, DispatchError::BackendError(_)));
}

#[test]
fn topo_order_csr_via_rejects_bad_csr() {
    let err = topo_order_csr_via(&ToposortDispatcher, 2, &[0, 2, 1], &[1]).unwrap_err();
    assert!(matches!(err, DispatchError::BadInputs(_)));
}

#[test]
fn production_source_keeps_cpu_toposort_helpers_out_of_via_path() {
    let source = include_str!("../dispatch.rs");
    let via_section = source
        .split("pub fn topo_order_csr_via")
        .nth(1)
        .expect("Fix: via section should exist")
        .split("fn map_toposort_csr_error")
        .next()
        .expect("Fix: dispatch section should end before error mapping");

    assert!(!via_section.contains("_cpu"));
    assert!(!via_section.contains("reference_"));
    assert!(!via_section.contains("fill_"));
}

#[test]
fn test_dispatcher_uses_primitive_csr_oracle_not_local_kahn_clone() {
    let source = include_str!("mod.rs");
    let dispatcher_section = source
        .split("struct ToposortDispatcher;")
        .nth(1)
        .expect("Fix: test dispatcher section should exist")
        .split("#[test]\n    fn topo_order_csr_via_dispatches_primitive_order")
        .next()
        .expect("Fix: dispatcher section should end before dispatch tests");

    assert!(dispatcher_section.contains("toposort_csr_into"));
    assert!(
            !dispatcher_section.contains("indeg")
                && !dispatcher_section.contains("queue")
                && !dispatcher_section.contains("while let Some"),
            "Fix: self-substrate topological-sort tests must not maintain a second Kahn implementation; use the primitive CSR oracle."
        );
}
