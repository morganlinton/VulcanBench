//! ROADMAP A30  -  polyhedral loop-bound normalization.
//!
//! Shipped variant: lower-bound normalization. Every literal-bounded
//! `Loop(i, lo, hi, body)` with `lo > 0` and `hi >= lo` rewrites to
//! `Loop(i', 0, hi - lo, body[i := i' + lo])`. The iteration space is
//! preserved exactly, the trip count is unchanged, and every body
//! expression that read the original loop variable now reads
//! `Var(i') + LitU32(lo)`. This is the polyhedral library's
//! `Affine::Translate(-lo)` rewrite  -  the simplest piece of a real
//! polyhedral substrate, and the prerequisite for the iteration-
//! space normalisation that A29 strip-mine, A26 fusion, and A28 peel
//! all assume.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_lower_bound_normalize`.
//! Soundness: `Exact`. The rewrite is a pure variable substitution
//! over an integer interval; the body sees `i' + lo` at every site
//! that previously read `i`, so every observable side effect (Store,
//! Atomic, Async, Trap) is keyed on the same value as before.
//! Cost direction: monotone-down on the canonical-form metric used
//! by downstream passes (A29 strip-mine refuses non-zero lower
//! bounds; A26 fusion's bounds-match check is symmetric over
//! normalised loops). Per-iteration cost rises by one Add at each
//! `Var(i)` read site; downstream const-fold + strength-reduce
//! collapses `i' + lo` back into a single offset register before
//! emit, so the net IR size after the next algebraic round is
//! unchanged.
//!
//! ## Pattern
//!
//! ```text
//! Loop(i, LitU32(lo), LitU32(hi), body)
//!     where lo > 0 AND hi >= lo
//! → Loop(i', LitU32(0), LitU32(hi - lo),
//!         body with every Var(i) replaced by (Var(i') + LitU32(lo)))
//! ```
//!
//! The fresh inner name `i'` is `{i}__norm_N` chosen so it doesn't
//! collide with any existing name in the loop body or the
//! surrounding scope.
//!
//! ## Conservatism
//!
//! - Both bounds must be `Expr::LitU32` literals. Non-literal lower
//!   bounds need symbolic interval arithmetic (the proper polyhedral
//!   substrate); literal bounds are the structural slice we can
//!   prove sound today.
//! - `lo == 0` is already canonical; the pass skips so we don't
//!   busy-loop the scheduler.
//! - `lo > hi` produces a zero-trip loop and is left for
//!   `loop_trip_zero_eliminate` to drop on its next pass.
//! - The loop variable must not be reassigned anywhere inside the
//!   body (no `Node::Assign { name: i, .. }`), and the loop must not
//!   contain a nested Loop that re-binds the same name. Both
//!   collisions block the rewrite.

use super::substitution::body_rebinds_var;
use crate::ir::{BinOp, Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Polyhedral lower-bound normalization pass.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_lower_bound_normalize",
    requires = ["const_fold"],
    invalidates = ["loop_unroll", "loop_strip_mine"]
)]
pub struct LoopLowerBoundNormalize;

impl LoopLowerBoundNormalize {
    /// Skip programs that have no normalizable Loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_normalizable_loop))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and normalize every eligible Loop.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| recurse(n, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

fn recurse(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| recurse(child, changed));
    match recursed {
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let (lo, hi) = match (&from, &to) {
                (Expr::LitU32(lo), Expr::LitU32(hi)) if *lo > 0 && *hi >= *lo => (*lo, *hi),
                _ => {
                    return Node::Loop {
                        var,
                        from,
                        to,
                        body,
                    };
                }
            };
            if body_rebinds_var(&body, &var) {
                return Node::Loop {
                    var,
                    from,
                    to,
                    body,
                };
            }
            let offset = Expr::u32(lo);
            let new_body: Vec<Node> = body
                .into_iter()
                .map(|n| substitute_var_in_node(n, &var, &var, &offset))
                .collect();
            *changed = true;
            Node::Loop {
                var,
                from: Expr::u32(0),
                to: Expr::u32(hi - lo),
                body: new_body,
            }
        }
        other => other,
    }
}

fn is_normalizable_loop(node: &Node) -> bool {
    if let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    {
        match (from, to) {
            (Expr::LitU32(lo), Expr::LitU32(hi)) if *lo > 0 && *hi >= *lo => {}
            _ => return false,
        }
        !body_rebinds_var(body, var)
    } else {
        false
    }
}

