use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_foundation::ir::Program;
use vyre_primitives::graph::union_find::{union_find_dispatch_grid, union_find_program};

#[test]
fn builds_backend_neutral_union_find_program() {
    let program = union_find_alias_program("parent", "a", "b", 16, 8);
    assert_eq!(program.buffers().len(), 3);
    assert_eq!(program.entry_op_id(), None);
}

#[test]
fn substrate_program_matches_primitive_shape() {
    let substrate = union_find_alias_program("parent", "a", "b", 16, 8);
    let primitive = union_find_program("parent", "a", "b", 16, 8);
    assert_eq!(substrate.buffers(), primitive.buffers());
    assert_eq!(substrate.workgroup_size(), primitive.workgroup_size());
}

#[test]
fn substrate_no_longer_emits_target_text() {
    let program = union_find_alias_program("parent", "a", "b", 16, 8);
    let dump = format!("{program:#?}");
    assert!(dump.contains("Atomic"));
    assert!(!dump.contains("ptr<storage"));
    assert!(!dump.contains("atomicCAS"));
}

struct UnionFindDispatcher;

impl OptimizerDispatcher for UnionFindDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 3);
        let mut parent = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let edge_a = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let edge_b = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
        assert_eq!(
            grid_override,
            Some(union_find_dispatch_grid(edge_a.len() as u32))
        );
        fn find(parent: &mut [u32], mut x: u32) -> u32 {
            while parent[x as usize] != x {
                let next = parent[x as usize];
                parent[x as usize] = parent[next as usize];
                x = next;
            }
            x
        }
        for (&a, &b) in edge_a.iter().zip(edge_b.iter()) {
            if a as usize >= parent.len() || b as usize >= parent.len() {
                continue;
            }
            let ra = find(&mut parent, a);
            let rb = find(&mut parent, b);
            if ra != rb {
                let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
                parent[hi as usize] = lo;
            }
        }
        Ok(vec![u32_slice_to_le_bytes(&parent)])
    }
}

#[test]
fn union_find_alias_via_dispatches_primitive() {
    let parent = vec![0, 1, 2, 3];
    let out = union_find_alias_via(&UnionFindDispatcher, &parent, &[0, 2], &[1, 3]).unwrap();

    assert_eq!(
        canonicalize_parent_to_roots(&out),
        canonicalize_parent_to_roots(&reference_union_find_alias(&parent, &[0, 2], &[1, 3]))
    );
}

#[test]
fn union_find_alias_via_dispatches_multi_block_edge_batch() {
    let node_count = 1026u32;
    let parent: Vec<u32> = (0..node_count).collect();
    let edge_a: Vec<u32> = (0..(node_count - 1)).collect();
    let edge_b: Vec<u32> = (1..node_count).collect();

    let out = union_find_alias_via(&UnionFindDispatcher, &parent, &edge_a, &edge_b).unwrap();
    let roots = canonicalize_parent_to_roots(&out);

    assert_eq!(union_find_dispatch_grid(edge_a.len() as u32), [5, 1, 1]);
    assert!(roots.iter().all(|&root| root == 0));
}

#[test]
fn union_find_alias_via_into_reuses_output() {
    let parent = vec![0, 1, 2, 3];
    let mut out = Vec::with_capacity(8);
    let ptr = out.as_ptr();

    union_find_alias_via_into(&UnionFindDispatcher, &parent, &[0, 2], &[1, 3], &mut out).unwrap();

    assert_eq!(out.as_ptr(), ptr);
    assert_eq!(canonicalize_parent_to_roots(&out), vec![0, 0, 2, 2]);
}

