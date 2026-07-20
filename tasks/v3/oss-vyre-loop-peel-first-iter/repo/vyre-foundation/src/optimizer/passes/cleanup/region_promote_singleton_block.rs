//! `region_promote_singleton_block`  -  unwrap `Region { body: [Block(inner)] }`
//! to `Region { body: inner }`.
//!
//! Op id: `vyre-foundation::optimizer::passes::region_promote_singleton_block`.
//! Soundness: `Exact`  -  `Node::Block` is a transparent container with no
//! observable behavior beyond grouping its children. A Region whose entire
//! body is a single `Block(inner)` is observationally equivalent to a Region
//! whose body is `inner` directly. Cost-direction: monotone-down on
//! `node_count`, `control_flow_count`, and `ir_heap_allocations` (Region
//! body Arc shrinks by one Vec layer). Preserves: every analysis. Invalidates:
//! nothing.
//!
//! ## Why
//!
//! Multiple substrate paths emit `Region { body: vec![Block(inner)] }`
//! shapes:
//!   - The default `Node::Region` constructor in primitive builders
//!     wraps the body in `Block` for visual symmetry with `If`/`Loop`.
//!   - `dce` collapses a multi-statement region down to one surviving
//!     `Block` of statements.
//!   - Macro-style code emitters
//!     wrap a Region body in a single Block to give the macro a stable
//!     handle to inject hooks into.
//!
//! Each extra Block layer costs a Vec allocation, an indirection in the
//! tree-walker (cost certificate, fingerprint, target emission), and one
//! nesting level in emitters that materialize lexical scopes.
//!
//! ## Rule
//!
//! ```text
//! Node::Region { body: Arc::new(vec![Node::Block(inner)]) }
//!   →
//! Node::Region { body: Arc::new(inner) }
//! ```
//!
//! Applied recursively: the unwrap fires anywhere a Region body contains
//! exactly one Block as its only child. Multi-child Region bodies (the
//! common case for non-trivial primitives) are unchanged.
//!
//! ## Why not generalize to "remove any singleton child container"
//!
//! `Region` carries an `op_id` and `source_region` for region-chain audits
//! and bench attribution. This pass only removes the transparent `Block`
//! layer when the parent is a `Region`; it never removes or merges a
//! `Region` node.

use std::sync::Arc;

use crate::ir::{Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Unwrap `Region { body: [Block(inner)] }` to `Region { body: inner }`.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "region_promote_singleton_block",
    requires = [],
    invalidates = []
)]
pub struct RegionPromoteSingletonBlockPass;

impl RegionPromoteSingletonBlockPass {
    /// Skip programs without any Region wrapping a singleton Block.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Need both a Region AND a Block; either missing → no work.
        use crate::ir::stats::{NODE_KIND_BLOCK, NODE_KIND_REGION};
        let stats = program.stats();
        if !stats.has_any_node_kind(NODE_KIND_REGION) || !stats.has_any_node_kind(NODE_KIND_BLOCK) {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_singleton_block_region))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and unwrap every singleton-block Region body.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| promote_node(n, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants. After recursion, if `node` is a
/// `Region` whose body is exactly a single `Block`, lift the Block's
/// children to be the Region's direct children.
fn promote_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| promote_node(child, changed));
    if let Node::Region {
        generator,
        source_region,
        body,
    } = recursed
    {
        // Fast path: peek at the shared Arc first. If it isn't a
        // singleton Block, we never unwrap or clone the body  -  just
        // hand the Arc back to a fresh Region. The previous shape
        // unconditionally unwrapped (or fully cloned) every Region
        // body even when the rule did not fire, which is the common
        // case for any program where most Regions wrap multiple
        // ops.
        if !matches!(body.as_slice(), [Node::Block(_)]) {
            return Node::Region {
                generator,
                source_region,
                body,
            };
        }
        let body_vec: Vec<Node> = match Arc::try_unwrap(body) {
            Ok(v) => v,
            Err(arc) => (*arc).clone(),
        };
        *changed = true;
        let mut iter = body_vec.into_iter();
        let Some(Node::Block(inner)) = iter.next() else {
            unreachable!("matched [Node::Block(_)] above");
        };
        return Node::Region {
            generator,
            source_region,
            body: Arc::new(inner),
        };
    }
    recursed
}

