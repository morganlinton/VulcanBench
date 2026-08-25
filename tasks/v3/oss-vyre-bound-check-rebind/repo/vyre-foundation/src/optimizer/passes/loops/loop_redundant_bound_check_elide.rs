//! `loop_redundant_bound_check_elide`  -  drop `if loop_var < to { ... }`
//! guards that re-check the enclosing loop's own upper bound.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_redundant_bound_check_elide`.
//! Soundness: `Exact`  -  the loop's `from..to` range guarantees `loop_var < to`
//! on every iteration, so the inner if-guard is statically true. Replacing
//! the if-node with its `then` branch wrapped in a `Block` is observationally
//! equivalent. Cost-direction: monotone-down on `node_count`,
//! `control_flow_count`, and `instruction_count` (one fewer comparison per
//! iteration). Preserves: every analysis. Invalidates: nothing.
//!
//! ## Why
//!
//! Vyre primitives uniformly emit a guard pattern at the start of every
//! per-thread Region body:
//!
//! ```text
//! let limb_idx = InvocationId { axis: 0 };
//! if limb_idx < N { ... }
//! ```
//!
//! When that pattern lives inside an unrolled `Node::Loop { from: 0, to: N }`
//! whose loop variable is also `limb_idx` (or a freshly bound variable
//! initialized from `InvocationId`), the inner if-guard is statically
//! redundant because the loop's own range gate already ensures it. Removing
//! the guard eliminates one comparison and branch per iteration in every
//! backend or interpreter that materializes structured control flow directly.
//!
//! ## Rule
//!
//! For every `Node::Loop { var, from: _, to: Lit, body }`, scan `body` for
//! `Node::If { cond: BinOp(Lt, Var(var), Lit_to), then, otherwise: empty }`
//! where `Lit_to` matches the loop's `to` literal. Replace the If with
//! `Node::Block(then)`.
//!
//! ## Conservatism
//!
//! Only fires when:
//!   1. Loop's `to` is a `LitU32` (literal upper bound).
//!   2. If-guard's RHS is the same literal.
//!   3. If-guard's LHS is `Var(loop_var)` (literal name match).
//!   4. If-guard's `otherwise` branch is empty (else-bodies require
//!      different rewrite rules).
//!   5. The comparison op is `BinOp::Lt`.
//!
//! Conservative because the IR value-set fact table only admits this exact
//! literal-bound implication today.

use crate::ir::{BinOp, Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Drop redundant `if loop_var < to { ... }` guards inside loops with
/// matching literal upper bounds.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_redundant_bound_check_elide",
    requires = [],
    invalidates = []
)]
pub struct LoopRedundantBoundCheckElidePass;

