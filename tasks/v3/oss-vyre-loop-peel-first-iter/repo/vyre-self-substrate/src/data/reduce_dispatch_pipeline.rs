//! Self-substrate wrappers for reduction-oriented primitive dispatch.
//!
//! These helpers keep reduction planning in `vyre-self-substrate` while the
//! executable IR and reference contracts stay in `vyre-primitives`.

use vyre_foundation::ir::{Node, Program};
use vyre_primitives::reduce::{
    multi_block_prefix_scan::{
        multi_block_prefix_scan_sum_u32, pass_a_local_scan, pass_c_broadcast_offsets, BLOCK_LANES,
    },
    radix_sort::radix_sort,
    range_counts::{range_counts_u32, range_counts_u32_body, range_counts_u32_child},
    workgroup_any::{
        workgroup_any_u32, workgroup_any_u32_body, workgroup_any_u32_body_prefixed,
        workgroup_any_u32_child, workgroup_any_u32_child_prefixed,
    },
    workgroup_tree::{
        max_f32_child, sum_f32_child, sum_u32_child, workgroup_max_f32, workgroup_sum_f32,
        workgroup_sum_u32, WorkgroupReductionScope,
    },
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::reduce::{
    multi_block_prefix_scan::cpu_ref as primitive_prefix_sum,
    radix_sort::cpu_ref as primitive_radix_sort, range_counts::cpu_ref as primitive_range_count,
    workgroup_any::cpu_ref as primitive_workgroup_any,
};

/// Build the self-substrate f32 workgroup sum dispatch program.
#[must_use]
pub fn dispatch_workgroup_sum_f32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    workgroup_sum_f32(values, out, count, tile)
}

/// Build the self-substrate u32 workgroup sum dispatch program.
#[must_use]
pub fn dispatch_workgroup_sum_u32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    workgroup_sum_u32(values, out, count, tile)
}

/// Build the self-substrate f32 workgroup max dispatch program.
#[must_use]
pub fn dispatch_workgroup_max_f32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    workgroup_max_f32(values, out, count, tile)
}

/// Emit a child f32 sum stage for a larger reduction pipeline.
#[must_use]
pub fn child_sum_f32_stage(parent_op_id: &str, tile: u32, scratch: &'static str) -> Node {
    sum_f32_child(
        parent_op_id,
        tile,
        scratch,
        WorkgroupReductionScope::EveryWorkgroup,
    )
}

/// Emit a child u32 sum stage for a larger reduction pipeline.
#[must_use]
pub fn child_sum_u32_stage(parent_op_id: &str, tile: u32, scratch: &'static str) -> Node {
    sum_u32_child(
        parent_op_id,
        tile,
        scratch,
        WorkgroupReductionScope::EveryWorkgroup,
    )
}

/// Emit a child f32 max stage for a larger reduction pipeline.
#[must_use]
pub fn child_max_f32_stage(parent_op_id: &str, tile: u32, scratch: &'static str) -> Node {
    max_f32_child(
        parent_op_id,
        tile,
        scratch,
        WorkgroupReductionScope::EveryWorkgroup,
    )
}

/// Build a range-count accumulator body for histogram-derived scheduling.
#[must_use]
pub fn range_count_accumulator_body(
    histogram: &str,
    out_var: &str,
    start: u32,
    end: u32,
) -> Vec<Node> {
    range_counts_u32_body(histogram, out_var, start, end)
}

/// Emit a child range-count stage for a parent pipeline.
#[must_use]
pub fn child_range_count_stage(
    parent_op_id: &str,
    histogram: &str,
    out_var: &str,
    start: u32,
    end: u32,
) -> Node {
    range_counts_u32_child(parent_op_id, histogram, out_var, start, end)
}

/// Build a standalone range-count dispatch program.
#[must_use]
pub fn dispatch_range_count_u32(histogram: &str, out: &str, start: u32, end: u32) -> Program {
    range_counts_u32(histogram, out, start, end)
}

/// Build a workgroup-any accumulator body.
#[must_use]
pub fn any_accumulator_body(values: &str, out_var: &str, count: u32) -> Vec<Node> {
    workgroup_any_u32_body(values, out_var, count)
}

