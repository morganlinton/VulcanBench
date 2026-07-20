//! Oracle-differential probe: read_only_load_hoist hoists a `let x` that
//! begins both arms of an `If` to BEFORE the If. That extends `x`'s live
//! range from arm-local (the reference interpreter pops arm bindings on arm
//! exit) to the ENCLOSING scope, where it now lives across the If and beyond.
//! If a later sibling in the same body rebinds `x` -- legal originally because
//! the arm-local `x` was already popped -- the hoisted `x` collides with it
//! ("duplicate local binding `x`"), turning a well-scoped program into one the
//! validator rejects. This is the scope-extension dual of the scope-motion
//! miscompiles (region_inline / tail_duplication).

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::memory::read_only_load_hoist::ReadOnlyLoadHoistPass;
use vyre_reference::value::Value;

/// `if (true) { let x = load(ro,0); store(out,0,x) }
///  else        { let x = load(ro,0); store(out,0,x) }
///  let x = 7; store(out,0,x)`. The two arm-local `x` bindings are popped at
/// arm exit, so the trailing `let x = 7` rebinds a free name -- valid. Hoisting
/// the arm prefix makes `x` enclosing-scoped and the trailing `let x` a
/// duplicate.
fn program_with_rebind_after_hoistable_if() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::storage("ro", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::if_then_else(
                Expr::bool(true),
                vec![
                    Node::let_bind("x", Expr::load("ro", Expr::u32(0))),
                    Node::store("out", Expr::u32(0), Expr::var("x")),
                ],
                vec![
                    Node::let_bind("x", Expr::load("ro", Expr::u32(0))),
                    Node::store("out", Expr::u32(0), Expr::var("x")),
                ],
            ),
            Node::let_bind("x", Expr::u32(7)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    )
}

#[test]
fn read_only_load_hoist_preserves_arm_local_rebind_scope() {
    let program = program_with_rebind_after_hoistable_if();
    let inputs = [Value::U32(42)]; // ro[0]; overwritten in observable output by the trailing x=7

    // Original: an arm runs (out=42), arm `x` pops, then `let x = 7`, out = 7.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(7u32.to_le_bytes().to_vec())],
        "original ends with out == 7 (the trailing rebind)"
    );

    let hoisted = ReadOnlyLoadHoistPass::transform(program).program;

    // The hoisted program must STILL run and produce the same result. If the
    // pass hoists `let x = load(ro,0)` out of the If, `x` becomes enclosing-
    // scoped and the trailing `let x = 7` is a duplicate binding the validator
    // rejects -- reference_eval errors and this `.expect` panics.
    let transformed = vyre_reference::reference_eval(&hoisted, &inputs).expect(
        "read_only_load_hoist must not extend a hoisted binding's scope so it \
         collides with a later rebind of the same name",
    );

    assert_eq!(
        transformed, original,
        "read_only_load_hoist must preserve observable semantics and scoping"
    );
}
