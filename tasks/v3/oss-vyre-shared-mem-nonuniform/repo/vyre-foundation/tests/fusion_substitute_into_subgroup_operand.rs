//! Regression: the fusion copy-propagation rewriter `substitute_expr` must
//! descend into subgroup operands.
//!
//! A single-use, fusable `let v = <expr>` is held as a PENDING replacement and
//! dropped from the emitted output, to be inlined at its one use site. If that
//! use is inside a subgroup operand (`SubgroupReduce` / `SubgroupBallot` /
//! `SubgroupShuffle`), the pre-fix `substitute_expr` cloned the subgroup node
//! VERBATIM in its terminal arm -- so `v` was never inlined.
//!
//! The asymmetry is what made this a miscompile rather than a missed
//! optimization: the use-counter (`fact_substrate::use_facts`) and
//! `collect_used_vars` BOTH descend into the subgroup operand (via
//! `push_expr_children`), so `v` was counted as a single use (selecting the
//! fusable path that drops the `let`) and then dropped from the pending map by
//! `drop_used`. Only `substitute_expr` failed to descend. Net effect: the fused
//! program referenced `v` inside the subgroup op with NO `let v` declaration --
//! a dangling-reference miscompile that `reference_eval` rejects as
//! "reference to undeclared variable `v`".

use std::collections::HashSet;

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::fusion::Fusion;
use vyre_reference::value::Value;

/// `let v = in[0] + in[1]` (single-use, fusable BinOp) consumed exactly once
/// inside `subgroup_add(v)`. The workgroup is a single lane, so `subgroup_add`
/// reduces over one element and equals `v`; out[0] = (3 + 5) = 8.
fn program_with_fused_let_in_subgroup_operand() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(2),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "v",
                Expr::add(
                    Expr::load("in", Expr::u32(0)),
                    Expr::load("in", Expr::u32(1)),
                ),
            ),
            Node::store("out", Expr::u32(0), Expr::subgroup_add(Expr::var("v"))),
        ],
    )
}

/// Every name a `let` / loop introduces, descending all node bodies.
fn collect_declared(nodes: &[Node], out: &mut HashSet<String>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => {
                out.insert(name.to_string());
            }
            Node::Loop { var, body, .. } => {
                out.insert(var.to_string());
                collect_declared(body, out);
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_declared(then, out);
                collect_declared(otherwise, out);
            }
            Node::Block(body) => collect_declared(body, out),
            Node::Region { body, .. } => collect_declared(body, out),
            _ => {}
        }
    }
}

/// Every local `Var` referenced anywhere in an expression, descending into
/// EVERY subexpression -- including subgroup operands, which is exactly the
/// position the buggy rewriter dropped.
fn collect_referenced_vars_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            out.insert(name.to_string());
        }
        Expr::Load { index, .. } => collect_referenced_vars_expr(index, out),
        Expr::BinOp { left, right, .. } => {
            collect_referenced_vars_expr(left, out);
            collect_referenced_vars_expr(right, out);
        }
        Expr::UnOp { operand, .. } => collect_referenced_vars_expr(operand, out),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_referenced_vars_expr(cond, out);
            collect_referenced_vars_expr(true_val, out);
            collect_referenced_vars_expr(false_val, out);
        }
        Expr::Cast { value, .. } => collect_referenced_vars_expr(value, out),
        Expr::Fma { a, b, c } => {
            collect_referenced_vars_expr(a, out);
            collect_referenced_vars_expr(b, out);
            collect_referenced_vars_expr(c, out);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            collect_referenced_vars_expr(index, out);
            if let Some(expected) = expected {
                collect_referenced_vars_expr(expected, out);
            }
            collect_referenced_vars_expr(value, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_referenced_vars_expr(arg, out);
            }
        }
        Expr::SubgroupBallot { cond } => collect_referenced_vars_expr(cond, out),
        Expr::SubgroupShuffle { value, lane } => {
            collect_referenced_vars_expr(value, out);
            collect_referenced_vars_expr(lane, out);
        }
        Expr::SubgroupReduce { value, .. } => collect_referenced_vars_expr(value, out),
        _ => {}
    }
}

fn collect_referenced_vars(nodes: &[Node], out: &mut HashSet<String>) {
    for node in nodes {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                collect_referenced_vars_expr(value, out);
            }
            Node::Store { index, value, .. } => {
                collect_referenced_vars_expr(index, out);
                collect_referenced_vars_expr(value, out);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_referenced_vars_expr(cond, out);
                collect_referenced_vars(then, out);
                collect_referenced_vars(otherwise, out);
            }
            Node::Loop { from, to, body, .. } => {
                collect_referenced_vars_expr(from, out);
                collect_referenced_vars_expr(to, out);
                collect_referenced_vars(body, out);
            }
            Node::Block(body) => collect_referenced_vars(body, out),
            Node::Region { body, .. } => collect_referenced_vars(body, out),
            Node::Trap { address, .. } => collect_referenced_vars_expr(address, out),
            Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                collect_referenced_vars_expr(offset, out);
                collect_referenced_vars_expr(size, out);
            }
            _ => {}
        }
    }
}

#[test]
fn fusion_substitutes_pending_binding_into_subgroup_operand() {
    let program = program_with_fused_let_in_subgroup_operand();
    let inputs = [Value::Array(vec![Value::U32(3), Value::U32(5)])];

    // Original is well-scoped and computes out[0] = subgroup_add(3 + 5) = 8.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");

    let fused = Fusion::transform(program).program;

    // STRUCTURAL CONTRACT: every variable referenced in the fused program must
    // be declared. Pre-fix, `v` was inlined everywhere EXCEPT inside the
    // subgroup operand (substitute_expr cloned `SubgroupReduce` verbatim) while
    // its pending `let v` was dropped -- leaving `subgroup_add(v)` dangling.
    let mut declared = HashSet::new();
    collect_declared(fused.entry(), &mut declared);
    let mut referenced = HashSet::new();
    collect_referenced_vars(fused.entry(), &mut referenced);
    let mut dangling: Vec<_> = referenced.difference(&declared).cloned().collect();
    dangling.sort();
    assert!(
        dangling.is_empty(),
        "fused program references undeclared variable(s) {dangling:?}: the fusion \
         rewriter dropped a single-use `let` but failed to inline it into a \
         subgroup operand (declared={declared:?}, referenced={referenced:?})"
    );

    // ORACLE DIFFERENTIAL: the fused program must remain well-scoped and produce
    // byte-identical results to the original. Pre-fix `reference_eval` rejects
    // the dangling `v` with "reference to undeclared variable `v`".
    let after = vyre_reference::reference_eval(&fused, &inputs)
        .expect("fused program must remain well-scoped (no dangling subgroup operand)");
    assert_eq!(
        after, original,
        "fusion changed observable output: subgroup_add(3 + 5) must still equal 8"
    );
}