impl LoopRedundantBoundCheckElidePass {
    /// Skip programs without any loop containing an inner if-guard.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Eliding the redundant guard requires a Loop AND an If
        // (the guard) inside it. Either missing → no work.
        use crate::ir::stats::{NODE_KIND_IF, NODE_KIND_LOOP};
        let stats = program.stats();
        if !stats.has_any_node_kind(NODE_KIND_LOOP) || !stats.has_any_node_kind(NODE_KIND_IF) {
            return PassAnalysis::SKIP;
        }
        if program.entry().iter().any(node_has_redundant_guard) {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and elide redundant bound checks.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| elide_in_node(n, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Try to extract a literal `u32` from `expr` (matches `Expr::LitU32`).
fn lit_u32_value(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::LitU32(n) => Some(*n),
        _ => None,
    }
}

/// Check if `cond` is `Var(var) < Lit_to`. Returns the literal upper bound
/// the cond compares against, when so.
fn cond_matches_loop_var_lt_lit(cond: &Expr, var: &str) -> Option<u32> {
    if let Expr::BinOp { op, left, right } = cond {
        if matches!(op, BinOp::Lt) {
            if let (Expr::Var(v), Some(rhs_lit)) = (left.as_ref(), lit_u32_value(right)) {
                if v.as_str() == var {
                    return Some(rhs_lit);
                }
            }
        }
    }
    None
}

/// Walk a sequence of nodes and elide redundant guards within any nested loop.
fn elide_in_sequence(
    body: Vec<Node>,
    loop_ctx: Option<(&str, u32)>,
    changed: &mut bool,
) -> Vec<Node> {
    body.into_iter()
        .map(|n| elide_in_node_with_ctx(n, loop_ctx, changed))
        .collect()
}

/// Recurse into a node with the surrounding-loop context (var name + to-lit).
fn elide_in_node_with_ctx(node: Node, loop_ctx: Option<(&str, u32)>, changed: &mut bool) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            // Check if this if matches the redundant pattern.
            if otherwise.is_empty() {
                if let Some((loop_var, loop_to)) = loop_ctx {
                    if let Some(rhs_lit) = cond_matches_loop_var_lt_lit(&cond, loop_var) {
                        if rhs_lit == loop_to {
                            *changed = true;
                            // Recurse into `then` with the same context, then
                            // wrap in a Block. (The Block makes the unwrap
                            // safe even if `then` contained nested Ifs that
                            // also got elided.)
                            let then_elided = elide_in_sequence(then, loop_ctx, changed);
                            return Node::Block(then_elided);
                        }
                    }
                }
            }
            // Not the redundant pattern  -  recurse into both branches.
            Node::If {
                cond,
                then: elide_in_sequence(then, loop_ctx, changed),
                otherwise: elide_in_sequence(otherwise, loop_ctx, changed),
            }
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            // The loop establishes a fresh ctx; only fires when `to` is a
            // literal (the conservative analysis we documented above).
            let new_ctx = lit_u32_value(&to).map(|to_lit| (var.as_str(), to_lit));
            let body = elide_in_sequence(body, new_ctx, changed);
            Node::Loop {
                var,
                from,
                to,
                body,
            }
        }
        Node::Block(body) => Node::Block(elide_in_sequence(body, loop_ctx, changed)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            // A Region is a fresh scope  -  drop the loop_ctx because the
            // inner body's loop var is in a different binding scope.
            let body_vec = elide_in_sequence(body_vec, None, changed);
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(body_vec),
            }
        }
        other => other,
    }
}

/// Top-level entry into a node (no enclosing loop context).
fn elide_in_node(node: Node, changed: &mut bool) -> Node {
    elide_in_node_with_ctx(node, None, changed)
}

/// True iff `node` (or any descendant) contains a Loop whose body holds
/// at least one redundant `if loop_var < to_lit` guard.
fn node_has_redundant_guard(node: &Node) -> bool {
    has_redundant_guard_with_ctx(node, None)
}

