//! Oracle-differential regression: tail_duplication must not sink a tail
//! that reads a binding declared *inside* the If arm.
//!
//! tail_duplication hoists an identical, observably-free tail out of both
//! arms: `If(c, [a, b], [a', b])` -> `If(c, [a], [a']); b`. When `b` reads
//! a variable bound inside the arm (e.g. `let y = t + 1` after
//! `let t = ...`), sinking `b` past the If moves the read out of `t`'s
//! lexical scope. The reference interpreter pops arm-local bindings on
//! arm exit (`Invocation::pop_scope` clears the slot), so a hoisted
//! `let y = t + 1` reads an unbound `t` -- a scope-invalid program.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::cleanup::tail_duplication::TailDuplicationPass;
use vyre_reference::value::Value;

/// Both arms are `let t = <k>; let y = t + 1`. The trailing `let y = t + 1`
/// is identical and pure (so tail_duplication is eligible to hoist it), but
/// `t` is arm-local.
fn program_with_arm_local_tail() -> Program {
    let tail = Node::let_bind("y", Expr::add(Expr::var("t"), Expr::u32(1)));
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::If {
                cond: Expr::eq(Expr::u32(0), Expr::u32(0)),
                then: vec![Node::let_bind("t", Expr::u32(5)), tail.clone()],
                otherwise: vec![Node::let_bind("t", Expr::u32(9)), tail],
            },
        ],
    )
}

#[test]
fn tail_duplication_preserves_arm_local_scope() {
    let program = program_with_arm_local_tail();
    let inputs = [Value::U32(0)];

    // The original program is well-scoped: `let y = t + 1` runs inside the
    // arm where `t` is bound. It must run cleanly.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");

    let result = TailDuplicationPass::transform(program);

    // The transformed program must STILL run and produce the same result.
    // Pre-fix this FAILS: tail_duplication sank `let y = t + 1` past the If,
    // where `t` is out of scope, so reference_eval errors on the unbound
    // read and this `.expect` panics. Post-fix the pass declines (the tail
    // reads an arm-local binding), so the program is unchanged and runs.
    let transformed = vyre_reference::reference_eval(&result.program, &inputs).expect(
        "tail_duplication must not sink a tail that reads an arm-local binding \
         out of its scope (transformed program reads unbound `t`)",
    );

    assert_eq!(
        transformed, original,
        "tail_duplication must preserve observable semantics; arm-local tail \
         read must not be hoisted past the If"
    );
}
