//! `empty_block_collapse`  -  drop `Node::Block(vec![])` from sibling sequences.
//!
//! Op id: `vyre-foundation::optimizer::passes::empty_block_collapse`.
//! Soundness: `Exact`  -  empty Blocks have no observable behavior. Removing
//! them strictly shrinks the IR. Cost-direction: monotone-down on
//! node_count + control_flow_count + ir_heap_allocations. Preserves: every
//! analysis. Invalidates: nothing.
//!
//! ## Why
//!
//! Other passes leave empty `Node::Block(vec![])` markers behind:
//!   - `loop_trip_zero_eliminate` replaces a dropped Loop with `Block([])`.
//!   - `dce` collapses an entire dead branch to `Block([])`.
//!   - Barrier/synchronization cleanup can leave empty control markers
//!     visible to downstream consumers.
//!
//! Without an empty-block collapse, those markers persist through
//! lowering  -  adding noise to the wire format, the codegen output, and
//! the cost certificate. This pass is the cleanup.
//!
//! ## Rule
//!
//! ```text
//! [..., Node::Block(vec![]), ...]  →  [..., ...]
//! ```
//!
//! Empty Block dropped from any sibling sequence at any nesting level.
//! Recurses into If/Loop/Block/Region bodies.

use crate::ir::{Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Drop empty `Node::Block(vec![])` markers from sibling sequences.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "empty_block_collapse",
    requires = [],
    invalidates = []
)]
pub struct EmptyBlockCollapsePass;

impl EmptyBlockCollapsePass {
    /// Skip programs without any empty Block.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // No Block in the program at all → no empty Block to collapse.
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_BLOCK)
        {
            return PassAnalysis::SKIP;
        }
        if program.entry().iter().any(|n| {
            node_map::any_descendant(
                n,
                &mut |child| matches!(child, Node::Block(b) if b.is_empty()),
            )
        }) {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; drop every `Node::Block(vec![])` from
    /// sibling sequences at every nesting level.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            drop_empty_blocks(
                entry
                    .into_iter()
                    .map(|n| collapse_node(n, &mut changed))
                    .collect(),
                &mut changed,
            )
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants (via `node_map::map_children`) and
/// then prune empty Block children from the resulting body sequence.
fn collapse_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| collapse_node(child, changed));
    node_map::map_body(recursed, &mut |body| drop_empty_blocks(body, changed))
}

/// Drop `Node::Block(vec![])` siblings from a body sequence, flipping
/// `changed` when at least one is dropped.
fn drop_empty_blocks(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut out = Vec::with_capacity(body.len());
    for node in body {
        match &node {
            Node::Block(inner) if inner.is_empty() => {
                *changed = true;
            }
            _ => out.push(node),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn count_empty_blocks(node: &Node) -> usize {
        let mut count = 0;
        match node {
            Node::Block(body) => {
                if body.is_empty() {
                    count += 1;
                }
                for child in body {
                    count += count_empty_blocks(child);
                }
            }
            Node::If {
                then, otherwise, ..
            } => {
                for n in then {
                    count += if matches!(n, Node::Block(b) if b.is_empty()) {
                        1
                    } else {
                        0
                    };
                    count += count_empty_blocks(n);
                }
                for n in otherwise {
                    count += if matches!(n, Node::Block(b) if b.is_empty()) {
                        1
                    } else {
                        0
                    };
                    count += count_empty_blocks(n);
                }
            }
            Node::Loop { body, .. } => {
                for n in body {
                    count += if matches!(n, Node::Block(b) if b.is_empty()) {
                        1
                    } else {
                        0
                    };
                    count += count_empty_blocks(n);
                }
            }
            Node::Region { body, .. } => {
                for n in body.iter() {
                    count += if matches!(n, Node::Block(b) if b.is_empty()) {
                        1
                    } else {
                        0
                    };
                    count += count_empty_blocks(n);
                }
            }
            _ => {}
        }
        count
    }

    #[test]
    fn drops_empty_block_from_top_level_sequence() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::Block(Vec::new()),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(
            total, 0,
            "no empty Blocks must remain after collapse; got {total}"
        );
    }

    #[test]
    fn drops_multiple_empty_blocks_in_sequence() {
        let entry = vec![
            Node::Block(Vec::new()),
            Node::Block(Vec::new()),
            Node::Block(Vec::new()),
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
        ];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn keeps_non_empty_blocks() {
        let entry = vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(
            !result.changed,
            "Block with content must not be touched by empty_block_collapse"
        );
    }

    #[test]
    fn drops_empty_block_inside_if_branch() {
        let entry = vec![Node::if_then(
            Expr::bool(true),
            vec![
                Node::store("buf", Expr::u32(0), Expr::u32(7)),
                Node::Block(Vec::new()),
            ],
        )];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed, "must recurse into If branches");
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn analyze_skips_program_with_no_empty_blocks() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&EmptyBlockCollapsePass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_empty_block_present() {
        let entry = vec![Node::Block(Vec::new())];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&EmptyBlockCollapsePass, &program),
            PassAnalysis::RUN
        );
    }

    // ── Task 3: adversarial twins ──────────────────────────────────────

    #[test]
    fn nested_empty_block_collapses() {
        // Block(vec![Block(vec![])])  -  the inner empty Block should be
        // dropped, leaving Block(vec![]).
        let entry = vec![Node::Block(vec![Node::Block(Vec::new())])];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed, "nested empty Block must trigger collapse");
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(
            total, 0,
            "nested empty block must collapse; got {total} empty blocks"
        );
    }

    #[test]
    fn block_with_store_is_preserved() {
        // Block(vec![Store(...)]) must NOT be touched  -  it has content.
        let entry = vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(
            !result.changed,
            "Block with Store content must not be removed by empty_block_collapse"
        );
    }
    #[test]
    fn adversarial_empty_region_inside_block() {
        let entry = vec![Node::Block(vec![Node::Region {
            body: std::sync::Arc::new(vec![]),
            generator: "test".into(),
            source_region: None,
        }])];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(
            !result.changed,
            "empty Region is not an empty Block, should not collapse"
        );
    }

    #[test]
    fn adversarial_three_levels_nested_empty_blocks() {
        let entry = vec![Node::Block(vec![Node::Block(vec![
            Node::Block(Vec::new()),
        ])])];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(total, 0, "all 3 levels must collapse bottom-up");
    }

    #[test]
    fn adversarial_empty_block_sibling_alongside_store() {
        let entry = vec![
            Node::Block(Vec::new()),
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::Block(Vec::new()),
        ];
        let program = program_with_entry(entry);
        let result = EmptyBlockCollapsePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_empty_blocks).sum();
        assert_eq!(total, 0);
        assert!(!result.program.entry().is_empty());
    }
}
