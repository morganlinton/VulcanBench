//! Oracle-differential probe: loop_fusion proves the two loops are independent
//! ONLY for buffers (`buffers_disjoint`) and one direction of scalar capture
//! (body_b reading a name body_a binds). It does NOT check cross-loop
//! dependencies through a shared *outer scalar* mutated by `Node::Assign`.
//!
//! Here body_a READS an outer scalar `s` and body_b WRITES it. In the original
//! program loop_a runs entirely before loop_b, so every `store(out, i, s)`
//! observes `s == 0`. Fusing interleaves them -- `store(out, i, s); s = i` --
//! so each store after the first observes the previous iteration's write. The
//! fused program is well-scoped (no validation error) but computes DIFFERENT
//! output: a silent value miscompile, distinct from the V032 binding collision.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_fusion::LoopFusion;
use vyre_reference::value::Value;

/// ```text
/// let s = 0;
/// loop i in 0..4 { store(out, i, s); }   // reads outer scalar s (== 0 throughout)
/// loop j in 0..4 { s = j; }              // writes outer scalar s
/// ```
/// Buffers are trivially disjoint (loop_b touches none), bounds match, loop
/// vars differ, and loop_b never reads a loop_a binding -- so every existing
/// fusion guard passes. out ends as [0, 0, 0, 0].
fn program_with_cross_loop_scalar_dependency() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![
            Node::let_bind("s", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::store("out", Expr::var("i"), Expr::var("s"))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::assign("s", Expr::var("j"))],
            ),
        ],
    )
}

#[test]
fn loop_fusion_does_not_reorder_a_cross_loop_scalar_dependency() {
    let program = program_with_cross_loop_scalar_dependency();
    let inputs: [Value; 0] = []; // `out` is the only buffer and it is an output.

    // Original: loop_a runs fully with s == 0, so out == [0, 0, 0, 0].
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(vec![0u8; 16])],
        "out == [0, 0, 0, 0]: loop_a observes s == 0 throughout"
    );

    let transformed = LoopFusion::transform(program).program;

    // The fused program still validates and runs -- but if loop_fusion merged
    // the two loops, the interleaved `store(out, i, s); s = i` makes each store
    // observe the previous iteration's write, so out becomes [0, 0, 1, 2].
    let after = vyre_reference::reference_eval(&transformed, &inputs)
        .expect("fused program must still be well-scoped");
    assert_eq!(
        after, original,
        "loop_fusion must not fuse loops with a cross-loop scalar dependency \
         (body_a reads an outer scalar that body_b writes)"
    );
}
