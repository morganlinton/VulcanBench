//! `aliases_dataflow`  -  bidirectional dataflow reachability over
//! the launch shape's `aliases($x, $y)` semantic.
//!
//! Pre-promotion this lived inline in an analysis stage `lower_aliases` as 9
//! `merge_programs` calls composing flows_to + bitset_or_into +
//! bitset_and. Every aliases-using launch shape (stack_overflow_*,
//! heap_overflow_*, oob_*, use_after_free_double_drop) re-emitted the
//! same composition. Promoted to a single primitive: analysis-stage callers
//! once, vyre owns the composition shape.
//!
//! ## Semantics
//!
//! `aliases($x, $y) := bitset_or(`
//! `    bitset_and(reach_from(x), y),`
//! `    bitset_and(reach_from(y), x))`
//!
//! "Either $x reaches $y, or $y reaches $x, under the dataflow graph"
//!  -  soundness `MayOver`. Catches the SSA def→use direction the
//! launch shape rules need (`$copy_dst aliases $dst` where $dst is a
//! decl and $copy_dst is the use, etc.).
//!
//! ## Lowering shape
//!
//! Composes [`flows_to`] (one BFS-step) + [`bitset_or_into`] (acc
//! merge) + [`bitset_and`] (intersect with opposite frontier) +
//! [`bitset_or_into`] (final OR into output). Caller drives the
//! one-step `flows_to` to fixpoint via the dispatcher's
//! `fixpoint_iterations` config  -  the same path single-direction
//! flows_to uses.

use std::sync::Arc;

use vyre::ir::Program;
use vyre_foundation::execution_plan::fusion::{fuse_programs, FusionError};
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};
use vyre_primitives::bitset::and::bitset_and;
use vyre_primitives::bitset::or_into::bitset_or_into;
use vyre_primitives::graph::csr_forward_traverse::bitset_words;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::security::flows_to::flows_to_alias_only;

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::security::aliases_dataflow";

/// Zero every word of a bitset buffer in-place.
///
/// `csr_forward_traverse` accumulates into `frontier_out` via `atomic_or`
/// and does not clear it first. The CPU reference (`cpu_ref_into`) starts
/// from an empty frontier each hop. Without this clear, `hop_*` buffers
/// that persist across fixpoint iterations (and are not backend-cleared
/// because they are scratch, not outputs) retain stale bits; merge then
/// ORs a polluted hop into `reach_*` and `aliases($x, $y)` misses on
/// multi-hop chains such as `uninit_callee_store.c`.
fn bitset_zero_inplace(target: &str, words: u32) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::store(target, t.clone(), Expr::u32(0))];
    Program::wrapped(
        vec![
            BufferDecl::storage(target, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from("vyre-libs::security::aliases_dataflow::zero"),
            source_region: None,
            body: Arc::new(vec![Node::if_then(Expr::lt(t, Expr::u32(words)), body)]),
        }],
    )
}

/// Build a Program: one bidirectional dataflow-aliases step.
///
/// Reads the per-node bitset frontiers `x_buf` and `y_buf`, plus
/// caller-provided scratch buffers (`reach_x_buf`, `reach_y_buf`,
/// `hop_x_buf`, `hop_y_buf`, `x_in_y_buf`, `y_in_x_buf`). Writes the
/// final per-node aliasing bitset to `out_buf`.
///
/// Caller drives convergence by setting
/// `DispatchConfig::fixpoint_iterations` to the desired BFS depth
/// (8 hops is the launch's intra-function ceiling; deeper reachability
/// requires a real bitset_fixpoint driver). Dispatch reuses the
/// persistent buffer handles across iterations so reach_x / reach_y
/// monotonically grow without host round-trips.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn aliases_dataflow(
    shape: ProgramGraphShape,
    x_buf: &str,
    y_buf: &str,
    reach_x_buf: &str,
    reach_y_buf: &str,
    hop_x_buf: &str,
    hop_y_buf: &str,
    x_in_y_buf: &str,
    y_in_x_buf: &str,
    out_buf: &str,
) -> Program {
    try_aliases_dataflow(
        shape,
        x_buf,
        y_buf,
        reach_x_buf,
        reach_y_buf,
        hop_x_buf,
        hop_y_buf,
        x_in_y_buf,
        y_in_x_buf,
        out_buf,
    )
    .unwrap_or_else(|error| {
        crate::builder::invalid_output_program(
            OP_ID,
            out_buf,
            DataType::U32,
            format!("Fix: aliases_dataflow failed to fuse: {error}"),
        )
    })
}