/// Build a workgroup-any accumulator body with a caller-selected loop variable.
#[must_use]
pub fn prefixed_any_accumulator_body(
    values: &str,
    out_var: &str,
    count: u32,
    iter_var: &str,
) -> Vec<Node> {
    workgroup_any_u32_body_prefixed(values, out_var, count, iter_var)
}

/// Emit a child workgroup-any stage.
#[must_use]
pub fn child_any_stage(parent_op_id: &str, values: &str, out_var: &str, count: u32) -> Node {
    workgroup_any_u32_child(parent_op_id, values, out_var, count)
}

/// Emit a child workgroup-any stage with a caller-selected loop variable.
#[must_use]
pub fn prefixed_child_any_stage(
    parent_op_id: &str,
    values: &str,
    out_var: &str,
    count: u32,
    iter_var: &str,
) -> Node {
    workgroup_any_u32_child_prefixed(parent_op_id, values, out_var, count, iter_var)
}

/// Build a standalone workgroup-any dispatch program.
#[must_use]
pub fn dispatch_workgroup_any_u32(values: &str, out: &str, count: u32) -> Program {
    workgroup_any_u32(values, out, count)
}

/// Build the arbitrary-length inclusive prefix-sum pipeline.
#[must_use]
pub fn dispatch_multi_block_prefix_sum(input: &str, output: &str, n: u32) -> Program {
    multi_block_prefix_scan_sum_u32(input, output, n)
}

/// Build pass A for a resident multi-block prefix-sum chain.
#[must_use]
pub fn prefix_pass_a(input: &str, partials: &str, block_totals: &str, n: u32) -> Program {
    let num_blocks = n.div_ceil(BLOCK_LANES).max(1);
    pass_a_local_scan(input, partials, block_totals, n, num_blocks)
}

/// Build pass C for a resident multi-block prefix-sum chain.
#[must_use]
pub fn prefix_pass_c(partials: &str, block_totals_scanned: &str, output: &str, n: u32) -> Program {
    let num_blocks = n.div_ceil(BLOCK_LANES).max(1);
    pass_c_broadcast_offsets(partials, block_totals_scanned, output, n, num_blocks)
}

/// Build a stable u32 radix-sort dispatch program.
#[must_use]
pub fn dispatch_radix_sort(input: &str, output: &str, count: u32, bits: u32) -> Program {
    radix_sort(input, output, count, bits)
}

/// Reference range-count contract used by CPU parity gates.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_range_count(histogram: &[u32], start: u32, end: u32) -> u32 {
    primitive_range_count(histogram, start, end)
}

/// Reference workgroup-any contract used by CPU parity gates.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_workgroup_any(values: &[u32]) -> u32 {
    primitive_workgroup_any(values)
}

/// Reference inclusive prefix-sum contract used by CPU parity gates.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_prefix_sum(values: &[u32]) -> Vec<u32> {
    primitive_prefix_sum(values)
}

/// Reference radix-sort contract used by CPU parity gates.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_radix_sort(values: &[u32], bits: u32) -> Vec<u32> {
    primitive_radix_sort(values, bits)
}

/// Allocation-free workgroup f32 sum reference for scheduler scoring.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sum_f32(values: &[f32]) -> f32 {
    values.iter().copied().sum()
}

/// Allocation-free workgroup u32 sum reference for scheduler scoring.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sum_u32(values: &[u32]) -> u32 {
    values.iter().copied().sum()
}

