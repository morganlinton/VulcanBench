//! Oracle-differential probe: folding a range-determined `If` must NOT dissolve
//! the chosen arm's scope into the enclosing loop body.
//!
//! `loop_var_range_fold` replaces `If(always-true, then, else)` with `then`'s
//! statements. The arm is a SCOPE (If arms pop their bindings on exit), but the
//! fold splices the arm's nodes directly into the parent via `flatten_block`
//! (and a bare single-node return). So an arm-local `let x` collides with a
//! sibling `let x` in the loop body that was previously well-scoped --
//! "V032: duplicate sibling let binding `x`", a class-(b) scope-extension
//! miscompile (valid IR -> validator-rejected IR).

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_var_range_fold::LoopVarRangeFoldPass;
use vyre_reference::value::Value;

/// ```text
/// loop i in 0..8 {
///   if i < 8 { let x = 5; }   // always true; x is scoped to the arm
///   let x = 9;                // sibling binding -- legal, the arm's x is popped
///   buf[0] = x;               // reads 9
/// }
/// ```
/// Folding `i < 8` to its then-arm must keep `let x = 5` scoped, not splice it
/// next to `let x = 9`.
fn program_arm_binding_collides_with_sibling() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::If {
                    cond: Expr::lt(Expr::var("i"), Expr::u32(8)),
                    then: vec![Node::let_bind("x", Expr::u32(5))],
                    otherwise: vec![],
                },
                Node::let_bind("x", Expr::u32(9)),
                Node::store("buf", Expr::u32(0), Expr::var("x")),
            ],
        }],
    )
}

#[test]
fn range_fold_does_not_dissolve_arm_scope_into_loop_body() {
    let program = program_arm_binding_collides_with_sibling();
    let inputs: [Value; 0] = []; // buf is the only buffer and it is an output.

    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped (arm `x` popped before sibling `x`)");
    assert_eq!(
        original,
        vec![Value::from(9u32.to_le_bytes().to_vec())],
        "buf[0] == 9 (the loop-body `x`, not the arm `x`)",
    );

    let result = LoopVarRangeFoldPass::transform(program);
    assert!(result.changed, "i < 8 is always true for i in [0,8) and must fold");

    let after = vyre_reference::reference_eval(&result.program, &inputs).expect(
        "folding the always-true If must keep the arm's `let x` scoped -- splicing \
         it next to the sibling `let x` is a V032 duplicate-binding miscompile",
    );
    assert_eq!(
        after, original,
        "range-fold must preserve semantics and scoping",
    );
}
