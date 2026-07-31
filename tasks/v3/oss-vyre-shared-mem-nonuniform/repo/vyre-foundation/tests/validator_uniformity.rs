//! Validator uniformity coverage.
//!
//! These tests pin the contract added with the uniformity analyzer:
//! `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }` is legal inside a `Node::Loop` whose bounds are
//! workgroup-uniform and inside a `Node::If` whose condition is
//! uniform; barriers in genuinely divergent control flow continue to
//! emit V010. Each positive case has a sanitized negative twin.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

fn output_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

fn has_v010(errors: &[vyre_foundation::validate::ValidationError]) -> bool {
    errors
        .iter()
        .any(|e| e.message().contains("V010: barrier may be reached"))
}

// ----------------------------------------------------------------------
// Positive: uniform-bound Loop allows Barrier in body.
// ----------------------------------------------------------------------

#[test]
fn loop_with_uniform_bound_allows_barrier_in_body() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(8),
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "uniform-bound Loop must accept Barrier in body, got {:?}",
        errors
    );
}

// Negative twin: a Loop bound by a per-lane id is divergent and must
// reject the same Barrier.
#[test]
fn loop_with_invocationid_bound_rejects_barrier_in_body() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::InvocationId { axis: 0 },
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        has_v010(&errors),
        "Loop with per-lane to-bound must reject Barrier, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Positive: uniform If condition allows Barrier in then-arm.
// ----------------------------------------------------------------------

#[test]
fn if_with_uniform_cond_allows_barrier_in_then() {
    let program = output_program(vec![Node::if_then(
        Expr::eq(Expr::buf_len("out"), Expr::u32(4)),
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "If with uniform cond must accept Barrier in then-arm, got {:?}",
        errors
    );
}

// Negative twin: per-lane cond keeps the branch divergent.
#[test]
fn if_with_invocationid_cond_rejects_barrier_in_then() {
    let program = output_program(vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        has_v010(&errors),
        "If with per-lane cond must reject Barrier, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Positive: a uniform Loop nested inside another uniform Loop still
// allows a Barrier  -  divergence is *or*-propagated through ancestors,
// and an all-uniform spine stays non-divergent.
// ----------------------------------------------------------------------

#[test]
fn nested_uniform_loop_allows_barrier() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(4),
        vec![Node::loop_for(
            "j",
            Expr::u32(0),
            Expr::u32(2),
            vec![Node::barrier()],
        )],
    )]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "nested uniform Loops must accept Barrier in inner body, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Positive: a Let bound to a uniform expression propagates uniformity
// to a downstream Loop bound that references it.
// ----------------------------------------------------------------------

#[test]
fn let_bound_uniform_var_propagates_to_loop_bound() {
    let program = output_program(vec![
        Node::let_bind("n", Expr::u32(16)),
        Node::loop_for("i", Expr::u32(0), Expr::var("n"), vec![Node::barrier()]),
    ]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "uniform Let-binding must propagate to Loop bound, got {:?}",
        errors
    );
}

// Negative twin: a Let bound to a per-lane id taints downstream uses.
#[test]
fn let_bound_invocationid_var_rejects_loop_barrier() {
    let program = output_program(vec![
        Node::let_bind("n", Expr::InvocationId { axis: 0 }),
        Node::loop_for("i", Expr::u32(0), Expr::var("n"), vec![Node::barrier()]),
    ]);
    let errors = validate(&program);
    assert!(
        has_v010(&errors),
        "per-lane Let-binding must taint downstream Loop bound, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Positive: the loop counter itself is uniform inside a uniform-bound
// loop and can serve as a uniform bound for a nested loop.
// ----------------------------------------------------------------------

#[test]
fn uniform_loop_counter_propagates_to_inner_bound() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(4),
        vec![Node::loop_for(
            "j",
            Expr::u32(0),
            Expr::var("i"),
            vec![Node::barrier()],
        )],
    )]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "uniform-loop counter must propagate as a uniform inner bound, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Negative: a Barrier inside a uniform child of a divergent parent
// stays divergent  -  divergence is contagious downward.
// ----------------------------------------------------------------------

#[test]
fn barrier_inside_divergent_parent_still_rejected() {
    let program = output_program(vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(4),
            vec![Node::barrier()],
        )],
    )]);
    let errors = validate(&program);
    assert!(
        has_v010(&errors),
        "uniform inner Loop inside divergent If must still reject Barrier, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Negative: BinOp with one divergent operand taints the whole tree.
// ----------------------------------------------------------------------

#[test]
fn binop_with_divergent_operand_rejects_loop_barrier() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::add(Expr::u32(8), Expr::InvocationId { axis: 0 }),
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        has_v010(&errors),
        "BinOp with one per-lane operand must taint the whole bound, got {:?}",
        errors
    );
}

// ----------------------------------------------------------------------
// Positive: BinOp with both operands uniform stays uniform.
// ----------------------------------------------------------------------

#[test]
fn binop_with_uniform_operands_allows_loop_barrier() {
    let program = output_program(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::mul(Expr::buf_len("out"), Expr::u32(2)),
        vec![Node::barrier()],
    )]);
    let errors = validate(&program);
    assert!(
        !has_v010(&errors),
        "uniform BinOp must allow Barrier in body, got {:?}",
        errors
    );
}
