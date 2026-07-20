//! Interprocedural callee-before-caller pass dispatch via #74 level_wave (#74 self-consumer).
//!
//! Closes the recursion thesis for #74  -  `level_wave_program` ships to
//! user dialects (whole-schema migrations, BFS layering, breadth-first
//! graph rewrites) AND drives vyre's interprocedural pass dispatch
//! when callees must finish before callers start.
//!
//! # The self-use
//!
//! Every interprocedural pass that walks a call graph has the same
//! shape: visit each function in callee-before-caller order, run a
//! per-function body, barrier between depth waves. Without
//! `level_wave_program`, each backend hand-codes that loop on the
//! host (one dispatch per depth, host-side termination check). With
//! it, the entire BFS becomes one Program and one dispatch.
//!
//! # Algorithm
//!
//! ```text
//! 1. Caller computes per-function depth in the call graph (leaves at
//!    depth 0, increasing toward main).
//! 2. Caller hands `step_body` (the per-function rewrite/analysis body)
//!    plus the depth array to `build_callee_before_caller_program`.
//! 3. Returned Program runs the body for every function at depth `d`,
//!    barriers, then advances to depth `d+1`  -  all in one dispatch.
//! ```
//!
//! P-DRIVER-10: every interprocedural callee-before-caller pass should
//! consume this rather than hand-rolling a host depth loop.

use vyre_foundation::ir::{Node, Program};
use vyre_primitives::graph::level_wave::level_wave_program;

/// Build a Program that visits every function in callee-before-caller
/// order using GPU-side level-wave dispatch.
///
/// `step_body`: per-function body. Reads/writes any caller-declared
/// buffer via `Expr::InvocationId { axis: 0 }` to address the function
/// being visited.
///
/// `depth_buf`: name of the buffer containing per-function depth in the
/// call graph (leaves at 0).
///
/// `max_depth`: number of waves (i.e., `max(depth) + 1`).
///
/// `function_count`: total functions in the dispatch grid.
#[must_use]
pub fn build_callee_before_caller_program(
    step_body: Vec<Node>,
    depth_buf: &str,
    max_depth: u32,
    function_count: u32,
) -> Program {
    use crate::observability::{bump, level_wave_pass_calls};
    bump(&level_wave_pass_calls);
    level_wave_program(step_body, depth_buf, max_depth, function_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_nonempty_program() {
        let body = vec![Node::barrier()];
        let program = build_callee_before_caller_program(body, "depths", 4, 16);
        assert_ne!(program.entry().len(), 0);
    }

    #[test]
    fn zero_depth_still_builds() {
        let body = vec![Node::barrier()];
        let program = build_callee_before_caller_program(body, "depths", 0, 1);
        // Even at depth=0 the wrapper builds a valid (empty-loop) Program.
        // Workgroup size matches the primitive's [256,1,1] standard tile.
        assert_eq!(program.workgroup_size(), [256, 1, 1]);
        assert!(!program.buffers().is_empty());
    }
}