#[test]
fn union_find_alias_via_with_scratch_reuses_dispatch_and_output_storage() {
    let parent = vec![0, 1, 2, 3];
    let mut scratch = UnionFindGpuScratch::default();
    let mut out = Vec::with_capacity(4);

    union_find_alias_via_with_scratch_into(
        &UnionFindDispatcher,
        &parent,
        &[0, 2],
        &[1, 3],
        &mut scratch,
        &mut out,
    )
    .unwrap();

    let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    let out_capacity = out.capacity();

    union_find_alias_via_with_scratch_into(
        &UnionFindDispatcher,
        &parent,
        &[0, 1],
        &[2, 3],
        &mut scratch,
        &mut out,
    )
    .unwrap();

    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(out.capacity(), out_capacity);
    assert_eq!(canonicalize_parent_to_roots(&out), vec![0, 1, 0, 1]);
}

#[test]
fn union_find_alias_via_with_scratch_refreshes_same_shape_parent_content() {
    let mut scratch = UnionFindGpuScratch::default();
    let mut out = Vec::with_capacity(4);

    union_find_alias_via_with_scratch_into(
        &UnionFindDispatcher,
        &[0, 1, 2, 3],
        &[0],
        &[1],
        &mut scratch,
        &mut out,
    )
    .unwrap();
    assert_eq!(canonicalize_parent_to_roots(&out), vec![0, 0, 2, 3]);

    union_find_alias_via_with_scratch_into(
        &UnionFindDispatcher,
        &[0, 1, 1, 3],
        &[2],
        &[3],
        &mut scratch,
        &mut out,
    )
    .unwrap();
    assert_eq!(canonicalize_parent_to_roots(&out), vec![0, 1, 1, 1]);
}

#[test]
fn union_find_alias_via_rejects_mismatched_edges() {
    let err = union_find_alias_via(&UnionFindDispatcher, &[0, 1], &[0], &[1, 0]).unwrap_err();

    assert!(matches!(err, DispatchError::BadInputs(_)));
}

#[test]
fn union_find_alias_via_rejects_malformed_parent_links() {
    let err = union_find_alias_via(&UnionFindDispatcher, &[0, 9], &[0], &[1]).unwrap_err();

    assert!(matches!(err, DispatchError::BadInputs(_)));
    assert!(err.to_string().contains("parent_init[1]=9"));
}

#[test]
fn union_find_alias_via_rejects_empty_parent_with_edges_before_dispatch() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("Fix: invalid empty-parent union-find input must not dispatch");
        }
    }

    let err = union_find_alias_via(&NoDispatch, &[], &[0], &[0])
        .expect_err("edges against empty parent set must be rejected");
    assert!(matches!(err, DispatchError::BadInputs(_)));
    assert!(err.to_string().contains("empty parent set"));
}

#[test]
fn union_find_alias_via_empty_edges_returns_parent_without_dispatch() {
    struct NoDispatch;

    impl OptimizerDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            panic!("Fix: empty union-find edge set must not submit a zero-work GPU dispatch");
        }
    }

    let mut out = Vec::with_capacity(8);
    union_find_alias_via_into(&NoDispatch, &[0, 1, 2], &[], &[], &mut out)
        .expect("Fix: empty union-find edge set must return parent_init");
    assert_eq!(out, vec![0, 1, 2]);
}

#[test]
fn release_path_does_not_export_union_find_reference_oracles() {
    let dispatch_source = include_str!("../dispatch.rs");
    let reference_source = include_str!("../reference.rs");
    let via_section = dispatch_source
        .split("pub fn union_find_alias_via(")
        .nth(1)
        .expect("Fix: release union-find via function must exist")
        .split("pub fn union_find_alias_via_with_scratch_into")
        .next()
        .expect("Fix: scratch-backed union-find function follows allocating wrapper");
    assert!(
        !via_section.contains("reference_union_find_alias")
            && !via_section.contains("canonicalize_parent_to_roots"),
        "release union-find path must not depend on host reference or canonicalization helpers"
    );
    assert!(
        reference_source.contains("#[cfg(any(test, feature = \"cpu-parity\"))]\n#[must_use]\npub fn reference_union_find_alias"),
        "union-find host reference must be compiled only for parity tests or explicit cpu-parity harnesses"
    );
    assert!(dispatch_source.contains("refresh_keyed_dispatch_inputs("));
    assert!(!dispatch_source.contains("dispatch_single_u32_output_into"));
}
