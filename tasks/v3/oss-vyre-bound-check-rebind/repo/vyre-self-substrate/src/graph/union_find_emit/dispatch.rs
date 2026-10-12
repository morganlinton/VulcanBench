use vyre_foundation::ir::Program;
use vyre_primitives::graph::union_find::{
    union_find_dispatch_grid, union_find_program, validate_union_find_inputs,
};

use crate::graph::dispatch_bridge::{
    dispatch_single_u32_output_from_prepared_into, fingerprint_u32_slice,
    refresh_keyed_dispatch_inputs, DispatchInput, U32SliceFingerprint,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Caller-owned GPU dispatch scratch for union-find emission.
#[derive(Debug, Default)]
pub struct UnionFindGpuScratch {
    pub(super) inputs: Vec<Vec<u8>>,
    pub(super) static_input_key: Option<UnionFindStaticInputKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UnionFindStaticInputKey {
    parent_init: U32SliceFingerprint,
    edge_a: U32SliceFingerprint,
    edge_b: U32SliceFingerprint,
    node_words: usize,
    edge_storage_words: usize,
}

/// Build the union-find alias-analysis program and record that the substrate
/// requested a dataflow-fixpoint primitive.
#[must_use]
pub fn union_find_alias_program(
    parent: &str,
    edge_a: &str,
    edge_b: &str,
    node_count: u32,
    edge_count: u32,
) -> Program {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    union_find_program(parent, edge_a, edge_b, node_count, edge_count)
}

/// GPU dispatch wrapper for the batched union-find primitive. Builds
/// the `union_find_program`, dispatches it through `dispatcher`, and
/// returns the post-batch parent vector. The backend owns union and
/// path-compression execution; host reference helpers are compiled only
/// for parity tests.
///
/// # Errors
///
/// Propagates any [`DispatchError`] surfaced by the dispatcher.
pub fn union_find_alias_via(
    dispatcher: &dyn OptimizerDispatcher,
    parent_init: &[u32],
    edge_a: &[u32],
    edge_b: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut parent = Vec::new();
    union_find_alias_via_into(dispatcher, parent_init, edge_a, edge_b, &mut parent)?;
    Ok(parent)
}

/// GPU dispatch wrapper for the batched union-find primitive into caller-owned
/// output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when inputs are malformed, dispatch fails, or the
/// backend returns a malformed parent buffer.
pub fn union_find_alias_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    parent_init: &[u32],
    edge_a: &[u32],
    edge_b: &[u32],
    parent_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = UnionFindGpuScratch::default();
    union_find_alias_via_with_scratch_into(
        dispatcher,
        parent_init,
        edge_a,
        edge_b,
        &mut scratch,
        parent_out,
    )
}

/// GPU dispatch wrapper for the batched union-find primitive into caller-owned
/// dispatch and output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when inputs are malformed, dispatch fails, or the
/// backend returns a malformed parent buffer.
pub fn union_find_alias_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    parent_init: &[u32],
    edge_a: &[u32],
    edge_b: &[u32],
    scratch: &mut UnionFindGpuScratch,
    parent_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let layout = validate_union_find_inputs(parent_init, edge_a, edge_b)
        .map_err(DispatchError::BadInputs)?;
    if layout.node_count == 0 {
        parent_out.clear();
        return Ok(());
    }
    if layout.edge_count == 0 {
        parent_out.clear();
        parent_out.extend_from_slice(parent_init);
        return Ok(());
    }

    let program = union_find_alias_program(
        "parent",
        "edge_a",
        "edge_b",
        layout.node_count,
        layout.edge_count,
    );

    let inputs = [
        DispatchInput::u32_slice(parent_init),
        DispatchInput::u32_slice_or_zero_words(
            edge_a,
            layout.edge_storage_words,
            "union_find_alias_via edge_a",
        ),
        DispatchInput::u32_slice_or_zero_words(
            edge_b,
            layout.edge_storage_words,
            "union_find_alias_via edge_b",
        ),
    ];
    let input_key = UnionFindStaticInputKey {
        parent_init: fingerprint_u32_slice(parent_init),
        edge_a: fingerprint_u32_slice(edge_a),
        edge_b: fingerprint_u32_slice(edge_b),
        node_words: layout.node_words,
        edge_storage_words: layout.edge_storage_words,
    };
    refresh_keyed_dispatch_inputs(
        &mut scratch.inputs,
        &mut scratch.static_input_key,
        input_key,
        &inputs,
        &[],
    )?;
    dispatch_single_u32_output_from_prepared_into(
        dispatcher,
        &program,
        &scratch.inputs,
        layout.node_words,
        "union_find_alias_via",
        Some(union_find_dispatch_grid(layout.edge_count)),
        parent_out,
    )
}
