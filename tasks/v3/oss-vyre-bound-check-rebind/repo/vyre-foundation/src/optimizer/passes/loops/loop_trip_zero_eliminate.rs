//! `loop_trip_zero_eliminate`  -  drop `Node::Loop` whose compile-time-known
//! trip count is zero.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_trip_zero_eliminate`.
//! Soundness: `Exact` over the `from..to` Loop semantics  -  when both bounds
//! are u32 literals and `from >= to`, the body is dead-code by construction
//! and the loop never executes. Cost-direction: monotone-down on every
//! tracked dimension (dropping a Loop strictly reduces node_count,
//! control_flow_count, instruction_count). Preserves: every analysis.
//! Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Loop { var, from: LitU32(a), to: LitU32(b), body }
//!   where a >= b
//!   →  Node::Block(vec![])
//! ```
//!
//! The `Block(vec![])` is canonical-empty  -  downstream passes
//! (`canonicalize` + `dce`) collapse the empty Block to nothing,
//! shrinking the surrounding sequence.
//!
//! Why a dedicated pass and not folded into `loop_unroll`:
//!   - `loop_unroll` only fires when trip count ≤ MAX_UNROLL_TRIP_COUNT
//!     (currently 16) AND the body cost is bounded. Trip-zero violates
//!     neither but its handling is an unconditional drop, not an unroll.
//!   - Empty-loop elimination is a precondition for further fusion: a
//!     downstream pass that sees `Loop` may emit a barrier/sync conservatively;
//!     dropping the loop first lets the downstream pass take a faster path.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Drop loops whose `from..to` range is empty at compile time.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_trip_zero_eliminate",
    requires = ["const_fold"],
    invalidates = []
)]
pub struct LoopTripZeroEliminatePass;

impl LoopTripZeroEliminatePass {
    /// Skip programs without any compile-time-empty loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_empty_loop))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; replace every `Node::Loop` with empty trip
    /// count by `Node::Block(vec![])`. Recurses into bodies so nested
    /// empty loops are also caught.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|node| eliminate_node(node, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants. After recursion, if `node` itself
/// is a literal-bounded empty Loop, replace it with `Block(vec![])`.
fn eliminate_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| eliminate_node(child, changed));
    if is_empty_loop(&recursed) {
        *changed = true;
        Node::Block(Vec::new())
    } else {
        recursed
    }
}

/// True iff `node` is a `Loop` whose `from..to` range is empty at compile time.
fn is_empty_loop(node: &Node) -> bool {
    if let Node::Loop { from, to, .. } = node {
        match (from, to) {
            (Expr::LitU32(a), Expr::LitU32(b)) => return *a >= *b,
            (Expr::LitI32(a), Expr::LitI32(b)) => return *a >= *b,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    /// Count `Node::Loop` occurrences anywhere in the program tree.
    fn count_loops(node: &Node) -> usize {
        match node {
            Node::Loop { body, .. } => 1 + body.iter().map(count_loops).sum::<usize>(),
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_loops).sum::<usize>()
                    + otherwise.iter().map(count_loops).sum::<usize>()
            }
            Node::Block(body) => body.iter().map(count_loops).sum(),
            Node::Region { body, .. } => body.iter().map(count_loops).sum(),
            _ => 0,
        }
    }

    fn make_loop(from: u32, to: u32, body: Vec<Node>) -> Node {
        Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(from),
            to: Expr::u32(to),
            body,
        }
    }

    #[test]
    fn empty_range_loop_dropped() {
        // Loop from 5 to 3  -  never executes.
        let entry = vec![make_loop(
            5,
            3,
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(result.changed);
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(
            total_loops, 0,
            "empty-range loop must be dropped; got {total_loops} loops remaining"
        );
    }

    #[test]
    fn equal_bounds_loop_dropped() {
        // Loop from 5 to 5  -  never executes (half-open range).
        let entry = vec![make_loop(
            5,
            5,
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(result.changed);
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(total_loops, 0);
    }

    #[test]
    fn non_empty_range_loop_kept() {
        // Loop from 0 to 10  -  executes 10 times. Must NOT be dropped.
        let entry = vec![make_loop(
            0,
            10,
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(!result.changed, "non-empty loop must be preserved");
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(total_loops, 1);
    }

    #[test]
    fn non_constant_bounds_loop_kept() {
        // Loop bounds reference Var; this pass conservatively skips.
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::var("start"),
            to: Expr::var("stop"),
            body: vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        }];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(
            !result.changed,
            "loops with non-literal bounds must be kept because the runtime trip count is unknown"
        );
    }

    #[test]
    fn nested_empty_loop_inside_outer_loop_dropped() {
        // Outer loop (1..3) contains an empty loop (0..0). The empty
        // INNER loop is dropped; the outer loop's body becomes empty.
        let inner_empty = make_loop(0, 0, vec![Node::store("buf", Expr::u32(0), Expr::u32(7))]);
        let outer = make_loop(1, 3, vec![inner_empty]);
        let entry = vec![outer];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(result.changed);
        // Outer loop kept; inner dropped.
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(
            total_loops, 1,
            "outer non-empty loop kept; inner empty loop dropped; got {total_loops}"
        );
    }

    #[test]
    fn analyze_skips_program_with_no_empty_loops() {
        let entry = vec![make_loop(0, 10, vec![])];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopTripZeroEliminatePass, &program),
            PassAnalysis::SKIP,
            "analyze must SKIP programs with no compile-time-empty loops"
        );
    }

    #[test]
    fn analyze_runs_for_program_with_one_empty_loop() {
        let entry = vec![make_loop(5, 3, vec![])];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopTripZeroEliminatePass, &program),
            PassAnalysis::RUN,
            "analyze must RUN when at least one compile-time-empty loop exists"
        );
    }

    // ── Task 1: i32 bounds ──────────────────────────────────────────────

    fn make_loop_i32(from: i32, to: i32, body: Vec<Node>) -> Node {
        Node::Loop {
            var: Ident::from("i"),
            from: Expr::i32(from),
            to: Expr::i32(to),
            body,
        }
    }

    #[test]
    fn i32_swapped_bounds_collapses() {
        // Loop from 5i32 to 3i32  -  never executes.
        let entry = vec![make_loop_i32(
            5,
            3,
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(
            result.changed,
            "i32 swapped bounds must trigger elimination"
        );
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(
            total_loops, 0,
            "i32 swapped-bounds loop must be dropped; got {total_loops} loops"
        );
    }

    #[test]
    fn i32_equal_bounds_collapses() {
        // Loop from 5i32 to 5i32  -  never executes (half-open range).
        let entry = vec![make_loop_i32(
            5,
            5,
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        let result = LoopTripZeroEliminatePass::transform(program);
        assert!(result.changed, "i32 equal bounds must trigger elimination");
        let total_loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert_eq!(total_loops, 0);
    }
}