// `body_rebinds_var` lives in `super::substitution` (one canonical copy shared
// with loop_var_range_fold; both fold against a derived induction-var range).

#[expect(
    clippy::too_many_lines,
    reason = "loop lower-bound substitution keeps Node variant reconstruction in one ownership-preserving pass"
)]
fn substitute_var_in_node(node: Node, from: &Ident, to: &Ident, offset: &Expr) -> Node {
    match node {
        Node::Let { name, value } => Node::Let {
            name,
            value: substitute_var_in_expr(value, from, to, offset),
        },
        Node::Assign { name, value } => Node::Assign {
            name,
            value: substitute_var_in_expr(value, from, to, offset),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::Store {
            buffer,
            index: substitute_var_in_expr(index, from, to, offset),
            value: substitute_var_in_expr(value, from, to, offset),
        },
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond: substitute_var_in_expr(cond, from, to, offset),
            then: then
                .into_iter()
                .map(|n| substitute_var_in_node(n, from, to, offset))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .map(|n| substitute_var_in_node(n, from, to, offset))
                .collect(),
        },
        Node::Loop {
            var,
            from: lo,
            to: hi,
            body,
        } => Node::Loop {
            var,
            from: substitute_var_in_expr(lo, from, to, offset),
            to: substitute_var_in_expr(hi, from, to, offset),
            body: body
                .into_iter()
                .map(|n| substitute_var_in_node(n, from, to, offset))
                .collect(),
        },
        Node::Block(body) => Node::Block(
            body.into_iter()
                .map(|n| substitute_var_in_node(n, from, to, offset))
                .collect(),
        ),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(
                    body_vec
                        .into_iter()
                        .map(|n| substitute_var_in_node(n, from, to, offset))
                        .collect(),
                ),
            }
        }
        Node::AsyncLoad {
            source,
            destination,
            offset: o,
            size,
            tag,
        } => Node::AsyncLoad {
            source,
            destination,
            tag,
            offset: Box::new(substitute_var_in_expr(*o, from, to, offset)),
            size: Box::new(substitute_var_in_expr(*size, from, to, offset)),
        },
        Node::AsyncStore {
            source,
            destination,
            offset: o,
            size,
            tag,
        } => Node::AsyncStore {
            source,
            destination,
            tag,
            offset: Box::new(substitute_var_in_expr(*o, from, to, offset)),
            size: Box::new(substitute_var_in_expr(*size, from, to, offset)),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(substitute_var_in_expr(*address, from, to, offset)),
            tag,
        },
        other => other,
    }
}

