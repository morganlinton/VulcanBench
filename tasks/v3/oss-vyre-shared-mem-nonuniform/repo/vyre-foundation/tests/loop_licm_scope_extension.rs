//! Oracle-differential probe: loop_licm hoists a loop-invariant `let t` out of
//! its loop and into the ENCLOSING body. Each loop body is its own scope (the
//! reference interpreter pops loop-body bindings at loop exit), so two sibling
//! loops may each bind the same name `t` without conflict. Hoisting both `let t`
//! into the shared enclosing body makes them duplicate sibling bindings
//! ("duplicate sibling let binding", V032), turning a well-scoped program into
//! one the validator rejects. This is the scope-extension dual already proven
//! for read_only_load_hoist / region_inline / tail_duplication.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_licm::LoopLicm;
use vyre_reference::value::Value;

/// `loop i in 0..2 { let t = 5; store(out,0,t) }
///  loop j in 0..2 { let t = 9; store(out,0,t) }`. Each `let t` lives in its
/// own loop-body scope, popped at loop exit, so the two never coexist -- valid.
/// The first loop leaves out[0] = 5, the second overwrites it with 9.
fn program_with_same_named_invariant_in_sibling_loops() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(2),
                vec![
                    Node::let_bind("t", Expr::u32(5)),
                    Node::store("out", Expr::u32(0), Expr::var("t")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(2),
                vec![
                    Node::let_bind("t", Expr::u32(9)),
                    Node::store("out", Expr::u32(0), Expr::var("t")),
                ],
            ),
        ],
    )
}

#[test]
fn loop_licm_does_not_create_duplicate_sibling_binding_across_loops() {
    let program = program_with_same_named_invariant_in_sibling_loops();
    let inputs: [Value; 0] = []; // `out` is the only buffer and it is an output.

    // Original: loop i writes 5, loop j overwrites with 9. Both `let t` live in
    // disjoint loop scopes, so the program is well-scoped and runs.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(9u32.to_le_bytes().to_vec())],
        "original ends with out == 9 (second loop overwrites the first)"
    );

    let transformed = LoopLicm::transform(program).program;

    // The transformed program must STILL run and produce the same result. If
    // LICM hoists both `let t` into the shared enclosing body, they become
    // duplicate sibling bindings the validator rejects -- reference_eval errors
    // and this `.expect` panics.
    let after = vyre_reference::reference_eval(&transformed, &inputs).expect(
        "loop_licm must not hoist a binding into an enclosing scope where the \
         same name is bound by another sibling loop (duplicate sibling binding)",
    );

    assert_eq!(
        after, original,
        "loop_licm must preserve observable semantics and scoping"
    );
}