fn has_redundant_guard_with_ctx(node: &Node, loop_ctx: Option<(&str, u32)>) -> bool {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            if otherwise.is_empty() {
                if let Some((loop_var, loop_to)) = loop_ctx {
                    if let Some(rhs_lit) = cond_matches_loop_var_lt_lit(cond, loop_var) {
                        if rhs_lit == loop_to {
                            return true;
                        }
                    }
                }
            }
            then.iter()
                .any(|n| has_redundant_guard_with_ctx(n, loop_ctx))
                || otherwise
                    .iter()
                    .any(|n| has_redundant_guard_with_ctx(n, loop_ctx))
        }
        Node::Loop { var, to, body, .. } => {
            let new_ctx = lit_u32_value(to).map(|to_lit| (var.as_str(), to_lit));
            body.iter()
                .any(|n| has_redundant_guard_with_ctx(n, new_ctx))
        }
        Node::Block(body) => body
            .iter()
            .any(|n| has_redundant_guard_with_ctx(n, loop_ctx)),
        Node::Region { body, .. } => body.iter().any(|n| has_redundant_guard_with_ctx(n, None)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::model::expr::Ident;
    use crate::ir::{BufferAccess, BufferDecl, DataType};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn loop_with_body(var: &str, to: u32, body: Vec<Node>) -> Node {
        Node::Loop {
            var: Ident::from(var),
            from: Expr::u32(0),
            to: Expr::u32(to),
            body,
        }
    }

    fn count_redundant_if_guards(node: &Node, loop_ctx: Option<(&str, u32)>) -> usize {
        let mut count = 0;
        match node {
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                if otherwise.is_empty() {
                    if let Some((loop_var, loop_to)) = loop_ctx {
                        if let Some(rhs_lit) = cond_matches_loop_var_lt_lit(cond, loop_var) {
                            if rhs_lit == loop_to {
                                count += 1;
                            }
                        }
                    }
                }
                for n in then {
                    count += count_redundant_if_guards(n, loop_ctx);
                }
                for n in otherwise {
                    count += count_redundant_if_guards(n, loop_ctx);
                }
            }
            Node::Loop { var, to, body, .. } => {
                let new_ctx = lit_u32_value(to).map(|to_lit| (var.as_str(), to_lit));
                for n in body {
                    count += count_redundant_if_guards(n, new_ctx);
                }
            }
            Node::Block(body) => {
                for n in body {
                    count += count_redundant_if_guards(n, loop_ctx);
                }
            }
            Node::Region { body, .. } => {
                for n in body.iter() {
                    count += count_redundant_if_guards(n, None);
                }
            }
            _ => {}
        }
        count
    }

    #[test]
    fn skip_analysis_on_program_without_loop() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(1))];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopRedundantBoundCheckElidePass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn skip_analysis_on_loop_without_redundant_guard() {
        // Loop body has no inner if at all.
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        )];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopRedundantBoundCheckElidePass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn run_analysis_on_loop_with_redundant_guard() {
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopRedundantBoundCheckElidePass, &program),
            PassAnalysis::RUN
        );
    }

    #[test]
    fn transform_elides_simple_redundant_guard() {
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(result.changed);
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(|n| count_redundant_if_guards(n, None))
            .sum();
        assert_eq!(
            total, 0,
            "no redundant guards must remain after the elision pass"
        );
    }

    #[test]
    fn transform_does_not_elide_when_lit_does_not_match_loop_to() {
        // Guard checks i < 8 inside a loop with to=10. The guard is NOT
        // redundant  -  inside the loop the i can hit 9.
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(
            !result.changed,
            "non-matching literal must not trigger elision"
        );
    }

    #[test]
    fn transform_does_not_elide_when_var_name_does_not_match() {
        // Guard checks j < 10 (unrelated name) inside a loop over i. The
        // guard is NOT redundant.
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("j"), Expr::u32(10)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(
            !result.changed,
            "different var name must not trigger elision"
        );
    }

    #[test]
    fn transform_does_not_elide_when_loop_to_is_not_literal() {
        // Loop bound is `Var("n")` not a literal  -  no redundancy proof.
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::var("n"),
            body: vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        }];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(
            !result.changed,
            "non-literal loop bound must not trigger elision"
        );
    }

    #[test]
    fn transform_does_not_elide_when_else_branch_is_nonempty() {
        // The pass refuses if-then-else patterns  -  only if-then.
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then_else(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
                vec![Node::store("buf", Expr::var("i"), Expr::u32(0))],
            )],
        )];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(
            !result.changed,
            "if-then-else (with else body) must not be elided"
        );
    }

    #[test]
    fn transform_handles_nested_loops_independently() {
        // Outer loop var i, to=10. Inner loop var j, to=5. The inner if
        // checks j < 5  -  redundant for the inner loop but not the outer.
        let inner = loop_with_body(
            "j",
            5,
            vec![Node::if_then(
                Expr::lt(Expr::var("j"), Expr::u32(5)),
                vec![Node::store("buf", Expr::var("j"), Expr::u32(7))],
            )],
        );
        let entry = vec![loop_with_body("i", 10, vec![inner])];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(result.changed, "inner-loop guard must be elided");
        let total: usize = result
            .program
            .entry()
            .iter()
            .map(|n| count_redundant_if_guards(n, None))
            .sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn transform_does_not_elide_inner_guard_against_outer_loop_var() {
        // Inner loop has its own var; the inner if checks i < 10 (outer
        // var). Inner ctx (j, 5) doesn't match (i, 10). Outer ctx (i, 10)
        // is shadowed once we enter the inner loop. The pass should NOT
        // elide because at the if-site, only the inner loop ctx is
        // available.
        let inner = Node::Loop {
            var: Ident::from("j"),
            from: Expr::u32(0),
            to: Expr::u32(5),
            body: vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("j"), Expr::u32(7))],
            )],
        };
        let entry = vec![loop_with_body("i", 10, vec![inner])];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(
            !result.changed,
            "inner-loop scope must shadow outer; if-guard against outer var stays"
        );
    }

    #[test]
    fn transform_is_idempotent() {
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        let once = LoopRedundantBoundCheckElidePass::transform(program);
        let twice = LoopRedundantBoundCheckElidePass::transform(Clone::clone(&once.program));
        assert!(once.changed);
        assert!(!twice.changed, "second run must report no change");
    }

    #[test]
    fn transform_handles_empty_program() {
        let program = Program::wrapped(vec![buf()], [1, 1, 1], vec![]);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn transform_does_not_drop_then_body_during_elision() {
        // The `then` body MUST survive the elision (wrapped in a Block).
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![
                    Node::store("buf", Expr::var("i"), Expr::u32(7)),
                    Node::store("buf", Expr::u32(0), Expr::u32(8)),
                ],
            )],
        )];
        let program = program_with_entry(entry);
        let result = LoopRedundantBoundCheckElidePass::transform(program);
        assert!(result.changed);
        // Walk the loop body and count Stores  -  must be 2.
        fn count_stores(node: &Node) -> usize {
            let mut count = 0;
            match node {
                Node::Store { .. } => count += 1,
                Node::Loop { body, .. } => {
                    for n in body {
                        count += count_stores(n);
                    }
                }
                Node::Block(body) => {
                    for n in body {
                        count += count_stores(n);
                    }
                }
                Node::Region { body, .. } => {
                    for n in body.iter() {
                        count += count_stores(n);
                    }
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    for n in then {
                        count += count_stores(n);
                    }
                    for n in otherwise {
                        count += count_stores(n);
                    }
                }
                _ => {}
            }
            count
        }
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(
            total, 2,
            "both stores from the elided then-body must still be present"
        );
    }

    #[test]
    fn fingerprint_returns_stable_value() {
        let entry = vec![loop_with_body(
            "i",
            10,
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(10)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let program = program_with_entry(entry);
        let fp1 =
            crate::optimizer::ProgramPass::fingerprint(&LoopRedundantBoundCheckElidePass, &program);
        let fp2 =
            crate::optimizer::ProgramPass::fingerprint(&LoopRedundantBoundCheckElidePass, &program);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn cond_matches_loop_var_lt_lit_extracts_correct_literal() {
        let cond = Expr::lt(Expr::var("i"), Expr::u32(42));
        assert_eq!(cond_matches_loop_var_lt_lit(&cond, "i"), Some(42));
    }

    #[test]
    fn cond_matches_loop_var_lt_lit_rejects_wrong_var_name() {
        let cond = Expr::lt(Expr::var("j"), Expr::u32(42));
        assert_eq!(cond_matches_loop_var_lt_lit(&cond, "i"), None);
    }

    #[test]
    fn cond_matches_loop_var_lt_lit_rejects_non_lt_op() {
        let cond = Expr::eq(Expr::var("i"), Expr::u32(42));
        assert_eq!(cond_matches_loop_var_lt_lit(&cond, "i"), None);
    }
}
