//! Oracle-differential probe: loop_fusion concatenates two adjacent loops'
//! bodies into ONE loop scope (`fused = body_a ++ rename(body_b, var_b->var_a)`).
//! Each original loop body is its own scope, so both may independently bind the
//! same local name `x`. After fusion those two `let x` become duplicate sibling
//! bindings in one scope ("duplicate sibling let binding", V032).
//!
//! The pre-existing fusion guard checks `body_a's let names ∩ body_b's VAR
//! READS` (a capture hazard: body_b *reading* a name body_a binds). It never
//! checks `body_a's let names ∩ body_b's let names` (the duplicate-BINDING
//! hazard), so when body_b binds `x` WITHOUT reading it, the guard passes and
//! fusion produces invalid IR. Here body_b's `x` captures an atomic's result
//! and is unused -- the atomic's side effect keeps the binding from being dead,
//! so this is not dismissible as DCE-removable. This is the scope-extension
//! dual (loop_licm 13th) in its loop-fusion form.

use vyre_foundation::ir::{
    AtomicOp, BufferAccess, BufferDecl, DataType, Expr, Ident, MemoryOrdering, Node, Program,
};
use vyre_foundation::optimizer::passes::loops::loop_fusion::LoopFusion;
use vyre_reference::value::Value;

/// ```text
/// loop i in 0..4 { let x = 1; store(a, 0, x); }              // a[0] = 1
/// loop j in 0..4 { let x = atomic_add(b, 0, 1); }            // b[0] += 1, x unused
/// ```
/// `a` and `b` are disjoint, bounds match (0..4), loop vars differ (i != j),
/// and body_b never *reads* `x` -- so every existing fusion guard passes. Both
/// bodies bind `x`, in separate loop scopes: valid pre-fusion.
fn program_with_same_named_local_in_fusable_loops() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("a", 0, DataType::U32).with_count(1),
            BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![
                    Node::let_bind("x", Expr::u32(1)),
                    Node::store("a", Expr::u32(0), Expr::var("x")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::let_bind(
                    "x",
                    Expr::Atomic {
                        op: AtomicOp::Add,
                        buffer: Ident::from("b"),
                        index: Box::new(Expr::u32(0)),
                        expected: None,
                        value: Box::new(Expr::u32(1)),
                        ordering: MemoryOrdering::Relaxed,
                    },
                )],
            ),
        ],
    )
}

#[test]
fn loop_fusion_does_not_merge_same_named_locals_into_one_scope() {
    let program = program_with_same_named_local_in_fusable_loops();
    let inputs = [Value::U32(0)]; // `b` (the only non-output buffer).

    // Original: two disjoint loops in separate scopes. a[0] ends at 1; b[0]
    // accumulates four atomic adds to 4. Both buffers are returned as outputs.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![
            Value::from(1u32.to_le_bytes().to_vec()),
            Value::from(4u32.to_le_bytes().to_vec()),
        ],
        "a[0] == 1, b[0] == 4 (four atomic adds)"
    );

    let transformed = LoopFusion::transform(program).program;

    // The transformed program must STILL validate and run. If loop_fusion merged
    // the two `let x` into one loop body, they are duplicate sibling bindings the
    // validator rejects -- reference_eval errors and this `.expect` panics.
    let after = vyre_reference::reference_eval(&transformed, &inputs).expect(
        "loop_fusion must not merge two loop bodies that bind the same local name \
         into a single scope (duplicate sibling binding, V032)",
    );

    assert_eq!(
        after, original,
        "loop_fusion must preserve observable semantics and scoping"
    );
}
