//! Region-inline pass registered through the unified pass substrate.
//!
//! Wraps the [`region_inline_engine`] transform so it runs under
//! scheduler control rather than as a hard-coded pre-pass.

use crate::ir::Program;
use crate::optimizer::{fingerprint_program, vyre_pass, PassAnalysis, PassResult};

#[vyre_pass(
    name = "region_inline",
    requires = [],
    invalidates = ["cse", "dce"],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "scalar"
)]
/// Built-in region-inline pass.
pub struct RegionInlinePass;

impl RegionInlinePass {
    /// Run when the program has at least one Region anywhere in the
    /// node tree, not just at the top level. The engine recurses into
    /// If/Loop/Block/Region children, so a nested-only Region is just
    /// as eligible as a top-level one.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // O(1)  -  the cached stats walk already recorded every kind it
        // visited. If no Region was observed (rare, since Program::wrapped
        // emits a top-level Region), the recursive any_descendant walk
        // can be skipped entirely.
        if program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_REGION)
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Flatten small regions into the surrounding body.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let before = fingerprint_program(&program);
        let optimized = super::region_inline_engine::run(program);
        PassResult {
            changed: fingerprint_program(&optimized) != before,
            program: optimized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, Node};

    #[test]
    fn region_inline_analyze_skips_without_regions() {
        let p = Program::new_raw(vec![], [1, 1, 1], vec![Node::let_bind("x", Expr::u32(1))]);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionInlinePass, &p),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn region_inline_analyze_runs_with_regions() {
        let p = Program::wrapped(
            vec![],
            [1, 1, 1],
            vec![Node::Region {
                generator: "test_gen".into(),
                source_region: None,
                body: vec![Node::let_bind("x", Expr::u32(1))].into(),
            }],
        );
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionInlinePass, &p),
            PassAnalysis::RUN
        );
    }

    /// A Region buried inside a Loop body is just as eligible as a
    /// top-level Region. The shallow check missed these.
    #[test]
    fn region_inline_analyze_runs_when_region_is_nested() {
        let p = Program::wrapped(
            vec![],
            [1, 1, 1],
            vec![Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::Region {
                    generator: "test_gen".into(),
                    source_region: None,
                    body: vec![Node::let_bind("x", Expr::u32(1))].into(),
                }],
            )],
        );
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionInlinePass, &p),
            PassAnalysis::RUN,
            "nested Region must trigger the pass; engine recurses through Loop bodies"
        );
    }

    #[test]
    fn region_inline_transform_flattens_regions() {
        let inner_let = Node::let_bind("x", Expr::u32(1));
        let p = Program::wrapped(
            vec![],
            [1, 1, 1],
            vec![Node::Region {
                generator: "test_gen".into(),
                source_region: None,
                body: vec![inner_let.clone()].into(),
            }],
        );
        let result = RegionInlinePass::transform(p);

        assert!(result.changed, "Region inline failed to detect change");
        assert!(
            !result
                .program
                .entry()
                .iter()
                .any(|n| matches!(n, Node::Region { .. })),
            "Region inline should have removed all Region nodes"
        );
        // The inner_let should now be at the top level
        assert_eq!(result.program.entry().len(), 1);
        assert!(matches!(result.program.entry()[0], Node::Let { .. }));
    }
}
