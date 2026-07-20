//! Oracle-differential probe: DCE must keep a `let` whose value is used ONLY
//! inside a subgroup operand.
//!
//! `collect_expr_refs` (the DCE liveness ref collector) did NOT descend into
//! `SubgroupBallot`/`SubgroupShuffle`/`SubgroupReduce` operands -- they sat in
//! the no-op arm. So a `let x` referenced only by `subgroup_add(x)` had its use
//! invisible to liveness: `x` never entered the live set, so its `let` was
//! dropped as dead -- dangling the `Var(x)` still inside the subgroup op
//! ("reference to undeclared variable `x`"). A deletion/completeness miscompile
//! (valid IR -> validator-rejected IR).
//!
//! Note: the cse `expr_has_effect` and the fusion-safety `collect_expr_accesses`
//! both descend into subgroup operands; `collect_expr_refs` was the odd one out.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::fusion_cse::dce::dce;
use vyre_reference::value::Value;

/// ```text
/// let x = buf[0];
/// let y = subgroup_add(x);   // x is used ONLY here
/// out[0] = y;
/// ```
/// Single-lane subgroup_add is the identity, so out[0] == buf[0].
fn program_let_used_only_in_subgroup() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("buf", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("buf", Expr::u32(0))),
            Node::let_bind("y", Expr::subgroup_add(Expr::var("x"))),
            Node::store("out", Expr::u32(0), Expr::var("y")),
        ],
    )
}

#[test]
fn dce_keeps_let_used_only_inside_subgroup_op() {
    let program = program_let_used_only_in_subgroup();
    let inputs = [Value::from(7u32.to_le_bytes().to_vec())]; // buf = [7]

    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(7u32.to_le_bytes().to_vec())],
        "out[0] == subgroup_add(buf[0]) == 7 (single-lane identity)",
    );

    let optimized = dce(program);
    let after = vyre_reference::reference_eval(&optimized, &inputs).expect(
        "DCE must keep `let x` -- it is referenced inside `subgroup_add(x)`; \
         dropping it dangles the use",
    );
    assert_eq!(
        after, original,
        "DCE must treat a subgroup operand as a use of its inner variables",
    );
}