fn substitute_var_in_expr(expr: Expr, from: &Ident, to: &Ident, offset: &Expr) -> Expr {
    match expr {
        Expr::Var(ref name) if name == from => Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(Expr::Var(to.clone())),
            right: Box::new(offset.clone()),
        },
        Expr::Var(_)
        | Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => expr,
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(substitute_var_in_expr(*index, from, to, offset)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op,
            left: Box::new(substitute_var_in_expr(*left, from, to, offset)),
            right: Box::new(substitute_var_in_expr(*right, from, to, offset)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(substitute_var_in_expr(*operand, from, to, offset)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|a| substitute_var_in_expr(a, from, to, offset))
                .collect(),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(substitute_var_in_expr(*cond, from, to, offset)),
            true_val: Box::new(substitute_var_in_expr(*true_val, from, to, offset)),
            false_val: Box::new(substitute_var_in_expr(*false_val, from, to, offset)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(substitute_var_in_expr(*value, from, to, offset)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(substitute_var_in_expr(*a, from, to, offset)),
            b: Box::new(substitute_var_in_expr(*b, from, to, offset)),
            c: Box::new(substitute_var_in_expr(*c, from, to, offset)),
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op,
            buffer,
            index: Box::new(substitute_var_in_expr(*index, from, to, offset)),
            expected: expected.map(|e| Box::new(substitute_var_in_expr(*e, from, to, offset))),
            value: Box::new(substitute_var_in_expr(*value, from, to, offset)),
            ordering,
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(substitute_var_in_expr(*cond, from, to, offset)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(substitute_var_in_expr(*value, from, to, offset)),
            lane: Box::new(substitute_var_in_expr(*lane, from, to, offset)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op,
            value: Box::new(substitute_var_in_expr(*value, from, to, offset)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn find_loop(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if matches!(n, Node::Loop { .. }) {
                return Some(n);
            }
            match n {
                Node::Block(body) => {
                    if let Some(found) = find_loop(body) {
                        return Some(found);
                    }
                }
                Node::Region { body, .. } => {
                    if let Some(found) = find_loop(body.as_ref()) {
                        return Some(found);
                    }
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    if let Some(found) = find_loop(then) {
                        return Some(found);
                    }
                    if let Some(found) = find_loop(otherwise) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Positive: `Loop(i, 4, 12, store(buf, i, ...))` rewrites to
    /// `Loop(i', 0, 8, store(buf, i' + 4, ...))`.
    #[test]
    fn rewrites_positive_lower_bound_to_zero() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(4),
            to: Expr::u32(12),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(result.changed, "loop with from=4 must normalize");
        let loop_node = find_loop(result.program.entry()).expect("Fix: loop present");
        match loop_node {
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                assert_eq!(var.as_str(), "i", "var is not freshened after #2734");
                assert_eq!(*from, Expr::LitU32(0), "from must be 0");
                assert_eq!(*to, Expr::LitU32(8), "to must be original (12) - lower (4)");

                match &body[0] {
                    Node::Store { index, .. } => match index {
                        Expr::BinOp { op, left, right } => {
                            assert_eq!(*op, BinOp::Add);
                            assert!(
                                matches!(left.as_ref(), Expr::Var(name) if name.as_str() == var.as_str())
                            );
                            assert_eq!(*right.as_ref(), Expr::LitU32(4));
                        }
                        other => panic!("expected Var(i') + 4, got {other:?}"),
                    },
                    other => panic!("expected Store, got {other:?}"),
                }
            }
            other => panic!("expected Loop, got {other:?}"),
        }
    }

    /// Negative: `Loop(i, 0, N, ...)` is already canonical and
    /// must not be touched.
    #[test]
    fn keeps_loop_with_zero_lower_bound() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(!result.changed, "from=0 is already canonical");
    }

    /// Negative: non-literal `from` skips (needs symbolic substrate).
    #[test]
    fn keeps_loop_with_runtime_lower_bound() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::var("k"),
            to: Expr::u32(10),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(!result.changed, "runtime from must skip");
    }

    /// Negative: non-literal `to` skips.
    #[test]
    fn keeps_loop_with_runtime_upper_bound() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(2),
            to: Expr::var("n"),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(!result.changed, "runtime to must skip");
    }

    /// Negative: `lo > hi` is a zero-trip loop; left for
    /// `loop_trip_zero_eliminate` to drop.
    #[test]
    fn keeps_loop_with_inverted_bounds() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(10),
            to: Expr::u32(4),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(
            !result.changed,
            "inverted bounds must be left for trip-zero pass"
        );
    }

    /// Negative: a loop body that reassigns the loop var blocks the
    /// rewrite  -  substitution would not preserve the in-body
    /// reassignment semantics.
    #[test]
    fn keeps_loop_when_body_assigns_loop_var() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(2),
            to: Expr::u32(10),
            body: vec![
                Node::Assign {
                    name: Ident::from("i"),
                    value: Expr::u32(99),
                },
                Node::store("buf", Expr::var("i"), Expr::u32(1)),
            ],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(!result.changed, "Assign to loop var must block rewrite");
    }

    /// Negative: a nested Loop that re-binds the same var name
    /// blocks the rewrite (would shadow the substituted name).
    #[test]
    fn keeps_loop_when_nested_loop_shadows_var() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(2),
            to: Expr::u32(10),
            body: vec![Node::Loop {
                var: Ident::from("i"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![],
            }],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(!result.changed, "shadowing nested Loop must block rewrite");
    }

    /// Positive: nested loop nests normalize bottom-up. Inner
    /// `Loop(j, 5, 10, ...)` normalizes; outer `Loop(i, 0, 4, ...)`
    /// stays canonical.
    #[test]
    fn normalizes_nested_loop_independently() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![Node::Loop {
                var: Ident::from("j"),
                from: Expr::u32(5),
                to: Expr::u32(10),
                body: vec![Node::store("buf", Expr::var("j"), Expr::u32(1))],
            }],
        }];
        let result = LoopLowerBoundNormalize::transform(program(entry));
        assert!(result.changed, "inner loop must normalize");
    }

    /// `analyze` short-circuits when no eligible Loop is present.
    #[test]
    fn analyze_skips_program_with_only_canonical_loops() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![],
        }];
        match crate::optimizer::ProgramPass::analyze(&LoopLowerBoundNormalize, &program(entry)) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }
}