/// Fallible aliases-dataflow builder.
///
/// # Errors
///
/// Returns [`FusionError`] if the composed seed/hop/merge/intersect/union
/// arms cannot be fused safely.
#[allow(clippy::too_many_arguments)]
pub fn try_aliases_dataflow(
    shape: ProgramGraphShape,
    x_buf: &str,
    y_buf: &str,
    reach_x_buf: &str,
    reach_y_buf: &str,
    hop_x_buf: &str,
    hop_y_buf: &str,
    x_in_y_buf: &str,
    y_in_x_buf: &str,
    out_buf: &str,
) -> Result<Program, FusionError> {
    let words = bitset_words(shape.node_count);

    // Seed reach_x = x; reach_y = y.
    let seed_x = bitset_or_into(reach_x_buf, x_buf, words);
    let seed_y = bitset_or_into(reach_y_buf, y_buf, words);

    // Zero hop scratch, then one BFS hop (matches CPU `cpu_ref_into` clearing
    // `frontier_out` before each forward step).
    let clear_hop_x = bitset_zero_inplace(hop_x_buf, words);
    let clear_hop_y = bitset_zero_inplace(hop_y_buf, words);
    let hop_x_step = flows_to_alias_only(shape, reach_x_buf, hop_x_buf);
    let hop_y_step = flows_to_alias_only(shape, reach_y_buf, hop_y_buf);

    // Merge hops back into accumulators.
    let merge_x = bitset_or_into(reach_x_buf, hop_x_buf, words);
    let merge_y = bitset_or_into(reach_y_buf, hop_y_buf, words);

    // Per-direction intersect with the opposite endpoint.
    let intersect_x = bitset_and(reach_y_buf, x_buf, x_in_y_buf, words);
    let intersect_y = bitset_and(reach_x_buf, y_buf, y_in_x_buf, words);

    // OR both directions into out_buf.
    let union_x = bitset_or_into(out_buf, x_in_y_buf, words);
    let union_y = bitset_or_into(out_buf, y_in_x_buf, words);

    // Compose via the hazard-aware fusion path so RAW/WAR barriers
    // get inserted between writers and later readers (e.g. seed_x
    // writes reach_x_buf, hop_x_step then reads it; without a
    // SeqCst barrier between those two arms threads from a later
    // warp would observe the pre-seed reach_x_buf state and the
    // BFS frontier propagation would silently drop nodes whose
    // gid lives past the warp boundary). Per-arm composition via
    // a flat name-dedup `merge_programs` skipped this and was the
    // headline-blocker on every aliases-using rule.
    fuse_programs(&[
        seed_x,
        seed_y,
        clear_hop_x,
        clear_hop_y,
        hop_x_step,
        hop_y_step,
        merge_x,
        merge_y,
        intersect_x,
        intersect_y,
        union_x,
        union_y,
    ])
}

/// CPU oracle. Mirrors the GPU semantic over a host-side dataflow
/// graph. Caller drives the BFS to fixpoint; this single-step
/// reference returns one hop's contribution to the alias set.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref_one_step(x: &[u32], y: &[u32], reach_x: &[u32], reach_y: &[u32]) -> Vec<u32> {
    // x_in_y = reach_y AND x; y_in_x = reach_x AND y; OR.
    let n = x.len();
    let mut out = vec![0u32; n];
    for i in 0..n {
        let x_in_y = reach_y.get(i).copied().unwrap_or(0) & x[i];
        let y_in_x = reach_x.get(i).copied().unwrap_or(0) & y[i];
        out[i] = x_in_y | y_in_x;
    }
    out
}

fn witness_program() -> Program {
    aliases_dataflow(
        ProgramGraphShape::new(4, 3),
        "x",
        "y",
        "reach_x",
        "reach_y",
        "hop_x",
        "hop_y",
        "x_in_y",
        "y_in_x",
        "out",
    )
}

fn witness_words(name: &str, expected: bool) -> Vec<u32> {
    match (name, expected) {
        ("x", false) => vec![0b0001],
        ("y", false) => vec![0b0010],
        ("reach_x", false) => vec![0b0001],
        ("reach_y", false) => vec![0b0010],
        ("pg_nodes", false) => vec![0, 0, 0, 0],
        ("pg_edge_offsets", false) => vec![0, 1, 2, 3, 3],
        ("pg_edge_targets", false) => vec![1, 2, 3],
        ("pg_edge_kind_mask", false) => vec![
            edge_kind::ASSIGNMENT,
            edge_kind::ASSIGNMENT,
            edge_kind::ASSIGNMENT,
        ],
        ("pg_node_tags", false) => vec![0, 0, 0, 0],
        ("hop_x" | "hop_y" | "x_in_y" | "y_in_x" | "out", false) => vec![0],
        ("reach_x", true) => vec![0b0011],
        ("reach_y", true) => vec![0b0110],
        ("hop_x", true) => vec![0b0010],
        ("hop_y", true) => vec![0b0100],
        ("x_in_y", true) => vec![0b0000],
        ("y_in_x", true) => vec![0b0010],
        ("out", true) => vec![0b0010],
        _ => Vec::new(),
    }
}