/// True iff `node` is a `Region` whose body is exactly a single `Block`.
fn is_singleton_block_region(node: &Node) -> bool {
    matches!(
        node,
        Node::Region { body, .. } if matches!(body.as_slice(), [Node::Block(_)])
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::model::expr::Ident;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn region_with_body(body: Vec<Node>) -> Node {
        Node::Region {
            generator: Ident::from("test_op"),
            source_region: None,
            body: Arc::new(body),
        }
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn count_singleton_block_regions(node: &Node) -> usize {
        let mut count = 0;
        if let Node::Region { body, .. } = node {
            if matches!(body.as_slice(), [Node::Block(_)]) {
                count += 1;
            }
            for child in body.iter() {
                count += count_singleton_block_regions(child);
            }
        }
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                for n in then {
                    count += count_singleton_block_regions(n);
                }
                for n in otherwise {
                    count += count_singleton_block_regions(n);
                }
            }
            Node::Loop { body, .. } => {
                for n in body {
                    count += count_singleton_block_regions(n);
                }
            }
            Node::Block(body) => {
                for n in body {
                    count += count_singleton_block_regions(n);
                }
            }
            _ => {}
        }
        count
    }

    #[test]
    fn skip_analysis_on_program_without_region() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionPromoteSingletonBlockPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn skip_analysis_on_region_with_multiple_children() {
        // Multi-child Region body must not match the singleton rule.
        let entry = vec![region_with_body(vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ])];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionPromoteSingletonBlockPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn run_analysis_on_singleton_block_region() {
        let entry = vec![region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionPromoteSingletonBlockPass, &program),
            PassAnalysis::RUN
        );
    }

    #[test]
    fn transform_unwraps_simple_singleton_block() {
        let entry = vec![region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(result.changed);
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(count_singleton_block_regions)
            .sum();
        assert_eq!(total, 0, "no singleton-block Regions must remain");
    }

    #[test]
    fn transform_preserves_region_op_id() {
        let entry = vec![region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        match &result.program.entry()[0] {
            Node::Region { generator, .. } => {
                assert_eq!(generator.as_str(), "test_op", "op id must be preserved");
            }
            _ => panic!(
                "expected Region at top of entry; got {:?}",
                result.program.entry()[0]
            ),
        }
    }

    #[test]
    fn transform_lifts_inner_children_to_region_body() {
        let entry = vec![region_with_body(vec![Node::Block(vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(1), Expr::u32(2)),
            Node::store("buf", Expr::u32(2), Expr::u32(3)),
        ])])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        match &result.program.entry()[0] {
            Node::Region { body, .. } => {
                assert_eq!(
                    body.len(),
                    3,
                    "the 3 inner Block children must be promoted to Region body"
                );
            }
            other => panic!("expected Region; got {other:?}"),
        }
    }

    #[test]
    fn transform_skips_multi_child_region_unchanged() {
        let entry = vec![region_with_body(vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(
            !result.changed,
            "multi-child Region must not match the singleton rule"
        );
    }

    #[test]
    fn transform_handles_nested_singleton_block_regions() {
        // Region wraps Block which wraps another Region wrapping a Block.
        let inner_region = region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(1),
        )])]);
        let entry = vec![region_with_body(vec![Node::Block(vec![inner_region])])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(result.changed);
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(count_singleton_block_regions)
            .sum();
        assert_eq!(
            total, 0,
            "every layer of singleton-block Region must be unwrapped, including nested"
        );
    }

    #[test]
    fn transform_is_idempotent() {
        let entry = vec![region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        let once = RegionPromoteSingletonBlockPass::transform(program);
        let twice_program = Clone::clone(&once.program);
        let twice = RegionPromoteSingletonBlockPass::transform(twice_program);
        assert!(once.changed);
        assert!(!twice.changed, "second run must report no change");
    }

    #[test]
    fn transform_preserves_region_inside_removed_block() {
        let entry = vec![Node::Block(vec![region_with_body(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        match &result.program.entry()[0] {
            Node::Region { body, .. } => {
                assert!(
                    matches!(body[0], Node::Region { .. }),
                    "inner Region must still be present after transparent Block removal"
                );
            }
            other => panic!("expected root Region; got {other:?}"),
        }
    }

    #[test]
    fn transform_handles_empty_program() {
        let program = Program::wrapped(vec![buf()], [1, 1, 1], vec![]);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn transform_unwraps_region_inside_if_branch() {
        let inner_region = region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(1),
        )])]);
        let entry = vec![Node::if_then(
            Expr::lt(Expr::u32(0), Expr::u32(1)),
            vec![inner_region],
        )];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(
            result.changed,
            "Regions inside If branches must be processed"
        );
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(count_singleton_block_regions)
            .sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn transform_unwraps_region_inside_loop_body() {
        let inner_region = region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(1),
        )])]);
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![inner_region],
        }];
        let program = program_with_entry(entry);
        let result = RegionPromoteSingletonBlockPass::transform(program);
        assert!(
            result.changed,
            "Regions inside Loop bodies must be processed"
        );
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(count_singleton_block_regions)
            .sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn fingerprint_returns_stable_value() {
        let entry = vec![region_with_body(vec![Node::Block(vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::u32(7),
        )])])];
        let program = program_with_entry(entry);
        let fp1 =
            crate::optimizer::ProgramPass::fingerprint(&RegionPromoteSingletonBlockPass, &program);
        let fp2 =
            crate::optimizer::ProgramPass::fingerprint(&RegionPromoteSingletonBlockPass, &program);
        assert_eq!(fp1, fp2);
    }
}