/// Allocation-free workgroup f32 max reference for scheduler scoring.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_max_f32(values: &[f32]) -> f32 {
    values.iter().copied().fold(f32::MIN, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region_generator(node: &Node) -> &str {
        let Node::Region { generator, .. } = node else {
            panic!("Fix: reduction helper must emit a Region child.");
        };
        generator.as_str()
    }

    fn program_generator(program: &Program) -> &str {
        let Some(Node::Region { generator, .. }) = program.entry.first() else {
            panic!("Fix: reduction Program must start with a Region.");
        };
        generator.as_str()
    }

    #[test]
    fn program_builders_emit_expected_reduce_primitives() {
        assert_eq!(
            program_generator(&dispatch_workgroup_sum_f32("values", "out", 8, 4)),
            "vyre-primitives::reduce::workgroup_sum_f32"
        );
        assert_eq!(
            program_generator(&dispatch_workgroup_sum_u32("values", "out", 8, 4)),
            "vyre-primitives::reduce::workgroup_sum_u32"
        );
        assert_eq!(
            program_generator(&dispatch_workgroup_max_f32("values", "out", 8, 4)),
            "vyre-primitives::reduce::workgroup_max_f32"
        );
        assert_eq!(
            program_generator(&dispatch_range_count_u32("histogram", "out", 2, 5)),
            "vyre-primitives::reduce::range_counts_u32"
        );
        assert_eq!(
            program_generator(&dispatch_workgroup_any_u32("values", "out", 4)),
            "vyre-primitives::reduce::workgroup_any_u32"
        );
        assert_eq!(
            program_generator(&dispatch_radix_sort("keys", "sorted", 8, 16)),
            "vyre-primitives::reduce::radix_sort"
        );
    }

    #[test]
    fn child_builders_keep_parent_region_context() {
        let parent = "vyre-self-substrate::data::reduce_dispatch_pipeline";
        assert_eq!(
            region_generator(&child_sum_f32_stage(parent, 8, "scratch")),
            "vyre-primitives::reduce::workgroup_sum_f32"
        );
        assert_eq!(
            region_generator(&child_sum_u32_stage(parent, 8, "scratch")),
            "vyre-primitives::reduce::workgroup_sum_u32"
        );
        assert_eq!(
            region_generator(&child_max_f32_stage(parent, 8, "scratch")),
            "vyre-primitives::reduce::workgroup_max_f32"
        );
        assert_eq!(
            region_generator(&child_range_count_stage(parent, "hist", "sum", 1, 4)),
            "vyre-primitives::reduce::range_counts_u32"
        );
        assert_eq!(
            region_generator(&child_any_stage(parent, "changed", "any", 8)),
            "vyre-primitives::reduce::workgroup_any_u32"
        );
        assert_eq!(
            region_generator(&prefixed_child_any_stage(
                parent, "changed", "any", 8, "any_i"
            )),
            "vyre-primitives::reduce::workgroup_any_u32"
        );
    }

    #[test]
    fn body_builders_emit_non_empty_composable_ir() {
        assert_ne!(range_count_accumulator_body("hist", "sum", 0, 8).len(), 0);
        assert_ne!(any_accumulator_body("changed", "any", 8).len(), 0);
        assert_ne!(
            prefixed_any_accumulator_body("changed", "any", 8, "changed_i").len(),
            0
        );
    }

    #[test]
    fn prefix_pipeline_exposes_fused_and_resident_passes() {
        let small = dispatch_multi_block_prefix_sum("input", "output", 64);
        assert!(!small.buffers.is_empty());

        let large = dispatch_multi_block_prefix_sum("input", "output", BLOCK_LANES + 17);
        assert!(!large.buffers.is_empty());

        let pass_a = prefix_pass_a("input", "partials", "totals", BLOCK_LANES + 1);
        assert_eq!(
            program_generator(&pass_a),
            "vyre-primitives::reduce::multi_block_prefix_scan_inclusive_sum::pass_a"
        );

        let pass_c = prefix_pass_c("partials", "totals_scanned", "output", BLOCK_LANES + 1);
        assert_eq!(
            program_generator(&pass_c),
            "vyre-primitives::reduce::multi_block_prefix_scan_inclusive_sum::pass_c"
        );
    }

    #[test]
    fn cpu_reference_wrappers_match_primitive_contracts() {
        assert_eq!(reference_sum_f32(&[1.25, -2.0, 5.5]), 4.75);
        assert_eq!(reference_sum_u32(&[1, 2, 3, 4]), 10);
        assert_eq!(reference_max_f32(&[-7.0, 3.5, 2.0]), 3.5);
        assert_eq!(reference_range_count(&[9, 2, 3, 5, 11], 1, 4), 10);
        assert_eq!(reference_workgroup_any(&[0, 2, 4, 0]), 6);
        assert_eq!(reference_prefix_sum(&[1, 2, 3, 4]), vec![1, 3, 6, 10]);
        assert_eq!(reference_radix_sort(&[3, 1, 4, 2], 32), vec![1, 2, 3, 4]);
    }
}