fn witness_inputs() -> Vec<Vec<u8>> {
    witness_program()
        .buffers()
        .iter()
        .filter(|decl| !decl.is_output() && decl.access() != BufferAccess::Workgroup)
        .map(|decl| vyre_primitives::wire::pack_u32_slice(&witness_words(decl.name(), false)))
        .collect()
}

fn witness_expected_outputs() -> Vec<Vec<u8>> {
    witness_program()
        .buffers()
        .iter()
        .filter(|decl| decl.is_output() || decl.access() == BufferAccess::ReadWrite)
        .map(|decl| vyre_primitives::wire::pack_u32_slice(&witness_words(decl.name(), true)))
        .collect()
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: witness_program,
        test_inputs: Some(|| vec![witness_inputs()]),
        expected_output: Some(|| vec![witness_expected_outputs()]),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    #[test]
    fn cpu_ref_unions_two_directions() {
        // x = {0}, y = {3}, reach_x = {0,1,2,3}, reach_y = {3,2,1,0}.
        // Both directions reach the other endpoint.
        let x = vec![0b0001];
        let y = vec![0b1000];
        let reach_x = vec![0b1111];
        let reach_y = vec![0b1111];
        let out = cpu_ref_one_step(&x, &y, &reach_x, &reach_y);
        assert_eq!(out, vec![0b1001]); // x ∪ y in the alias bitset
    }

    #[test]
    fn cpu_ref_disjoint_reach_yields_zero() {
        let x = vec![0b0001];
        let y = vec![0b1000];
        let reach_x = vec![0b0001]; // x reaches only itself
        let reach_y = vec![0b1000]; // y reaches only itself
        let out = cpu_ref_one_step(&x, &y, &reach_x, &reach_y);
        assert_eq!(out, vec![0]); // no overlap
    }

    /// RAW-hazard regression. seed_x writes reach_x_buf; hop_x_step
    /// then reads it. The fused entry MUST contain a Barrier between
    /// those arms, otherwise threads in later warps observe the pre-
    /// seed state of reach_x_buf and the BFS frontier silently drops
    /// nodes past the warp boundary. The pre-fix local merge_programs
    /// produced a flat unbarriered entry  -  this test catches that
    /// regression.
    #[test]
    fn fused_entry_contains_barrier_between_raw_arms() {
        let p = aliases_dataflow(
            ProgramGraphShape::new(64, 16),
            "x",
            "y",
            "rx",
            "ry",
            "hx",
            "hy",
            "xy",
            "yx",
            "out",
        );
        // The fused Program is wrapped in a Region; flatten one level
        // to inspect the per-arm entry sequence.
        let mut barrier_count = 0usize;
        fn count_barriers(node: &Node, n: &mut usize) {
            match node {
                Node::Barrier { .. } => *n += 1,
                Node::Region { body, .. } => {
                    for child in body.iter() {
                        count_barriers(child, n);
                    }
                }
                _ => {}
            }
        }
        for node in p.entry.iter() {
            count_barriers(node, &mut barrier_count);
        }
        assert!(
            barrier_count >= 1,
            "aliases_dataflow fused program has no barriers; RAW hazards \
             between seed/hop/merge/intersect/union arms will race. \
             Found {} barriers in the entry tree.",
            barrier_count
        );
    }

    /// Buffer-binding uniqueness regression. The pre-fix local
    /// merge_programs preserved per-sub-program binding indices
    /// verbatim, so e.g. seed_x's reach_x_buf at binding 0 and
    /// seed_y's reach_y_buf at binding 0 collided in the merged
    /// declaration table. fuse_programs renumbers every non-Workgroup
    /// buffer with a fresh `next_binding` slot  -  this test pins
    /// that contract so a future refactor can't silently regress it.
    #[test]
    fn fused_program_has_unique_non_workgroup_bindings() {
        use vyre_foundation::ir::BufferAccess;
        let p = aliases_dataflow(
            ProgramGraphShape::new(64, 16),
            "x",
            "y",
            "rx",
            "ry",
            "hx",
            "hy",
            "xy",
            "yx",
            "out",
        );
        let mut bindings: Vec<u32> = p
            .buffers
            .iter()
            .filter(|b| b.access != BufferAccess::Workgroup)
            .map(|b| b.binding)
            .collect();
        bindings.sort_unstable();
        let mut deduped = bindings.clone();
        deduped.dedup();
        assert_eq!(
            bindings, deduped,
            "duplicate non-Workgroup bindings in fused aliases_dataflow program: {:?}",
            bindings
        );
    }
}
