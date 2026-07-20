//! Soundness-invariant lock for branch_value_hoist's scope handling.
//!
//! branch_value_hoist hoists a `let x` that begins both arms of an `If`. Like
//! read_only_load_hoist, that could extend `x`'s live range from arm-local
//! (popped at arm exit) to the enclosing scope and collide with a later
//! rebind of `x` ("duplicate sibling let binding", V032). branch_value_hoist
//! AVOIDS that because it wraps a multi-node hoist result in a `Node::Block`
//! (`hoist_prefix`), which re-scopes the hoisted binding so it never becomes a
//! bare sibling -- exactly the Block-wrapping that read_only_load_hoist's
//! flat-splice DROPPED (the bug fixed in commit `008a7ac3d7`).
//!
//! This test pins that invariant: on a program where the hoisted name `x` is
//! rebound by a later sibling, the pass must still fire (the hoist is real)
//! AND the result must stay well-scoped (reference_eval accepts it, semantics
//! unchanged). If a future change removes the Block-wrapping the way
//! read_only_load_hoist once did, this test fails.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::cleanup::branch_value_hoist::BranchValueHoistPass;
use vyre_reference::value::Value;

/// `if (true) { let x = 5; out[0] = x } else { let x = 5; out[0] = x }
///  let x = 7; out[0] = x`. The arm-local `x` bindings pop at arm exit, so the
/// trailing `let x = 7` rebinds a free name -- valid. Hoisting the arm prefix
/// makes `x` enclosing-scoped and the trailing `let x` a duplicate.
fn program_with_rebind_after_hoistable_if() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::if_then_else(
                Expr::bool(true),
                vec![
                    Node::let_bind("x", Expr::u32(5)),
                    Node::store("out", Expr::u32(0), Expr::var("x")),
                ],
                vec![
                    Node::let_bind("x", Expr::u32(5)),
                    Node::store("out", Expr::u32(0), Expr::var("x")),
                ],
            ),
            Node::let_bind("x", Expr::u32(7)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    )
}

#[test]
fn branch_value_hoist_preserves_arm_local_rebind_scope() {
    let program = program_with_rebind_after_hoistable_if();
    let inputs: [Value; 0] = []; // `out` is the only buffer and is output-allocated

    // Original: an arm runs (out=5), arm `x` pops, then `let x = 7`, out = 7.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(7u32.to_le_bytes().to_vec())],
        "original ends with out == 7 (the trailing rebind)"
    );

    let result = BranchValueHoistPass::transform(program);
    assert!(
        result.changed,
        "the pass must actually hoist the common `let x` prefix (else this test \
         would vacuously pass without exercising the scope path)"
    );
    let hoisted = result.program;

    // The hoist fired, yet the result must STILL run: the hoisted `let x = 5`
    // is wrapped in a Block, so it does NOT collide with the trailing
    // `let x = 7`. If a refactor drops that Block-wrapping (as
    // read_only_load_hoist once did), `x` leaks to the enclosing scope, the
    // trailing `let x` becomes a duplicate sibling, and reference_eval errors.
    let transformed = vyre_reference::reference_eval(&hoisted, &inputs).expect(
        "branch_value_hoist must keep a hoisted binding scoped (Block-wrapped) \
         so it does not collide with a later rebind of the same name",
    );

    assert_eq!(
        transformed, original,
        "branch_value_hoist must preserve observable semantics and scoping"
    );
}
