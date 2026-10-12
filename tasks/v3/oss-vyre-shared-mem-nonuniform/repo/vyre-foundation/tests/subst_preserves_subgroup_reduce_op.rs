//! Regression probe: the shared induction-variable substitution
//! (`transform::subst`, driven by loop_unroll / strip_mine / loop_peel and
//! reverse-mode autodiff) must preserve a `SubgroupReduce`'s operator. The
//! substitution recursed into a `SubgroupReduce` but rebuilt it unconditionally
//! as `subgroup_add`, silently rewriting `Max`/`Min`/`Mul`/bitwise reductions to
//! `Add` -- a wrong reduction (and a wrong reversed gradient in autodiff).
//!
//! The single-workgroup reference makes a 1-lane subgroup reduction the identity
//! (max == add == the lane value), so the corruption is invisible to a value
//! oracle; this asserts the operator structurally instead.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program, SubgroupReduceOp};
use vyre_foundation::optimizer::passes::loops::loop_unroll::LoopUnroll;

/// Collect, in document order, the operator of every `SubgroupReduce` reachable
/// in `nodes` (through node bodies and store/let/assign value expressions).
fn collect_subgroup_reduce_ops(nodes: &[Node]) -> Vec<SubgroupReduceOp> {
    fn walk_expr(expr: &Expr, out: &mut Vec<SubgroupReduceOp>) {
        match expr {
            Expr::SubgroupReduce { op, value } => {
                out.push(*op);
                walk_expr(value, out);
            }
            Expr::Load { index, .. } => walk_expr(index, out),
            Expr::BinOp { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => walk_expr(operand, out),
            Expr::Fma { a, b, c } => {
                walk_expr(a, out);
                walk_expr(b, out);
                walk_expr(c, out);
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                walk_expr(cond, out);
                walk_expr(true_val, out);
                walk_expr(false_val, out);
            }
            Expr::Call { args, .. } => args.iter().for_each(|a| walk_expr(a, out)),
            Expr::SubgroupShuffle { value, lane } => {
                walk_expr(value, out);
                walk_expr(lane, out);
            }
            Expr::SubgroupBallot { cond } => walk_expr(cond, out),
            _ => {}
        }
    }
    fn walk_node(node: &Node, out: &mut Vec<SubgroupReduceOp>) {
        match node {
            Node::Store { index, value, .. } => {
                walk_expr(index, out);
                walk_expr(value, out);
            }
            Node::Let { value, .. } | Node::Assign { value, .. } => walk_expr(value, out),
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                walk_expr(cond, out);
                then.iter().for_each(|n| walk_node(n, out));
                otherwise.iter().for_each(|n| walk_node(n, out));
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                body.iter().for_each(|n| walk_node(n, out))
            }
            Node::Region { body, .. } => body.iter().for_each(|n| walk_node(n, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    nodes.iter().for_each(|n| walk_node(n, &mut out));
    out
}

#[test]
fn loop_unroll_preserves_subgroup_reduce_op() {
    // `loop i in 0..2 { store(out, i, subgroup_max(i + 1)); }` -- unrolling
    // substitutes `i := 0` then `i := 1` via transform::subst, recursing into
    // the `subgroup_max`. The operator must remain `Max`.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(2)],
        [1, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(2),
            vec![Node::store(
                "out",
                Expr::var("i"),
                Expr::subgroup_max(Expr::add(Expr::var("i"), Expr::u32(1))),
            )],
        )],
    );

    let result = LoopUnroll::transform(program);
    assert!(result.changed, "the small loop must unroll");

    let ops = collect_subgroup_reduce_ops(result.program.entry());
    assert_eq!(
        ops,
        vec![SubgroupReduceOp::Max, SubgroupReduceOp::Max],
        "subgroup_max must survive induction-variable substitution as Max, \
         not be silently rewritten to Add"
    );
}
