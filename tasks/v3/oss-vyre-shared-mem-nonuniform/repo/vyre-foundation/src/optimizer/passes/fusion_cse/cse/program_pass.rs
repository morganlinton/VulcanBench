//! Common-subexpression elimination  -  registered ProgramPass.
//!
//! The engine itself lives at `super::engine`; this module hooks it
//! into the scheduler's fixpoint loop and invalidation tracking.

use super::engine;
use crate::ir::Program;
use crate::optimizer::{fingerprint_program, vyre_pass, PassAnalysis, PassResult};

#[vyre_pass(
    name = "cse",
    requires = ["canonicalize"],
    invalidates = ["fusion"],
    phase = "fusion_cse",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
/// Built-in CSE pass.
pub struct CsePass;

impl CsePass {
    /// Run only when the program has at least one binding the engine
    /// could fold into a previous one.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // CSE folds Let bindings that share a key with an earlier Let.
        // Without any Let in the program there is nothing to dedup.
        if program.entry().is_empty() || !program.stats().has_node_let() {
            PassAnalysis::SKIP
        } else {
            PassAnalysis::RUN
        }
    }

    /// Run CSE over the program entry.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let before = fingerprint_program(&program);
        let optimized = engine::cse(program);
        PassResult {
            changed: fingerprint_program(&optimized) != before,
            program: optimized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, Node, Program};

    #[test]
    fn cse_analyze_skips_empty() {
        let empty = Program::new_raw(vec![], [1, 1, 1], vec![]);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&CsePass, &empty),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn cse_transform_detects_changes() {
        // Create an IR where the same heavy expression is bound twice.
        let heavy_expr = Expr::add(Expr::var("x"), Expr::var("y"));
        let node1 = Node::let_bind("first", heavy_expr.clone());
        let node2 = Node::let_bind("second", heavy_expr);
        let p = Program::new_raw(vec![], [1, 1, 1], vec![node1, node2]);
        let result = CsePass::transform(p);

        assert!(
            result.changed,
            "CSE failed to detect change on redundant expressions"
        );

        let entry = result.program.entry();
        assert_eq!(entry.len(), 2);
        if let Node::Let { value, .. } = &entry[1] {
            assert!(
                matches!(value, Expr::Var(v) if v.as_ref() == "first"),
                "CSE should have replaced the second binding with a reference to the first"
            );
        } else {
            panic!("Expected Let node");
        }
    }

    #[test]
    fn cse_merge_commutative_add_in_different_order() {
        // CSE must recognize Add(a,b) and Add(b,a) as the same expression
        // when the operands are sorted by canonicalize.
        let a = Expr::var("a");
        let b = Expr::var("b");
        let expr1 = Expr::add(a.clone(), b.clone());
        let expr2 = Expr::add(b, a);
        let n1 = Node::let_bind("first", expr1);
        let n2 = Node::let_bind("second", expr2);
        let p = Program::new_raw(vec![], [1, 1, 1], vec![n1, n2]);
        let result = CsePass::transform(p);
        assert!(result.changed);
        if let Node::Let { value, .. } = &result.program.entry()[1] {
            assert!(
                matches!(value, Expr::Var(v) if v.as_ref() == "first"),
                "CSE must merge commutative Add regardless of operand order. Got: {:?}",
                value
            );
        }
    }

    #[test]
    fn cse_no_change_on_unique_expressions() {
        let a = Expr::var("a");
        let b = Expr::var("b");
        let c = Expr::var("c");
        let n1 = Node::let_bind("x1", Expr::add(a.clone(), b));
        let n2 = Node::let_bind("x2", Expr::add(a, c));
        let p = Program::new_raw(vec![], [1, 1, 1], vec![n1, n2]);
        let result = CsePass::transform(p);
        assert!(
            !result.changed,
            "CSE must not change program with no common subexpressions"
        );
        assert_eq!(result.program.entry().len(), 2);
    }

    #[test]
    fn cse_preserves_single_node_program() {
        let n1 = Node::let_bind("x", Expr::u32(42));
        let p = Program::new_raw(vec![], [1, 1, 1], vec![n1]);
        let result = CsePass::transform(p);
        assert!(!result.changed);
        assert_eq!(result.program.entry().len(), 1);
    }

    #[test]
    fn cse_handles_nested_identical_subtrees() {
        // add( add(x, y), add(x, y) )  -  the inner add(x,y) appears twice
        let x = Expr::var("x");
        let y = Expr::var("y");
        let inner1 = Expr::add(x.clone(), y.clone());
        let inner2 = Expr::add(x.clone(), y.clone());
        let outer1 = Node::let_bind("inner", inner1);
        let outer2 = Node::let_bind("outer", Expr::add(Expr::var("inner"), inner2));
        let p = Program::new_raw(vec![], [1, 1, 1], vec![outer1, outer2]);
        let result = CsePass::transform(p);
        assert!(
            result.changed,
            "CSE must deduplicate nested identical subtrees"
        );
        // outer add should reference "inner" for both operands
        if let Node::Let { value, .. } = &result.program.entry()[1] {
            match value {
                Expr::BinOp { left, right, .. } => {
                    assert!(matches!(left.as_ref(), Expr::Var(v) if v.as_ref() == "inner"));
                    assert!(matches!(right.as_ref(), Expr::Var(v) if v.as_ref() == "inner"));
                }
                _ => panic!("Expected BinOp in outer binding"),
            }
        }
    }

    #[test]
    fn cse_skips_effectful_loads() {
        // Two Loads of the same buffer at the same index must NOT merge
        // because memory may change between them.
        let buf = "buf";
        let idx = Expr::u32(0);
        let load1 = Node::let_bind("a", Expr::load(buf, idx.clone()));
        let load2 = Node::let_bind("b", Expr::load(buf, idx));
        let p = Program::new_raw(vec![], [1, 1, 1], vec![load1, load2]);
        let result = CsePass::transform(p);
        // Load is effectful; CSE must not merge them.
        assert!(
            result.changed,
            "CSE treats Load as pure and merges identical loads of the same buffer"
        );
        // Both bindings should remain as Load expressions
        assert_eq!(result.program.entry().len(), 2);
    }

    #[test]
    fn cse_idempotent() {
        let x = Expr::var("x");
        let y = Expr::var("y");
        let heavy = Expr::mul(Expr::add(x.clone(), y.clone()), Expr::add(x, y));
        let n1 = Node::let_bind("a", heavy.clone());
        let n2 = Node::let_bind("b", heavy);
        let p = Program::new_raw(vec![], [1, 1, 1], vec![n1, n2]);
        let r1 = CsePass::transform(p);
        let r2 = CsePass::transform(Clone::clone(&r1.program));
        assert!(
            !r2.changed,
            "CSE must be idempotent  -  second pass must not change output"
        );
        assert_eq!(
            r1.program.entry().len(),
            r2.program.entry().len(),
            "CSE idempotent: node count stable after first pass"
        );
    }

    #[test]
    fn cse_handles_if_block_with_common_expr_in_both_branches() {
        // If(cond, Block{ let a = add(x,y) }, Block{ let b = add(x,y) })
        let x = Expr::var("x");
        let y = Expr::var("y");
        let then_body = vec![Node::let_bind("a_then", Expr::add(x.clone(), y.clone()))];
        let else_body = vec![Node::let_bind("a_else", Expr::add(x, y))];
        let if_node = Node::If {
            cond: Expr::var("cond"),
            then: then_body,
            otherwise: else_body,
        };
        let p = Program::new_raw(vec![], [1, 1, 1], vec![if_node]);
        let result = CsePass::transform(p);
        assert!(
            !result.changed,
            "CSE must NOT merge expressions across If branches  -  scoped separately"
        );
    }
}
